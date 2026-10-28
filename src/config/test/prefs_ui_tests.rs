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
    use crate::config::theme::parse_key_token;

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

    #[test]
    fn test_parse_color_and_color_to_hex() {
        assert_eq!(
            parse_color("#c80003"),
            Some(egui::Color32::from_rgb(200, 0, 3))
        );
        assert_eq!(
            parse_color("  #2B2B2B  "),
            Some(egui::Color32::from_rgb(43, 43, 43))
        );
        assert_eq!(
            parse_color("rgb(25, 118, 210)"),
            Some(egui::Color32::from_rgb(25, 118, 210))
        );
        assert_eq!(parse_color(""), None);
        assert_eq!(parse_color("#12345"), None);
        assert_eq!(parse_color("#gggggg"), None);
        assert_eq!(parse_color("rgb(300, 0, 0)"), None);
        assert_eq!(parse_color("rgb(10, 20)"), None);

        let hex = color_to_hex(egui::Color32::from_rgb(200, 0, 3));
        assert_eq!(hex, "#c80003");
        assert_eq!(
            parse_color(&hex),
            Some(egui::Color32::from_rgb(200, 0, 3))
        );
    }

    #[test]
    fn test_theme_settings_clamping_and_color_fallbacks() {
        let mut s = ThemeSettings::default();
        s.ui_font_size = 2.0;
        assert_eq!(s.effective_ui_font_size(), 8.0);
        s.ui_font_size = 100.0;
        assert_eq!(s.effective_ui_font_size(), 32.0);

        s.terminal_font_size = 1.0;
        assert_eq!(s.effective_terminal_font_size(), 8.0);
        s.terminal_font_size = 99.0;
        assert_eq!(s.effective_terminal_font_size(), 48.0);

        s.frame_border_width = 0.0;
        assert_eq!(s.effective_border_width(), 1.0);
        s.frame_border_width = 50.0;
        assert_eq!(s.effective_border_width(), 20.0);

        s.separator_width = 0.0;
        assert_eq!(s.effective_separator_width(), 1.0);
        s.separator_width = 40.0;
        assert_eq!(s.effective_separator_width(), 16.0);

        s.target_fps = 0;
        assert_eq!(s.effective_fps(), 1);
        s.target_fps = 500;
        assert_eq!(s.effective_fps(), 240);

        // Invalid color strings fall back to defaults
        s.title_active_bg = "invalid".to_string();
        s.title_inactive_bg = String::new();
        assert_eq!(s.titlebar_bg(true), egui::Color32::from_rgb(117, 33, 0));
        assert_eq!(s.titlebar_bg(false), egui::Color32::from_rgb(75, 75, 75));

        s.title_active_fg = "invalid".to_string();
        s.title_inactive_fg = String::new();
        assert_eq!(s.titlebar_fg(true), egui::Color32::WHITE);
        assert_eq!(s.titlebar_fg(false), egui::Color32::from_rgb(210, 210, 210));

        // Opacity application on terminal_bg_color and frame_bg_color
        s.terminal_bg_color = "#102030".to_string();
        s.terminal_bg_opacity = 0.5;
        let bg = s.terminal_bg_color_with_opacity();
        assert_eq!(bg.to_srgba_unmultiplied(), [0x10, 0x20, 0x30, 128]);

        // Broadcast group palette cycling (groups 1..=6 are distinct, group 7 wraps to group 1)
        for gid in 1..=6 {
            assert_ne!(
                s.broadcast_group_titlebar_bg(gid, true),
                s.broadcast_group_titlebar_bg(gid, false)
            );
        }
        assert_eq!(
            s.broadcast_group_titlebar_bg(1, true),
            s.broadcast_group_titlebar_bg(7, true)
        );
    }

    #[test]
    fn test_font_bytes_validation_and_classification() {
        assert!(!is_valid_font_bytes(b"short"));
        assert!(!is_valid_font_bytes(b"not a valid font header at all!!"));

        // Minimal synthetic TrueType header: magic 0x00010000, num_tables = 1, plus 16-byte table record (total >= 28 bytes)
        let mut ttf = vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        ttf.extend_from_slice(&[0u8; 16]);
        assert!(is_valid_font_bytes(&ttf));

        // Truncated table directory should fail
        assert!(!is_valid_font_bytes(&ttf[..20]));

        // Monospace and CJK classification
        assert!(is_monospace_font("DejaVuSansMono.ttf", "/usr/share/fonts/DejaVuSansMono.ttf"));
        assert!(is_monospace_font("VL-Gothic-Regular.ttf", "/usr/share/fonts/vlgothic/VL-Gothic-Regular.ttf"));
        assert!(!is_monospace_font("Lato-Regular.ttf", "/usr/share/fonts/lato/Lato-Regular.ttf"));

        assert!(is_cjk_font("NotoSansMonoCJKjp-Regular.otf", "/usr/share/fonts/opentype/noto/NotoSansMonoCJKjp-Regular.otf"));
        assert!(is_cjk_font("ipag.ttf", "/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf"));
        assert!(!is_cjk_font("DejaVuSansMono.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"));
    }

    #[test]
    fn test_shortcut_parsing_splitting_matching_and_formatting() {
        // Splitting multiple bindings while preserving "Ctrl+,"
        let bindings = split_shortcut_bindings("Ctrl+, / Super+C | Alt+Shift+T, F11");
        assert_eq!(
            bindings,
            vec!["Ctrl+,", "Super+C", "Alt+Shift+T", "F11"]
        );

        let ctrl_shift = egui::Modifiers {
            ctrl: true,
            shift: true,
            ..egui::Modifiers::NONE
        };
        assert!(shortcut_matches_event(
            "Ctrl+Shift+C",
            egui::Key::C,
            Some(egui::Key::C),
            ctrl_shift,
            false
        ));
        assert!(!shortcut_matches_event(
            "Ctrl+Shift+C",
            egui::Key::C,
            Some(egui::Key::C),
            egui::Modifiers::CTRL,
            false
        ));

        // Comma key binding
        assert!(shortcut_matches_event(
            "Ctrl+,",
            egui::Key::Comma,
            Some(egui::Key::Comma),
            egui::Modifiers::CTRL,
            false
        ));

        // Super key binding
        assert!(shortcut_matches_event(
            "Super+T",
            egui::Key::T,
            Some(egui::Key::T),
            egui::Modifiers::NONE,
            true
        ));

        // Plus and Equals equivalence for zoom in
        assert!(shortcut_matches_event(
            "Ctrl++ / Ctrl+=",
            egui::Key::Plus,
            Some(egui::Key::Equals),
            egui::Modifiers::CTRL,
            false
        ));

        // Modifier-only keys are rejected
        assert!(is_modifier_only_key(egui::Key::SuperLeft));
        assert!(!is_modifier_only_key(egui::Key::A));
        assert_eq!(
            format_key_shortcut(egui::Key::SuperLeft, None, egui::Modifiers::NONE, true),
            None
        );

        // Formatting valid key combination
        let formatted = format_key_shortcut(
            egui::Key::K,
            Some(egui::Key::K),
            egui::Modifiers {
                ctrl: true,
                alt: true,
                ..egui::Modifiers::NONE
            },
            true,
        );
        assert_eq!(formatted.as_deref(), Some("Super+Ctrl+Alt+K"));

        // Key token parser coverage
        assert_eq!(parse_key_token("Enter"), Some(egui::Key::Enter));
        assert_eq!(parse_key_token("Esc"), Some(egui::Key::Escape));
        assert_eq!(parse_key_token("PageUp"), Some(egui::Key::PageUp));
        assert_eq!(parse_key_token("F12"), Some(egui::Key::F12));
        assert_eq!(parse_key_token("Plus"), Some(egui::Key::Plus));
        assert_eq!(parse_key_token("UnknownKeyToken"), None);
    }

    #[test]
    fn test_custom_config_file_save_load_and_json_file_override() {
        let tmp_dir = std::env::temp_dir().join(format!("termin_test_cfg_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp_dir);
        let cfg_file = tmp_dir.join("custom_config.json");
        let override_file = tmp_dir.join("override.json");

        ThemeSettings::set_custom_config_path(Some(cfg_file.clone()));
        assert_eq!(ThemeSettings::config_path(), cfg_file);

        let mut s = ThemeSettings::default();
        s.target_fps = 90;
        s.title_active_bg = "#112233".to_string();
        s.save();

        let mut loaded = ThemeSettings::load();
        assert_eq!(loaded.target_fps, 90);
        assert_eq!(loaded.title_active_bg, "#112233");

        // Override from a JSON file path via apply_config_json
        std::fs::write(
            &override_file,
            r##"{"target_fps": 165, "shortcuts": {"copy": "Alt+C"}}"##,
        )
        .unwrap();
        loaded
            .apply_config_json(override_file.to_str().unwrap())
            .expect("should apply JSON override from file");
        assert_eq!(loaded.target_fps, 165);
        assert_eq!(loaded.shortcuts.copy, "Alt+C");
        assert_eq!(loaded.title_active_bg, "#112233");

        ThemeSettings::set_custom_config_path(None);
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_preferences_window_all_tabs_and_close_shortcut() {
        let ctx = egui::Context::default();
        let mut settings = ThemeSettings::default();
        settings.apply_fonts(&ctx);

        let mut prefs = PreferencesWindow::default();
        prefs.open = true;
        prefs.fonts_loaded = true; // Avoid full filesystem scan in unit test

        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1024.0, 768.0));

        for tab in [
            PrefsTab::TitlebarAndFrame,
            PrefsTab::Font,
            PrefsTab::Shortcuts,
            PrefsTab::Language,
        ] {
            prefs.active_tab = tab;
            let mut raw = egui::RawInput::default();
            raw.screen_rect = Some(screen);
            raw.focused = true;
            ctx.run_ui(raw, |ui| {
                let ctx_clone = ui.ctx().clone();
                prefs.ui(&ctx_clone, &mut settings);
            })
            .textures_delta
            .clear();
            assert!(prefs.any_open());
        }

        // Pressing Ctrl+Shift+W (default close_preferences shortcut) closes the preferences window
        let mut raw_close = egui::RawInput::default();
        raw_close.screen_rect = Some(screen);
        raw_close.focused = true;
        raw_close.events.push(egui::Event::Key {
            key: egui::Key::W,
            physical_key: Some(egui::Key::W),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                shift: true,
                ..egui::Modifiers::NONE
            },
        });
        ctx.run_ui(raw_close, |ui| {
            let ctx_clone = ui.ctx().clone();
            prefs.ui(&ctx_clone, &mut settings);
        })
        .textures_delta
        .clear();

        assert!(!prefs.open, "Ctrl+Shift+W shortcut must close PreferencesWindow");
    }
}
