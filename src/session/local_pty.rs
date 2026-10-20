// Copyright (C) [2026/10/19] kotaro,termin
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; version 2
// of the License.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program; if not, write to the Free Software
// Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301, USA.
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
    #[allow(dead_code)]
    pub fn new(rows: u16, cols: u16) -> Self {
        Self::new_with_command(rows, cols, None)
    }

    pub fn new_with_command(rows: u16, cols: u16, startup_command: Option<&str>) -> Self {
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

impl Drop for LocalPtySession {
    fn drop(&mut self) {
        // Ensure child process is terminated and reaped when pane is closed
        self.writer.take();
        self.master.take();
        if let Some(child) = &mut self.child {
            if let Ok(None) = child.try_wait() {
                let _ = child.kill();
                let _ = child.wait();
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
