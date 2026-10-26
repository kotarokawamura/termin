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
pub mod presets;
pub mod renderer;
pub mod types;

use std::collections::HashMap;
use egui::Response;
use crate::config::theme::ShortcutSettings;
use types::{MenuAction, MenuContext, MenuEntry, MenuTargetKind};

#[derive(Default)]
pub struct MenuManager {
    /// プラグインから追加されたメニュー項目をターゲット種類ごとに保持
    plugin_entries: HashMap<MenuTargetKind, Vec<MenuEntry>>,
}

impl MenuManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 【プラグイン用API】特定の画面のコンテキストメニューに項目を追加する
    pub fn register_plugin_entry(&mut self, target: MenuTargetKind, entry: MenuEntry) {
        self.plugin_entries
            .entry(target)
            .or_default()
            .push(entry);
    }

    /// 【各UI部品用API】指定したコンテキストに応じた右クリックメニューを表示する
    pub fn show(
        &self,
        response: &Response,
        ctx: MenuContext,
        shortcuts: &ShortcutSettings,
    ) -> Option<(MenuContext, MenuAction)> {
        let kind = ctx.kind();
        let entries = presets::default_entries_for(kind, shortcuts);
        
        // 描画してクリックされたアクションがあれば、どの画面(MenuContext)で起きたかとセットで返す
        renderer::render_context_menu(response, &entries).map(|action| (ctx, action))
    }
}