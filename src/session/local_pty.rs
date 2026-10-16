use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use crate::session::SessionDriver;

pub struct LocalPtySession {
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    rx: Receiver<Vec<u8>>,
}

impl LocalPtySession {
    pub fn new(rows: u16, cols: u16) -> Self {
        let (tx, rx) = mpsc::channel();
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });

        if let Ok(pair) = pair {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
            let cmd = CommandBuilder::new(shell);
            let child = pair.slave.spawn_command(cmd).ok();
            let writer = pair.master.take_writer().ok();

            if let Ok(mut reader) = pair.master.try_clone_reader() {
                thread::spawn(move || {
                    let mut buf = [0u8; 4096];
                    while let Ok(n) = reader.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                });
            }

            Self {
                master: Some(pair.master),
                writer,
                child,
                rx,
            }
        } else {
            Self {
                master: None,
                writer: None,
                child: None,
                rx,
            }
        }
    }
}

impl SessionDriver for LocalPtySession {
    fn read_output(&mut self) -> Vec<u8> {
        let mut combined = Vec::new();
        while let Ok(chunk) = self.rx.try_recv() {
            combined.extend_from_slice(&chunk);
        }
        combined
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
