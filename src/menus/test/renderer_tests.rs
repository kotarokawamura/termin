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
}