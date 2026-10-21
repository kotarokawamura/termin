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
    fn test_font_sample_catalog_does_not_panic_on_lazy_load() {
        let ctx = egui::Context::default();
        let mut settings = ThemeSettings::default();
        settings.apply_fonts(&ctx);

        let mut prefs = PreferencesWindow::default();
        prefs.open = true;
        prefs.active_tab = PrefsTab::Font;
        prefs.font_sample_window_open = true;

        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1024.0, 768.0));

        // Run several frames so lazy font loading triggers in frame 0 and binds in subsequent frames
        for _ in 0..4 {
            let mut raw = egui::RawInput::default();
            raw.screen_rect = Some(screen);
            ctx.run_ui(raw, |ui| {
                let ctx_clone = ui.ctx().clone();
                prefs.ui(&ctx_clone, &mut settings);
            })
                .textures_delta
                .clear();
        }

        if !prefs.available_fonts.is_empty() {
            assert!(
                !prefs.loaded_preview_fonts.is_empty(),
                "At least one system font preview should be loaded"
            );
        }
    }
}
