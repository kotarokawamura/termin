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
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::Arc;
use eframe::egui;
use crate::config::theme::{
    color_to_hex, discover_system_fonts, format_key_shortcut, is_cjk_font, is_monospace_font,
    is_valid_font_bytes, parse_color, ShortcutSettings, ThemeSettings,is_super_down
};
use crate::i18n::{active_catalog_info, discover_po_files, tr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PrefsTab {
    #[default]
    TitlebarAndFrame,
    Font,
    Shortcuts,
    Language,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FontCategoryFilter {
    #[default]
    All,
    Monospace,
    JapaneseCjk,
}

#[derive(Default)]
pub struct PreferencesWindow {
    pub open: bool,
    pub font_sample_window_open: bool,
    pub active_tab: PrefsTab,
    pub available_fonts: Vec<(String, String)>,
    pub fonts_loaded: bool,
    pub font_filter: String,
    pub sample_filter: String,
    pub sample_category: FontCategoryFilter,
    pub sample_text_ascii: String,
    pub sample_text_cjk: String,
    pub loaded_preview_fonts: HashMap<String, (String, Arc<egui::FontData>)>,
    pub skipped_preview_fonts: HashSet<String>,
    pub next_preview_font_id: usize,
    pub recording_shortcut: Option<&'static str>,
    pub save_layout_requested: bool,
    pub load_layout_requested: bool,
    pub window_rects: Vec<egui::Rect>,
    pub pointer_captured_by_subwindow: bool,
    pub subwindow_dragging: bool,
}

impl PreferencesWindow {
    pub fn any_open(&self) -> bool {
        self.open || self.font_sample_window_open
    }

    pub fn is_capturing_pointer(&self, ctx: &egui::Context) -> bool {
        if !self.any_open() {
            return false;
        }
        if self.pointer_captured_by_subwindow || self.subwindow_dragging {
            return true;
        }
        let pointer_pos = ctx.input(|i| i.pointer.interact_pos().or_else(|| i.pointer.hover_pos()));
        if let Some(pos) = pointer_pos {
            if self.window_rects.iter().any(|r| r.expand(4.0).contains(pos)) {
                return true;
            }
            if ctx
                .layer_id_at(pos)
                .is_some_and(|l| l.order != egui::Order::Background)
            {
                return true;
            }
        }
        false
    }

    pub fn ui(&mut self, ctx: &egui::Context, settings: &mut ThemeSettings) {
        self.window_rects.clear();
        self.subwindow_dragging = false;

        if !self.open && !self.font_sample_window_open {
            self.pointer_captured_by_subwindow = false;
            return;
        }

        if !self.fonts_loaded {
            self.available_fonts = discover_system_fonts();
            self.fonts_loaded = true;
        }
        if self.sample_text_ascii.is_empty() {
            self.sample_text_ascii =
                "ASCII: ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789 !@#$%^&*()".to_string();
        }
        if self.sample_text_cjk.is_empty() {
            self.sample_text_cjk =
                "Japanese: 日本語テキストの表示テスト（コピー・ペースト・ペイン分割）".to_string();
        }

        let mut is_open = self.open;
        let mut changed = false;
        let mut font_changed = false;
        let mut lang_changed = false;

        if self.open {
            if let Some(inner) = egui::Window::new(
                egui::RichText::new(tr("CustomDragBar Configuration"))
                    .size(14.5)
                    .color(egui::Color32::WHITE)
                    .strong(),
            )
            .id(egui::Id::new("custom_drag_bar_prefs_window_v2"))
            .open(&mut is_open)
            .resizable(true)
            .collapsible(true)
            .default_size([680.0, 480.0])
            .min_width(480.0)
            .min_height(320.0)
            .show(ctx, |ui| {
                apply_prefs_window_style(ui);

                // Top tab bar
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut self.active_tab,
                        PrefsTab::TitlebarAndFrame,
                        egui::RichText::new(tr("Titlebar & Frame")).strong(),
                    );
                    ui.selectable_value(
                        &mut self.active_tab,
                        PrefsTab::Font,
                        egui::RichText::new(tr("Font")).strong(),
                    );
                    ui.selectable_value(
                        &mut self.active_tab,
                        PrefsTab::Shortcuts,
                        egui::RichText::new(tr("Shortcuts")).strong(),
                    );
                    ui.selectable_value(
                        &mut self.active_tab,
                        PrefsTab::Language,
                        egui::RichText::new(tr("Language")).strong(),
                    );
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

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
                            PrefsTab::Shortcuts => {
                                changed |= self.render_shortcuts_tab(ui, settings);
                            }
                            PrefsTab::Language => {
                                if self.render_language_tab(ui, settings) {
                                    changed = true;
                                    lang_changed = true;
                                }
                            }
                        }

                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
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
            }) {
                self.window_rects.push(inner.response.rect);
                if inner.response.dragged() || inner.response.is_pointer_button_down_on() {
                    self.subwindow_dragging = true;
                }
            }
            self.open = is_open;
        }

        if self.font_sample_window_open {
            if self.render_font_sample_window(ctx, settings) {
                changed = true;
                font_changed = true;
            }
        }

        // Update sub-window pointer capture state so dragging a sub-window titlebar never leaks to background panels
        let pointer_down = ctx.input(|i| i.pointer.any_down());
        let pointer_pressed = ctx.input(|i| i.pointer.any_pressed());
        let pointer_pos = ctx.input(|i| i.pointer.interact_pos().or_else(|| i.pointer.hover_pos()));
        let over_any_subwindow = pointer_pos.is_some_and(|pos| {
            self.window_rects.iter().any(|r| r.expand(6.0).contains(pos))
                || ctx
                    .layer_id_at(pos)
                    .is_some_and(|l| l.order != egui::Order::Background)
        });
        if pointer_pressed && over_any_subwindow {
            self.pointer_captured_by_subwindow = true;
        }
        if self.subwindow_dragging {
            self.pointer_captured_by_subwindow = true;
        }
        if !pointer_down {
            self.pointer_captured_by_subwindow = false;
        } else if self.pointer_captured_by_subwindow {
            self.subwindow_dragging = true;
        }

        if lang_changed {
            settings.apply_language();
        }
        if font_changed {
            let previews: Vec<(String, Arc<egui::FontData>)> =
                self.loaded_preview_fonts.values().cloned().collect();
            settings.apply_fonts_with_previews(ctx, &previews);
        }
        if changed {
            settings.save();
        }
    }

    fn render_titlebar_tab(&mut self, ui: &mut egui::Ui, settings: &mut ThemeSettings) -> bool {
        let mut changed = false;

        ui.label(tr(
            "Enter color codes (e.g. #2b2b2b, rgb(40,40,40)) or select using the color picker.\n\
             Empty or invalid values automatically fallback to default colors.",
        ));
        ui.add_space(10.0);

        egui::Grid::new("prefscolor_grid")
            .num_columns(4)
            .spacing([14.0, 10.0])
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
                    .add(
                        egui::Slider::new(&mut settings.frame_border_width, 1.0..=20.0)
                            .step_by(1.0),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label(tr("Target FPS:"));
                if ui
                    .add(
                        egui::Slider::new(&mut settings.target_fps, 1..=240)
                            .suffix(" FPS"),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();            });

        changed
    }

    fn render_font_tab(&mut self, ui: &mut egui::Ui, settings: &mut ThemeSettings) -> bool {
        let mut changed = false;

        ui.label(tr(
            "Select a font installed on your Debian system or enter a custom font file path (.ttf / .otf / .ttc).",
        ));
        ui.add_space(10.0);

        egui::Grid::new("prefs_font_grid")
            .num_columns(2)
            .spacing([14.0, 12.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(tr("Filter System Fonts:"));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.font_filter)
                            .desired_width(240.0)
                            .hint_text(tr("Type to filter (e.g. noto, mono, vl)...")),
                    );
                    if ui.button(tr("Rescan")).clicked() {
                        self.available_fonts = discover_system_fonts();
                    }
                    if ui
                        .button(
                            egui::RichText::new(format!("🗚 {}", tr("Open Font Sample Catalog...")))
                                .strong(),
                        )
                        .clicked()
                    {
                        self.font_sample_window_open = true;
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
                    .width(360.0)
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
                            .desired_width(360.0)
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
                    .add(
                        egui::Slider::new(&mut settings.terminal_font_size, 8.0..=36.0)
                            .step_by(0.5),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.group(|ui| {
            ui.label(egui::RichText::new(tr("Preview:")).strong());
            ui.label("ASCII: ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789 !@#$%^&*()");
            ui.label("Japanese: 日本語テキストの表示テスト（コピー・ペースト・ペイン分割）");
        });

        changed
    }

    fn render_font_sample_window(
        &mut self,
        ctx: &egui::Context,
        settings: &mut ThemeSettings,
    ) -> bool {
        let mut changed = false;
        let mut is_open = self.font_sample_window_open;
        let mut newly_loaded_preview = false;
        let mut loads_this_frame = 0usize;
        let mut pending_more_loads = false;

        if let Some(inner) = egui::Window::new(
            egui::RichText::new(tr("System Font Samples & Selector"))
                .size(14.5)
                .color(egui::Color32::WHITE)
                .strong(),
        )
        .id(egui::Id::new("system_font_sample_catalog_window_v2"))
        .open(&mut is_open)
        .resizable(true)
        .collapsible(true)
        .default_size([720.0, 500.0])
        .min_width(500.0)
        .min_height(340.0)
        .show(ctx, |ui| {
            apply_prefs_window_style(ui);

            ui.label(tr(
                "Browse installed system fonts with live text samples and click 'Use This Font' to apply.",
            ));
            ui.add_space(6.0);

            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Category:"));
                ui.selectable_value(
                    &mut self.sample_category,
                    FontCategoryFilter::All,
                    tr("All Fonts"),
                );
                ui.selectable_value(
                    &mut self.sample_category,
                    FontCategoryFilter::Monospace,
                    tr("Monospace"),
                );
                ui.selectable_value(
                    &mut self.sample_category,
                    FontCategoryFilter::JapaneseCjk,
                    tr("Japanese / CJK"),
                );
            });

            ui.horizontal(|ui| {
                ui.label(tr("Filter System Fonts:"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.sample_filter)
                        .desired_width(240.0)
                        .hint_text(tr("Type to filter (e.g. noto, mono, vl, gothic)...")),
                );
                if ui.button(tr("Rescan")).clicked() {
                    self.available_fonts = discover_system_fonts();
                }
            });

            ui.horizontal(|ui| {
                ui.label(tr("Sample Text:"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.sample_text_ascii).desired_width(260.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.sample_text_cjk).desired_width(260.0),
                );
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            let filter_lower = self.sample_filter.to_lowercase();
            let filtered_fonts: Vec<(String, String)> = self
                .available_fonts
                .iter()
                .filter(|(name, path)| {
                    let cat_ok = match self.sample_category {
                        FontCategoryFilter::All => true,
                        FontCategoryFilter::Monospace => is_monospace_font(name, path),
                        FontCategoryFilter::JapaneseCjk => is_cjk_font(name, path),
                    };
                    cat_ok
                        && (filter_lower.is_empty()
                            || name.to_lowercase().contains(&filter_lower)
                            || path.to_lowercase().contains(&filter_lower))
                })
                .cloned()
                .collect();

            ui.label(format!(
                "{}: {} / {}",
                tr("Matching Fonts"),
                filtered_fonts.len(),
                self.available_fonts.len()
            ));
            ui.add_space(4.0);

            let row_height = 88.0;
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show_rows(ui, row_height, filtered_fonts.len(), |ui, row_range| {
                    let visible_paths: Vec<String> = row_range
                        .clone()
                        .map(|i| filtered_fonts[i].1.clone())
                        .collect();

                    for idx in row_range {
                        let (name, path) = &filtered_fonts[idx];
                        let is_selected = &settings.font_path == path;

                        // Lazily load font data for visible rows so each sample renders in its own typeface.
                        // Note: `ctx.set_fonts` only takes effect at the start of the NEXT pass, so we also
                        // verify below that the font family is bound in `ui.ctx().fonts` before using it.
                        let fam_key = if let Some((key, _)) = self.loaded_preview_fonts.get(path) {
                            Some(key.clone())
                        } else if self.skipped_preview_fonts.contains(path) {
                            None
                        } else if loads_this_frame < 6 {
                            loads_this_frame += 1;
                            let is_reasonable_size = fs::metadata(path)
                                .map(|m| m.len() <= 35 * 1024 * 1024)
                                .unwrap_or(false);
                            let loaded_bytes = if is_reasonable_size {
                                fs::read(path).ok().filter(|b| is_valid_font_bytes(b))
                            } else {
                                None
                            };
                            if let Some(bytes) = loaded_bytes {
                                if self.loaded_preview_fonts.len() >= 96 {
                                    self.loaded_preview_fonts
                                        .retain(|k, _| visible_paths.contains(k));
                                }
                                self.next_preview_font_id += 1;
                                let key = format!("sample_font_{}", self.next_preview_font_id);
                                self.loaded_preview_fonts.insert(
                                    path.clone(),
                                    (
                                        key.clone(),
                                        Arc::new(egui::FontData::from_owned(bytes)),
                                    ),
                                );
                                newly_loaded_preview = true;
                                Some(key)
                            } else {
                                self.skipped_preview_fonts.insert(path.clone());
                                None
                            }
                        } else {
                            pending_more_loads = true;
                            None
                        };

                        let card_fill = if is_selected {
                            egui::Color32::from_rgb(35, 55, 42)
                        } else {
                            egui::Color32::from_rgb(30, 30, 34)
                        };
                        let card_stroke = if is_selected {
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(74, 222, 128))
                        } else {
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(65, 65, 72))
                        };

                        egui::Frame::new()
                            .fill(card_fill)
                            .stroke(card_stroke)
                            .inner_margin(egui::Margin::same(6))
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(name)
                                            .size(13.5)
                                            .strong()
                                            .color(egui::Color32::WHITE),
                                    );
                                    if is_monospace_font(name, path) {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(96, 165, 250),
                                            "[Mono]",
                                        );
                                    }
                                    if is_cjk_font(name, path) {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(250, 204, 21),
                                            "[CJK]",
                                        );
                                    }
                                    ui.label(
                                        egui::RichText::new(path)
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(160, 165, 175)),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if is_selected {
                                                ui.colored_label(
                                                    egui::Color32::from_rgb(74, 222, 128),
                                                    format!("✔ {}", tr("Selected")),
                                                );
                                            } else if ui.button(tr("Use This Font")).clicked() {
                                                settings.font_path = path.clone();
                                                changed = true;
                                            }
                                        },
                                    );
                                });

                                let sample_font_id = match &fam_key {
                                    Some(k) => {
                                        let family = egui::FontFamily::Name(k.clone().into());
                                        let is_bound = ui.ctx().fonts(|f| {
                                            let defs = f.definitions();
                                            defs.families.get(&family).is_some_and(|list| {
                                                !list.is_empty()
                                                    && list
                                                        .iter()
                                                        .all(|n| defs.font_data.contains_key(n))
                                            })
                                        });
                                        if is_bound {
                                            egui::FontId::new(14.5, family)
                                        } else {
                                            newly_loaded_preview = true;
                                            egui::FontId::monospace(14.5)
                                        }
                                    }
                                    None => egui::FontId::monospace(14.5),
                                };

                                ui.label(
                                    egui::RichText::new(&self.sample_text_ascii)
                                        .font(sample_font_id.clone())
                                        .color(egui::Color32::from_rgb(230, 230, 235)),
                                );
                                ui.label(
                                    egui::RichText::new(&self.sample_text_cjk)
                                        .font(sample_font_id)
                                        .color(egui::Color32::from_rgb(210, 220, 230)),
                                );
                            });
                        ui.add_space(4.0);
                    }
                });
        }) {
            self.window_rects.push(inner.response.rect);
            if inner.response.dragged() || inner.response.is_pointer_button_down_on() {
                self.subwindow_dragging = true;
            }
        }

        if newly_loaded_preview {
            let previews: Vec<(String, Arc<egui::FontData>)> =
                self.loaded_preview_fonts.values().cloned().collect();
            settings.apply_fonts_with_previews(ctx, &previews);
            ctx.request_repaint();
        } else if pending_more_loads {
            ctx.request_repaint();
        }

        self.font_sample_window_open = is_open;
        changed
    }

    fn render_shortcuts_tab(&mut self, ui: &mut egui::Ui, settings: &mut ThemeSettings) -> bool {
        let mut changed = false;

        ui.label(tr(
            "Configure keyboard shortcuts for terminal and workspace actions.\n\
             You can type a shortcut directly (e.g. Ctrl+Shift+T, Super+Enter, Alt+D, F11) or click 'Record' and press a key combination.",
        ));
        ui.add_space(10.0);

        // If recording is active, capture the next non-modifier key press
        let mut recorded_binding: Option<(&'static str, String)> = None;
        let mut cancel_recording = false;
        if let Some(target_field) = self.recording_shortcut {
            // 録画中は TextEdit 等にキー入力が吸われないようフォーカスを外す
            if let Some(focused_id) = ui.ctx().memory(|m| m.focused()) {
                ui.ctx().memory_mut(|m| m.surrender_focus(focused_id));
            }

            let current_mods = ui.input(|i| i.modifiers);
            let super_held = ui.input(|i| is_super_down(i, None));
            let events = ui.input(|i| i.events.clone());

            for ev in events {
                match ev {
                    egui::Event::Key {
                        key,
                        physical_key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        let ev_super = super_held || modifiers.mac_cmd;
                        // modifiers は Event 側のものと InputState 側の論理和をとる（複数修飾キー同時押し時の取りこぼし防止）
                        let merged_mods = egui::Modifiers {
                            alt: modifiers.alt || current_mods.alt,
                            ctrl: modifiers.ctrl || current_mods.ctrl,
                            shift: modifiers.shift || current_mods.shift,
                            mac_cmd: modifiers.mac_cmd || current_mods.mac_cmd,
                            command: modifiers.command || current_mods.command,
                        };

                        if key == egui::Key::Escape && merged_mods.is_none() && !ev_super {
                            cancel_recording = true;
                            break;
                        }
                        if let Some(formatted) =
                            format_key_shortcut(key, physical_key, merged_mods, ev_super)
                        {
                            recorded_binding = Some((target_field, formatted));
                            break;
                        }
                    }
                    // egui-winit が Ctrl+C / Ctrl+Shift+C 等を Event::Copy / Cut / Paste に変換してしまうケースへの対応
                    egui::Event::Copy => {
                        let mut mods = current_mods;
                        mods.ctrl = true;
                        if let Some(formatted) =
                            format_key_shortcut(egui::Key::C, Some(egui::Key::C), mods, super_held)
                        {
                            recorded_binding = Some((target_field, formatted));
                            break;
                        }
                    }
                    egui::Event::Cut => {
                        let mut mods = current_mods;
                        mods.ctrl = true;
                        if let Some(formatted) =
                            format_key_shortcut(egui::Key::X, Some(egui::Key::X), mods, super_held)
                        {
                            recorded_binding = Some((target_field, formatted));
                            break;
                        }
                    }
                    egui::Event::Paste(_) => {
                        let mut mods = current_mods;
                        mods.ctrl = true;
                        if let Some(formatted) =
                            format_key_shortcut(egui::Key::V, Some(egui::Key::V), mods, super_held)
                        {
                            recorded_binding = Some((target_field, formatted));
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        if cancel_recording {
            self.recording_shortcut = None;
        }
        let defaults = ShortcutSettings::default();

        egui::Grid::new("prefs_shortcuts_grid")
            .num_columns(4)
            .spacing([14.0, 10.0])
            .striped(true)
            .show(ui, |ui| {
                changed |= shortcut_row(
                    ui,
                    "copy",
                    &tr("Copy Selection:"),
                    &mut settings.shortcuts.copy,
                    &defaults.copy,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "paste",
                    &tr("Paste Clipboard:"),
                    &mut settings.shortcuts.paste,
                    &defaults.paste,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "split_horizontal",
                    &tr("Split Horizontally:"),
                    &mut settings.shortcuts.split_horizontal,
                    &defaults.split_horizontal,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "split_vertical",
                    &tr("Split Vertically:"),
                    &mut settings.shortcuts.split_vertical,
                    &defaults.split_vertical,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "new_tab",
                    &tr("New Tab:"),
                    &mut settings.shortcuts.new_tab,
                    &defaults.new_tab,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "close_pane",
                    &tr("Close Pane:"),
                    &mut settings.shortcuts.close_pane,
                    &defaults.close_pane,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "focus_next_pane",
                    &tr("Focus Next Pane:"),
                    &mut settings.shortcuts.focus_next_pane,
                    &defaults.focus_next_pane,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "focus_prev_pane",
                    &tr("Focus Previous Pane:"),
                    &mut settings.shortcuts.focus_prev_pane,
                    &defaults.focus_prev_pane,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "open_preferences",
                    &tr("Open Preferences:"),
                    &mut settings.shortcuts.open_preferences,
                    &defaults.open_preferences,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "toggle_fullscreen",
                    &tr("Toggle Fullscreen:"),
                    &mut settings.shortcuts.toggle_fullscreen,
                    &defaults.toggle_fullscreen,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "zoom_in",
                    &tr("Zoom In:"),
                    &mut settings.shortcuts.zoom_in,
                    &defaults.zoom_in,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "zoom_out",
                    &tr("Zoom Out:"),
                    &mut settings.shortcuts.zoom_out,
                    &defaults.zoom_out,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    "zoom_reset",
                    &tr("Reset Zoom:"),
                    &mut settings.shortcuts.zoom_reset,
                    &defaults.zoom_reset,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
            });

        ui.add_space(10.0);
        if ui.button(tr("Reset Shortcuts to Defaults")).clicked() {
            settings.shortcuts = ShortcutSettings::default();
            self.recording_shortcut = None;
            changed = true;
        }

        changed
    }

    fn render_language_tab(&mut self, ui: &mut egui::Ui, settings: &mut ThemeSettings) -> bool {
        let mut changed = false;

        ui.label(tr(
            "Select UI language or load a custom GNU gettext (.po) translation file.",
        ));
        ui.add_space(10.0);

        let discovered_po = discover_po_files();
        let (active_locale, entry_count) = active_catalog_info();

        egui::Grid::new("prefs_lang_grid")
            .num_columns(2)
            .spacing([14.0, 12.0])
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
                    .width(280.0)
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
                                .desired_width(260.0)
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

fn apply_prefs_window_style(ui: &mut egui::Ui) {
    let base_font = ui
        .ctx()
        .global_style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .map(|f| f.size)
        .unwrap_or(13.0)
        .clamp(11.0, 24.0);
    let style = ui.style_mut();
    style.spacing.item_spacing = egui::vec2(10.0, 7.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.5);
    style.spacing.interact_size = egui::vec2(52.0, 24.0);
    style.spacing.slider_width = 200.0;
    style.spacing.combo_width = 300.0;
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(240, 242, 246));
    style.visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(235, 238, 244);
    style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(242, 244, 248);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(48, 52, 62);
    style.visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(95, 102, 118));
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(64, 70, 84);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(20, 22, 27);
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(base_font + 3.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(base_font, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(base_font, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(base_font, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new((base_font - 2.0).max(10.0), egui::FontFamily::Proportional),
    );
}

fn shortcut_row(
    ui: &mut egui::Ui,
    field_id: &'static str,
    label: &str,
    value: &mut String,
    default_val: &str,
    recording_shortcut: &mut Option<&'static str>,
    recorded_binding: &Option<(&'static str, String)>,
) -> bool {
    let mut changed = false;
    if let Some((rec_id, new_sc)) = recorded_binding {
        if *rec_id == field_id {
            *value = new_sc.clone();
            *recording_shortcut = None;
            changed = true;
        }
    }

    ui.label(label);

    let resp = ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(180.0)
            .font(egui::TextStyle::Monospace),
    );
    if resp.changed() {
        changed = true;
    }

    let is_recording = *recording_shortcut == Some(field_id);
    let btn_text = if is_recording {
        egui::RichText::new(tr("Press keys... (Esc to cancel)"))
            .color(egui::Color32::from_rgb(250, 204, 21))
            .strong()
    } else {
        egui::RichText::new(tr("Record"))
    };

    if ui.button(btn_text).clicked() {
        if is_recording {
            *recording_shortcut = None;
        } else {
            *recording_shortcut = Some(field_id);
        }
    }

    if value != default_val {
        if ui.small_button(tr("Default")).clicked() {
            *value = default_val.to_string();
            changed = true;
        }
    } else {
        ui.colored_label(
            egui::Color32::from_rgb(140, 145, 155),
            format!("{} ({})", tr("Default"), default_val),
        );
    }

    ui.end_row();
    changed
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
            .desired_width(130.0)
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


#[cfg(test)]
#[path = "test/prefs_ui_tests.rs"]
mod tests;