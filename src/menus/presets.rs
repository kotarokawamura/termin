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
use crate::menus::types::{MenuAction, MenuEntry, MenuTargetKind};

pub fn default_entries_for(target: MenuTargetKind) -> Vec<MenuEntry> {
    match target {
        MenuTargetKind::Pane => vec![
            MenuEntry::item("📋",
                            "コピー", Some("Ctrl+Shift+C"), MenuAction::Copy),
            MenuEntry::item("📥", "ペースト", Some("Ctrl+Shift+V"), MenuAction::Paste),
            MenuEntry::Separator,
            // ネストされたサブメニュー
            MenuEntry::submenu(
                "🪟",
                "ペインを分割",
                vec![
                    MenuEntry::item("↔", "左右に分割", Some("Alt+D"), MenuAction::SplitHorizontal),
                    MenuEntry::item("↕", "上下に分割", Some("Alt+Shift+D"), MenuAction::SplitVertical),
                ],
            ),
            MenuEntry::Separator,
            MenuEntry::item("✖", "ペインを閉じる", Some("Ctrl+Shift+W"), MenuAction::ClosePane),
        ],

        MenuTargetKind::Titlebar => vec![
            MenuEntry::item("➕", "新しいタブ", Some("Ctrl+Shift+T"), MenuAction::NewTab),
            MenuEntry::Separator,
            MenuEntry::item("⛶", "フルスクリーン切替", Some("F11"), MenuAction::ToggleFullscreen),
            MenuEntry::item("⚙", "設定を開く", Some("Ctrl+,"), MenuAction::OpenSettings),
        ],
    }
}