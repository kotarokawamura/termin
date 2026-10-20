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

/// Returns the default pane title based on the current `$SHELL` environment variable.
pub fn default_shell_title() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
}

pub struct PaneUiOutput {
    pub action: Option<TitleBarAction>,
    pub start_tile_drag: bool,
    pub clicked_or_focused: bool,
}

const SCROLLBAR_WIDTH: f32 = 10.0;

pub struct TerminalPane {
    pub id: usize,
    pub title: String,
    pub editing_title: bool,
    pub zoom_scale: f32,
    pub scroll_accum_y: f32,
    pub backend: Box<dyn TerminalBackend>,
    pub session: Box<dyn SessionDriver>,
    pub selection_start: Option<(u16, u16)>,
    pub selection_end: Option<(u16, u16)>,
    select_anchor_pos: Option<egui::Pos2>,
    pub pending_paste: bool,
}

impl TerminalPane {
    pub fn new(id: usize, title: &str) -> Self {
        Self::new_with_command(id, title, None)
    }

    pub fn new_with_command(id: usize, title: &str, startup_command: Option<&str>) -> Self {
        let rows = 24;
        let cols = 80;
        let backend = Box::new(Vt100Adapter::new(rows, cols));
        let session = Box::new(LocalPtySession::new_with_command(rows, cols, startup_command));

        Self {
            id,
            title: title.to_string(),
            editing_title: false,
            zoom_scale: 1.0,
            scroll_accum_y: 0.0,
            backend,
            session,
            selection_start: None,
            selection_end: None,
            select_anchor_pos: None,
            pending_paste: false,
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

    pub fn zoom_in(&mut self) {
        self.zoom_scale = (self.zoom_scale * 1.1).clamp(0.4, 3.5);
    }

    pub fn zoom_out(&mut self) {
        self.zoom_scale = (self.zoom_scale / 1.1).clamp(0.4, 3.5);
    }

    pub fn zoom_reset(&mut self) {
        self.zoom_scale = 1.0;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        tile_id: TileId,
        menus: &MenuManager,
        settings: &ThemeSettings,
        is_focused: bool,
        is_top_right: bool,
        prefs_open: bool,
        any_editing_title: bool,
        workspace_dragging: bool,
        external_zoom_delta: f32,
    ) -> PaneUiOutput {
        let mut clicked_or_focused = false;

        // Visually treat pane as inactive when it cannot be interacted with
        // (e.g., during a drag operation, while a context menu is open, or while Preferences is open)
        let popup_open = egui::Popup::is_any_open(ui.ctx());
        let any_dragging = workspace_dragging || ui.ctx().dragged_id().is_some();
        let visually_active = is_focused && !prefs_open && !popup_open && !any_dragging;

        // 1. Render Terminator-style titlebar
        let tb_out = render_titlebar(
            ui,
            tile_id,
            menus,
            &mut self.title,
            &mut self.editing_title,
            visually_active,
            is_top_right,
            settings,
        );
        if tb_out.clicked {
            clicked_or_focused = true;
        }

        // 2. Render terminal screen canvas + right-side scrollbar
        let available = ui.available_size();
        let (full_rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
        let sb_width = SCROLLBAR_WIDTH.min((full_rect.width() * 0.2).max(4.0));
        let rect = egui::Rect::from_min_max(
            full_rect.min,
            egui::pos2((full_rect.max.x - sb_width).max(full_rect.min.x), full_rect.max.y),
        );
        let scrollbar_rect = egui::Rect::from_min_max(
            egui::pos2((full_rect.max.x - sb_width).max(full_rect.min.x), full_rect.min.y),
            full_rect.max,
        );

        let term_resp = ui.interact(
            rect,
            egui::Id::new(("term_canvas", tile_id)),
            egui::Sense::click_and_drag(),
        );
        let scrollbar_resp = ui.interact(
            scrollbar_rect,
            egui::Id::new(("term_scrollbar", tile_id)),
            egui::Sense::click_and_drag(),
        );
        if term_resp.clicked()
            || term_resp.drag_started()
            || scrollbar_resp.clicked()
            || scrollbar_resp.drag_started()
        {
            clicked_or_focused = true;
        }

        let pane_hovered = term_resp.hovered() || scrollbar_resp.hovered();
        let super_held = ui.input(|i| {
            i.key_down(egui::Key::SuperLeft)
                || i.key_down(egui::Key::SuperRight)
                || i.modifiers.mac_cmd
        });

        // Pinch-out and pinch-in zoom gestures on the focused or hovered panel
        let zoom_delta = if (is_focused || pane_hovered)
            && !self.editing_title
            && !any_editing_title
            && !prefs_open
        {
            let mut zd = ui.input(|i| i.zoom_delta());
            if is_focused || pane_hovered {
                zd *= external_zoom_delta;
            }
            if super_held && pane_hovered {
                let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll_y.abs() > 0.1 {
                    zd *= (scroll_y / 200.0).exp();
                }
            }
            zd
        } else {
            1.0
        };
        if (zoom_delta - 1.0).abs() > 0.001 {
            self.zoom_scale = (self.zoom_scale * zoom_delta).clamp(0.4, 3.5);
            if pane_hovered {
                clicked_or_focused = true;
            }
        }

        let term_font_size =
            (settings.effective_terminal_font_size() * self.zoom_scale).clamp(6.0, 64.0);
        let char_w = (term_font_size * 0.6).max(4.0);
        let char_h = (term_font_size * 1.15).max(8.0);
        let pad = 4.0_f32;

        let inner_w = (rect.width() - pad * 2.0).max(0.0);
        let inner_h = (rect.height() - pad * 2.0).max(0.0);
        let new_cols = ((inner_w / char_w).floor() as u16).max(10);
        let new_rows = ((inner_h / char_h).floor() as u16).max(4);
        let (cur_rows, cur_cols) = self.backend.size();
        if (new_rows, new_cols) != (cur_rows, cur_cols) {
            self.backend.resize(new_rows, new_cols);
            self.session.resize(new_rows, new_cols);
        }

        // Handle mouse drag selection on the terminal grid (supports 1+ character selections)
        let (rows, cols) = self.backend.size();
        let pos_to_cell = |pos: egui::Pos2| -> (u16, u16) {
            let rel_x = (pos.x - (rect.min.x + pad)).max(0.0);
            let rel_y = (pos.y - (rect.min.y + pad)).max(0.0);
            let c = ((rel_x / char_w).floor() as u16).min(cols.saturating_sub(1));
            let r = ((rel_y / char_h).floor() as u16).min(rows.saturating_sub(1));
            (r, c)
        };

        let primary_pressed = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        let primary_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
        let primary_released = ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
        let pointer_pos = ui.input(|i| i.pointer.interact_pos());

        if primary_pressed && !popup_open {
            if let Some(pos) = pointer_pos {
                if rect.contains(pos) {
                    self.select_anchor_pos = Some(pos);
                    self.selection_start = None;
                    self.selection_end = None;
                    clicked_or_focused = true;
                }
            }
        }
        if primary_down {
            if let (Some(anchor), Some(pos)) = (self.select_anchor_pos, pointer_pos) {
                if pos.distance(anchor) >= 2.0 {
                    self.selection_start = Some(pos_to_cell(anchor));
                    self.selection_end = Some(pos_to_cell(pos));
                }
            }
        }
        if primary_released {
            self.select_anchor_pos = None;
        }

        // Handle mouse wheel / touchpad scrollback when hovering over this pane or its scrollbar
        if pane_hovered && !super_held && (zoom_delta - 1.0).abs() <= 0.001 {
            let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_y.abs() > 0.01 {
                self.scroll_accum_y += scroll_y;
                let step_px = (char_h * 0.65).max(8.0);
                if self.scroll_accum_y.abs() >= step_px {
                    let delta_rows = (self.scroll_accum_y / step_px).trunc() as isize;
                    self.scroll_accum_y -= (delta_rows as f32) * step_px;
                    self.backend.scroll(delta_rows);
                }
            }
        }

        // Handle interactive scrollbar click/drag
        let max_scroll = self.backend.max_scroll_offset();
        let total_lines = (rows as usize) + max_scroll;
        let track_pad = 2.0_f32;
        let track_h = (scrollbar_rect.height() - track_pad * 2.0).max(1.0);
        let visible_frac = if total_lines > 0 {
            ((rows as f32) / (total_lines as f32)).clamp(0.05, 1.0)
        } else {
            1.0
        };
        let thumb_h = (track_h * visible_frac).clamp(18.0_f32.min(track_h), track_h);
        let scrollable_track_h = (track_h - thumb_h).max(1.0);

        if max_scroll > 0 && (scrollbar_resp.clicked() || scrollbar_resp.dragged()) {
            if let Some(pos) = scrollbar_resp.interact_pointer_pos() {
                let rel_y = ((pos.y - (scrollbar_rect.min.y + track_pad) - thumb_h * 0.5)
                    / scrollable_track_h)
                    .clamp(0.0, 1.0);
                let target_offset = ((1.0 - rel_y) * (max_scroll as f32)).round() as usize;
                self.backend.set_scroll_offset(target_offset);
            }
        }

        // 3. Context menu for the terminal screen canvas
        let mut menu_action_output = None;
        if let Some((_ctx, menu_action)) = menus.show(&term_resp, MenuContext::Pane(tile_id)) {
            clicked_or_focused = true;
            match menu_action {
                MenuAction::Copy => {
                    self.copy_selection_or_screen(ui.ctx());
                }
                MenuAction::Paste => {
                    self.pending_paste = true;
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }
                MenuAction::SplitHorizontal => {
                    menu_action_output = Some(TitleBarAction::SplitHorizontal);
                }
                MenuAction::SplitVertical => {
                    menu_action_output = Some(TitleBarAction::SplitVertical);
                }
                MenuAction::NewTab => {
                    menu_action_output = Some(TitleBarAction::NewTab);
                }
                MenuAction::ClearScrollback => {
                    self.backend.clear_scrollback();
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

        let pane_context_menu_open =
            tb_out.context_menu_open || term_resp.context_menu_opened() || egui::Popup::is_any_open(ui.ctx());
        let pane_dragging = tb_out.is_dragging
            || any_dragging
            || term_resp.dragged_by(egui::PointerButton::Primary)
            || scrollbar_resp.dragged_by(egui::PointerButton::Primary);
        let can_interact =
            is_focused && !self.editing_title && !any_editing_title && !prefs_open && !pane_context_menu_open;

        // Forward keyboard events to PTY when this pane can be interacted with
        let mut shortcut_key_action = None;
        if can_interact {
            // If Tab was pressed, clear any accidental egui focus so Tab stays in the terminal
            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                if let Some(focused_id) = ui.ctx().memory(|m| m.focused()) {
                    ui.ctx().memory_mut(|m| m.surrender_focus(focused_id));
                }
            }
            shortcut_key_action = self.handle_shortcut_keyboard_input(ui);
            self.handle_keyboard_input(ui, shortcut_key_action.is_some());
        }

        let painter = ui.painter_at(full_rect);
        painter.rect_filled(full_rect, 0.0, egui::Color32::from_rgb(18, 18, 18));

        // Draw cells in two passes:
        // Pass 1: Backgrounds and selection highlights (prevents wide CJK glyphs from being clipped by the next cell's background)
        // Pass 2: Foreground text glyphs and underlines
        let (rows, cols) = self.backend.size();
        let font_id = egui::FontId::monospace(term_font_size);
        let norm_selection = self.normalized_selection();

        for r in 0..rows {
            for c in 0..cols {
                let cell = self.backend.read_cell(r, c);
                let cell_min = egui::pos2(
                    rect.min.x + pad + (c as f32) * char_w,
                    rect.min.y + pad + (r as f32) * char_h,
                );
                let cell_rect = egui::Rect::from_min_size(cell_min, egui::vec2(char_w, char_h));

                let (_fg, bg) = if cell.inverse {
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

                if let Some((start, end)) = norm_selection {
                    if (r, c) >= start && (r, c) <= end {
                        painter.rect_filled(
                            cell_rect,
                            0.0,
                            egui::Color32::from_rgba_unmultiplied(70, 130, 210, 110),
                        );
                    }
                }
            }
        }

        for r in 0..rows {
            for c in 0..cols {
                let cell = self.backend.read_cell(r, c);
                if cell.wide_continuation {
                    continue;
                }

                let cell_min = egui::pos2(
                    rect.min.x + pad + (c as f32) * char_w,
                    rect.min.y + pad + (r as f32) * char_h,
                );

                let (mut fg, _bg) = if cell.inverse {
                    (cell.bg, cell.fg)
                } else {
                    (cell.fg, cell.bg)
                };

                if cell.bold {
                    fg = crate::vte::RgbColor(
                        fg.0.saturating_add(25),
                        fg.1.saturating_add(25),
                        fg.2.saturating_add(25),
                    );
                }

                let fg_color = egui::Color32::from_rgb(fg.0, fg.1, fg.2);

                if cell.ch != ' ' && cell.ch != '\0' {
                    painter.text(
                        cell_min,
                        egui::Align2::LEFT_TOP,
                        cell.ch,
                        font_id.clone(),
                        fg_color,
                    );
                }

                if cell.underline {
                    let width_cells = if cell.wide { 2.0 } else { 1.0 };
                    let y = cell_min.y + char_h - 1.5;
                    painter.line_segment(
                        [
                            egui::pos2(cell_min.x, y),
                            egui::pos2(cell_min.x + char_w * width_cells, y),
                        ],
                        egui::Stroke::new(1.0, fg_color),
                    );
                }
            }
        }

        // Draw cursor
        let cursor = self.backend.cursor();
        let cursor_visually_active = is_focused && !pane_context_menu_open && !pane_dragging && !prefs_open;
        if cursor.visible && cursor.row < rows && cursor.col < cols {
            let cur_min = egui::pos2(
                rect.min.x + pad + (cursor.col as f32) * char_w,
                rect.min.y + pad + (cursor.row as f32) * char_h,
            );
            let cur_rect = egui::Rect::from_min_size(cur_min, egui::vec2(char_w, char_h));
            if cursor_visually_active {
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

        // Draw right-side scrollbar track and thumb
        let scroll_off = self.backend.scroll_offset();
        let max_scroll = self.backend.max_scroll_offset();
        painter.rect_filled(
            scrollbar_rect,
            0.0,
            egui::Color32::from_rgb(26, 26, 26),
        );
        let top_ratio = if max_scroll > 0 {
            1.0 - (scroll_off as f32 / max_scroll as f32).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let thumb_y = scrollbar_rect.min.y + track_pad + top_ratio * scrollable_track_h;
        let thumb_rect = egui::Rect::from_min_size(
            egui::pos2(scrollbar_rect.min.x + 2.0, thumb_y),
            egui::vec2((scrollbar_rect.width() - 4.0).max(2.0), thumb_h),
        );
        let thumb_color = if scrollbar_resp.dragged() {
            egui::Color32::from_rgb(175, 175, 175)
        } else if scrollbar_resp.hovered() || scroll_off > 0 {
            egui::Color32::from_rgb(135, 135, 135)
        } else if max_scroll > 0 {
            egui::Color32::from_rgb(85, 85, 85)
        } else {
            egui::Color32::from_rgb(48, 48, 48)
        };
        painter.rect_filled(thumb_rect, 3.0, thumb_color);

        // Draw scrollback indicator badge if scrolled up into history
        if scroll_off > 0 {
            let badge_text = format!(" ↑ -{}/{} ", scroll_off, max_scroll.max(scroll_off));
            let badge_pos = egui::pos2(rect.max.x - 6.0, rect.min.y + 6.0);
            painter.text(
                badge_pos,
                egui::Align2::RIGHT_TOP,
                badge_text,
                egui::FontId::monospace(11.0),
                egui::Color32::from_rgb(250, 204, 21),
            );
        }

        PaneUiOutput {
            action: tb_out.action.or(shortcut_key_action).or(menu_action_output),
            start_tile_drag: tb_out.start_tile_drag,
            clicked_or_focused,
        }
    }

    fn normalized_selection(&self) -> Option<((u16, u16), (u16, u16))> {
        let start = self.selection_start?;
        let end = self.selection_end?;
        if start <= end {
            Some((start, end))
        } else {
            Some((end, start))
        }
    }

    pub fn extract_selected_text(&self) -> Option<String> {
        let (start, end) = self.normalized_selection()?;
        let (rows, cols) = self.backend.size();
        let mut lines = Vec::new();

        for r in start.0..=end.0.min(rows.saturating_sub(1)) {
            let c_start = if r == start.0 { start.1 } else { 0 };
            let c_end = if r == end.0 {
                end.1.min(cols.saturating_sub(1))
            } else {
                cols.saturating_sub(1)
            };

            let mut line = String::new();
            for c in c_start..=c_end {
                let cell = self.backend.read_cell(r, c);
                if !cell.wide_continuation {
                    line.push(if cell.ch == '\0' { ' ' } else { cell.ch });
                }
            }
            lines.push(line.trim_end().to_string());
        }

        Some(lines.join("\n"))
    }

    fn copy_selection_or_screen(&mut self, ctx: &egui::Context) {
        if let Some(selected) = self.extract_selected_text() {
            if !selected.is_empty() {
                ctx.copy_text(selected);
                return;
            }
        }
        // Fallback: copy non-empty screen lines
        let (rows, cols) = self.backend.size();
        let mut lines = Vec::new();
        for r in 0..rows {
            let mut line = String::new();
            for c in 0..cols {
                let cell = self.backend.read_cell(r, c);
                if !cell.wide_continuation {
                    line.push(if cell.ch == '\0' { ' ' } else { cell.ch });
                }
            }
            lines.push(line.trim_end().to_string());
        }
        while lines.last().map(|l| l.is_empty()).unwrap_or(false) {
            lines.pop();
        }
        ctx.copy_text(lines.join("\n"));
    }

    fn handle_keyboard_input(&mut self, ui: &mut egui::Ui, shortcut_handled: bool) {
        let events = ui.input(|i| i.events.clone());
        let shift_held = ui.input(|i| i.modifiers.shift);
        let super_held = ui.input(|i| {
            i.key_down(egui::Key::SuperLeft)
                || i.key_down(egui::Key::SuperRight)
                || i.modifiers.mac_cmd
        });

        for event in events {
            match event {
                egui::Event::Text(text) => {
                    if shortcut_handled || super_held {
                        continue;
                    }
                    if self.backend.scroll_offset() > 0 {
                        self.backend.set_scroll_offset(0);
                    }
                    self.session.write_input(text.as_bytes());
                }
                // Paste event triggered by RequestPaste (context menu / Ctrl+Shift+V) or OS paste
                egui::Event::Paste(text) => {
                    if self.pending_paste || shift_held {
                        self.pending_paste = false;
                        if self.backend.scroll_offset() > 0 {
                            self.backend.set_scroll_offset(0);
                        }
                        self.session.write_input(text.as_bytes());
                    } else {
                        // Plain Ctrl+V (without Shift) sends literal-next (0x16) to PTY
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
        let shift_held = ui.input(|i| i.modifiers.shift);
        let super_held = ui.input(|i| {
            i.key_down(egui::Key::SuperLeft)
                || i.key_down(egui::Key::SuperRight)
                || i.modifiers.mac_cmd
        });
        let has_paste_event = events.iter().any(|e| matches!(e, egui::Event::Paste(_)));

        for event in events {
            match event {
                // Note: egui-winit converts both Ctrl+C and Ctrl+Shift+C into Event::Copy
                egui::Event::Copy => {
                    if shift_held || self.normalized_selection().is_some() {
                        self.copy_selection_or_screen(ui.ctx());
                        if !shift_held {
                            self.selection_start = None;
                            self.selection_end = None;
                        }
                    } else {
                        self.session.write_input(&[0x03]); // Ctrl+C (SIGINT)
                    }
                }
                // Note: egui-winit converts Ctrl+X / Ctrl+Shift+X into Event::Cut
                egui::Event::Cut => {
                    if shift_held && self.normalized_selection().is_some() {
                        self.copy_selection_or_screen(ui.ctx());
                    } else if !shift_held {
                        self.session.write_input(&[0x18]); // Ctrl+X
                    }
                }
                egui::Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    // Super+= / Super+- / Super+0 zoom shortcuts
                    if super_held || modifiers.mac_cmd {
                        match key {
                            egui::Key::Plus | egui::Key::Equals => {
                                action = Some(TitleBarAction::ZoomIn);
                                continue;
                            }
                            egui::Key::Minus => {
                                action = Some(TitleBarAction::ZoomOut);
                                continue;
                            }
                            egui::Key::Num0 => {
                                action = Some(TitleBarAction::ZoomReset);
                                continue;
                            }
                            _ => match physical_key {
                                Some(egui::Key::Plus | egui::Key::Equals) => {
                                    action = Some(TitleBarAction::ZoomIn);
                                    continue;
                                }
                                Some(egui::Key::Minus) => {
                                    action = Some(TitleBarAction::ZoomOut);
                                    continue;
                                }
                                Some(egui::Key::Num0) => {
                                    action = Some(TitleBarAction::ZoomReset);
                                    continue;
                                }
                                _ => {}
                            },
                        }
                    }

                    if modifiers.ctrl && modifiers.shift {
                        match key {
                            egui::Key::Tab => {
                                action = Some(TitleBarAction::FocusPrevPane);
                            }
                            egui::Key::C => {
                                self.copy_selection_or_screen(ui.ctx());
                            }
                            egui::Key::V => {
                                if !has_paste_event {
                                    self.pending_paste = true;
                                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                                }
                            }
                            egui::Key::E => {
                                action = Some(TitleBarAction::SplitHorizontal);
                            }
                            egui::Key::O => {
                                action = Some(TitleBarAction::SplitVertical);
                            }
                            egui::Key::T => {
                                action = Some(TitleBarAction::NewTab);
                            }
                            egui::Key::W => {
                                action = Some(TitleBarAction::ClosePane);
                            }
                            egui::Key::Plus | egui::Key::Equals => {
                                action = Some(TitleBarAction::ZoomIn);
                            }
                            egui::Key::Minus => {
                                action = Some(TitleBarAction::ZoomOut);
                            }
                            egui::Key::Num0 => {
                                action = Some(TitleBarAction::ZoomReset);
                            }
                            _ => {}
                        }
                    } else if modifiers.alt && modifiers.shift {
                        if key == egui::Key::D {
                            action = Some(TitleBarAction::SplitVertical);
                        }
                    } else if modifiers.ctrl {
                        match key {
                            egui::Key::Tab => {
                                action = Some(TitleBarAction::FocusNextPane);
                                continue;
                            }
                            egui::Key::Comma => {
                                action = Some(TitleBarAction::OpenPreferences);
                                continue;
                            }
                            egui::Key::Plus | egui::Key::Equals => {
                                action = Some(TitleBarAction::ZoomIn);
                                continue;
                            }
                            egui::Key::Minus => {
                                action = Some(TitleBarAction::ZoomOut);
                                continue;
                            }
                            egui::Key::Num0 => {
                                action = Some(TitleBarAction::ZoomReset);
                                continue;
                            }
                            _ => {}
                        }

                        // Generic Ctrl + A..Z handling (0x01..0x1A)
                        let key_name = format!("{:?}", key);
                        if key_name.len() == 1 {
                            let ch = key_name.chars().next().unwrap();
                            if ch.is_ascii_uppercase() {
                                if ch == 'C' && self.normalized_selection().is_some() {
                                    self.copy_selection_or_screen(ui.ctx());
                                    self.selection_start = None;
                                    self.selection_end = None;
                                } else {
                                    let ctrl_code = (ch as u8) & 0x1F;
                                    self.session.write_input(&[ctrl_code]);
                                }
                            }
                        } else {
                            match key {
                                egui::Key::Space => self.session.write_input(&[0x00]), // Ctrl+Space (Emacs mark)
                                egui::Key::Slash => self.session.write_input(&[0x1f]), // Ctrl+/ (Emacs undo)
                                egui::Key::OpenBracket => self.session.write_input(&[0x1b]), // Ctrl+[ (ESC)
                                egui::Key::Backslash => self.session.write_input(&[0x1c]),
                                egui::Key::CloseBracket => self.session.write_input(&[0x1d]),
                                _ => {}
                            }
                        }
                    } else if modifiers.alt {
                        if key == egui::Key::D {
                            action = Some(TitleBarAction::SplitHorizontal);
                            continue;
                        }
                        // Alt (Meta) key support for Emacs (e.g., M-x sends ESC + x)
                        let key_name = format!("{:?}", key);
                        if key_name.len() == 1 {
                            let ch = key_name.chars().next().unwrap().to_ascii_lowercase();
                            self.session.write_input(&[0x1b, ch as u8]);
                        }
                    } else if modifiers.shift {
                        let (rows, _) = self.backend.size();
                        let page_step = (rows as isize / 2).max(1);
                        match key {
                            egui::Key::PageUp => {
                                self.backend.scroll(page_step);
                            }
                            egui::Key::PageDown => {
                                self.backend.scroll(-page_step);
                            }
                            egui::Key::ArrowUp => {
                                self.backend.scroll(1);
                            }
                            egui::Key::ArrowDown => {
                                self.backend.scroll(-1);
                            }
                            egui::Key::Home => {
                                self.backend.set_scroll_offset(self.backend.max_scroll_offset());
                            }
                            egui::Key::End => {
                                self.backend.set_scroll_offset(0);
                            }
                            egui::Key::Insert => {
                                self.pending_paste = true;
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                            }
                            egui::Key::Tab => {
                                self.session.write_input(b"\x1b[Z"); // Shift+Tab (Backtab)
                            }
                            _ => {}
                        }
                    } else {
                        if self.backend.scroll_offset() > 0 {
                            self.backend.set_scroll_offset(0);
                        }
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
                            egui::Key::Insert => self.session.write_input(b"\x1b[2~"),
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
                            egui::Key::F11 => {
                                action = Some(TitleBarAction::ToggleMaximizeWindow);
                            }
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

#[cfg(test)]
mod tests {
    use super::*;

    struct DummySession;
    impl SessionDriver for DummySession {
        fn write_input(&mut self, _data: &[u8]) {}
        fn read_output(&mut self) -> Vec<u8> {
            Vec::new()
        }
        fn resize(&mut self, _rows: u16, _cols: u16) {}
        fn is_alive(&mut self) -> bool {
            true
        }
    }

    fn make_test_pane() -> TerminalPane {
        let mut backend = Box::new(Vt100Adapter::new(24, 80));
        backend.process_bytes(b"Hello, World!\r\nSecond Line\r\n");
        TerminalPane {
            id: 1,
            title: "/bin/bash".to_string(),
            editing_title: false,
            zoom_scale: 1.0,
            scroll_accum_y: 0.0,
            backend,
            session: Box::new(DummySession),
            selection_start: None,
            selection_end: None,
            select_anchor_pos: None,
            pending_paste: false,
        }
    }

    #[test]
    fn test_copy_selection_range_and_single_char() {
        let mut pane = make_test_pane();

        // Single-char selection (start == end)
        pane.selection_start = Some((0, 0));
        pane.selection_end = Some((0, 0));
        assert_eq!(pane.extract_selected_text().as_deref(), Some("H"));

        // Range selection on row 0 ("Hello")
        pane.selection_start = Some((0, 0));
        pane.selection_end = Some((0, 4));
        assert_eq!(pane.extract_selected_text().as_deref(), Some("Hello"));

        // Verify Event::Copy (emitted by egui-winit on Ctrl+Shift+C / Ctrl+C) copies the selection
        let ctx = egui::Context::default();
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Copy);
        let mut out = ctx.run_ui(raw, |ui| {
            pane.handle_shortcut_keyboard_input(ui);
        });
        out.textures_delta.clear();
        let copied = out.platform_output.commands.iter().find_map(|cmd| {
            if let egui::OutputCommand::CopyText(t) = cmd {
                Some(t.clone())
            } else {
                None
            }
        });
        assert_eq!(copied.as_deref(), Some("Hello"));
    }

    #[test]
    fn test_super_zoom_shortcuts() {
        let mut pane = make_test_pane();
        let ctx = egui::Context::default();

        // Super + = -> ZoomIn
        let mut raw_in = egui::RawInput::default();
        raw_in.events.push(egui::Event::Key {
            key: egui::Key::SuperLeft,
            physical_key: Some(egui::Key::SuperLeft),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        raw_in.events.push(egui::Event::Key {
            key: egui::Key::Equals,
            physical_key: Some(egui::Key::Equals),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut action = None;
        ctx.run_ui(raw_in, |ui| {
            action = pane.handle_shortcut_keyboard_input(ui);
        })
        .textures_delta
        .clear();
        assert_eq!(action, Some(TitleBarAction::ZoomIn));

        // Super + - -> ZoomOut
        let mut raw_out = egui::RawInput::default();
        raw_out.events.push(egui::Event::Key {
            key: egui::Key::Minus,
            physical_key: Some(egui::Key::Minus),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_ui(raw_out, |ui| {
            action = pane.handle_shortcut_keyboard_input(ui);
        })
        .textures_delta
        .clear();
        assert_eq!(action, Some(TitleBarAction::ZoomOut));
    }
}
