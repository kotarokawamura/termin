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
use eframe::egui;
use egui_tiles::TileId;
use crate::config::theme::ThemeSettings;
use crate::menus::{
    MenuManager,
    types::{MenuAction, MenuContext},
};
use crate::session::local_pty::LocalPtySession;
use crate::session::SessionDriver;
use crate::ui::titlebar::{render_titlebar, TitleBarAction};
use crate::vte::vt100_adapter::Vt100Adapter;
use crate::vte::TerminalBackend;

pub struct PaneUiOutput {
    pub action: Option<TitleBarAction>,
    pub start_tile_drag: bool,
    pub clicked_or_focused: bool,
}

pub struct TerminalPane {
    pub id: usize,
    pub title: String,
    pub editing_title: bool,
    pub backend: Box<dyn TerminalBackend>,
    pub session: Box<dyn SessionDriver>,
}

impl TerminalPane {
    pub fn new(id: usize, title: &str) -> Self {
        let rows = 24;
        let cols = 80;
        let backend = Box::new(Vt100Adapter::new(rows, cols));
        let session = Box::new(LocalPtySession::new(rows, cols));

        Self {
            id,
            title: title.to_string(),
            editing_title: false,
            backend,
            session,
        }
    }

    /// Read available bytes from PTY and feed them into the VTE backend.
    pub fn poll_pty(&mut self) -> bool {
        let bytes = self.session.read_output();
        if !bytes.is_empty() {
            self.backend.process_bytes(&bytes);
            return true;
        }
        false
    }

    pub fn is_alive(&mut self) -> bool {
        self.session.is_alive()
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        tile_id: TileId,
        menus: &MenuManager,
        settings: &ThemeSettings,
        is_focused: bool,
        is_top_right: bool,
    ) -> PaneUiOutput {
        let mut clicked_or_focused = false;

        // 1. Render Terminator-style titlebar
        let tb_out = render_titlebar(
            ui,
            tile_id,
            menus,
            &mut self.title,
            &mut self.editing_title,
            is_focused,
            is_top_right,
            settings,
        );
        if tb_out.clicked {
            clicked_or_focused = true;
        }

        // 2. Render terminal screen canvas
        let available = ui.available_size();
        let (rect, term_resp) = ui.allocate_exact_size(available, egui::Sense::click());
        if term_resp.clicked() {
            clicked_or_focused = true;
        }

        // 3. Context menu for the terminal screen canvas
        let mut menu_action_output = None;
        if let Some((_ctx, menu_action)) = menus.show(&term_resp, MenuContext::Pane(tile_id)) {
            clicked_or_focused = true;
            match menu_action {
                MenuAction::Copy => {
                    // Request copy operation
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestCopy);
                }
                MenuAction::Paste => {
                    // Request paste operation from system clipboard
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }
                MenuAction::SplitHorizontal => {
                    menu_action_output = Some(TitleBarAction::SplitHorizontal);
                }
                MenuAction::SplitVertical => {
                    menu_action_output = Some(TitleBarAction::SplitVertical);
                }
                MenuAction::ClosePane => {
                    menu_action_output = Some(TitleBarAction::ClosePane);
                }
                MenuAction::OpenSettings => {
                    menu_action_output = Some(TitleBarAction::OpenPreferences);
                }
                _ => {}
            }
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 18, 18));

        let char_w = 8.4_f32;
        let char_h = 16.0_f32;
        let pad = 4.0_f32;

        let new_cols = (((rect.width() - pad * 2.0) / char_w).floor() as u16).max(10);
        let new_rows = (((rect.height() - pad * 2.0) / char_h).floor() as u16).max(4);
        let (cur_rows, cur_cols) = self.backend.size();
        if (new_rows, new_cols) != (cur_rows, cur_cols) {
            self.backend.resize(new_rows, new_cols);
            self.session.resize(new_rows, new_cols);
        }

        // Forward keyboard events to PTY when this pane is focused and not editing title
        if is_focused && !self.editing_title {
            self.handle_keyboard_input(ui);
        }

        let mut shortcut_key_action = None;
        if is_focused && !self.editing_title {
            shortcut_key_action = self.handle_shortcut_keyboard_input(ui);
        }

        // Draw cells
        let (rows, cols) = self.backend.size();
        let font_id = egui::FontId::monospace(14.0);

        for r in 0..rows {
            for c in 0..cols {
                let cell = self.backend.read_cell(r, c);
                let cell_min = egui::pos2(
                    rect.min.x + pad + (c as f32) * char_w,
                    rect.min.y + pad + (r as f32) * char_h,
                );
                let cell_rect = egui::Rect::from_min_size(cell_min, egui::vec2(char_w, char_h));

                let (fg, bg) = if cell.inverse {
                    (cell.bg, cell.fg)
                } else {
                    (cell.fg, cell.bg)
                };

                if bg != crate::vte::RgbColor(18, 18, 18) {
                    painter.rect_filled(
                        cell_rect,
                        0.0,
                        egui::Color32::from_rgb(bg.0, bg.1, bg.2),
                    );
                }

                if cell.ch != ' ' && cell.ch != '\0' {
                    painter.text(
                        cell_min,
                        egui::Align2::LEFT_TOP,
                        cell.ch,
                        font_id.clone(),
                        egui::Color32::from_rgb(fg.0, fg.1, fg.2),
                    );
                }
            }
        }

        // Draw cursor
        let cursor = self.backend.cursor();
        if cursor.visible && cursor.row < rows && cursor.col < cols {
            let cur_min = egui::pos2(
                rect.min.x + pad + (cursor.col as f32) * char_w,
                rect.min.y + pad + (cursor.row as f32) * char_h,
            );
            let cur_rect = egui::Rect::from_min_size(cur_min, egui::vec2(char_w, char_h));
            if is_focused {
                painter.rect_filled(
                    cur_rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(220, 220, 220, 160),
                );
            } else {
                painter.rect_stroke(
                    cur_rect,
                    0.0,
                    egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(160, 160, 160)),
                    egui::StrokeKind::Outside,
                );
            }
        }

        PaneUiOutput {
            action: tb_out.action.or(shortcut_key_action).or(menu_action_output),
            start_tile_drag: tb_out.start_tile_drag,
            clicked_or_focused,
        }
    }

    fn handle_keyboard_input(&mut self, ui: &mut egui::Ui) {
        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Text(text) => {
                    self.session.write_input(text.as_bytes());
                }
                // egui intercepts Ctrl+C as Event::Copy and Ctrl+X as Event::Cut
                egui::Event::Copy => {
                    self.session.write_input(&[0x03]); // Ctrl+C (0x03)
                }
                egui::Event::Cut => {
                    self.session.write_input(&[0x18]); // Ctrl+X (0x18)
                }
                // egui intercepts Ctrl+V as Event::Paste
                egui::Event::Paste(text) => {
                    let shift = ui.input(|i| i.modifiers.shift);
                    if shift {
                        // Ctrl+Shift+V: Paste clipboard text into terminal
                        self.session.write_input(text.as_bytes());
                    } else {
                        // Plain Ctrl+V: Send C-v (0x16, page down in Emacs)
                        self.session.write_input(&[0x16]);
                    }
                }
                _ => {}
            }
        }
    }

    fn handle_shortcut_keyboard_input(&mut self, ui: &mut egui::Ui) -> Option<TitleBarAction> {
        let mut action = None;
        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if modifiers.ctrl && modifiers.shift {
                        match key {
                            egui::Key::E => {
                                action = Some(TitleBarAction::SplitHorizontal);
                            }
                            egui::Key::O => {
                                action = Some(TitleBarAction::SplitVertical);
                            }
                            _ => {}
                        }
                    } else if modifiers.ctrl {
                        // Generic Ctrl + A..Z handling (0x01..0x1A)
                        let key_name = format!("{:?}", key);
                        if key_name.len() == 1 {
                            let ch = key_name.chars().next().unwrap();
                            if ch.is_ascii_uppercase() {
                                let ctrl_code = (ch as u8) & 0x1F;
                                self.session.write_input(&[ctrl_code]);
                            }
                        } else {
                            match key {
                                egui::Key::Space => self.session.write_input(&[0x00]), // Ctrl+Space (Emacs mark)
                                egui::Key::Slash => self.session.write_input(&[0x1f]), // Ctrl+/ (Emacs undo)
                                _ => {}
                            }
                        }
                    } else if modifiers.alt {
                        // Alt (Meta) key support for Emacs (e.g., M-x sends ESC + x)
                        let key_name = format!("{:?}", key);
                        if key_name.len() == 1 {
                            let ch = key_name.chars().next().unwrap().to_ascii_lowercase();
                            self.session.write_input(&[0x1b, ch as u8]);
                        }
                    } else {
                        match key {
                            egui::Key::Enter => self.session.write_input(b"\r"),
                            egui::Key::Backspace => self.session.write_input(&[0x7f]),
                            egui::Key::Tab => self.session.write_input(b"\t"),
                            egui::Key::Escape => self.session.write_input(&[0x1b]),
                            egui::Key::ArrowUp => self.session.write_input(b"\x1b[A"),
                            egui::Key::ArrowDown => self.session.write_input(b"\x1b[B"),
                            egui::Key::ArrowRight => self.session.write_input(b"\x1b[C"),
                            egui::Key::ArrowLeft => self.session.write_input(b"\x1b[D"),
                            egui::Key::Home => self.session.write_input(b"\x1b[H"),
                            egui::Key::End => self.session.write_input(b"\x1b[F"),
                            egui::Key::Delete => self.session.write_input(b"\x1b[3~"),
                            egui::Key::PageUp => self.session.write_input(b"\x1b[5~"),
                            egui::Key::PageDown => self.session.write_input(b"\x1b[6~"),
                            egui::Key::F1 => self.session.write_input(b"\x1bOP"),
                            egui::Key::F2 => self.session.write_input(b"\x1bOQ"),
                            egui::Key::F3 => self.session.write_input(b"\x1bOR"),
                            egui::Key::F4 => self.session.write_input(b"\x1bOS"),
                            egui::Key::F5 => self.session.write_input(b"\x1b[15~"),
                            egui::Key::F6 => self.session.write_input(b"\x1b[17~"),
                            egui::Key::F7 => self.session.write_input(b"\x1b[18~"),
                            egui::Key::F8 => self.session.write_input(b"\x1b[19~"),
                            egui::Key::F9 => self.session.write_input(b"\x1b[20~"),
                            egui::Key::F10 => self.session.write_input(b"\x1b[21~"),
                            egui::Key::F11 => self.session.write_input(b"\x1b[23~"),
                            egui::Key::F12 => self.session.write_input(b"\x1b[24~"),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        action
    }
}