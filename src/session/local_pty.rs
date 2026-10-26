// src/session/local_pty.rs
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc::{self, Receiver}, Arc};
use std::thread;
use eframe::egui;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use crate::session::SessionDriver;

pub struct LocalPtySession {
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    rx: Receiver<Vec<u8>>,
    wakeup_pending: Arc<AtomicBool>,
    exited: Arc<AtomicBool>,
}

impl LocalPtySession {
    #[allow(dead_code)]
    pub fn new(rows: u16, cols: u16) -> Self {
        Self::new_with_command(rows, cols, None, None)
    }

    pub fn new_with_command(
        rows: u16,
        cols: u16,
        startup_command: Option<&str>,
        wakeup_ctx: Option<egui::Context>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let wakeup_pending = Arc::new(AtomicBool::new(false));
        let exited = Arc::new(AtomicBool::new(false));
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });

        if let Ok(pair) = pair {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
            let mut cmd = CommandBuilder::new(&shell);
            if let Some(command_str) = startup_command {
                if !command_str.trim().is_empty() {
                    cmd.arg("-c");
                    cmd.arg(command_str);
                }
            }

            cmd.env("TERM", "xterm-256color");
            cmd.env("COLORTERM", "truecolor");

            let child = pair.slave.spawn_command(cmd).ok();
            let writer = pair.master.take_writer().ok();

            if let Ok(mut reader) = pair.master.try_clone_reader() {
                let pending_flag = Arc::clone(&wakeup_pending);
                let exited_flag = Arc::clone(&exited);
                thread::spawn(move || {
                    let mut buf = [0u8; 8192];
                    while let Ok(n) = reader.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                        // UIスレッドがまだ読み出していなければ、1回だけ request_repaint を送る
                        if !pending_flag.swap(true, Ordering::AcqRel) {
                            if let Some(ctx) = &wakeup_ctx {
                                ctx.request_repaint();
                            }
                        }
                    }
                    exited_flag.store(true, Ordering::Release);
                    if let Some(ctx) = &wakeup_ctx {
                        ctx.request_repaint();
                    }
                });
            }

            Self {
                master: Some(pair.master),
                writer,
                child,
                rx,
                wakeup_pending,
                exited,
            }
        } else {
            exited.store(true, Ordering::Release);
            Self {
                master: None,
                writer: None,
                child: None,
                rx,
                wakeup_pending,
                exited,
            }
        }
    }
}

impl SessionDriver for LocalPtySession {
    fn read_output(&mut self) -> Vec<u8> {
        // 読み出し開始時にフラグをクリア（これ以降の到着で再度 Wakeup が鳴るようにする）
        self.wakeup_pending.store(false, Ordering::Release);
        let Ok(mut first) = self.rx.try_recv() else {
            return Vec::new();
        };
        while let Ok(chunk) = self.rx.try_recv() {
            first.extend_from_slice(&chunk);
        }
        first
    }

    fn write_input(&mut self, data: &[u8]) {
        if let Some(w) = &mut self.writer {
            let _ = w.write_all(data);
            let _ = w.flush();
        }
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        if let Some(master) = &self.master {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    fn is_alive(&mut self) -> bool {
        // リーダーがEOFを検知していない間は waitpid システムコールを一切呼ばない！
        if !self.exited.load(Ordering::Acquire) {
            return true;
        }
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(_exit_status)) => false,
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }
}