use eframe::egui;
use crate::config::theme::{color_to_hex, parse_color, ThemeSettings};

#[derive(Default)]
pub struct PreferencesWindow {
    pub open: bool,
}

impl PreferencesWindow {
    pub fn ui(&mut self, ctx: &egui::Context, settings: &mut ThemeSettings) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        let mut changed = false;

        egui::Window::new("CustomDragBar Configuration")
            .open(&mut is_open)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| {
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

                ui.add_space(10.0);
                if ui.button("Restore Defaults").clicked() {
                    *settings = ThemeSettings::default();
                    changed = true;
                }
            });

        if changed {
            settings.save();
        }
        self.open = is_open;
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
