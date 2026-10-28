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
use egui_tiles::{Container, Tile, TileId, Tiles};
use crate::config::layout_preset::LayoutNode;
use crate::ui::pane::{default_shell_title, TerminalPane};
use crate::ui::workspace::{SplitAxis, Workspace};

impl Workspace {
    /// 現在の `focused_tile` が有効な `Pane` を指しているか検証し、
    /// ツリー簡略化（Simplification）や削除で無効化されていた場合は即座に有効なパネルへ復旧する。
    pub fn ensure_valid_focus(&mut self) {
        let ordered_panes = self.ordered_pane_ids();

        let current_valid = self.focused_tile.is_some_and(|id| {
            ordered_panes.contains(&id) && matches!(self.tree.tiles.get(id), Some(Tile::Pane(_)))
        });

        if !current_valid {
            // 直近フレームで画面上に描画されていたパネルを優先してフォーカスする
            let visible_fallback = ordered_panes
                .iter()
                .copied()
                .find(|id| self.recorded_rects.contains_key(id))
                .or_else(|| ordered_panes.first().copied());
            self.focused_tile = visible_fallback;
        }

        self.activate_tab_for_focused_pane();
    }

    /// フォーカス中のパネルが Tabs コンテナ内にある場合、そのタブをアクティブにする
    pub fn activate_tab_for_focused_pane(&mut self) {
        if let Some(focused_id) = self.focused_tile {
            for (_, tile) in self.tree.tiles.iter_mut() {
                if let Tile::Container(Container::Tabs(tabs)) = tile {
                    if tabs.children.contains(&focused_id) {
                        tabs.set_active(focused_id);
                    }
                }
            }
        }
    }

    /// 2つのパネルの位置を入れ替え、移動したパネルをアクティブにする
    pub fn swap_panes(&mut self, src_id: TileId, dst_id: TileId) {
        if src_id == dst_id {
            self.focused_tile = Some(src_id);
            return;
        }
        let src_tile = self.tree.tiles.remove(src_id);
        let dst_tile = self.tree.tiles.remove(dst_id);
        match (src_tile, dst_tile) {
            (Some(s), Some(d)) => {
                self.tree.tiles.insert(dst_id, s);
                self.tree.tiles.insert(src_id, d);
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

    /// パネルを削除し、隣接する生存パネルへ確実にフォーカスを移す
    pub fn remove_pane(&mut self, target_id: TileId) {
        let mut ordered_before = self.ordered_pane_ids();
        let removed_idx = ordered_before.iter().position(|&id| id == target_id);

        self.tree.tiles.remove(target_id);
        self.recorded_rects.remove(&target_id);

        for (_, tile) in self.tree.tiles.iter_mut() {
            if let Tile::Container(container) = tile {
                container.retain(|child_id| child_id != target_id);
            }
        }

        ordered_before.retain(|&id| {
            id != target_id && matches!(self.tree.tiles.get(id), Some(Tile::Pane(_)))
        });

        let neighbor_pane = if let Some(idx) = removed_idx {
            ordered_before
                .get(idx)
                .or_else(|| idx.checked_sub(1).and_then(|i| ordered_before.get(i)))
                .copied()
        } else {
            ordered_before.first().copied()
        }
            .or_else(|| {
                self.tree
                    .tiles
                    .iter()
                    .find_map(|(id, t)| matches!(t, Tile::Pane(_)).then_some(*id))
            });

        if self.focused_tile == Some(target_id)
            || !self
            .focused_tile
            .is_some_and(|id| matches!(self.tree.tiles.get(id), Some(Tile::Pane(_))))
        {
            self.focused_tile = neighbor_pane;
        }

        if self.top_right_tile == Some(target_id) {
            self.top_right_tile = neighbor_pane;
        }
        if self.moving_tile == Some(target_id) {
            self.moving_tile = None;
        }
        if self.selecting_broadcast_tile == Some(target_id) {
            self.selecting_broadcast_tile = None;
        }

        self.cleanup_singleton_broadcast_groups();
        self.activate_tab_for_focused_pane();
    }

    /// 現在未使用の最小のブロードキャストグループ番号（1始まり）を返す
    pub fn next_available_broadcast_group_id(&self) -> usize {
        let mut candidate = 1usize;
        loop {
            let used = self.tree.tiles.iter().any(|(_, tile)| {
                matches!(tile, Tile::Pane(p) if p.broadcast_group == Some(candidate))
            });
            if !used {
                return candidate;
            }
            candidate += 1;
        }
    }

    /// 2つのパネルをブロードキャストグループとして紐付ける
    /// （選択先が既にグループに属していればそのグループへ追加、未所属同士なら新規グループ番号を割り当てる）
    pub fn link_broadcast_panes(&mut self, src_id: TileId, dst_id: TileId) {
        if src_id == dst_id {
            return;
        }
        let src_exists = matches!(self.tree.tiles.get(src_id), Some(Tile::Pane(_)));
        let dst_exists = matches!(self.tree.tiles.get(dst_id), Some(Tile::Pane(_)));
        if !src_exists || !dst_exists {
            return;
        }

        let dst_group = match self.tree.tiles.get(dst_id) {
            Some(Tile::Pane(p)) => p.broadcast_group,
            _ => None,
        };
        let src_group = match self.tree.tiles.get(src_id) {
            Some(Tile::Pane(p)) => p.broadcast_group,
            _ => None,
        };

        let group_id = dst_group
            .or(src_group)
            .unwrap_or_else(|| self.next_available_broadcast_group_id());

        if let Some(Tile::Pane(src_pane)) = self.tree.tiles.get_mut(src_id) {
            src_pane.broadcast_group = Some(group_id);
        }
        if let Some(Tile::Pane(dst_pane)) = self.tree.tiles.get_mut(dst_id) {
            dst_pane.broadcast_group = Some(group_id);
        }
        self.cleanup_singleton_broadcast_groups();
    }

    /// 指定したパネルをブロードキャストグループから解除する（残り1パネル以下になったグループも自動解除）
    pub fn leave_broadcast_group(&mut self, target_id: TileId) {
        if let Some(Tile::Pane(pane)) = self.tree.tiles.get_mut(target_id) {
            pane.broadcast_group = None;
        }
        self.cleanup_singleton_broadcast_groups();
    }

    /// 所属パネルが1つ以下になったブロードキャストグループを自動的に解除する
    pub fn cleanup_singleton_broadcast_groups(&mut self) {
        let mut counts: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for (_, tile) in self.tree.tiles.iter() {
            if let Tile::Pane(pane) = tile {
                if let Some(gid) = pane.broadcast_group {
                    *counts.entry(gid).or_insert(0) += 1;
                }
            }
        }
        for (_, tile) in self.tree.tiles.iter_mut() {
            if let Tile::Pane(pane) = tile {
                if let Some(gid) = pane.broadcast_group {
                    if counts.get(&gid).copied().unwrap_or(0) < 2 {
                        pane.broadcast_group = None;
                    }
                }
            }
        }
    }

    /// 同一ブロードキャストグループ内の他パネルへキー入力バイト列を転送する
    pub fn broadcast_input_to_group(&mut self, src_id: TileId, group_id: usize, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        for (tile_id, tile) in self.tree.tiles.iter_mut() {
            if *tile_id != src_id {
                if let Tile::Pane(pane) = tile {
                    if pane.broadcast_group == Some(group_id) {
                        pane.receive_broadcast_input(bytes);
                    }
                }
            }
        }
    }

    /// アクティブなパネル（または分割対象パネル）のカレントディレクトリを取得する
    fn active_pane_working_dir(&self, target_id: TileId) -> Option<std::path::PathBuf> {
        self.focused_tile
            .and_then(|id| match self.tree.tiles.get(id) {
                Some(Tile::Pane(pane)) => pane.current_working_dir(),
                _ => None,
            })
            .or_else(|| match self.tree.tiles.get(target_id) {
                Some(Tile::Pane(pane)) => pane.current_working_dir(),
                _ => None,
            })
    }

    pub fn split_pane(&mut self, target_id: TileId, axis: SplitAxis) {
        let working_dir = self.active_pane_working_dir(target_id);
        let Some(old_tile) = self.tree.tiles.remove(target_id) else {
            return;
        };

        let moved_existing_id = self.tree.tiles.insert_new(old_tile);

        let id_num = self.next_pane_number;
        self.next_pane_number += 1;
        let new_pane = TerminalPane::new_with_wakeup_and_cwd(
            id_num,
            &default_shell_title(),
            None,
            self.pty_wakeup_ctx.clone(),
            working_dir.as_deref(),
        );
        let new_pane_id = self.tree.tiles.insert_pane(new_pane);

        let container_tile = match axis {
            SplitAxis::Horizontal => Tile::Container(Container::new_horizontal(vec![
                moved_existing_id,
                new_pane_id,
            ])),
            SplitAxis::Vertical => Tile::Container(Container::new_vertical(vec![
                moved_existing_id,
                new_pane_id,
            ])),
        };

        self.tree.tiles.insert(target_id, container_tile);
        self.focused_tile = Some(new_pane_id);
    }

    pub fn add_tab(&mut self, target_id: TileId) {
        let working_dir = self.active_pane_working_dir(target_id);
        let id_num = self.next_pane_number;
        self.next_pane_number += 1;
        let new_pane = TerminalPane::new_with_wakeup_and_cwd(
            id_num,
            &default_shell_title(),
            None,
            self.pty_wakeup_ctx.clone(),
            working_dir.as_deref(),
        );
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


    pub fn focus_adjacent_pane(&mut self, offset: isize) {
        let ordered_panes = self.ordered_pane_ids();
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
        self.activate_tab_for_focused_pane();
    }

    pub fn save_layout(&self) {
        if let Some(layout) = LayoutNode::from_tree(&self.tree) {
            let _ = layout.save_default();
        }
    }

    pub fn load_layout(&mut self) {
        if let Some(layout) = LayoutNode::load_default() {
            let mut next_num = 1;
            // wakeup_ctx を追加
            let (new_tree, first_pane) = layout.build_tree(&mut next_num, self.pty_wakeup_ctx.clone());
            self.tree = new_tree;
            self.next_pane_number = next_num;
            self.focused_tile = first_pane;
            self.top_right_tile = first_pane;
            self.moving_tile = None;
            self.selecting_broadcast_tile = None;
            self.recorded_rects.clear();
        }
    }

    pub fn pane_count(&self) -> usize {
        self.tree
            .tiles
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Pane(_)))
            .count()
    }

    pub fn update_top_right_tile(&mut self) {
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

    fn ordered_pane_ids(&self) -> Vec<TileId> {
        let mut ordered = Vec::new();
        if let Some(root_id) = self.tree.root() {
            Self::collect_panes_in_order(&self.tree.tiles, root_id, &mut ordered);
        }
        if ordered.is_empty() {
            ordered.extend(
                self.tree
                    .tiles
                    .iter()
                    .filter_map(|(id, t)| matches!(t, Tile::Pane(_)).then_some(*id)),
            );
        }
        ordered
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
}