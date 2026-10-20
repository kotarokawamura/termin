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
use eframe::egui;
use crate::config::theme::{
    color_to_hex, discover_system_fonts, parse_color, ThemeSettings,
};
use crate::i18n::{active_catalog_info, discover_po_files, tr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PrefsTab {
    #[default]
    TitlebarAndFrame,
    Font,
    Language,
}

#[derive(Default)]
pub struct PreferencesWindow {
    pub open: bool,
    pub active_tab: PrefsTab,
    pub available_fonts: Vec<(String, String)>,
    pub fonts_loaded: bool,
    pub font_filter: String,
    pub save_layout_requested: bool,
    pub load_layout_requested: bool,
}

impl PreferencesWindow {
    pub fn ui(&mut self, ctx: &egui::Context, settings: &mut ThemeSettings) {
        if !self.open {
            return;
        }

        if !self.fonts_loaded {
            self.available_fonts = discover_system_fonts();
            self.fonts_loaded = true;
        }

        let mut is_open = self.open;
        let mut changed = false;
        let mut font_changed = false;
        let mut lang_changed = false;

        egui::Window::new(tr("CustomDragBar Configuration"))
            .id(egui::Id::new("custom_drag_bar_prefs_window"))
            .open(&mut is_open)
            .resizable(true)
            .default_size([560.0, 410.0])
            .min_width(420.0)
            .min_height(240.0)
            .show(ctx, |ui| {
                // Top tab bar
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.active_tab,
                        PrefsTab::TitlebarAndFrame,
                        tr("Titlebar & Frame"),
                    );
                    ui.selectable_value(
                        &mut self.active_tab,
                        PrefsTab::Font,
                        tr("Font"),
                    );
                    ui.selectable_value(
                        &mut self.active_tab,
                        PrefsTab::Language,
                        tr("Language"),
                    );
                });
                ui.separator();

                // Use auto_shrink([false, false]) so the window can be freely resized vertically
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        match self.active_tab {
                            PrefsTab::TitlebarAndFrame => {
                                changed |= self.render_titlebar_tab(ui, settings);
                            }
                            PrefsTab::Font => {
                                if self.render_font_tab(ui, settings) {
                                    changed = true;
                                    font_changed = true;
                                }
                            }
                            PrefsTab::Language => {
                                if self.render_language_tab(ui, settings) {
                                    changed = true;
                                    lang_changed = true;
                                }
                            }
                        }

                        ui.add_space(12.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(tr("Restore Defaults")).clicked() {
                                *settings = ThemeSettings::default();
                                changed = true;
                                font_changed = true;
                                lang_changed = true;
                            }
                            if ui.button(tr("Save Current Layout")).clicked() {
                                self.save_layout_requested = true;
                            }
                            if ui.button(tr("Load Saved Layout")).clicked() {
                                self.load_layout_requested = true;
                            }
                        });
                    });
            });

        if lang_changed {
            settings.apply_language();
        }
        if font_changed {
            settings.apply_fonts(ctx);
        }
        if changed {
            settings.save();
        }
        self.open = is_open;
    }

    fn render_titlebar_tab(&mut self, ui: &mut egui::Ui, settings: &mut ThemeSettings) -> bool {
        let mut changed = false;

        ui.label(tr(
            "Enter color codes (e.g. #2b2b2b, rgb(40,40,40)) or select using the color picker.\n\
             Empty or invalid values automatically fallback to default colors.",
        ));
        ui.add_space(8.0);

        egui::Grid::new("prefscolor_grid")
            .num_columns(4)
            .spacing([10.0, 8.0])
            .striped(true)
            .show(ui, |ui| {
                changed |= color_row(
                    ui,
                    &tr("Active Title BG:"),
                    &mut settings.title_active_bg,
                    egui::Color32::from_rgb(200, 0, 3),
                    "Default (#c80003)",
                );
                changed |= color_row(
                    ui,
                    &tr("Active Title FG:"),
                    &mut settings.title_active_fg,
                    egui::Color32::WHITE,
                    "Default (#ffffff)",
                );
                changed |= color_row(
                    ui,
                    &tr("Inactive Title BG:"),
                    &mut settings.title_inactive_bg,
                    egui::Color32::from_rgb(75, 75, 75),
                    "Default (#4b4b4b)",
                );
                changed |= color_row(
                    ui,
                    &tr("Inactive Title FG:"),
                    &mut settings.title_inactive_fg,
                    egui::Color32::from_rgb(210, 210, 210),
                    "Default (#d2d2d2)",
                );
                changed |= color_row(
                    ui,
                    &tr("Window Frame BG:"),
                    &mut settings.frame_bg_color,
                    egui::Color32::from_rgb(43, 43, 43),
                    "Default (#2b2b2b)",
                );
                changed |= color_row(
                    ui,
                    &tr("Window Border Color:"),
                    &mut settings.frame_border_color,
                    egui::Color32::from_rgb(90, 90, 90),
                    "Default (#5a5a5a)",
                );
                changed |= color_row(
                    ui,
                    &tr("Pane Separator Color:"),
                    &mut settings.separator_color,
                    egui::Color32::from_rgb(74, 74, 74),
                    "Default (#4a4a4a)",
                );

                ui.label(tr("Pane Separator Width (px):"));
                if ui
                    .add(egui::Slider::new(&mut settings.separator_width, 1.0..=16.0).step_by(1.0))
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label(tr("Window Border Width (px):"));
                if ui
                    .add(egui::Slider::new(&mut settings.frame_border_width, 1.0..=20.0).step_by(1.0))
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();
            });

        changed
    }

    fn render_font_tab(&mut self, ui: &mut egui::Ui, settings: &mut ThemeSettings) -> bool {
        let mut changed = false;

        ui.label(tr(
            "Select a font installed on your Debian system or enter a custom font file path (.ttf / .otf / .ttc).",
        ));
        ui.add_space(8.0);

        egui::Grid::new("prefs_font_grid")
            .num_columns(2)
            .spacing([12.0, 10.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(tr("Filter System Fonts:"));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.font_filter)
                            .desired_width(220.0)
                            .hint_text(tr("Type to filter (e.g. noto, mono, vl)...")),
                    );
                    if ui.button(tr("Rescan")).clicked() {
                        self.available_fonts = discover_system_fonts();
                    }
                });
                ui.end_row();

                ui.label(tr("System Font:"));
                let default_font_label = tr("Default (Auto-detect)");
                let current_name = std::path::Path::new(&settings.font_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&default_font_label);

                let filter_lower = self.font_filter.to_lowercase();
                egui::ComboBox::from_id_salt("debian_system_font_combo")
                    .selected_text(current_name)
                    .width(320.0)
                    .show_ui(ui, |ui| {
                        for (name, path) in &self.available_fonts {
                            if !filter_lower.is_empty()
                                && !name.to_lowercase().contains(&filter_lower)
                            {
                                continue;
                            }
                            if ui
                                .selectable_value(&mut settings.font_path, path.clone(), name)
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    });
                ui.end_row();

                ui.label(tr("Font File Path:"));
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.font_path)
                            .desired_width(320.0)
                            .hint_text("/usr/share/fonts/..."),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label(tr("UI & Menu Font Size (px):"));
                if ui
                    .add(egui::Slider::new(&mut settings.ui_font_size, 9.0..=28.0).step_by(0.5))
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label(tr("Terminal Font Size (px):"));
                if ui
                    .add(egui::Slider::new(&mut settings.terminal_font_size, 8.0..=36.0).step_by(0.5))
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();
            });

        ui.add_space(10.0);
        ui.group(|ui| {
            ui.label(tr("Preview:"));
            ui.label("ASCII: ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789 !@#$%^&*()");
            ui.label("Japanese: 日本語テキストの表示テスト（コピー・ペースト・ペイン分割）");
        });

        changed
    }

    fn render_language_tab(&mut self, ui: &mut egui::Ui, settings: &mut ThemeSettings) -> bool {
        let mut changed = false;

        ui.label(tr(
            "Select UI language or load a custom GNU gettext (.po) translation file.",
        ));
        ui.add_space(8.0);

        let discovered_po = discover_po_files();
        let (active_locale, entry_count) = active_catalog_info();

        egui::Grid::new("prefs_lang_grid")
            .num_columns(2)
            .spacing([12.0, 10.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(tr("Language (.po):"));
                let selected_label = match settings.language.as_str() {
                    "auto" | "" => tr("Auto (System Locale)"),
                    "en" => "English (en.po)".to_string(),
                    "ja" => "日本語 (ja.po)".to_string(),
                    other => format!("{}.po", other),
                };

                egui::ComboBox::from_id_salt("language_po_combo")
                    .selected_text(selected_label)
                    .width(260.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_value(
                                &mut settings.language,
                                "auto".to_string(),
                                tr("Auto (System Locale)"),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        if ui
                            .selectable_value(
                                &mut settings.language,
                                "en".to_string(),
                                "English (en.po)",
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        if ui
                            .selectable_value(
                                &mut settings.language,
                                "ja".to_string(),
                                "日本語 (ja.po)",
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        for (lang_code, path) in &discovered_po {
                            if lang_code == "en" || lang_code == "ja" {
                                continue;
                            }
                            let label = format!("{} ({})", lang_code, path.display());
                            if ui
                                .selectable_value(
                                    &mut settings.language,
                                    lang_code.clone(),
                                    label,
                                )
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    });
                ui.end_row();

                ui.label(tr("Custom .po File Path:"));
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut settings.custom_po_path)
                                .desired_width(240.0)
                                .hint_text("po/ja.po"),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui.button(tr("Reload .po Files")).clicked() {
                        changed = true;
                    }
                });
                ui.end_row();

                ui.label(tr("Active Catalog:"));
                ui.label(format!("{} ({} strings)", active_locale, entry_count));
                ui.end_row();
            });

        changed
    }
}

fn color_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    fallback_preview: egui::Color32,
    fallback_text: &str,
) -> bool {
    let mut changed = false;
    ui.label(label);

    let resp = ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(110.0)
            .hint_text(tr("Empty = Default")),
    );
    if resp.changed() {
        changed = true;
    }

    let parsed = parse_color(value);
    let mut picker_color = parsed.unwrap_or(fallback_preview);
    if ui.color_edit_button_srgba(&mut picker_color).changed() {
        *value = color_to_hex(picker_color);
        changed = true;
    }

    if parsed.is_some() {
        ui.colored_label(egui::Color32::from_rgb(74, 222, 128), tr("Applied"));
    } else if value.trim().is_empty() {
        ui.colored_label(egui::Color32::from_rgb(156, 163, 175), fallback_text);
    } else {
        ui.colored_label(
            egui::Color32::from_rgb(251, 191, 36),
            format!("{} -> {}", tr("Invalid"), fallback_text),
        );
    }

    ui.end_row();
    changed
}
