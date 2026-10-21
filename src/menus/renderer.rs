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
        apply_context_menu_style(ui, 175.0, 280.0);
        render_entries(ui, entries, &mut triggered_action);
    });

    triggered_action
}

fn apply_context_menu_style(ui: &mut Ui, min_width: f32, max_width: f32) {
    ui.set_min_width(min_width);
    ui.set_max_width(max_width);
    let base_font_size = ui
        .ctx()
        .global_style()
        .text_styles
        .get(&egui::TextStyle::Button)
        .map(|f| f.size)
        .unwrap_or(13.0)
        .clamp(10.0, 22.0);
    let shortcut_font_size = (base_font_size - 1.5).max(10.0);
    let row_height = (base_font_size + 7.0).max(19.0);
    let style = ui.style_mut();
    style.spacing.button_padding = egui::vec2(6.0, 2.5);
    style.spacing.item_spacing = egui::vec2(4.0, 1.5);
    style.spacing.interact_size.y = row_height;
    style.spacing.window_margin = egui::Margin::same(4);
    style.spacing.menu_margin = egui::Margin::same(4);
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(base_font_size, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(base_font_size, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(shortcut_font_size, egui::FontFamily::Proportional),
    );
}

/// 再帰的にメニュー項目を描画する（サブメニューのネストに対応）
fn render_entries(
    ui: &mut Ui,
    entries: &[MenuEntry],
    triggered_action: &mut Option<MenuAction>,
) {
    let base_font_size = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Button)
        .map(|f| f.size)
        .unwrap_or(13.0);
    let shortcut_font_size = (base_font_size - 1.5).max(10.0);
    let row_height = (base_font_size + 7.0).max(19.0);

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
                let mut button = Button::new(RichText::new(display_text).size(base_font_size))
                    .min_size(egui::vec2(ui.available_width(), row_height));

                // 右側にショートカットキーを表示
                if let Some(sc) = shortcut {
                    button = button.shortcut_text(RichText::new(sc).size(shortcut_font_size).weak());
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

                ui.menu_button(RichText::new(display_text).size(base_font_size), |sub_ui| {
                    apply_context_menu_style(sub_ui, 150.0, 260.0);
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

#[cfg(test)]
#[path = "test/renderer_tests.rs"]
mod tests;