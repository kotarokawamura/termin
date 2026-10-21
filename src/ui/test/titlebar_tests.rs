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

    #[test]
    fn test_titlebar_rename_button() {
        let ctx = egui::Context::default();
        let menus = MenuManager::new();
        let settings = ThemeSettings::default();
        settings.apply_fonts(&ctx);
        let mut title = "/bin/bash".to_string();
        let mut editing = false;
        let tile_id = egui_tiles::TileId::from_u64(1);

        let raw_with_screen = || {
            let mut r = egui::RawInput::default();
            r.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(500.0, 400.0),
            ));
            r
        };

        // Frame 1: initial layout
        ctx.run_ui(raw_with_screen(), |ui| {
            render_titlebar(
                ui,
                tile_id,
                &menus,
                &mut title,
                &mut editing,
                true,
                true,
                false,
                &settings,
            );
        })
            .textures_delta
            .clear();

        let lbl_rect: egui::Rect = ctx
            .data(|d| d.get_temp(egui::Id::new("title_lbl_rect")))
            .unwrap();
        assert!(
            (lbl_rect.center().x - 250.0).abs() < 2.0,
            "Title label should be centered horizontally in the titlebar (got center x = {})",
            lbl_rect.center().x
        );

        let btn_rect: egui::Rect = ctx
            .data(|d| d.get_temp(egui::Id::new("edit_btn_rect")))
            .unwrap();
        let click_pos = btn_rect.center();

        // Frame 2: press down on edit button
        let mut press_input = raw_with_screen();
        press_input.events.push(egui::Event::PointerMoved(click_pos));
        press_input.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_ui(press_input, |ui| {
            render_titlebar(
                ui,
                tile_id,
                &menus,
                &mut title,
                &mut editing,
                true,
                true,
                false,
                &settings,
            );
        })
            .textures_delta
            .clear();

        // Frame 3: release on edit button -> enters editing mode on this exact frame!
        let mut release_input = raw_with_screen();
        release_input.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_ui(release_input, |ui| {
            render_titlebar(
                ui,
                tile_id,
                &menus,
                &mut title,
                &mut editing,
                true,
                true,
                false,
                &settings,
            );
        })
            .textures_delta
            .clear();
        assert!(editing, "Clicking pencil button should enable title editing");

        // Frame 4: ArrowLeft navigation inside TextEdit must NOT lose focus
        let mut arrow_input = raw_with_screen();
        arrow_input.events.push(egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_ui(arrow_input, |ui| {
            render_titlebar(
                ui,
                tile_id,
                &menus,
                &mut title,
                &mut editing,
                true,
                true,
                false,
                &settings,
            );
        })
            .textures_delta
            .clear();
        assert!(
            editing,
            "Pressing ArrowLeft inside title TextEdit must not lose focus"
        );

        // Frame 5: Type text into TextEdit
        let mut type_input = raw_with_screen();
        type_input.events.push(egui::Event::Text("-custom".to_string()));
        ctx.run_ui(type_input, |ui| {
            render_titlebar(
                ui,
                tile_id,
                &menus,
                &mut title,
                &mut editing,
                true,
                true,
                false,
                &settings,
            );
        })
            .textures_delta
            .clear();
        assert!(title.contains("-custom"));
    }

    #[test]
    fn test_inactive_titlebar_and_other_screen_open_never_move_panel() {
        let ctx = egui::Context::default();
        let menus = MenuManager::new();
        let settings = ThemeSettings::default();
        settings.apply_fonts(&ctx);
        let mut title = "/bin/bash".to_string();
        let mut editing = false;
        let tile_id = egui_tiles::TileId::from_u64(1);

        let raw_with_screen = || {
            let mut r = egui::RawInput::default();
            r.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(500.0, 400.0),
            ));
            r
        };

        // 1. Dragging the grip or background of an INACTIVE titlebar must never move the panel or window
        for start_pos in [egui::pos2(12.0, 12.0), egui::pos2(250.0, 12.0)] {
            let mut press = raw_with_screen();
            press.events.push(egui::Event::PointerMoved(start_pos));
            press.events.push(egui::Event::PointerButton {
                pos: start_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            ctx.run_ui(press, |ui| {
                let out = render_titlebar(
                    ui,
                    tile_id,
                    &menus,
                    &mut title,
                    &mut editing,
                    false, // inactive titlebar
                    true,
                    false,
                    &settings,
                );
                assert!(!out.start_tile_drag);
                assert_ne!(out.action, Some(TitleBarAction::StartWindowMove));
            })
                .textures_delta
                .clear();

            let drag_pos = egui::pos2(start_pos.x + 40.0, start_pos.y + 20.0);
            let mut drag = raw_with_screen();
            drag.events.push(egui::Event::PointerMoved(drag_pos));
            ctx.run_ui(drag, |ui| {
                let out = render_titlebar(
                    ui,
                    tile_id,
                    &menus,
                    &mut title,
                    &mut editing,
                    false, // inactive titlebar
                    true,
                    false,
                    &settings,
                );
                assert!(
                    !out.start_tile_drag,
                    "Dragging an inactive titlebar grip must never move the panel"
                );
                assert_ne!(
                    out.action,
                    Some(TitleBarAction::StartWindowMove),
                    "Dragging an inactive titlebar must never move the window"
                );
            })
                .textures_delta
                .clear();

            let mut rel = raw_with_screen();
            rel.events.push(egui::Event::PointerButton {
                pos: drag_pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            ctx.run_ui(rel, |ui| {
                render_titlebar(
                    ui,
                    tile_id,
                    &menus,
                    &mut title,
                    &mut editing,
                    false,
                    true,
                    false,
                    &settings,
                );
            })
                .textures_delta
                .clear();
        }

        // 2. When another screen is open (other_screen_open = true), even clicking does not interact
        let click_pos = egui::pos2(250.0, 12.0);
        let mut press = raw_with_screen();
        press.events.push(egui::Event::PointerMoved(click_pos));
        press.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_ui(press, |ui| {
            let out = render_titlebar(
                ui,
                tile_id,
                &menus,
                &mut title,
                &mut editing,
                true,
                true,
                true, // other_screen_open = true
                &settings,
            );
            assert!(!out.clicked);
            assert!(!out.start_tile_drag);
            assert!(out.action.is_none());
        })
            .textures_delta
            .clear();

        let mut release = raw_with_screen();
        release.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_ui(release, |ui| {
            let out = render_titlebar(
                ui,
                tile_id,
                &menus,
                &mut title,
                &mut editing,
                true,
                true,
                true, // other_screen_open = true
                &settings,
            );
            assert!(
                !out.clicked && !out.start_tile_drag && out.action.is_none(),
                "Titlebar must not respond to clicks when another screen is open"
            );
        })
            .textures_delta
            .clear();
    }
}

