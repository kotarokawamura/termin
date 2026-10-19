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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PrefsTab {
    #[default]
    TitlebarAndFrame,
    Font,
}

#[derive(Default)]
pub struct PreferencesWindow {
    pub open: bool,
    pub active_tab: PrefsTab,
    pub available_fonts: Vec<(String, String)>,
    pub fonts_loaded: bool,
    pub font_filter: String,
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

        egui::Window::new("CustomDragBar Configuration")
            .open(&mut is_open)
            .resizable(true)
            .default_size([540.0, 380.0])
            .min_width(420.0)
            .min_height(240.0)
            .show(ctx, |ui| {
                // Top tab bar
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.active_tab,
                        PrefsTab::TitlebarAndFrame,
                        "Titlebar & Frame",
                    );
                    ui.selectable_value(
                        &mut self.active_tab,
                        PrefsTab::Font,
                        "Font",
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
                        }

                        ui.add_space(12.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Restore Defaults").clicked() {
                                *settings = ThemeSettings::default();
                                changed = true;
                                font_changed = true;
                            }
                        });
                    });
            });

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

        ui.label(
            "Enter color codes (e.g. #2b2b2b, rgb(40,40,40)) or select using the color picker.\n\
             Empty or invalid values automatically fallback to default colors.",
        );
        ui.add_space(8.0);

        egui::Grid::new("prefscolor_grid")
            .num_columns(4)
            .spacing([10.0, 8.0])
            .striped(true)
            .show(ui, |ui| {
                changed |= color_row(
                    ui,
                    "Active Title BG:",
                    &mut settings.title_active_bg,
                    egui::Color32::from_rgb(200, 0, 3),
                    "Default (#c80003)",
                );
                changed |= color_row(
                    ui,
                    "Active Title FG:",
                    &mut settings.title_active_fg,
                    egui::Color32::WHITE,
                    "Default (#ffffff)",
                );
                changed |= color_row(
                    ui,
                    "Inactive Title BG:",
                    &mut settings.title_inactive_bg,
                    egui::Color32::from_rgb(75, 75, 75),
                    "Default (#4b4b4b)",
                );
                changed |= color_row(
                    ui,
                    "Inactive Title FG:",
                    &mut settings.title_inactive_fg,
                    egui::Color32::from_rgb(210, 210, 210),
                    "Default (#d2d2d2)",
                );
                changed |= color_row(
                    ui,
                    "Window Frame BG:",
                    &mut settings.frame_bg_color,
                    egui::Color32::from_rgb(43, 43, 43),
                    "Default (#2b2b2b)",
                );
                changed |= color_row(
                    ui,
                    "Window Border Color:",
                    &mut settings.frame_border_color,
                    egui::Color32::from_rgb(90, 90, 90),
                    "Default (#5a5a5a)",
                );
                changed |= color_row(
                    ui,
                    "Pane Separator Color:",
                    &mut settings.separator_color,
                    egui::Color32::from_rgb(74, 74, 74),
                    "Default (#4a4a4a)",
                );

                ui.label("Window Border Width (px):");
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

        ui.label("Select a font installed on your Debian system or enter a custom font file path (.ttf / .otf / .ttc).");
        ui.add_space(8.0);

        egui::Grid::new("prefs_font_grid")
            .num_columns(2)
            .spacing([12.0, 10.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Filter System Fonts:");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.font_filter)
                            .desired_width(220.0)
                            .hint_text("Type to filter (e.g. noto, mono, vl)..."),
                    );
                    if ui.button("Rescan").clicked() {
                        self.available_fonts = discover_system_fonts();
                    }
                });
                ui.end_row();

                ui.label("System Font:");
                let current_name = std::path::Path::new(&settings.font_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Default (Auto-detect)");

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

                ui.label("Font File Path:");
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

                ui.label("UI & Menu Font Size (px):");
                if ui
                    .add(egui::Slider::new(&mut settings.ui_font_size, 9.0..=28.0).step_by(0.5))
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Terminal Font Size (px):");
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
            ui.label("Preview:");
            ui.label("ASCII: ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789 !@#$%^&*()");
            ui.label("Japanese: 日本語テキストの表示テスト（コピー・ペースト・ペイン分割）");
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
            .hint_text("Empty = Default"),
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
        ui.colored_label(egui::Color32::from_rgb(74, 222, 128), "Applied");
    } else if value.trim().is_empty() {
        ui.colored_label(egui::Color32::from_rgb(156, 163, 175), fallback_text);
    } else {
        ui.colored_label(
            egui::Color32::from_rgb(251, 191, 36),
            format!("Invalid -> {}", fallback_text),
        );
    }

    ui.end_row();
    changed
}