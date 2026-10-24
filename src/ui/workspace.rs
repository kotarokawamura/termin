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
use egui_tiles::{Container, Tile, TileId, Tiles, Tree, UiResponse};
use crate::config::layout_preset::LayoutNode;
use crate::config::theme::ThemeSettings;
use crate::menus::MenuManager;
use crate::ui::pane::{default_shell_title, TerminalPane};
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
    recorded_rects: HashMap<TileId, egui::Rect>,
    pub moving_tile: Option<TileId>,
}

impl Workspace {
    pub fn new() -> Self {
        let mut tiles = Tiles::default();
        let first_pane = TerminalPane::new(1, &default_shell_title());
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

        let window_focused = ui.input(|i| i.focused);
        let popup_open = egui::Popup::is_any_open(ctx);

        // Cancel pane move mode if window loses focus, Escape is pressed, or another screen opens
        if !window_focused
            || other_screen_open
            || popup_open
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.moving_tile = None;
        }

        if let Some(m_id) = self.moving_tile {
            if self.tree.tiles.get(m_id).is_none() {
                self.moving_tile = None;
            }
        }

        let any_editing_title = self
            .tree
            .tiles
            .iter()
            .any(|(_, tile)| matches!(tile, Tile::Pane(p) if p.editing_title));

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
            other_screen_open,
            prefs_capturing_pointer,
            popup_open,
            window_focused,
            any_editing_title,
            moving_tile: self.moving_tile,
            external_zoom_delta,
        };

        self.tree.ui(&mut behavior, ui);

        let pending_action = behavior.pending_action;
        let pending_move_toggle = behavior.pending_move_toggle;
        let pending_move_target = behavior.pending_move_target;

        // Handle pane move mode state transitions
        if let Some(toggled_id) = pending_move_toggle {
            if self.moving_tile == Some(toggled_id) {
                // Clicked [ ✥ ] on the same pane again -> confirm at current position and keep active
                self.moving_tile = None;
                self.focused_tile = Some(toggled_id);
            } else if let Some(src_id) = self.moving_tile {
                // Clicked [ ✥ ] on another pane while already in move mode -> swap and activate moved pane
                self.swap_panes(src_id, toggled_id);
                self.moving_tile = None;
            } else {
                // Enter pane move mode
                self.moving_tile = Some(toggled_id);
                self.focused_tile = Some(toggled_id);
            }
        } else if let Some(target_id) = pending_move_target {
            if let Some(src_id) = self.moving_tile {
                self.swap_panes(src_id, target_id);
                self.moving_tile = None;
            }
        }

        // Recompute the top-right pane from this frame's allocated rects
        self.update_top_right_tile();

        // Execute deferred tree mutations
        if let Some((tile_id, action)) = pending_action {
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
                    // Clear egui's internal pointer-down & drag state before handing grab to OS compositor
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

        if self.pane_count() == 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Swap two panes in the tree and activate the moved pane at its new destination.
    fn swap_panes(&mut self, src_id: TileId, dst_id: TileId) {
        if src_id == dst_id {
            self.focused_tile = Some(src_id);
            return;
        }
        let src_tile = self.tree.tiles.remove(src_id);
        let dst_tile = self.tree.tiles.remove(dst_id);
        match (src_tile, dst_tile) {
            (Some(s), Some(d)) => {
                // Put the moved pane (`s`) into `dst_id` and `d` into `src_id`
                self.tree.tiles.insert(dst_id, s);
                self.tree.tiles.insert(src_id, d);
                // The moved pane now lives at `dst_id`, so focus `dst_id`
                self.focused_tile = Some(dst_id);
            }
            (Some(s), None) => {
                self.tree.tiles.insert(src_id, s);
            }
            (None, Some(d)) => {
                self.tree.tiles.insert(dst_id, d);
            }
            (None, None) => {}
        }
    }

    pub fn save_layout(&self) {
        if let Some(layout) = LayoutNode::from_tree(&self.tree) {
            let _ = layout.save_default();
        }
    }

    pub fn load_layout(&mut self) {
        if let Some(layout) = LayoutNode::load_default() {
            let mut next_num = 1;
            let (new_tree, first_pane) = layout.build_tree(&mut next_num);
            self.tree = new_tree;
            self.next_pane_number = next_num;
            self.focused_tile = first_pane;
            self.top_right_tile = first_pane;
            self.moving_tile = None;
            self.recorded_rects.clear();
        }
    }

    /// Move focus to the next (`offset = 1`) or previous (`offset = -1`) pane in tree order.
    pub fn focus_adjacent_pane(&mut self, offset: isize) {
        let mut ordered_panes = Vec::new();
        if let Some(root_id) = self.tree.root() {
            Self::collect_panes_in_order(&self.tree.tiles, root_id, &mut ordered_panes);
        }
        if ordered_panes.is_empty() {
            ordered_panes.extend(
                self.tree
                    .tiles
                    .iter()
                    .filter_map(|(id, t)| matches!(t, Tile::Pane(_)).then_some(*id)),
            );
        }
        if ordered_panes.is_empty() {
            return;
        }

        let cur_idx = self
            .focused_tile
            .and_then(|cur| ordered_panes.iter().position(|&id| id == cur))
            .unwrap_or(0);
        let len = ordered_panes.len() as isize;
        let next_idx = (cur_idx as isize + offset).rem_euclid(len) as usize;
        let target_id = ordered_panes[next_idx];

        self.focused_tile = Some(target_id);

        for (_, tile) in self.tree.tiles.iter_mut() {
            if let Tile::Container(Container::Tabs(tabs)) = tile {
                if tabs.children.contains(&target_id) {
                    tabs.set_active(target_id);
                }
            }
        }
    }

    fn collect_panes_in_order(
        tiles: &Tiles<TerminalPane>,
        tile_id: TileId,
        out: &mut Vec<TileId>,
    ) {
        let Some(tile) = tiles.get(tile_id) else {
            return;
        };
        match tile {
            Tile::Pane(_) => {
                if !out.contains(&tile_id) {
                    out.push(tile_id);
                }
            }
            Tile::Container(container) => {
                for &child_id in container.children() {
                    Self::collect_panes_in_order(tiles, child_id, out);
                }
            }
        }
    }

    fn pane_count(&self) -> usize {
        self.tree
            .tiles
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Pane(_)))
            .count()
    }

    fn update_top_right_tile(&mut self) {
        if self.recorded_rects.is_empty() {
            return;
        }

        let min_y = self
            .recorded_rects
            .values()
            .map(|r| r.min.y)
            .fold(f32::INFINITY, f32::min);

        let best = self
            .recorded_rects
            .iter()
            .filter(|(_, rect)| rect.min.y <= min_y + 20.0)
            .max_by(|(_, a), (_, b)| {
                a.max
                    .x
                    .partial_cmp(&b.max.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| *id);

        if best.is_some() {
            self.top_right_tile = best;
        }
    }

    fn remove_pane(&mut self, target_id: TileId) {
        self.tree.tiles.remove(target_id);

        for (_, tile) in self.tree.tiles.iter_mut() {
            if let Tile::Container(container) = tile {
                container.retain(|child_id| child_id != target_id);
            }
        }

        let fallback_pane = self
            .tree
            .tiles
            .iter()
            .find_map(|(id, t)| matches!(t, Tile::Pane(_)).then_some(*id));

        if self.focused_tile == Some(target_id) {
            self.focused_tile = fallback_pane;
        }
        if self.top_right_tile == Some(target_id) {
            self.top_right_tile = fallback_pane;
        }
        if self.moving_tile == Some(target_id) {
            self.moving_tile = None;
        }
    }

    fn split_pane(&mut self, target_id: TileId, axis: SplitAxis) {
        let Some(old_tile) = self.tree.tiles.remove(target_id) else {
            return;
        };

        let moved_existing_id = self.tree.tiles.insert_new(old_tile);

        let id_num = self.next_pane_number;
        self.next_pane_number += 1;
        let new_pane = TerminalPane::new(id_num, &default_shell_title());
        let new_pane_id = self.tree.tiles.insert_pane(new_pane);

        let container_tile = match axis {
            SplitAxis::Horizontal => Tile::Container(egui_tiles::Container::new_horizontal(
                vec![moved_existing_id, new_pane_id],
            )),
            SplitAxis::Vertical => Tile::Container(egui_tiles::Container::new_vertical(
                vec![moved_existing_id, new_pane_id],
            )),
        };

        self.tree.tiles.insert(target_id, container_tile);
        self.focused_tile = Some(new_pane_id);
    }

    fn add_tab(&mut self, target_id: TileId) {
        let id_num = self.next_pane_number;
        self.next_pane_number += 1;
        let new_pane = TerminalPane::new(id_num, &default_shell_title());
        let new_pane_id = self.tree.tiles.insert_pane(new_pane);

        for (_, tile) in self.tree.tiles.iter_mut() {
            if let Tile::Container(Container::Tabs(tabs)) = tile {
                if tabs.children.contains(&target_id) {
                    tabs.add_child(new_pane_id);
                    tabs.set_active(new_pane_id);
                    self.focused_tile = Some(new_pane_id);
                    return;
                }
            }
        }

        let Some(old_tile) = self.tree.tiles.remove(target_id) else {
            return;
        };
        let moved_existing_id = self.tree.tiles.insert_new(old_tile);
        let mut tabs = egui_tiles::Tabs::new(vec![moved_existing_id, new_pane_id]);
        tabs.set_active(new_pane_id);
        self.tree
            .tiles
            .insert(target_id, Tile::Container(Container::Tabs(tabs)));
        self.focused_tile = Some(new_pane_id);
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
    other_screen_open: bool,
    prefs_capturing_pointer: bool,
    popup_open: bool,
    window_focused: bool,
    any_editing_title: bool,
    moving_tile: Option<TileId>,
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

        let is_focused = *self.focused_tile == Some(tile_id);
        let is_top_right = self.top_right_tile == Some(tile_id) || self.top_right_tile.is_none();

        let output = pane.ui(
            ui,
            tile_id,
            self.menus,
            self.settings,
            is_focused,
            is_top_right,
            self.other_screen_open,
            self.prefs_capturing_pointer,
            self.any_editing_title,
            self.moving_tile,
            self.external_zoom_delta,
        );

        if !self.other_screen_open && !self.prefs_capturing_pointer {
            if output.toggle_tile_move && !self.popup_open {
                self.pending_move_toggle = Some(tile_id);
            } else if self.moving_tile.is_some() && !self.popup_open {
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
        // Disable egui_tiles' built-in hold-drag in favor of click-to-move mode
        false
    }

    fn is_container_resizable(&self, _tiles: &Tiles<TerminalPane>, _tile_id: TileId) -> bool {
        !self.other_screen_open
            && !self.popup_open
            && !self.prefs_capturing_pointer
            && !self.any_editing_title
            && self.moving_tile.is_none()
            && self.window_focused
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