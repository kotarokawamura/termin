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
use egui::{Button, Response, RichText, Ui};
use crate::menus::types::{MenuAction, MenuEntry};

/// Responseに対してコンテキストメニューを表示し、クリックされたActionを返す
pub fn render_context_menu(
    response: &Response,
    entries: &[MenuEntry],
) -> Option<MenuAction> {
    let mut triggered_action = None;

    response.context_menu(|ui| {
        ui.set_min_width(220.0);
        render_entries(ui, entries, &mut triggered_action);
    });

    triggered_action
}

/// 再帰的にメニュー項目を描画する（サブメニューのネストに対応）
fn render_entries(
    ui: &mut Ui,
    entries: &[MenuEntry],
    triggered_action: &mut Option<MenuAction>,
) {
    for entry in entries {
        match entry {
            MenuEntry::Separator => {
                ui.separator();
            }
            MenuEntry::Item {
                icon,
                label,
                shortcut,
                action,
                enabled,
            } => {
                // 左側にアイコンとラベルを合成（アイコンがない場合もインデントを揃える）
                let display_text = format_icon_and_label(icon.as_deref(), label);
                let mut button = Button::new(display_text);

                // 右側にショートカットキーを表示
                if let Some(sc) = shortcut {
                    button = button.shortcut_text(RichText::new(sc).weak());
                }

                let res = ui.add_enabled(*enabled, button);
                if res.clicked() {
                    *triggered_action = Some(action.clone());
                    ui.close(); // クリック時にメニューを閉じる
                }
            }
            MenuEntry::SubMenu { icon, label, items } => {
                // ui.menu_button は標準で右端にサブメニュー用の矢印アイコンを描画し、
                // ホバーまたはクリックでネストしたコンテキストメニューを展開する
                let display_text = format_icon_and_label(icon.as_deref(), label);

                ui.menu_button(display_text, |sub_ui| {
                    sub_ui.set_min_width(180.0);
                    // 再帰呼び出しで何階層でもネスト可能
                    render_entries(sub_ui, items, triggered_action);
                });
            }
        }
    }
}

/// アイコンとラベルのテキスト位置を綺麗に揃えるヘルパー
fn format_icon_and_label(icon: Option<&str>, label: &str) -> String {
    match icon {
        Some(ic) if !ic.is_empty() => format!("{:<2}  {}", ic, label),
        _ => format!("    {}", label), // アイコン無しでも文字の開始位置を揃える
    }
}