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
use egui_tiles::TileId;

/// ワークスペース全体の現在の操作モード
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionMode {
    /// 通常操作モード
    Normal,
    /// `[ ✥ ]` クリックによるパネル移動（入れ替え先選択）モード
    MovingPane(TileId),
    /// いずれかのパネルでタイトル名をインライン編集中
    EditingTitle(TileId),
    /// 設定ウィンドウ等のモーダル・別画面が開いている状態
    BlockedByModal,
}

/// 各パネル（およびタイトルバー）の描画・入力判定に必要なコンテキスト
#[derive(Clone, Copy, Debug)]
pub struct PaneContext {
    pub tile_id: TileId,
    pub is_focused: bool,
    pub is_top_right: bool,
    pub window_focused: bool,
    pub popup_open: bool,
    pub mode: InteractionMode,
    pub external_zoom_delta: f32,
    pub is_selecting_text: bool,
}

impl PaneContext {
    /// 設定画面や「他パネルのタイトル編集」によってこのパネルの操作がブロックされているか
    pub fn is_blocked(&self) -> bool {
        match self.mode {
            InteractionMode::BlockedByModal => true,
            InteractionMode::EditingTitle(editing_id) => editing_id != self.tile_id,
            _ => false,
        }
    }

    /// このパネル自身が `[ ✥ ]` パネル移動モードの移動元になっているか
    pub fn is_moving_self(&self) -> bool {
        self.mode == InteractionMode::MovingPane(self.tile_id)
    }

    /// ワークスペース内のいずれかのパネルが `[ ✥ ]` パネル移動モード中か
    pub fn is_any_moving(&self) -> bool {
        matches!(self.mode, InteractionMode::MovingPane(_))
    }

    /// パネルおよびタイトルバーを「アクティブ色」で表示すべきか
    pub fn is_visually_active(&self) -> bool {
        (self.is_focused || self.is_moving_self())
            && self.window_focused
            && !self.popup_open
            && !self.is_blocked()
    }

    /// タイトルバー上のボタンやドラッグ操作が有効か
    pub fn can_interact_titlebar(&self) -> bool {
        !self.is_blocked() && !self.popup_open
    }

    /// ターミナルキャンバス（キー入力・スクロール・テキスト選択）が操作可能か
    pub fn can_interact_terminal(&self, editing_own_title: bool, context_menu_open: bool) -> bool {
        self.is_visually_active()
            && !editing_own_title
            && !self.is_any_moving()
            && !context_menu_open
    }
}