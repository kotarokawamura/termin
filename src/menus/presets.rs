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
use crate::config::theme::ShortcutSettings;
use crate::i18n::tr;
use crate::menus::types::{MenuAction, MenuEntry, MenuTargetKind};

fn opt_sc(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn default_entries_for(target: MenuTargetKind, shortcuts: &ShortcutSettings) -> Vec<MenuEntry> {
    match target {
        MenuTargetKind::Pane => vec![
            MenuEntry::item("📋", tr("Copy"), opt_sc(&shortcuts.copy), MenuAction::Copy),
            MenuEntry::item("📥", tr("Paste"), opt_sc(&shortcuts.paste), MenuAction::Paste),
            MenuEntry::Separator,
            // ネストされたサブメニュー
            MenuEntry::submenu(
                "🪟",
                tr("Split Pane"),
                vec![
                    MenuEntry::item(
                        "↔",
                        tr("Split Horizontally"),
                        opt_sc(&shortcuts.split_horizontal),
                        MenuAction::SplitHorizontal,
                    ),
                    MenuEntry::item(
                        "↕",
                        tr("Split Vertically"),
                        opt_sc(&shortcuts.split_vertical),
                        MenuAction::SplitVertical,
                    ),
                ],
            ),
            MenuEntry::item("➕", tr("New Tab"), opt_sc(&shortcuts.new_tab), MenuAction::NewTab),
            MenuEntry::item("🧹", tr("Clear Scrollback"), None, MenuAction::ClearScrollback),
            MenuEntry::Separator,
            MenuEntry::item("⚙", tr("Open Settings"), opt_sc(&shortcuts.open_preferences), MenuAction::OpenSettings),
            MenuEntry::item("✖", tr("Close Pane"), opt_sc(&shortcuts.close_pane), MenuAction::ClosePane),
        ],

        MenuTargetKind::Titlebar => vec![
            MenuEntry::item("➕", tr("New Tab"), opt_sc(&shortcuts.new_tab), MenuAction::NewTab),
            MenuEntry::submenu(
                "🪟",
                tr("Split Pane"),
                vec![
                    MenuEntry::item(
                        "↔",
                        tr("Split Horizontally"),
                        opt_sc(&shortcuts.split_horizontal),
                        MenuAction::SplitHorizontal,
                    ),
                    MenuEntry::item(
                        "↕",
                        tr("Split Vertically"),
                        opt_sc(&shortcuts.split_vertical),
                        MenuAction::SplitVertical,
                    ),
                ],
            ),
            MenuEntry::submenu(
                "💾",
                tr("Layout"),
                vec![
                    MenuEntry::item("💾", tr("Save Current Layout"), None, MenuAction::SaveLayout),
                    MenuEntry::item("📂", tr("Load Saved Layout"), None, MenuAction::LoadLayout),
                ],
            ),
            MenuEntry::Separator,
            MenuEntry::item(
                "⛶",
                tr("Toggle Fullscreen"),
                opt_sc(&shortcuts.toggle_fullscreen),
                MenuAction::ToggleFullscreen,
            ),
            MenuEntry::item("⚙", tr("Open Settings"), opt_sc(&shortcuts.open_preferences), MenuAction::OpenSettings),
            MenuEntry::Separator,
            MenuEntry::item("✖", tr("Close Pane"), opt_sc(&shortcuts.close_pane), MenuAction::ClosePane),
        ],
    }
}
