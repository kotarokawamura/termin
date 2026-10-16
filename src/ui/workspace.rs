use std::collections::HashMap;
use eframe::egui;
use egui_tiles::{Tile, TileId, Tiles, Tree, UiResponse};
use crate::config::theme::ThemeSettings;
use crate::ui::pane::TerminalPane;
use crate::ui::titlebar::TitlebarAction;

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
}

impl Workspace {
    pub fn new() -> Self {
        let mut tiles = Tiles::default();
        let first_pane = TerminalPane::new(1, "Terminal 1");
        let first_id = tiles.insert_pane(first_pane);
        let root = tiles.insert_horizontal_tile(vec![first_id]);
        let tree = Tree::new("terminator_workspace", root, tiles);

        Self {
            tree,
            focused_tile: Some(first_id),
            top_right_tile: Some(first_id),
            next_pane_number: 2,
            recorded_rects: HashMap::new(),
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

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        settings: &ThemeSettings,
        prefs_open: &mut bool,
    ) {
        self.recorded_rects.clear();

        let mut behavior = TreeBehavior {
            settings,
            focused_tile: &mut self.focused_tile,
            top_right_tile: self.top_right_tile,
            recorded_rects: &mut self.recorded_rects,
            pending_action: None,
            prefs_open,
        };

        self.tree.ui(&mut behavior, ui);

        let pending_action = behavior.pending_action;

        // Recompute the top-right pane from this frame's allocated rects
        self.update_top_right_tile();

        // Execute deferred tree mutations (split, close, window controls)
        if let Some((tile_id, action)) = pending_action {
            match action {
                TitlebarAction::ClosePane => {
                    self.remove_pane(tile_id);
                    if self.pane_count() == 0 {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                TitlebarAction::SplitHorizontal => {
                    self.split_pane(tile_id, SplitAxis::Horizontal);
                }
                TitlebarAction::SplitVertical => {
                    self.split_pane(tile_id, SplitAxis::Vertical);
                }
                TitlebarAction::MinimizeWindow => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                TitlebarAction::ToggleMaximizeWindow => {
                    let is_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_max));
                }
                TitlebarAction::StartWindowMove => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                TitlebarAction::OpenPreferences => {
                    *prefs_open = true;
                }
            }
        }

        if self.pane_count() == 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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

        if self.focused_tile == Some(target_id) {
            self.focused_tile = self
                .tree
                .tiles
                .iter()
                .find_map(|(id, t)| matches!(t, Tile::Pane(_)).then_some(*id));
        }
    }

    fn split_pane(&mut self, target_id: TileId, axis: SplitAxis) {
        let Some(old_tile) = self.tree.tiles.remove(target_id) else {
            return;
        };

        let moved_existing_id = self.tree.tiles.insert_new(old_tile);

        let id_num = self.next_pane_number;
        self.next_pane_number += 1;
        let new_pane = TerminalPane::new(id_num, &format!("Terminal {}", id_num));
        let new_pane_id = self.tree.tiles.insert_pane(new_pane);

        let container_tile = match axis {
            SplitAxis::Horizontal => Tile::Container(egui_tiles::Container::new_horizontal(
                vec![moved_existing_id, new_pane_id],
            )),
            SplitAxis::Vertical => Tile::Container(egui_tiles::Container::new_vertical(
                vec![moved_existing_id, new_pane_id],
            )),
        };

        // Put the new split container at `target_id` so parent links stay valid
        self.tree.tiles.insert(target_id, container_tile);
        self.focused_tile = Some(new_pane_id);
    }
}

struct TreeBehavior<'a> {
    settings: &'a ThemeSettings,
    focused_tile: &'a mut Option<TileId>,
    top_right_tile: Option<TileId>,
    recorded_rects: &'a mut HashMap<TileId, egui::Rect>,
    pending_action: Option<(TileId, TitlebarAction)>,
    prefs_open: &'a mut bool,
}

impl<'a> egui_tiles::Behavior<TerminalPane> for TreeBehavior<'a> {
    fn tab_title_for_pane(&mut self, pane: &TerminalPane) -> egui::WidgetText {
        pane.title.clone().into()
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: false,
            ..Default::default()
        }
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        2.0
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        tile_id: TileId,
        pane: &mut TerminalPane,
    ) -> UiResponse {
        self.recorded_rects.insert(tile_id, ui.max_rect());

        let is_focused = *self.focused_tile == Some(tile_id);
        let is_top_right = self.top_right_tile == Some(tile_id) || self.top_right_tile.is_none();

        let output = pane.ui(ui, self.settings, is_focused, is_top_right);

        if output.clicked_or_focused {
            *self.focused_tile = Some(tile_id);
        }

        if let Some(action) = output.action {
            if matches!(action, TitlebarAction::OpenPreferences) {
                *self.prefs_open = true;
            } else {
                self.pending_action = Some((tile_id, action));
            }
        }

        if output.start_tile_drag {
            UiResponse::DragStarted
        } else {
            UiResponse::None
        }
    }
}
