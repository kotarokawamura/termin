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
    use crate::ui::state::{InteractionMode, PaneContext};

    fn make_ctx(
        tile_id: egui_tiles::TileId,
        is_focused: bool,
        is_top_right: bool,
        blocked: bool,
        broadcast_group: Option<usize>,
    ) -> PaneContext {
        PaneContext {
            tile_id,
            is_focused,
            is_top_right,
            window_focused: true,
            popup_open: false,
            mode: if blocked {
                InteractionMode::BlockedByModal
            } else {
                InteractionMode::Normal
            },
            external_zoom_delta: 1.0,
            is_selecting_text: false,
            broadcast_group,
        }
    }

    #[test]
    fn test_titlebar_double_click_rename() {
        let ctx = egui::Context::default();
        let menus = MenuManager::new();
        let settings = ThemeSettings::default();
        settings.apply_fonts(&ctx);
        let mut title = "/bin/bash".to_string();
        let mut editing = false;
        let tile_id = egui_tiles::TileId::from_u64(1);
        let pane_ctx = make_ctx(tile_id, true, true, false, None);

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
                &pane_ctx,
                &menus,
                &mut title,
                &mut editing,
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

        // Enable editing and verify keyboard navigation and typing
        editing = true;
        ctx.run_ui(raw_with_screen(), |ui| {
            render_titlebar(
                ui,
                &pane_ctx,
                &menus,
                &mut title,
                &mut editing,
                &settings,
            );
        })
            .textures_delta
            .clear();

        // ArrowLeft navigation inside TextEdit must NOT lose focus
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
                &pane_ctx,
                &menus,
                &mut title,
                &mut editing,
                &settings,
            );
        })
            .textures_delta
            .clear();
        assert!(
            editing,
            "Pressing ArrowLeft inside title TextEdit must not lose focus"
        );

        // Type text into TextEdit
        let mut type_input = raw_with_screen();
        type_input.events.push(egui::Event::Text("-custom".to_string()));
        ctx.run_ui(type_input, |ui| {
            render_titlebar(
                ui,
                &pane_ctx,
                &menus,
                &mut title,
                &mut editing,
                &settings,
            );
        })
            .textures_delta
            .clear();
        assert!(title.contains("-custom"));
    }

    #[test]
    fn test_grip_right_click_toggles_broadcast_and_shows_group_badge() {
        let ctx = egui::Context::default();
        let menus = MenuManager::new();
        let settings = ThemeSettings::default();
        settings.apply_fonts(&ctx);
        let mut title = "/bin/bash".to_string();
        let mut editing = false;
        let tile_id = egui_tiles::TileId::from_u64(1);
        let pane_ctx_ungrouped = make_ctx(tile_id, true, true, false, None);

        let raw_with_screen = || {
            let mut r = egui::RawInput::default();
            r.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(500.0, 400.0),
            ));
            r
        };

        // Frame 1: initial layout (ungrouped -> no badge)
        ctx.run_ui(raw_with_screen(), |ui| {
            render_titlebar(
                ui,
                &pane_ctx_ungrouped,
                &menus,
                &mut title,
                &mut editing,
                &settings,
            );
        })
            .textures_delta
            .clear();

        let badge_none: Option<usize> =
            ctx.data(|d| d.get_temp(egui::Id::new(("broadcast_group_badge", tile_id))));
        assert!(badge_none.is_none());

        let grip_rect: egui::Rect = ctx
            .data(|d| d.get_temp(egui::Id::new("grip_btn_rect")))
            .unwrap();
        let click_pos = grip_rect.center();

        // Frame 2: Secondary press on grip button
        let mut press_input = raw_with_screen();
        press_input.events.push(egui::Event::PointerMoved(click_pos));
        press_input.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_ui(press_input, |ui| {
            render_titlebar(
                ui,
                &pane_ctx_ungrouped,
                &menus,
                &mut title,
                &mut editing,
                &settings,
            );
        })
            .textures_delta
            .clear();

        // Frame 3: Secondary release on grip button -> triggers toggle_broadcast_select
        let mut release_input = raw_with_screen();
        release_input.events.push(egui::Event::PointerMoved(click_pos));
        release_input.events.push(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut toggled_broadcast = false;
        ctx.run_ui(release_input, |ui| {
            let out = render_titlebar(
                ui,
                &pane_ctx_ungrouped,
                &menus,
                &mut title,
                &mut editing,
                &settings,
            );
            toggled_broadcast = out.toggle_broadcast_select;
        })
            .textures_delta
            .clear();
        assert!(
            toggled_broadcast,
            "Right-clicking the grip button must trigger toggle_broadcast_select"
        );

        // Frame 4: When broadcast_group is Some(1), group badge is displayed next to grip button
        let pane_ctx_g1 = make_ctx(tile_id, true, true, false, Some(1));
        ctx.run_ui(raw_with_screen(), |ui| {
            render_titlebar(
                ui,
                &pane_ctx_g1,
                &menus,
                &mut title,
                &mut editing,
                &settings,
            );
        })
            .textures_delta
            .clear();

        let badge_g1: Option<usize> =
            ctx.data(|d| d.get_temp(egui::Id::new(("broadcast_group_badge", tile_id))));
        assert_eq!(badge_g1, Some(1));
        let badge_rect: egui::Rect = ctx
            .data(|d| d.get_temp(egui::Id::new("broadcast_group_badge_rect")))
            .unwrap();
        assert!(
            badge_rect.min.x >= grip_rect.max.x,
            "Broadcast group badge must be positioned to the right of the grip button"
        );

        // Verify broadcast group titlebar colors are distinct from normal colors and between groups
        assert_ne!(
            settings.broadcast_group_titlebar_bg(1, true),
            settings.titlebar_bg(true)
        );
        assert_ne!(
            settings.broadcast_group_titlebar_bg(1, false),
            settings.titlebar_bg(false)
        );
        assert_ne!(
            settings.broadcast_group_titlebar_bg(1, true),
            settings.broadcast_group_titlebar_bg(2, true)
        );
    }

    #[test]
    fn test_blocked_modal_never_interacts_with_titlebar() {
        let ctx = egui::Context::default();
        let menus = MenuManager::new();
        let settings = ThemeSettings::default();
        settings.apply_fonts(&ctx);
        let mut title = "/bin/bash".to_string();
        let mut editing = false;
        let tile_id = egui_tiles::TileId::from_u64(1);
        let blocked_ctx = make_ctx(tile_id, true, true, true, None);

        let raw_with_screen = || {
            let mut r = egui::RawInput::default();
            r.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(500.0, 400.0),
            ));
            r
        };

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
                &blocked_ctx,
                &menus,
                &mut title,
                &mut editing,
                &settings,
            );
            assert!(!out.clicked);
            assert!(!out.toggle_tile_move);
            assert!(!out.toggle_broadcast_select);
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
                &blocked_ctx,
                &menus,
                &mut title,
                &mut editing,
                &settings,
            );
            assert!(
                !out.clicked && !out.toggle_tile_move && out.action.is_none(),
                "Titlebar must not respond to clicks when another screen is open"
            );
        })
            .textures_delta
            .clear();
    }
}

