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
use std::env;
use std::fs;
use std::path::PathBuf;
use egui_tiles::{Container, LinearDir, Tile, TileId, Tiles, Tree};
use serde::{Deserialize, Serialize};
use crate::ui::pane::TerminalPane;

/// Serializable layout preset structure for saving and restoring workspace splits and tabs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LayoutNode {
    Pane {
        title: String,
        command: Option<String>,
    },
    HorizontalSplit {
        children: Vec<LayoutNode>,
    },
    VerticalSplit {
        children: Vec<LayoutNode>,
    },
    Tabs {
        children: Vec<LayoutNode>,
    },
}

impl Default for LayoutNode {
    fn default() -> Self {
        Self::Pane {
            title: crate::ui::pane::default_shell_title(),
            command: None,
        }
    }
}

impl LayoutNode {
    pub fn layout_path() -> PathBuf {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/termin/layout.json")
    }

    pub fn save_default(&self) -> Result<(), String> {
        let path = Self::layout_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn load_default() -> Option<Self> {
        let content = fs::read_to_string(Self::layout_path()).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Snapshot an active `egui_tiles::Tree<TerminalPane>` into a serializable `LayoutNode`.
    pub fn from_tree(tree: &Tree<TerminalPane>) -> Option<Self> {
        let root_id = tree.root()?;
        Self::from_tile(&tree.tiles, root_id)
    }

    fn from_tile(tiles: &Tiles<TerminalPane>, tile_id: TileId) -> Option<Self> {
        match tiles.get(tile_id)? {
            Tile::Pane(pane) => Some(Self::Pane {
                title: pane.title.clone(),
                command: None,
            }),
            Tile::Container(container) => match container {
                Container::Linear(linear) => {
                    let children: Vec<LayoutNode> = linear
                        .children
                        .iter()
                        .filter_map(|&cid| Self::from_tile(tiles, cid))
                        .collect();
                    if children.is_empty() {
                        return None;
                    }
                    match linear.dir {
                        LinearDir::Horizontal => Some(Self::HorizontalSplit { children }),
                        LinearDir::Vertical => Some(Self::VerticalSplit { children }),
                    }
                }
                Container::Tabs(tabs) => {
                    let children: Vec<LayoutNode> = tabs
                        .children
                        .iter()
                        .filter_map(|&cid| Self::from_tile(tiles, cid))
                        .collect();
                    if children.is_empty() {
                        return None;
                    }
                    Some(Self::Tabs { children })
                }
                Container::Grid(grid) => {
                    let children: Vec<LayoutNode> = grid
                        .children()
                        .filter_map(|&cid| Self::from_tile(tiles, cid))
                        .collect();
                    if children.is_empty() {
                        return None;
                    }
                    Some(Self::HorizontalSplit { children })
                }
            },
        }
    }

    /// Reconstruct an `egui_tiles::Tree<TerminalPane>` from this `LayoutNode`.
    /// Reconstruct an `egui_tiles::Tree<TerminalPane>` from this `LayoutNode`.
    pub fn build_tree(
        &self,
        next_pane_number: &mut usize,
        wakeup_ctx: Option<eframe::egui::Context>,
    ) -> (Tree<TerminalPane>, Option<TileId>) {
        let mut tiles = Tiles::default();
        let mut first_pane_id = None;
        let root_id = self.insert_into_tiles(
            &mut tiles,
            next_pane_number,
            &mut first_pane_id,
            wakeup_ctx,
        );
        let tree = Tree::new(egui::Id::unique("termin_workspace"), root_id, tiles);
        (tree, first_pane_id)
    }

    fn insert_into_tiles(
        &self,
        tiles: &mut Tiles<TerminalPane>,
        next_pane_number: &mut usize,
        first_pane_id: &mut Option<TileId>,
        wakeup_ctx: Option<egui::Context>,
    ) -> TileId {
        match self {
            Self::Pane { title, command } => {
                let id = *next_pane_number;
                *next_pane_number += 1;
                let pane = TerminalPane::new_with_wakeup(id, title, command.as_deref(), wakeup_ctx);
                let tile_id = tiles.insert_pane(pane);
                if first_pane_id.is_none() {
                    *first_pane_id = Some(tile_id);
                }
                tile_id
            }
            Self::HorizontalSplit { children } => {
                let child_ids: Vec<TileId> = children
                    .iter()
                    .map(|c| c.insert_into_tiles(tiles, next_pane_number, first_pane_id, wakeup_ctx.clone()))
                    .collect();
                if child_ids.is_empty() {
                    let id = *next_pane_number;
                    *next_pane_number += 1;
                    let tile_id = tiles.insert_pane(TerminalPane::new_with_wakeup(id, &crate::ui::pane::default_shell_title(), None, wakeup_ctx));
                    if first_pane_id.is_none() {
                        *first_pane_id = Some(tile_id);
                    }
                    tile_id
                } else {
                    tiles.insert_horizontal_tile(child_ids)
                }
            }
            Self::VerticalSplit { children } => {
                let child_ids: Vec<TileId> = children
                    .iter()
                    .map(|c| c.insert_into_tiles(tiles, next_pane_number, first_pane_id, wakeup_ctx.clone()))
                    .collect();
                if child_ids.is_empty() {
                    let id = *next_pane_number;
                    *next_pane_number += 1;
                    let tile_id = tiles.insert_pane(TerminalPane::new_with_wakeup(id, &crate::ui::pane::default_shell_title(), None, wakeup_ctx));
                    if first_pane_id.is_none() {
                        *first_pane_id = Some(tile_id);
                    }
                    tile_id
                } else {
                    tiles.insert_vertical_tile(child_ids)
                }
            }
            Self::Tabs { children } => {
                let child_ids: Vec<TileId> = children
                    .iter()
                    .map(|c| c.insert_into_tiles(tiles, next_pane_number, first_pane_id, wakeup_ctx.clone()))
                    .collect();
                if child_ids.is_empty() {
                    let id = *next_pane_number;
                    *next_pane_number += 1;
                    let tile_id = tiles.insert_pane(TerminalPane::new_with_wakeup(id, &crate::ui::pane::default_shell_title(), None, wakeup_ctx));
                    if first_pane_id.is_none() {
                        *first_pane_id = Some(tile_id);
                    }
                    tile_id
                } else {
                    tiles.insert_tab_tile(child_ids)
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "test/layout_preset_tests.rs"]
mod tests;
