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
use super::*;
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

    #[test]
    fn test_menu_context_presets_and_plugin_registration() {
        use crate::menus::types::{MenuAction, MenuContext, MenuEntry};
        use crate::menus::MenuManager;

        let tile_id = egui_tiles::TileId::from_u64(42);
        let pane_ctx = MenuContext::Pane(tile_id);
        let title_ctx = MenuContext::Titlebar(tile_id);

        assert_eq!(pane_ctx.kind(), MenuTargetKind::Pane);
        assert_eq!(pane_ctx.tile_id(), tile_id);
        assert_eq!(title_ctx.kind(), MenuTargetKind::Titlebar);
        assert_eq!(title_ctx.tile_id(), tile_id);

        let mut sc = ShortcutSettings::default();
        sc.copy = "  ".to_string(); // Empty/whitespace shortcut should map to None

        let pane_entries = default_entries_for(MenuTargetKind::Pane, &sc);
        match &pane_entries[0] {
            MenuEntry::Item {
                shortcut,
                action,
                enabled,
                ..
            } => {
                assert_eq!(*action, MenuAction::Copy);
                assert!(shortcut.is_none());
                assert!(*enabled);
            }
            _ => panic!("Expected Copy item first in Pane menu"),
        }

        let title_entries = default_entries_for(MenuTargetKind::Titlebar, &sc);
        assert!(title_entries.iter().any(|e| matches!(
            e,
            MenuEntry::SubMenu { items, .. }
            if items.iter().any(|i| matches!(i, MenuEntry::Item { action: MenuAction::SaveLayout, .. }))
        )));

        let mut mgr = MenuManager::new();
        mgr.register_plugin_entry(
            MenuTargetKind::Pane,
            MenuEntry::item(
                "🔌",
                "Run Plugin",
                Some("Ctrl+P"),
                MenuAction::PluginCommand("my_plugin".to_string()),
            ),
        );
        assert_eq!(mgr.plugin_entries.get(&MenuTargetKind::Pane).unwrap().len(), 1);
    }
}