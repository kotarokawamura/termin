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
use std::collections::HashMap;
use eframe::egui;
use egui_tiles::{Tile, TileId, Tiles, Tree, UiResponse};
use crate::config::theme::ThemeSettings;
use crate::menus::MenuManager;
use crate::ui::pane::{default_shell_title, TerminalPane};
use crate::ui::state::{InteractionMode, PaneContext};
use crate::ui::titlebar::TitleBarAction;

#[derive(Clone, Copy)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

pub struct Workspace {
    pub tree: Tree<TerminalPane>,
    pub focused_tile: Option<TileId>,
    pub top_right_tile: Option<TileId>,
    pub next_pane_number: usize,
    pub(crate) recorded_rects: HashMap<TileId, egui::Rect>,
    pub moving_tile: Option<TileId>,
    pub pty_wakeup_ctx: Option<egui::Context>,
}

impl Workspace {
    pub fn new() -> Self {
        Self::new_with_wakeup(None)
    }

    pub fn new_with_wakeup(pty_wakeup_ctx: Option<egui::Context>) -> Self {
        let mut tiles = Tiles::default();
        let first_pane = TerminalPane::new_with_wakeup(
            1,
            &default_shell_title(),
            None,
            pty_wakeup_ctx.clone(),
        );
        let first_id = tiles.insert_pane(first_pane);
        let root = tiles.insert_horizontal_tile(vec![first_id]);
        let tree = Tree::new("termin_workspace", root, tiles);

        Self {
            tree,
            focused_tile: Some(first_id),
            top_right_tile: Some(first_id),
            next_pane_number: 2,
            recorded_rects: HashMap::new(),
            moving_tile: None,
            pty_wakeup_ctx,
        }
    }

    /// Poll all panes for PTY output or shell exit (`Ctrl+D`).
    pub fn poll_sessions(&mut self) -> bool {
        let mut needs_repaint = false;
        let mut exited_tiles = Vec::new();

        for (tile_id, tile) in self.tree.tiles.iter_mut() {
            if let Tile::Pane(pane) = tile {
                if pane.poll_pty() {
                    needs_repaint = true;
                }
                if !pane.is_alive() {
                    exited_tiles.push(*tile_id);
                    needs_repaint = true;
                }
            }
        }

        for tile_id in exited_tiles {
            self.remove_pane(tile_id);
        }

        needs_repaint
    }

    /// 現在のフレームにおけるワークスペースの `InteractionMode` を算出する
    fn current_interaction_mode(
        &self,
        other_screen_open: bool,
        prefs_capturing_pointer: bool,
    ) -> InteractionMode {
        if other_screen_open || prefs_capturing_pointer {
            return InteractionMode::BlockedByModal;
        }
        if let Some((editing_id, _)) = self
            .tree
            .tiles
            .iter()
            .find(|(_, tile)| matches!(tile, Tile::Pane(p) if p.editing_title))
        {
            return InteractionMode::EditingTitle(*editing_id);
        }
        if let Some(moving_id) = self.moving_tile {
            return InteractionMode::MovingPane(moving_id);
        }
        InteractionMode::Normal
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        settings: &mut ThemeSettings,
        menus: &MenuManager,
        prefs_open: &mut bool,
        other_screen_open: bool,
        prefs_capturing_pointer: bool,
        _prefs_dragging: bool,
        external_zoom_delta: f32,
    ) {
        self.recorded_rects.clear();
        self.ensure_valid_focus();

        let window_focused = ui.input(|i| i.focused);
        let popup_open = egui::Popup::is_any_open(ctx);

        // フォーカス喪失・Esc押下・別画面オープン時はパネル移動モードを解除
        if !window_focused
            || other_screen_open
            || popup_open
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
            || self
            .moving_tile
            .is_some_and(|id| !matches!(self.tree.tiles.get(id), Some(Tile::Pane(_))))
        {
            self.moving_tile = None;
        }

        let mode = self.current_interaction_mode(other_screen_open, prefs_capturing_pointer);

        let mut behavior = TreeBehavior {
            settings,
            menus,
            focused_tile: &mut self.focused_tile,
            top_right_tile: self.top_right_tile,
            recorded_rects: &mut self.recorded_rects,
            pending_action: None,
            pending_move_toggle: None,
            pending_move_target: None,
            prefs_open,
            window_focused,
            popup_open,
            mode,
            external_zoom_delta,
        };

        self.tree.ui(&mut behavior, ui);

        let pending_action = behavior.pending_action;
        let pending_move_toggle = behavior.pending_move_toggle;
        let pending_move_target = behavior.pending_move_target;

        // 1. パネル移動モード（[ ✥ ]）の状態遷移を処理
        if let Some(toggled_id) = pending_move_toggle {
            match self.moving_tile {
                Some(same_id) if same_id == toggled_id => {
                    self.moving_tile = None;
                    self.focused_tile = Some(toggled_id);
                }
                Some(src_id) => {
                    self.swap_panes(src_id, toggled_id);
                    self.moving_tile = None;
                }
                None => {
                    self.moving_tile = Some(toggled_id);
                    self.focused_tile = Some(toggled_id);
                }
            }
        } else if let Some(target_id) = pending_move_target {
            if let Some(src_id) = self.moving_tile {
                self.swap_panes(src_id, target_id);
                self.moving_tile = None;
            }
        }

        // 2. パネル・タイトルバーからのアクションを実行
        if let Some((tile_id, action)) = pending_action {
            self.execute_action(ctx, tile_id, action, prefs_open);
        }

        // 3. ツリー簡略化後のフォーカス検証と右上パネルの再計算
        self.ensure_valid_focus();
        self.update_top_right_tile();

        if self.pane_count() == 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn execute_action(
        &mut self,
        ctx: &egui::Context,
        tile_id: TileId,
        action: TitleBarAction,
        prefs_open: &mut bool,
    ) {
        match action {
            TitleBarAction::ClosePane => {
                self.remove_pane(tile_id);
                if self.pane_count() == 0 {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            TitleBarAction::SplitHorizontal => {
                self.split_pane(tile_id, SplitAxis::Horizontal);
            }
            TitleBarAction::SplitVertical => {
                self.split_pane(tile_id, SplitAxis::Vertical);
            }
            TitleBarAction::NewTab => {
                self.add_tab(tile_id);
            }
            TitleBarAction::FocusNextPane => {
                self.focus_adjacent_pane(1);
            }
            TitleBarAction::FocusPrevPane => {
                self.focus_adjacent_pane(-1);
            }
            TitleBarAction::SaveLayout => {
                self.save_layout();
            }
            TitleBarAction::LoadLayout => {
                self.load_layout();
            }
            TitleBarAction::MinimizeWindow => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            TitleBarAction::ToggleMaximizeWindow => {
                let is_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_max));
            }
            TitleBarAction::StartWindowMove => {
                ctx.stop_dragging();
                let pos = ctx
                    .input(|i| i.pointer.latest_pos())
                    .unwrap_or(egui::Pos2::ZERO);
                ctx.input_mut(|i| {
                    i.events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    });
                });
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            TitleBarAction::OpenPreferences => {
                *prefs_open = true;
            }
            TitleBarAction::ZoomIn => {
                if let Some(Tile::Pane(pane)) = self.tree.tiles.get_mut(tile_id) {
                    pane.zoom_in();
                }
            }
            TitleBarAction::ZoomOut => {
                if let Some(Tile::Pane(pane)) = self.tree.tiles.get_mut(tile_id) {
                    pane.zoom_out();
                }
            }
            TitleBarAction::ZoomReset => {
                if let Some(Tile::Pane(pane)) = self.tree.tiles.get_mut(tile_id) {
                    pane.zoom_reset();
                }
            }
        }
    }
}

struct TreeBehavior<'a> {
    settings: &'a ThemeSettings,
    menus: &'a MenuManager,
    focused_tile: &'a mut Option<TileId>,
    top_right_tile: Option<TileId>,
    recorded_rects: &'a mut HashMap<TileId, egui::Rect>,
    pending_action: Option<(TileId, TitleBarAction)>,
    pending_move_toggle: Option<TileId>,
    pending_move_target: Option<TileId>,
    prefs_open: &'a mut bool,
    window_focused: bool,
    popup_open: bool,
    mode: InteractionMode,
    external_zoom_delta: f32,
}

impl<'a> egui_tiles::Behavior<TerminalPane> for TreeBehavior<'a> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        tile_id: TileId,
        pane: &mut TerminalPane,
    ) -> UiResponse {
        self.recorded_rects.insert(tile_id, ui.max_rect());

        let pane_ctx = PaneContext {
            tile_id,
            is_focused: *self.focused_tile == Some(tile_id),
            is_top_right: self.top_right_tile == Some(tile_id) || self.top_right_tile.is_none(),
            window_focused: self.window_focused,
            popup_open: self.popup_open,
            mode: self.mode,
            external_zoom_delta: self.external_zoom_delta,
        };

        let output = pane.ui(ui, &pane_ctx, self.menus, self.settings);

        if self.mode != InteractionMode::BlockedByModal {
            if output.toggle_tile_move && !self.popup_open {
                self.pending_move_toggle = Some(tile_id);
            } else if pane_ctx.is_any_moving() && !self.popup_open {
                if output.clicked_or_focused {
                    self.pending_move_target = Some(tile_id);
                }
            } else {
                if output.clicked_or_focused {
                    *self.focused_tile = Some(tile_id);
                }

                if let Some(action) = output.action {
                    if matches!(action, TitleBarAction::OpenPreferences) {
                        *self.prefs_open = true;
                    } else {
                        self.pending_action = Some((tile_id, action));
                    }
                }
            }
        }

        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &TerminalPane) -> egui::WidgetText {
        pane.title.clone().into()
    }

    fn is_tile_draggable(&self, _tiles: &Tiles<TerminalPane>, _tile_id: TileId) -> bool {
        false
    }

    fn is_container_resizable(&self, _tiles: &Tiles<TerminalPane>, _tile_id: TileId) -> bool {
        self.mode == InteractionMode::Normal && !self.popup_open && self.window_focused
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        self.settings.effective_separator_width()
    }

    fn resize_stroke(
        &self,
        _style: &egui::Style,
        resize_state: egui_tiles::ResizeState,
    ) -> egui::Stroke {
        let width = self.settings.effective_separator_width();
        let base_color = self.settings.separator_color();
        let color = match resize_state {
            egui_tiles::ResizeState::Idle => base_color,
            egui_tiles::ResizeState::Hovering => egui::Color32::from_rgb(
                base_color.r().saturating_add(45),
                base_color.g().saturating_add(45),
                base_color.b().saturating_add(45),
            ),
            egui_tiles::ResizeState::Dragging => egui::Color32::from_rgb(
                base_color.r().saturating_add(85),
                base_color.g().saturating_add(85),
                base_color.b().saturating_add(85),
            ),
        };
        egui::Stroke::new(width, color)
    }

    fn tab_bar_hline_stroke(&self, _visuals: &egui::Visuals) -> egui::Stroke {
        egui::Stroke::new(1.0, self.settings.separator_color())
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: false,
            ..Default::default()
        }
    }
}