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
mod tests {
    use super::*;
    use crate::config::theme::{ShortcutSettings, ThemeSettings};
    use crate::menus::presets::default_entries_for;
    use crate::menus::types::MenuTargetKind;

    #[test]
    fn test_context_menu_dimensions() {
        let ctx = egui::Context::default();
        let settings = ThemeSettings::default();
        settings.apply_fonts(&ctx);
        let entries = default_entries_for(MenuTargetKind::Pane, &ShortcutSettings::default());
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 800.0));

        // Frame 0: initial layout so egui knows widget rect
        let mut raw0 = egui::RawInput::default();
        raw0.screen_rect = Some(screen);
        ctx.run_ui(raw0, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let (_, resp) =
                    ui.allocate_exact_size(egui::vec2(800.0, 600.0), egui::Sense::click());
                render_context_menu(&resp, &entries);
            });
        })
        .textures_delta
        .clear();

        // Frame 1: Secondary press
        let mut raw1 = egui::RawInput::default();
        raw1.screen_rect = Some(screen);
        let pos = egui::pos2(200.0, 200.0);
        raw1.events.push(egui::Event::PointerMoved(pos));
        raw1.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_ui(raw1, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let (_, resp) =
                    ui.allocate_exact_size(egui::vec2(800.0, 600.0), egui::Sense::click());
                render_context_menu(&resp, &entries);
            });
        })
        .textures_delta
        .clear();

        // Frame 2: Secondary release -> opens context menu
        let mut raw2 = egui::RawInput::default();
        raw2.screen_rect = Some(screen);
        raw2.events.push(egui::Event::PointerMoved(pos));
        raw2.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut menu_size = egui::Vec2::ZERO;
        ctx.run_ui(raw2, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let (_, resp) =
                    ui.allocate_exact_size(egui::vec2(800.0, 600.0), egui::Sense::click());
                resp.context_menu(|ui| {
                    apply_context_menu_style(ui, 175.0, 280.0);
                    let mut act = None;
                    render_entries(ui, &entries, &mut act);
                    menu_size = ui.min_rect().size();
                });
            });
        })
        .textures_delta
        .clear();

        assert!(
            menu_size.x >= 170.0 && menu_size.x <= 285.0 && menu_size.y >= 130.0 && menu_size.y <= 260.0,
            "Context menu should be compact and properly sized (got {:?})",
            menu_size
        );
    }
}