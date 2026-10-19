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
use serde::{Deserialize, Serialize};

/// どのUI部品で右クリックされたかを表す識別子
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MenuTargetKind {
    Pane,
    Titlebar,
}

/// 実行時に渡すコンテキスト情報
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuContext {
    Pane(TileId),
    Titlebar(TileId),
}

impl MenuContext {
    pub fn kind(&self) -> MenuTargetKind {
        match self {
            Self::Pane(_) => MenuTargetKind::Pane,
            Self::Titlebar(_) => MenuTargetKind::Titlebar,
        }
    }

    pub fn tile_id(&self) -> TileId {
        match self {
            Self::Pane(id) | Self::Titlebar(id) => *id,
        }
    }
}

/// メニューをクリックしたときに発火するアクション
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuAction {
    // Pane用アクション
    Copy,
    Paste,
    SplitHorizontal,
    SplitVertical,
    ClosePane,

    // Titlebar用アクション
    NewTab,
    OpenSettings,
    ToggleFullscreen,

    // プラグイン用の動的アクション
    PluginCommand(String),
}

/// コンテキストメニューの1行分のデータ構造
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MenuEntry {
    /// 通常のクリック可能なアクション項目
    Item {
        icon: Option<String>,
        label: String,
        shortcut: Option<String>,
        action: MenuAction,
        enabled: bool,
    },
    /// ネストしたコンテキストメニュー（サブメニュー）
    SubMenu {
        icon: Option<String>,
        label: String,
        items: Vec<MenuEntry>,
    },
    /// 区切り線
    Separator,
}

impl MenuEntry {
    pub fn item(
        icon: impl Into<String>,
        label: impl Into<String>,
        shortcut: Option<&str>,
        action: MenuAction,
    ) -> Self {
        Self::Item {
            icon: Some(icon.into()),
            label: label.into(),
            shortcut: shortcut.map(|s| s.to_string()),
            action,
            enabled: true,
        }
    }

    pub fn submenu(
        icon: impl Into<String>,
        label: impl Into<String>,
        items: Vec<MenuEntry>,
    ) -> Self {
        Self::SubMenu {
            icon: Some(icon.into()),
            label: label.into(),
            items,
        }
    }
}