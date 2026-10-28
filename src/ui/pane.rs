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
use std::hash::{Hash, Hasher};
use std::num::NonZeroU16;
use std::sync::Arc;
use eframe::egui;
use crate::config::theme::{
    detect_super_held, shortcut_matches_event, ShortcutSettings, ThemeSettings,
};
use crate::menus::{
    MenuManager,
    types::{MenuAction, MenuContext},
};
use crate::session::local_pty::LocalPtySession;
use crate::session::SessionDriver;
use crate::ui::state::PaneContext;
use crate::ui::titlebar::{render_titlebar, TitleBarAction};
use crate::vte::Vt100Adapter;

/// Returns the default pane title based on the current `$SHELL` environment variable.
pub fn default_shell_title() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
}

pub struct PaneUiOutput {
    pub action: Option<TitleBarAction>,
    pub toggle_tile_move: bool,
    pub toggle_broadcast_select: bool,
    pub clicked_or_focused: bool,
    pub broadcast_input: Vec<u8>,
}

const SCROLLBAR_WIDTH: f32 = 10.0;

#[derive(Clone, Default)]
struct RowCache {
    content_hash: u64,
    bg_and_lines: Vec<(egui::Rect, egui::Color32)>,
    text_shapes: Vec<egui::Shape>,
}

#[derive(Clone, PartialEq)]
struct GridCacheMeta {
    rect_min_x: i32,
    rect_min_y: i32,
    rows: NonZeroU16,
    cols: NonZeroU16,
    font_size_bits: u32,
}

pub struct TerminalPane {
    pub id: usize,
    pub title: String,
    pub editing_title: bool,
    pub broadcast_group: Option<usize>,
    pub zoom_scale: f32,
    pub scroll_accum_y: f32,
    pub backend: Vt100Adapter,
    pub session: Box<dyn SessionDriver>,
    pub selection_start: Option<(u16, u16)>,
    pub selection_end: Option<(u16, u16)>,
    /// スクロールバック先頭からの絶対行インデックス(usize)と列(u16)による選択開始位置
    pub abs_selection_start: Option<(usize, u16)>,
    /// スクロールバック先頭からの絶対行インデックス(usize)と列(u16)による選択終了位置
    pub abs_selection_end: Option<(usize, u16)>,
    pub select_anchor_pos: Option<egui::Pos2>,
    pub pending_paste: bool,
    pub super_key_latched: bool,
    pending_broadcast_input: Vec<u8>,
    grid_cache_meta: Option<GridCacheMeta>,
    row_caches: Vec<RowCache>,
    cached_terminal_mesh: Arc<egui::Mesh>,
    last_rendered_gen: u64,
    last_rendered_scroll: usize,
    last_rendered_sel: Option<((u16, u16), (u16, u16))>,
    last_max_scroll: usize,
}

impl TerminalPane {
    #[allow(dead_code)]
    pub fn new(id: usize, title: &str) -> Self {
        Self::new_with_wakeup(id, title, None, None)
    }

    pub fn new_with_command(id: usize, title: &str, startup_command: Option<&str>) -> Self {
        Self::new_with_wakeup(id, title, startup_command, None)
    }

    pub fn new_with_wakeup(
        id: usize,
        title: &str,
        startup_command: Option<&str>,
        wakeup_ctx: Option<egui::Context>,
    ) -> Self {
        Self::new_with_wakeup_and_cwd(id, title, startup_command, wakeup_ctx, None)
    }

    pub fn new_with_wakeup_and_cwd(
        id: usize,
        title: &str,
        startup_command: Option<&str>,
        wakeup_ctx: Option<egui::Context>,
        working_dir: Option<&std::path::Path>,
    ) -> Self {
        let rows: core::num::NonZeroU16 = core::num::NonZeroU16::new(24).unwrap();
        let cols: core::num::NonZeroU16 = core::num::NonZeroU16::new(80).unwrap();
        let backend = Vt100Adapter::new(rows, cols);
        let session = Box::new(LocalPtySession::new_with_cwd(
            rows.get(),
            cols.get(),
            startup_command,
            wakeup_ctx,
            working_dir,
        ));

        Self {
            id,
            title: title.to_string(),
            editing_title: false,
            broadcast_group: None,
            zoom_scale: 1.0,
            scroll_accum_y: 0.0,
            backend,
            session,
            selection_start: None,
            selection_end: None,
            abs_selection_start: None,
            abs_selection_end: None,
            select_anchor_pos: None,
            pending_paste: false,
            super_key_latched: false,
            pending_broadcast_input: Vec::new(),
            grid_cache_meta: None,
            row_caches: Vec::new(),
            cached_terminal_mesh: Arc::new(egui::Mesh::default()),
            last_rendered_gen: u64::MAX,
            last_rendered_scroll: usize::MAX,
            last_rendered_sel: None,
            last_max_scroll: 0,
        }
    }

    fn write_pty_input(&mut self, data: &[u8]) {
        self.session.write_input(data);
        if self.broadcast_group.is_some() {
            self.pending_broadcast_input.extend_from_slice(data);
        }
    }

    pub fn receive_broadcast_input(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        if self.backend.scroll_offset() > 0 {
            self.backend.set_scroll_offset(0);
            self.sync_viewport_selection_from_abs();
        }
        self.session.write_input(data);
    }

    pub fn current_working_dir(&self) -> Option<std::path::PathBuf> {
        self.session.current_working_dir()
    }

    /// Read available bytes from PTY and feed them into the VTE backend.
    pub fn poll_pty(&mut self) -> bool {
        let old_max_scroll = self.backend.max_scroll_offset();
        let bytes = self.session.read_output();
        if !bytes.is_empty() {
            self.backend.process_bytes(&bytes);
            let new_max_scroll = self.backend.max_scroll_offset();
            let scrolled_lines = new_max_scroll.saturating_sub(old_max_scroll);
            if scrolled_lines > 0 && self.backend.scroll_offset() == 0 {
                self.shift_selection_up(scrolled_lines);
            }
            self.last_max_scroll = new_max_scroll;
            return true;
        }
        false
    }

    fn shift_selection_up(&mut self, lines: usize) {
        if let (Some((sr, sc)), Some((er, ec))) = (self.selection_start, self.selection_end) {
            let shift = lines as i32;
            let new_sr = (sr as i32) - shift;
            let new_er = (er as i32) - shift;
            // 選択範囲が完全に画面上端より上に流れ去ったら選択解除
            if new_er < 0 && self.abs_selection_start.is_none() {
                self.selection_start = None;
                self.selection_end = None;
            } else {
                self.selection_start = Some((new_sr.max(0) as u16, sc));
                self.selection_end = Some((new_er.max(0) as u16, ec));
            }
        }
        self.sync_viewport_selection_from_abs();
    }

    /// `abs_selection_start` / `abs_selection_end` から現在画面に見えている範囲の `selection_start` / `selection_end` を更新する
    fn sync_viewport_selection_from_abs(&mut self) {
        let (Some(a_start), Some(a_end)) = (self.abs_selection_start, self.abs_selection_end) else {
            return;
        };
        let (start, end) = if a_start <= a_end {
            (a_start, a_end)
        } else {
            (a_end, a_start)
        };

        let (rows, cols) = self.backend.size();
        let rows_usize = rows.get() as usize;
        let last_col = cols.get().saturating_sub(1);
        let max_scroll = self.backend.max_scroll_offset();
        let cur_scroll = self.backend.scroll_offset();
        let view_top = max_scroll.saturating_sub(cur_scroll);
        let view_bottom = view_top + rows_usize.saturating_sub(1);

        // 選択範囲が現在の表示領域と全く重なっていない場合
        if end.0 < view_top || start.0 > view_bottom {
            self.selection_start = None;
            self.selection_end = None;
            return;
        }

        let vis_sr = if start.0 <= view_top {
            0
        } else {
            (start.0 - view_top) as u16
        };
        let vis_sc = if start.0 < view_top { 0 } else { start.1 };

        let vis_er = if end.0 >= view_bottom {
            rows.get().saturating_sub(1)
        } else {
            (end.0 - view_top) as u16
        };
        let vis_ec = if end.0 > view_bottom { last_col } else { end.1 };

        self.selection_start = Some((vis_sr, vis_sc));
        self.selection_end = Some((vis_er, vis_ec));
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

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &PaneContext,
        menus: &MenuManager,
        settings: &ThemeSettings,
    ) -> PaneUiOutput {
        self.pending_broadcast_input.clear();
        let mut clicked_or_focused = false;

        let tile_id = ctx.tile_id;
        let pane_rect = ui.max_rect();
        let canvas_id = egui::Id::unique(("term_canvas", tile_id));
        let scrollbar_id = egui::Id::unique(("term_scrollbar", tile_id));

        let blocked = ctx.is_blocked();
        let visually_active = ctx.is_visually_active();
        let any_moving = ctx.is_any_moving();
        let any_selecting_broadcast = ctx.is_any_selecting_broadcast();
        let any_selecting_pane = any_moving || any_selecting_broadcast;

        // 1. Render Terminator-style titlebar
        let tb_out = render_titlebar(
            ui,
            ctx,
            menus,
            &mut self.title,
            &mut self.editing_title,
            settings,
        );

        if tb_out.clicked && !blocked && !ctx.popup_open {
            clicked_or_focused = true;
        }

        if blocked || ctx.popup_open || !visually_active || any_selecting_pane {
            self.select_anchor_pos = None;
        }

        // 2. Render terminal screen canvas + right-side scrollbar
        let available = ui.available_size();
        let (full_rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
        let sb_width = if settings.show_scrollbar {
            SCROLLBAR_WIDTH.min((full_rect.width() * 0.2).max(4.0))
        } else {
            0.0
        };
        let rect = egui::Rect::from_min_max(
            full_rect.min,
            egui::pos2((full_rect.max.x - sb_width).max(full_rect.min.x), full_rect.max.y),
        );
        let scrollbar_rect = egui::Rect::from_min_max(
            egui::pos2((full_rect.max.x - sb_width).max(full_rect.min.x), full_rect.min.y),
            full_rect.max,
        );

        let pointer_over_floating = ui
            .input(|i| {
                i.pointer
                    .interact_pos()
                    .or_else(|| i.pointer.hover_pos())
                    .or_else(|| i.pointer.latest_pos())
            })
            .and_then(|pos| ui.ctx().layer_id_at(pos))
            .is_some_and(|layer| layer.order != egui::Order::Background);

        let canvas_sense = if ctx.popup_open || pointer_over_floating {
            egui::Sense::hover()
        } else if visually_active && !any_selecting_pane {
            egui::Sense::click_and_drag()
        } else {
            egui::Sense::CLICK
        };

        let term_resp = ui.interact(rect, canvas_id, canvas_sense);
        let scrollbar_sense = if settings.show_scrollbar {
            canvas_sense
        } else {
            egui::Sense::hover()
        };
        let scrollbar_resp = ui.interact(scrollbar_rect, scrollbar_id, scrollbar_sense);

        if !blocked
            && !ctx.popup_open
            && !pointer_over_floating
            && (term_resp.clicked()
            || term_resp.drag_started()
            || (settings.show_scrollbar
                && (scrollbar_resp.clicked() || scrollbar_resp.drag_started()))
            || (!any_selecting_pane
                && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
                && ui
                    .input(|i| i.pointer.interact_pos())
                    .is_some_and(|p| pane_rect.contains(p)))
            || (any_selecting_pane
                && (term_resp.secondary_clicked()
                    || (settings.show_scrollbar && scrollbar_resp.secondary_clicked()))))
        {
            if let Some(focused_id) = ui.ctx().memory(|m| m.focused()) {
                if focused_id != canvas_id {
                    ui.ctx().memory_mut(|m| m.surrender_focus(focused_id));
                }
            }
            clicked_or_focused = true;
        }

        // Super キーの押下状態をラッチ追跡
        ui.input(|i| {
            for ev in &i.events {
                if let egui::Event::Key {
                    key,
                    physical_key,
                    pressed,
                    ..
                } = ev
                {
                    if matches!(key, egui::Key::SuperLeft | egui::Key::SuperRight)
                        || matches!(
                            physical_key,
                            Some(egui::Key::SuperLeft | egui::Key::SuperRight)
                        )
                    {
                        self.super_key_latched = *pressed;
                    }
                }
            }
            if !i.focused {
                self.super_key_latched = false;
            }
        });

        let pane_hovered = (term_resp.hovered()
            || (settings.show_scrollbar && scrollbar_resp.hovered()))
            && !pointer_over_floating
            && !blocked
            && !ctx.popup_open;
        let super_held = self.super_key_latched || ui.input(detect_super_held);

        let can_use_terminal = ctx.can_interact_terminal(self.editing_title, false);

        // Pinch-out and pinch-in zoom gestures on the active panel
        let zoom_delta = if can_use_terminal {
            let mut zd = ui.input(|i| i.zoom_delta()) * ctx.external_zoom_delta;
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
        }

        let term_font_size =
            (settings.effective_terminal_font_size() * self.zoom_scale).clamp(6.0, 64.0);
        let font_id = egui::FontId::monospace(term_font_size);
        // Stop hardcoding 0.6 and retrieve the actual width of a half-width character ('M' or '0') from the font.
        let char_w = ui
            .fonts_mut(|f| f.glyph_width(&font_id, '0').max(f.glyph_width(&font_id, 'M')))
            .max(4.0);

        let ink_max_y = ui.fonts_mut(|f| f.row_height(&font_id));
        let ink_h = ink_max_y.max(term_font_size * 0.85);
        let char_h = (ink_h + (term_font_size * 0.28).max(2.0))
            .round()
            .max(8.0);
        let glyph_y_offset = (char_h - ink_max_y) * 0.5;
        let pad = 2.0_f32;

        let inner_w = (rect.width() - pad * 2.0).max(0.0);
        let inner_h = (rect.height() - pad * 2.0).max(0.0);
        let new_cols: NonZeroU16 = NonZeroU16::new(((inner_w / char_w).floor() as u16).max(10)).unwrap();
        let new_rows: NonZeroU16 = NonZeroU16::new(((inner_h / char_h).floor() as u16).max(4)).unwrap();
        let (cur_rows, cur_cols): (NonZeroU16, NonZeroU16) = self.backend.size();
        if (new_rows, new_cols) != (cur_rows, cur_cols) {
            self.backend.resize(new_rows, new_cols);
            self.session.resize(new_rows, new_cols);
        }

        // Handle mouse drag selection on the terminal grid
        let (rows, cols) = self.backend.size();
        let pos_to_cell = |pos: egui::Pos2| -> (u16, u16) {
            let rel_x = (pos.x - (rect.min.x + pad)).max(0.0);
            let rel_y = (pos.y - (rect.min.y + pad)).max(0.0);
            let c = ((rel_x / char_w).floor() as u16).min(cols.get().saturating_sub(1));
            let r = ((rel_y / char_h).floor() as u16).min(rows.get().saturating_sub(1));
            (r, c)
        };

        let can_select_text = can_use_terminal && !pointer_over_floating;

        let primary_pressed = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        let primary_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
        let primary_released =
            ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
        let pointer_pos = ui.input(|i| {
            i.pointer
                .interact_pos()
                .or_else(|| i.pointer.latest_pos())
        });

        if primary_pressed && can_select_text {
            if let Some(pos) = pointer_pos {
                if rect.contains(pos) {
                    self.select_anchor_pos = Some(pos);
                    self.selection_start = None;
                    self.selection_end = None;
                    self.abs_selection_start = None;
                    self.abs_selection_end = None;
                }
            }
        }
        if primary_down && can_select_text {
            if let (Some(anchor), Some(pos)) = (self.select_anchor_pos, pointer_pos) {
                // 画面外へのドラッグによるオートスクロール処理
                let mut auto_scrolled = 0isize;
                if pos.y < rect.min.y {
                    let old_sc = self.backend.scroll_offset();
                    self.backend.scroll(1);
                    auto_scrolled = (self.backend.scroll_offset() as isize) - (old_sc as isize);
                    ui.ctx().request_repaint();
                } else if pos.y > rect.max.y {
                    let old_sc = self.backend.scroll_offset();
                    self.backend.scroll(-1);
                    auto_scrolled = (self.backend.scroll_offset() as isize) - (old_sc as isize);
                    ui.ctx().request_repaint();
                }

                let mut current_anchor = anchor;
                if auto_scrolled != 0 {
                    current_anchor.y += (auto_scrolled as f32) * char_h;
                    self.select_anchor_pos = Some(current_anchor);
                }

                if pos.distance(current_anchor) >= 2.0 {
                    let max_sc = self.backend.max_scroll_offset();
                    let cur_sc = self.backend.scroll_offset();
                    let view_top = max_sc.saturating_sub(cur_sc);

                    if self.abs_selection_start.is_none() {
                        let (ar, ac) = pos_to_cell(current_anchor);
                        self.abs_selection_start = Some((view_top + ar as usize, ac));
                    }
                    let (pr, pc) = pos_to_cell(pos);
                    self.abs_selection_end = Some((view_top + pr as usize, pc));
                    self.sync_viewport_selection_from_abs();
                }
            }
        }
        if primary_released || !can_select_text {
            self.select_anchor_pos = None;
        }

        // Handle mouse wheel / touchpad scrollback
        if can_use_terminal && pane_hovered && !super_held && (zoom_delta - 1.0).abs() <= 0.001 {
            let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_y.abs() > 0.01 {
                self.scroll_accum_y += scroll_y;
                let step_px = (char_h * 0.65).max(8.0);
                if self.scroll_accum_y.abs() >= step_px {
                    let delta_rows = (self.scroll_accum_y / step_px).trunc() as isize;
                    self.scroll_accum_y -= (delta_rows as f32) * step_px;
                    self.backend.scroll(delta_rows);
                    self.sync_viewport_selection_from_abs();
                }
            }
        }

        // Handle interactive scrollbar click/drag
        let max_scroll = self.backend.max_scroll_offset();
        let total_lines = (rows.get() as usize) + max_scroll;
        let track_pad = 2.0_f32;
        let track_h = (scrollbar_rect.height() - track_pad * 2.0).max(1.0);
        let visible_frac = if total_lines > 0 {
            ((rows.get() as f32) / (total_lines as f32)).clamp(0.05, 1.0)
        } else {
            1.0
        };
        let thumb_h = (track_h * visible_frac).clamp(18.0_f32.min(track_h), track_h);
        let scrollable_track_h = (track_h - thumb_h).max(1.0);

        if settings.show_scrollbar
            && can_select_text
            && max_scroll > 0
            && (scrollbar_resp.clicked() || scrollbar_resp.dragged())
        {
            if let Some(pos) = scrollbar_resp.interact_pointer_pos() {
                let rel_y = ((pos.y - (scrollbar_rect.min.y + track_pad) - thumb_h * 0.5)
                    / scrollable_track_h)
                    .clamp(0.0, 1.0);
                let target_offset = ((1.0 - rel_y) * (max_scroll as f32)).round() as usize;
                self.backend.set_scroll_offset(target_offset);
                self.sync_viewport_selection_from_abs();
            }
        }

        // 3. Context menu for the terminal screen canvas
        let mut menu_action_output = None;

        if !any_selecting_pane {
            if let Some((_ctx, menu_action)) =
                menus.show(&term_resp, MenuContext::Pane(tile_id), &settings.shortcuts)
            {
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
                        self.abs_selection_start = None;
                        self.abs_selection_end = None;
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
        }

        let pane_context_menu_open = !blocked
            && (tb_out.context_menu_open
            || term_resp.context_menu_opened()
            || egui::Popup::is_any_open(ui.ctx()));
        let can_interact = ctx.can_interact_terminal(self.editing_title, pane_context_menu_open);

        // Forward keyboard events to PTY when this pane can be interacted with
        let mut shortcut_key_action = None;
        if can_interact {
            if let Some(focused_id) = ui.ctx().memory(|m| m.focused()) {
                if focused_id != canvas_id {
                    ui.ctx().memory_mut(|m| m.surrender_focus(focused_id));
                }
            }
            let (shortcut_consumed, act) =
                self.handle_shortcut_keyboard_input(ui, &settings.shortcuts);
            shortcut_key_action = act;
            self.handle_keyboard_input(ui, shortcut_consumed || shortcut_key_action.is_some());
        } else if self.pending_paste {
            self.handle_keyboard_input(ui, true);
        }

        let painter = ui.painter_at(full_rect);
        painter.rect_filled(full_rect, 0.0, settings.terminal_bg_color_with_opacity());

        // Draw cells with Pre-Tessellated Mesh + Text Caching
        let (rows, cols) = self.backend.size();
        let norm_selection = self.normalized_selection();
        let cur_gen = self.backend.generation();
        let cur_scroll = self.backend.scroll_offset();

        let meta = GridCacheMeta {
            rect_min_x: (rect.min.x * 10.0).round() as i32,
            rect_min_y: (rect.min.y * 10.0).round() as i32,
            rows,
            cols,
            font_size_bits: term_font_size.to_bits(),
        };

        let layout_changed = self.grid_cache_meta.as_ref() != Some(&meta);
        if layout_changed {
            self.row_caches.clear();
            self.row_caches.resize_with(rows.get() as usize, RowCache::default);
            self.grid_cache_meta = Some(meta);
        }

        if layout_changed
            || cur_gen != self.last_rendered_gen
            || cur_scroll != self.last_rendered_scroll
            || norm_selection != self.last_rendered_sel
        {
            self.last_rendered_gen = cur_gen;
            self.last_rendered_scroll = cur_scroll;
            self.last_rendered_sel = norm_selection;

            let is_default_bg =
                |cell: &crate::vte::RenderableCell, bg: crate::vte::RgbColor| -> bool {
                    !cell.inverse
                        && (bg == crate::vte::RgbColor(18, 18, 18)
                        || bg == crate::vte::RgbColor(0, 0, 0))
                };

            let resolve_fg = |cell: &crate::vte::RenderableCell| -> egui::Color32 {
                let mut fg = if cell.inverse { cell.bg } else { cell.fg };
                if cell.bold {
                    fg = crate::vte::RgbColor(
                        fg.0.saturating_add(25),
                        fg.1.saturating_add(25),
                        fg.2.saturating_add(25),
                    );
                }
                egui::Color32::from_rgb(fg.0, fg.1, fg.2)
            };

            let mut row_cells = Vec::with_capacity(cols.get() as usize);

            for r in 0..rows.get() {
                row_cells.clear();
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                let cols_get: u16 = cols.get();

                let row_sel = norm_selection.and_then(|(start, end)| {
                    if r >= start.0 && r <= end.0 {
                        let sc = if r == start.0 { start.1 } else { 0 };
                        let ec = if r == end.0 {
                            end.1.min(cols_get.saturating_sub(1))
                        } else {
                            cols_get.saturating_sub(1)
                        };
                        (sc <= ec).then_some((sc, ec))
                    } else {
                        None
                    }
                });
                row_sel.hash(&mut hasher);

                for c in 0..cols_get {
                    let cell = self.backend.read_cell(r, c);
                    cell.ch.hash(&mut hasher);
                    (cell.fg.0, cell.fg.1, cell.fg.2).hash(&mut hasher);
                    (cell.bg.0, cell.bg.1, cell.bg.2).hash(&mut hasher);
                    (
                        cell.bold,
                        cell.underline,
                        cell.inverse,
                        cell.wide,
                        cell.wide_continuation,
                    )
                        .hash(&mut hasher);
                    row_cells.push(cell);
                }

                let row_hash = hasher.finish();
                let cache = &mut self.row_caches[r as usize];

                if !layout_changed && cache.content_hash == row_hash {
                    continue;
                }

                cache.content_hash = row_hash;
                cache.bg_and_lines.clear();
                cache.text_shapes.clear();

                let row_y = rect.min.y + pad + (r as f32) * char_h;

                // Pass 1 (Row): 背景矩形と選択範囲
                let mut c = 0;
                while c < cols_get {
                    let cell = &row_cells[c as usize];
                    let bg = if cell.inverse { cell.fg } else { cell.bg };
                    if !is_default_bg(cell, bg) {
                        let start_c = c;
                        c += 1;
                        while c < cols_get {
                            let next_cell = &row_cells[c as usize];
                            let next_bg = if next_cell.inverse {
                                next_cell.fg
                            } else {
                                next_cell.bg
                            };
                            if is_default_bg(next_cell, next_bg) || next_bg != bg {
                                break;
                            }
                            c += 1;
                        }
                        let run_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.min.x + pad + (start_c as f32) * char_w, row_y),
                            egui::vec2(((c - start_c) as f32) * char_w, char_h),
                        );
                        cache.bg_and_lines.push((
                            run_rect,
                            egui::Color32::from_rgb(bg.0, bg.1, bg.2),
                        ));
                    } else {
                        c += 1;
                    }
                }

                if let Some((sel_c_start, sel_c_end)) = row_sel {
                    let sel_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.min.x + pad + (sel_c_start as f32) * char_w, row_y),
                        egui::vec2(((sel_c_end - sel_c_start + 1) as f32) * char_w, char_h),
                    );
                    cache.bg_and_lines.push((
                        sel_rect,
                        egui::Color32::from_rgba_unmultiplied(70, 130, 210, 110),
                    ));
                }

                //Text rendering loop
                let mut c: u16 = 0;
                while c < cols.get() {
                    let cell = &row_cells[c as usize];
                    if cell.wide_continuation
                        || ((cell.ch == ' ' || cell.ch == '\0') && !cell.underline)
                    {
                        c += 1;
                        continue;
                    }

                    let fg_color = resolve_fg(cell);
                    let cell_x = rect.min.x + pad + (c as f32) * char_w;
                    let cell_span = if cell.wide { 2.0 } else { 1.0 };

                    if cell.ch != ' ' && cell.ch != '\0' {
                        let galley = ui.fonts_mut(|f| {
                            f.layout_no_wrap(cell.ch.to_string(), font_id.clone(), fg_color)
                        });
                        cache.text_shapes.push(egui::Shape::galley(
                            egui::pos2(cell_x, row_y + glyph_y_offset),
                            galley,
                            fg_color,
                        ));
                    }

                    if cell.underline {
                        let y = (row_y + glyph_y_offset + ink_max_y).min(row_y + char_h - 1.0);
                        let ul_rect = egui::Rect::from_min_size(
                            egui::pos2(cell_x, y),
                            egui::vec2(char_w * cell_span, 1.0),
                        );
                        cache.bg_and_lines.push((ul_rect, fg_color));
                    }

                    c += 1;
                }
            }

            let mut combined_mesh = egui::Mesh::with_texture(egui::TextureId::default());
            for row_cache in &self.row_caches {
                for &(r_rect, color) in &row_cache.bg_and_lines {
                    combined_mesh.add_colored_rect(r_rect, color);
                }
            }

            // テキストの Shape 群をその場で1回だけ Tessellate して combined_mesh に結合する
            let text_shapes: Vec<egui::epaint::ClippedShape> = self
                .row_caches
                .iter()
                .flat_map(|rc| rc.text_shapes.iter().cloned())
                .map(|shape| egui::epaint::ClippedShape {
                    clip_rect: egui::Rect::EVERYTHING,
                    shape,
                    transform_after_tessellation: Default::default(),
                })
                .collect();

            if !text_shapes.is_empty() {
                let pixels_per_point = ui.ctx().pixels_per_point();
                let primitives = ui.ctx().tessellate(text_shapes, pixels_per_point);
                for prim in primitives {
                    if let egui::epaint::Primitive::Mesh(mesh) = prim.primitive {
                        if mesh.texture_id == combined_mesh.texture_id {
                            combined_mesh.append(mesh);
                        }
                    }
                }
            }

            self.cached_terminal_mesh = Arc::new(combined_mesh);
        }

        if !self.cached_terminal_mesh.is_empty() {
            painter.add(egui::Shape::mesh(Arc::clone(&self.cached_terminal_mesh)));
        }

        // Draw cursor
        let cursor = self.backend.cursor();
        let cursor_visually_active = visually_active && !pane_context_menu_open;
        if cursor.visible && cursor.row < rows.get() && cursor.col < cols.get() {
            let cur_min = egui::pos2(
                rect.min.x + pad + (cursor.col as f32) * char_w,
                rect.min.y + pad + (cursor.row as f32) * char_h,
            );
            let cursor_cell = self.backend.read_cell(cursor.row, cursor.col);
            let cur_w = if cursor_cell.wide { char_w * 2.0 } else { char_w };
            let cur_rect = egui::Rect::from_min_size(cur_min, egui::vec2(cur_w, char_h));
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
        if settings.show_scrollbar {
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
        }

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

        // Highlight border during pane move mode or broadcast selection mode
        if ctx.is_moving_self() {
            painter.rect_stroke(
                full_rect.shrink(1.5),
                0.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(250, 204, 21)),
                egui::StrokeKind::Inside,
            );
        } else if ctx.is_selecting_broadcast_self() {
            painter.rect_stroke(
                full_rect.shrink(1.5),
                0.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(56, 189, 248)),
                egui::StrokeKind::Inside,
            );
        } else if any_selecting_pane && pointer_pos.is_some_and(|p| pane_rect.contains(p)) {
            painter.rect_stroke(
                full_rect.shrink(1.5),
                0.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(80, 180, 255)),
                egui::StrokeKind::Inside,
            );
        }

        PaneUiOutput {
            action: tb_out.action.or(shortcut_key_action).or(menu_action_output),
            toggle_tile_move: tb_out.toggle_tile_move,
            toggle_broadcast_select: tb_out.toggle_broadcast_select,
            clicked_or_focused,
            broadcast_input: std::mem::take(&mut self.pending_broadcast_input),
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

        for r in start.0..=end.0.min(rows.get().saturating_sub(1)) {
            let c_start = if r == start.0 { start.1 } else { 0 };
            let c_end = if r == end.0 {
                end.1.min(cols.get().saturating_sub(1))
            } else {
                cols.get().saturating_sub(1)
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

    /// スクロールバック領域を含む絶対行ベースでの選択テキストをすべて抽出する
    pub fn extract_full_selected_text(&mut self) -> Option<String> {
        if let (Some(a_start), Some(a_end)) = (self.abs_selection_start, self.abs_selection_end) {
            let (start, end) = if a_start <= a_end {
                (a_start, a_end)
            } else {
                (a_end, a_start)
            };

            let (rows, cols) = self.backend.size();
            let rows_usize = rows.get() as usize;
            let last_col = cols.get().saturating_sub(1);
            let orig_scroll = self.backend.scroll_offset();
            let max_scroll = self.backend.max_scroll_offset();
            let max_abs_row = max_scroll + rows_usize.saturating_sub(1);

            let start_r = start.0.min(max_abs_row);
            let end_r = end.0.min(max_abs_row);
            let mut lines = Vec::with_capacity(end_r.saturating_sub(start_r) + 1);

            for abs_r in start_r..=end_r {
                // 対象の絶対行 `abs_r` が画面内の行インデックス `local_r` に来るよう一時的にスクロール位置を変更
                let (target_scroll, local_r) = if abs_r < max_scroll {
                    (max_scroll - abs_r, 0u16)
                } else {
                    (0, (abs_r - max_scroll) as u16)
                };
                self.backend.set_scroll_offset(target_scroll);

                let c_start = if abs_r == start.0 { start.1.min(last_col) } else { 0 };
                let c_end = if abs_r == end.0 { end.1.min(last_col) } else { last_col };

                let mut line = String::new();
                if c_start <= c_end && local_r < rows.get() {
                    for c in c_start..=c_end {
                        let cell = self.backend.read_cell(local_r, c);
                        if !cell.wide_continuation {
                            line.push(if cell.ch == '\0' { ' ' } else { cell.ch });
                        }
                    }
                }
                lines.push(line.trim_end().to_string());
            }

            self.backend.set_scroll_offset(orig_scroll);
            return Some(lines.join("\n"));
        }

        self.extract_selected_text()
    }

    fn copy_selection_or_screen(&mut self, ctx: &egui::Context) {
        if let Some(selected) = self.extract_full_selected_text() {
            if !selected.is_empty() {
                ctx.copy_text(selected);
                return;
            }
        }
        let (rows, cols) = self.backend.size();
        let mut lines = Vec::new();
        for r in 0..rows.get() {
            let mut line = String::new();
            for c in 0..cols.get() {
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
        if ui.input(|i| {
            !i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(_) | egui::Event::Paste(_)))
        }) {
            return;
        }
        let events = ui.input(|i| i.events.clone());
        let current_mods = ui.input(|i| i.modifiers);
        let super_held = self.super_key_latched || ui.input(detect_super_held);

        for event in events {
            match event {
                egui::Event::Text(text) => {
                    if shortcut_handled || super_held || current_mods.ctrl || current_mods.alt {
                        continue;
                    }
                    if self.backend.scroll_offset() > 0 {
                        self.backend.set_scroll_offset(0);
                        self.sync_viewport_selection_from_abs();
                    }
                    self.write_pty_input(text.as_bytes());
                }
                egui::Event::Paste(text) => {
                    if self.pending_paste {
                        self.pending_paste = false;
                        if self.backend.scroll_offset() > 0 {
                            self.backend.set_scroll_offset(0);
                            self.sync_viewport_selection_from_abs();
                        }
                        self.write_pty_input(text.as_bytes());
                    } else if !shortcut_handled && !current_mods.shift && !super_held {
                        self.write_pty_input(&[0x16]);
                    }
                }
                _ => {}
            }
        }
    }

    fn try_match_configured_shortcuts(
        &mut self,
        ui: &mut egui::Ui,
        shortcuts: &ShortcutSettings,
        key: egui::Key,
        physical_key: Option<egui::Key>,
        modifiers: egui::Modifiers,
        super_held: bool,
        has_paste_event: bool,
        paste_text: Option<&str>,
    ) -> (bool, Option<TitleBarAction>) {
        if shortcut_matches_event(&shortcuts.copy, key, physical_key, modifiers, super_held) {
            self.copy_selection_or_screen(ui.ctx());
            return (true, None);
        }
        if shortcut_matches_event(&shortcuts.paste, key, physical_key, modifiers, super_held) {
            if let Some(text) = paste_text {
                if self.backend.scroll_offset() > 0 {
                    self.backend.set_scroll_offset(0);
                    self.sync_viewport_selection_from_abs();
                }
                self.write_pty_input(text.as_bytes());
                self.pending_paste = false;
            } else if !has_paste_event {
                self.pending_paste = true;
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            } else {
                self.pending_paste = true;
            }
            return (true, None);
        }

        if shortcut_matches_event(&shortcuts.split_horizontal, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::SplitHorizontal));
        }
        if shortcut_matches_event(&shortcuts.split_vertical, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::SplitVertical));
        }
        if shortcut_matches_event(&shortcuts.new_tab, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::NewTab));
        }
        if shortcut_matches_event(&shortcuts.close_pane, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::ClosePane));
        }
        if shortcut_matches_event(&shortcuts.focus_next_pane, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::FocusNextPane));
        }
        if shortcut_matches_event(&shortcuts.focus_prev_pane, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::FocusPrevPane));
        }
        if shortcut_matches_event(&shortcuts.open_preferences, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::OpenPreferences));
        }
        if shortcut_matches_event(&shortcuts.close_preferences, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::ClosePreferences));
        }
        if shortcut_matches_event(&shortcuts.toggle_collapse_preferences, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::ToggleCollapsePreferences));
        }
        if shortcut_matches_event(&shortcuts.toggle_fullscreen, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::ToggleMaximizeWindow));
        }
        if shortcut_matches_event(&shortcuts.zoom_in, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::ZoomIn));
        }
        if shortcut_matches_event(&shortcuts.zoom_out, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::ZoomOut));
        }
        if shortcut_matches_event(&shortcuts.zoom_reset, key, physical_key, modifiers, super_held) {
            return (true, Some(TitleBarAction::ZoomReset));
        }
        (false, None)
    }

    fn handle_shortcut_keyboard_input(
        &mut self,
        ui: &mut egui::Ui,
        shortcuts: &ShortcutSettings,
    ) -> (bool, Option<TitleBarAction>) {
        if ui.input(|i| {
            !i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key { pressed: true, .. }
                        | egui::Event::Copy
                        | egui::Event::Cut
                        | egui::Event::Paste(_)
                )
            })
        }) {
            return (false, None);
        }

        let mut action = None;
        let mut any_shortcut_consumed = false;

        let events = ui.input(|i| i.events.clone());
        let current_mods = ui.input(|i| i.modifiers);
        let super_held = self.super_key_latched || ui.input(detect_super_held);
        let has_paste_event = events.iter().any(|e| matches!(e, egui::Event::Paste(_)));

        for event in &events {
            match event {
                egui::Event::Copy => {
                    let mut mods = current_mods;
                    if !super_held {
                        mods.ctrl = true;
                    }
                    let (matched, act) = self.try_match_configured_shortcuts(
                        ui,
                        shortcuts,
                        egui::Key::C,
                        Some(egui::Key::C),
                        mods,
                        super_held,
                        has_paste_event,
                        None,
                    );
                    if matched {
                        any_shortcut_consumed = true;
                        if act.is_some() {
                            action = act;
                        }
                    } else if !mods.shift && !super_held {
                        self.write_pty_input(&[0x03]);
                    }
                }
                egui::Event::Cut => {
                    let mut mods = current_mods;
                    if !super_held {
                        mods.ctrl = true;
                    }
                    let (matched, act) = self.try_match_configured_shortcuts(
                        ui,
                        shortcuts,
                        egui::Key::X,
                        Some(egui::Key::X),
                        mods,
                        super_held,
                        has_paste_event,
                        None,
                    );
                    if matched {
                        any_shortcut_consumed = true;
                        if act.is_some() {
                            action = act;
                        }
                    } else if !mods.shift && !super_held {
                        self.write_pty_input(&[0x18]);
                    }
                }
                egui::Event::Paste(text) => {
                    if self.pending_paste {
                        continue;
                    }
                    let mut mods = current_mods;
                    if !super_held {
                        mods.ctrl = true;
                    }
                    let (matched, act) = self.try_match_configured_shortcuts(
                        ui,
                        shortcuts,
                        egui::Key::V,
                        Some(egui::Key::V),
                        mods,
                        super_held,
                        true,
                        Some(text.as_str()),
                    );
                    if matched {
                        any_shortcut_consumed = true;
                        if act.is_some() {
                            action = act;
                        }
                    }
                }
                egui::Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let key = *key;
                    let physical_key = *physical_key;
                    let merged_mods = egui::Modifiers {
                        alt: modifiers.alt || current_mods.alt,
                        ctrl: modifiers.ctrl || current_mods.ctrl,
                        shift: modifiers.shift || current_mods.shift,
                        mac_cmd: modifiers.mac_cmd || current_mods.mac_cmd,
                        command: modifiers.command || current_mods.command,
                    };
                    let ev_super = super_held
                        || merged_mods.mac_cmd
                        || (merged_mods.command && !merged_mods.ctrl);

                    let (matched, act) = self.try_match_configured_shortcuts(
                        ui,
                        shortcuts,
                        key,
                        physical_key,
                        merged_mods,
                        ev_super,
                        has_paste_event,
                        None,
                    );
                    if matched {
                        any_shortcut_consumed = true;
                        if act.is_some() {
                            action = act;
                        }
                        continue;
                    }

                    if ev_super {
                        continue;
                    }

                    if merged_mods.ctrl && !merged_mods.shift && !merged_mods.alt {
                        let effective_key = physical_key.unwrap_or(key);
                        let key_name = effective_key.name();
                        if key_name.len() == 1 {
                            let ch = key_name.chars().next().unwrap();
                            if ch.is_ascii_uppercase() {
                                let ctrl_code = (ch as u8) & 0x1F;
                                self.write_pty_input(&[ctrl_code]);
                            }
                        } else {
                            match effective_key {
                                egui::Key::Space => self.write_pty_input(&[0x00]),
                                egui::Key::Slash => self.write_pty_input(&[0x1f]),
                                egui::Key::OpenBracket => self.write_pty_input(&[0x1b]),
                                egui::Key::Backslash => self.write_pty_input(&[0x1c]),
                                egui::Key::CloseBracket => self.write_pty_input(&[0x1d]),
                                _ => {}
                            }
                        }
                    } else if merged_mods.alt && !merged_mods.ctrl && !merged_mods.shift {
                        let effective_key = physical_key.unwrap_or(key);
                        let key_name = effective_key.name();
                        if key_name.len() == 1 {
                            let ch = key_name.chars().next().unwrap().to_ascii_lowercase();
                            self.write_pty_input(&[0x1b, ch as u8]);
                        }
                    } else if merged_mods.shift && !merged_mods.ctrl && !merged_mods.alt {
                        let (rows, _) = self.backend.size();
                        let page_step = (rows.get() as isize / 2).max(1);
                        match key {
                            egui::Key::PageUp => {
                                self.backend.scroll(page_step);
                                self.sync_viewport_selection_from_abs();
                            }
                            egui::Key::PageDown => {
                                self.backend.scroll(-page_step);
                                self.sync_viewport_selection_from_abs();
                            }
                            egui::Key::ArrowUp => {
                                self.backend.scroll(1);
                                self.sync_viewport_selection_from_abs();
                            }
                            egui::Key::ArrowDown => {
                                self.backend.scroll(-1);
                                self.sync_viewport_selection_from_abs();
                            }
                            egui::Key::Home => {
                                self.backend.set_scroll_offset(self.backend.max_scroll_offset());
                                self.sync_viewport_selection_from_abs();
                            }
                            egui::Key::End => {
                                self.backend.set_scroll_offset(0);
                                self.sync_viewport_selection_from_abs();
                            }
                            egui::Key::Insert => {
                                self.pending_paste = true;
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                            }
                            egui::Key::Tab => {
                                self.write_pty_input(b"\x1b[Z");
                            }
                            _ => {}
                        }
                    } else if merged_mods.is_none() {
                        if self.backend.scroll_offset() > 0 {
                            self.backend.set_scroll_offset(0);
                            self.sync_viewport_selection_from_abs();
                        }
                        match key {
                            egui::Key::Enter => self.write_pty_input(b"\r"),
                            egui::Key::Backspace => self.write_pty_input(&[0x7f]),
                            egui::Key::Tab => self.write_pty_input(b"\t"),
                            egui::Key::Escape => self.write_pty_input(&[0x1b]),
                            egui::Key::ArrowUp => self.write_pty_input(b"\x1b[A"),
                            egui::Key::ArrowDown => self.write_pty_input(b"\x1b[B"),
                            egui::Key::ArrowRight => self.write_pty_input(b"\x1b[C"),
                            egui::Key::ArrowLeft => self.write_pty_input(b"\x1b[D"),
                            egui::Key::Home => self.write_pty_input(b"\x1b[H"),
                            egui::Key::End => self.write_pty_input(b"\x1b[F"),
                            egui::Key::Insert => self.write_pty_input(b"\x1b[2~"),
                            egui::Key::Delete => self.write_pty_input(b"\x1b[3~"),
                            egui::Key::PageUp => self.write_pty_input(b"\x1b[5~"),
                            egui::Key::PageDown => self.write_pty_input(b"\x1b[6~"),
                            egui::Key::F1 => self.write_pty_input(b"\x1bOP"),
                            egui::Key::F2 => self.write_pty_input(b"\x1bOQ"),
                            egui::Key::F3 => self.write_pty_input(b"\x1bOR"),
                            egui::Key::F4 => self.write_pty_input(b"\x1bOS"),
                            egui::Key::F5 => self.write_pty_input(b"\x1b[15~"),
                            egui::Key::F6 => self.write_pty_input(b"\x1b[17~"),
                            egui::Key::F7 => self.write_pty_input(b"\x1b[18~"),
                            egui::Key::F8 => self.write_pty_input(b"\x1b[19~"),
                            egui::Key::F9 => self.write_pty_input(b"\x1b[20~"),
                            egui::Key::F10 => self.write_pty_input(b"\x1b[21~"),
                            egui::Key::F11 => self.write_pty_input(b"\x1b[23~"),
                            egui::Key::F12 => self.write_pty_input(b"\x1b[24~"),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        (any_shortcut_consumed, action)
    }

    #[allow(dead_code)]
    fn send_paste_text(&mut self, text: &str) {
        if self.backend.scroll_offset() > 0 {
            self.backend.set_scroll_offset(0);
            self.sync_viewport_selection_from_abs();
        }
        let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
        if self.backend.bracketed_paste_enabled() {
            let mut payload = Vec::with_capacity(normalized.len() + 12);
            payload.extend_from_slice(b"\x1b[200~");
            payload.extend_from_slice(normalized.as_bytes());
            payload.extend_from_slice(b"\x1b[201~");
            self.write_pty_input(&payload);
        } else {
            self.write_pty_input(normalized.as_bytes());
        }
    }
}

#[cfg(test)]
#[path = "test/pane_tests.rs"]
mod tests;