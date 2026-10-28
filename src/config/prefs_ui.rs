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
    color_to_hex, detect_super_held, discover_system_fonts, format_key_shortcut, is_cjk_font,
    is_modifier_only_key, is_monospace_font, is_valid_font_bytes, parse_color,
    shortcut_matches_event, split_shortcut_bindings, ShortcutSettings, ThemeSettings,
};
use crate::i18n::{active_catalog_info, discover_po_files, tr};

const LABEL_COL_WIDTH: f32 = 220.0;
const GRID_COL_SPACING: f32 = 14.0;
const FOOTER_RESERVED_HEIGHT: f32 = 58.0;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordMode {
    Replace,
    Append,
}

#[derive(Default)]
pub struct PreferencesWindow {
    pub open: bool,
    pub toggle_collapse_requested: bool,
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
    pub recording_shortcut: Option<(&'static str, RecordMode)>,
    pub super_key_latched: bool,
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
        let pointer_pos = ctx.input(|i| {
            i.pointer
                .interact_pos()
                .or_else(|| i.pointer.hover_pos())
                .or_else(|| i.pointer.latest_pos())
        });
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

    pub fn is_capturing_keyboard(&self, ctx: &egui::Context) -> bool {
        if !self.any_open() {
            return false;
        }
        if self.recording_shortcut.is_some() {
            return true;
        }
        ctx.egui_wants_keyboard_input()
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

        ctx.input(|i| {
            for ev in &i.events {
                if let egui::Event::Key {
                    key,
                    physical_key,
                    pressed,
                    ..
                } = ev
                {
                    if matches!(key, egui::Key::SuperLeft | egui::Key::SuperRight)
                        || matches!(
                            physical_key,
                            Some(egui::Key::SuperLeft | egui::Key::SuperRight)
                        )
                    {
                        self.super_key_latched = *pressed;
                    }
                }
            }
            if !i.focused {
                self.super_key_latched = false;
            }
        });

        if self.open && self.recording_shortcut.is_none() && !ctx.egui_wants_keyboard_input() {
            let current_mods = ctx.input(|i| i.modifiers);
            let super_held = self.super_key_latched || ctx.input(detect_super_held);
            let events = ctx.input(|i| i.events.clone());
            for ev in events {
                if let egui::Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    modifiers,
                    ..
                } = ev
                {
                    let merged_mods = egui::Modifiers {
                        alt: modifiers.alt || current_mods.alt,
                        ctrl: modifiers.ctrl || current_mods.ctrl,
                        shift: modifiers.shift || current_mods.shift,
                        mac_cmd: modifiers.mac_cmd || current_mods.mac_cmd,
                        command: modifiers.command || current_mods.command,
                    };
                    let ev_super = super_held
                        || merged_mods.mac_cmd
                        || (merged_mods.command && !merged_mods.ctrl);

                    if shortcut_matches_event(
                        &settings.shortcuts.close_preferences,
                        key,
                        physical_key,
                        merged_mods,
                        ev_super,
                    ) {
                        self.open = false;
                    } else if shortcut_matches_event(
                        &settings.shortcuts.toggle_collapse_preferences,
                        key,
                        physical_key,
                        merged_mods,
                        ev_super,
                    ) {
                        self.toggle_collapse_requested = true;
                    }
                }
            }
        }

        let prefs_win_id = egui::Id::unique("termin_prefs_window_v5");
        if self.toggle_collapse_requested {
            self.toggle_collapse_requested = false;
            if !self.open {
                self.open = true;
            } else if let Some(mut state) =
                egui::collapsing_header::CollapsingState::load(ctx, prefs_win_id)
            {
                state.set_open(!state.is_open());
                state.store(ctx);
            }
        }

        let mut is_open = self.open;
        let mut close_clicked = false;
        let mut changed = false;
        let mut font_changed = false;
        let mut lang_changed = false;

        if self.open {
            if let Some(inner) = egui::Window::new(
                egui::RichText::new(tr("Termin Preferences"))
                    .size(14.5)
                    .color(egui::Color32::WHITE)
                    .strong(),
            )
                .id(prefs_win_id)
                .open(&mut is_open)
                .resizable(true)
                .collapsible(true)
                .default_size([760.0, 520.0])
                .min_width(640.0)
                .min_height(380.0)
                .show(ctx, |ui| {
                    apply_prefs_window_style(ui);

                    // 現在のウィンドウ内サイズを取得し、内部要素がこれを超えて親を押し広げるのを防ぐ
                    let win_width = ui.available_width();
                    let win_height = ui.available_height();
                    ui.set_max_size(egui::vec2(win_width, win_height));

                    // 1. 固定ヘッダー（タブバー）
                    ui.horizontal(|ui| {
                        ui.set_max_width(win_width);
                        ui.spacing_mut().item_spacing.x = 6.0;
                        terminator_tab_button(
                            ui,
                            &mut self.active_tab,
                            PrefsTab::TitlebarAndFrame,
                            &tr("Titlebar & Frame"),
                        );
                        terminator_tab_button(
                            ui,
                            &mut self.active_tab,
                            PrefsTab::Font,
                            &tr("Font"),
                        );
                        terminator_tab_button(
                            ui,
                            &mut self.active_tab,
                            PrefsTab::Shortcuts,
                            &tr("Shortcuts"),
                        );
                        terminator_tab_button(
                            ui,
                            &mut self.active_tab,
                            PrefsTab::Language,
                            &tr("Language"),
                        );
                    });
                    ui.separator();

                    // 2. 中央スクロール領域
                    // フッター＋区切り線＋余白の高さ(58px)を差し引いて、下端を絶対に押し下げないようにする
                    let scroll_height = (ui.available_height() - FOOTER_RESERVED_HEIGHT).max(120.0);
                    // スクロールバー幅(24px)と固定1列目幅(LABEL_COL_WIDTH)を引いた2列目の正確な幅
                    let inner_width = (win_width - 24.0).max(400.0);
                    let col2_width =
                        (inner_width - LABEL_COL_WIDTH - GRID_COL_SPACING - 16.0).max(200.0);

                    egui::ScrollArea::vertical()
                        .id_salt("prefs_main_scroll_area")
                        .max_height(scroll_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_max_width(inner_width);

                            match self.active_tab {
                                PrefsTab::TitlebarAndFrame => {
                                    changed |= self.render_titlebar_tab(ui, settings, col2_width);
                                }
                                PrefsTab::Font => {
                                    if self.render_font_tab(ui, settings, col2_width, inner_width) {
                                        changed = true;
                                        font_changed = true;
                                    }
                                }
                                PrefsTab::Shortcuts => {
                                    changed |= self.render_shortcuts_tab(ui, settings, col2_width);
                                }
                                PrefsTab::Language => {
                                    if self.render_language_tab(ui, settings, col2_width) {
                                        changed = true;
                                        lang_changed = true;
                                    }
                                }
                            }
                        });

                    // 3. 固定フッター
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.set_max_width(win_width);
                        if ui.button(tr("Restore Defaults")).clicked() {
                            *settings = ThemeSettings::default();
                            self.recording_shortcut = None;
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

                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                let close_btn = egui::Button::new(
                                    egui::RichText::new(tr("Close")).strong(),
                                )
                                    .min_size(egui::vec2(90.0, 24.0));
                                if ui.add(close_btn).clicked() {
                                    close_clicked = true;
                                }
                            },
                        );
                    });
                }) {
                self.window_rects.push(inner.response.rect);
                if inner.response.dragged() || inner.response.is_pointer_button_down_on() {
                    self.subwindow_dragging = true;
                }
            }
            if close_clicked {
                is_open = false;
            }
            self.open = is_open;
        }

        if self.font_sample_window_open {
            if self.render_font_sample_window(ctx, settings) {
                changed = true;
                font_changed = true;
            }
        }

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

    fn render_titlebar_tab(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut ThemeSettings,
        col2_width: f32,
    ) -> bool {
        let mut changed = false;

        ui.label(
            egui::RichText::new(tr(
                "Enter color codes (e.g. #2b2b2b, rgb(40,40,40)) or select using the color picker. Empty values fallback to default colors.",
            ))
                .color(egui::Color32::from_rgb(185, 190, 200)),
        );
        ui.add_space(6.0);

        section_heading(ui, &tr("Titlebar Colors"));
        egui::Grid::new("prefs_titlebar_color_grid")
            .num_columns(2)
            .spacing([GRID_COL_SPACING, 8.0])
            .striped(true)
            .show(ui, |ui| {
                changed |= color_row(
                    ui,
                    col2_width,
                    &tr("Active Title BG:"),
                    &mut settings.title_active_bg,
                    egui::Color32::from_rgb(200, 0, 3),
                    "Default (#c80003)",
                );
                changed |= color_row(
                    ui,
                    col2_width,
                    &tr("Active Title FG:"),
                    &mut settings.title_active_fg,
                    egui::Color32::WHITE,
                    "Default (#ffffff)",
                );
                changed |= color_row(
                    ui,
                    col2_width,
                    &tr("Inactive Title BG:"),
                    &mut settings.title_inactive_bg,
                    egui::Color32::from_rgb(75, 75, 75),
                    "Default (#4b4b4b)",
                );
                changed |= color_row(
                    ui,
                    col2_width,
                    &tr("Inactive Title FG:"),
                    &mut settings.title_inactive_fg,
                    egui::Color32::from_rgb(210, 210, 210),
                    "Default (#d2d2d2)",
                );
            });

        ui.add_space(10.0);
        section_heading(ui, &tr("Window & Terminal Colors"));
        egui::Grid::new("prefs_window_color_grid")
            .num_columns(2)
            .spacing([GRID_COL_SPACING, 8.0])
            .striped(true)
            .show(ui, |ui| {
                changed |= color_row(
                    ui,
                    col2_width,
                    &tr("Window Frame BG:"),
                    &mut settings.frame_bg_color,
                    egui::Color32::from_rgb(43, 43, 43),
                    "Default (#2b2b2b)",
                );
                changed |= color_row(
                    ui,
                    col2_width,
                    &tr("Window Border Color:"),
                    &mut settings.frame_border_color,
                    egui::Color32::from_rgb(90, 90, 90),
                    "Default (#5a5a5a)",
                );
                changed |= color_row(
                    ui,
                    col2_width,
                    &tr("Pane Separator Color:"),
                    &mut settings.separator_color,
                    egui::Color32::from_rgb(74, 74, 74),
                    "Default (#4a4a4a)",
                );
                changed |= color_row(
                    ui,
                    col2_width,
                    &tr("Terminal Panel BG:"),
                    &mut settings.terminal_bg_color,
                    egui::Color32::BLACK,
                    "Default (#000000)",
                );
            });

        ui.add_space(10.0);
        section_heading(ui, &tr("Appearance & Dimensions"));
        egui::Grid::new("prefs_dimensions_grid")
            .num_columns(2)
            .spacing([GRID_COL_SPACING, 8.0])
            .striped(true)
            .show(ui, |ui| {
                grid_label(ui, &tr("Terminal BG Opacity:"));
                changed |= responsive_slider_f32(
                    ui,
                    col2_width,
                    &mut settings.terminal_bg_opacity,
                    0.0..=1.0,
                    0.05,
                    2,
                    "",
                );
                ui.end_row();

                grid_label(ui, &tr("Pane Separator Width (px):"));
                changed |= responsive_slider_f32(
                    ui,
                    col2_width,
                    &mut settings.separator_width,
                    1.0..=16.0,
                    1.0,
                    0,
                    " px",
                );
                ui.end_row();

                grid_label(ui, &tr("Window Border Width (px):"));
                changed |= responsive_slider_f32(
                    ui,
                    col2_width,
                    &mut settings.frame_border_width,
                    1.0..=20.0,
                    1.0,
                    0,
                    " px",
                );
                ui.end_row();

                grid_label(ui, &tr("Target FPS:"));
                changed |= responsive_slider_u32(
                    ui,
                    col2_width,
                    &mut settings.target_fps,
                    1..=240,
                    5,
                    " FPS",
                );
                ui.end_row();
            });

        changed
    }

    fn render_font_tab(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut ThemeSettings,
        col2_width: f32,
        inner_width: f32,
    ) -> bool {
        let mut changed = false;

        ui.label(
            egui::RichText::new(tr(
                "Select a font installed on your Debian system or enter a custom font file path (.ttf / .otf / .ttc).",
            ))
                .color(egui::Color32::from_rgb(185, 190, 200)),
        );
        ui.add_space(6.0);

        section_heading(ui, &tr("Font Selection"));
        egui::Grid::new("prefs_font_grid")
            .num_columns(2)
            .spacing([GRID_COL_SPACING, 8.0])
            .striped(true)
            .show(ui, |ui| {
                grid_label(ui, &tr("Filter System Fonts:"));
                ui.horizontal(|ui| {
                    ui.set_max_width(col2_width);
                    let filter_w = (col2_width - 100.0).max(80.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.font_filter)
                            .desired_width(filter_w)
                            .hint_text(tr("Type to filter (e.g. noto, mono, vl)...")),
                    );
                    if ui.button(tr("Rescan")).clicked() {
                        self.available_fonts = discover_system_fonts();
                    }
                });
                ui.end_row();

                grid_label(ui, "");
                if ui
                    .button(
                        egui::RichText::new(format!(
                            "🗚 {}",
                            tr("Open Font Sample Catalog...")
                        ))
                            .strong(),
                    )
                    .clicked()
                {
                    self.font_sample_window_open = true;
                }
                ui.end_row();

                grid_label(ui, &tr("System Font:"));
                let default_font_label = tr("Default (Auto-detect)");
                let current_name = std::path::Path::new(&settings.font_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&default_font_label);

                let filter_lower = self.font_filter.to_lowercase();
                let combo_w = (col2_width - 12.0).max(140.0);
                egui::ComboBox::from_id_salt("debian_system_font_combo")
                    .selected_text(current_name)
                    .width(combo_w)
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

                grid_label(ui, &tr("Font File Path:"));
                let path_w = (col2_width - 12.0).max(140.0);
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.font_path)
                            .desired_width(path_w)
                            .hint_text("/usr/share/fonts/..."),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();
            });

        ui.add_space(10.0);
        section_heading(ui, &tr("Font Sizes"));
        egui::Grid::new("prefs_font_size_grid")
            .num_columns(2)
            .spacing([GRID_COL_SPACING, 8.0])
            .striped(true)
            .show(ui, |ui| {
                grid_label(ui, &tr("UI & Menu Font Size (px):"));
                changed |= responsive_slider_f32(
                    ui,
                    col2_width,
                    &mut settings.ui_font_size,
                    9.0..=28.0,
                    0.5,
                    1,
                    " px",
                );
                ui.end_row();

                grid_label(ui, &tr("Terminal Font Size (px):"));
                changed |= responsive_slider_f32(
                    ui,
                    col2_width,
                    &mut settings.terminal_font_size,
                    8.0..=36.0,
                    0.5,
                    1,
                    " px",
                );
                ui.end_row();
            });

        ui.add_space(10.0);
        section_heading(ui, &tr("Preview"));
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(20, 22, 27))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 75, 88)))
            .inner_margin(egui::Margin::same(8))
            .corner_radius(4.0)
            .show(ui, |ui| {
                let preview_w = (inner_width - 28.0).max(200.0);
                ui.set_max_width(preview_w);
                let term_size = settings.effective_terminal_font_size();
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(
                            "ASCII: ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789 !@#$%^&*()",
                        )
                            .font(egui::FontId::monospace(term_size)),
                    )
                        .truncate(),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(
                            "Japanese: 日本語テキストの表示テスト（コピー・ペースト・ペイン分割）",
                        )
                            .font(egui::FontId::monospace(term_size)),
                    )
                        .truncate(),
                );
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
        let mut close_clicked = false;
        let mut newly_loaded_preview = false;
        let mut loads_this_frame = 0usize;
        let mut pending_more_loads = false;

        if let Some(inner) = egui::Window::new(
            egui::RichText::new(tr("System Font Samples & Selector"))
                .size(14.5)
                .color(egui::Color32::WHITE)
                .strong(),
        )
            .id(egui::Id::unique("system_font_sample_catalog_window_v5"))
            .open(&mut is_open)
            .resizable(true)
            .collapsible(true)
            .default_size([760.0, 520.0])
            .min_width(560.0)
            .min_height(380.0)
            .show(ctx, |ui| {
                apply_prefs_window_style(ui);

                let win_width = ui.available_width();
                let win_height = ui.available_height();
                ui.set_max_size(egui::vec2(win_width, win_height));

                ui.label(tr(
                    "Browse installed system fonts with live text samples and click 'Use This Font' to apply.",
                ));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.set_max_width(win_width);
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
                    ui.set_max_width(win_width);
                    ui.label(tr("Filter System Fonts:"));
                    let filter_w = (win_width - 250.0).max(100.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.sample_filter)
                            .desired_width(filter_w)
                            .hint_text(tr("Type to filter (e.g. noto, mono, vl, gothic)...")),
                    );
                    if ui.button(tr("Rescan")).clicked() {
                        self.available_fonts = discover_system_fonts();
                    }
                });

                ui.horizontal(|ui| {
                    ui.set_max_width(win_width);
                    ui.label(tr("Sample Text:"));
                    let avail_for_samples = (win_width - 130.0).max(160.0);
                    let half_w = (avail_for_samples * 0.5 - 8.0).max(80.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.sample_text_ascii)
                            .desired_width(half_w),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.sample_text_cjk)
                            .desired_width(half_w),
                    );
                });

                ui.separator();

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

                let list_height = (ui.available_height() - FOOTER_RESERVED_HEIGHT).max(120.0);
                let card_width = (win_width - 24.0).max(300.0);
                let row_height = 88.0;

                egui::ScrollArea::vertical()
                    .id_salt("font_sample_scroll_area")
                    .max_height(list_height)
                    .auto_shrink([false, false])
                    .show_rows(ui, row_height, filtered_fonts.len(), |ui, row_range| {
                        ui.set_max_width(card_width);

                        let visible_paths: Vec<String> = row_range
                            .clone()
                            .map(|i| filtered_fonts[i].1.clone())
                            .collect();

                        for idx in row_range {
                            let (name, path) = &filtered_fonts[idx];
                            let is_selected = &settings.font_path == path;

                            let fam_key =
                                if let Some((key, _)) = self.loaded_preview_fonts.get(path) {
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
                                        let key =
                                            format!("sample_font_{}", self.next_preview_font_id);
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
                                    let inner_card_w = (card_width - 20.0).max(200.0);
                                    ui.set_max_width(inner_card_w);

                                    ui.horizontal(|ui| {
                                        ui.set_max_width(inner_card_w);
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
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if is_selected {
                                                    ui.colored_label(
                                                        egui::Color32::from_rgb(74, 222, 128),
                                                        format!("✔ {}", tr("Selected")),
                                                    );
                                                } else if ui
                                                    .button(tr("Use This Font"))
                                                    .clicked()
                                                {
                                                    settings.font_path = path.clone();
                                                    changed = true;
                                                }
                                            },
                                        );
                                    });

                                    let sample_font_id = match &fam_key {
                                        Some(k) => {
                                            let family =
                                                egui::FontFamily::Name(k.clone().into());
                                            let is_bound = ui.ctx().fonts(|f| {
                                                let defs = f.definitions();
                                                defs.families.get(&family).is_some_and(|list| {
                                                    !list.is_empty()
                                                        && list.iter().all(|n| {
                                                        defs.font_data.contains_key(n)
                                                    })
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

                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&self.sample_text_ascii)
                                                .font(sample_font_id.clone())
                                                .color(egui::Color32::from_rgb(230, 230, 235)),
                                        )
                                            .truncate(),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&self.sample_text_cjk)
                                                .font(sample_font_id)
                                                .color(egui::Color32::from_rgb(210, 220, 230)),
                                        )
                                            .truncate(),
                                    );
                                });
                            ui.add_space(4.0);
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.set_max_width(win_width);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let close_btn =
                            egui::Button::new(egui::RichText::new(tr("Close")).strong())
                                .min_size(egui::vec2(90.0, 24.0));
                        if ui.add(close_btn).clicked() {
                            close_clicked = true;
                        }
                    });
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

        if close_clicked {
            is_open = false;
        }
        self.font_sample_window_open = is_open;
        changed
    }

    fn render_shortcuts_tab(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut ThemeSettings,
        col2_width: f32,
    ) -> bool {
        let mut changed = false;

        ui.label(
            egui::RichText::new(tr(
                "Configure keyboard shortcuts. Click '+ Add' to add another shortcut field, or 'Record' to replace a specific binding.",
            ))
                .color(egui::Color32::from_rgb(185, 190, 200)),
        );
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            section_heading(ui, &tr("Keybindings"));
            ui.add_space(12.0);
            if ui.small_button(tr("Reset Shortcuts to Defaults")).clicked() {
                settings.shortcuts = ShortcutSettings::default();
                self.recording_shortcut = None;
                changed = true;
            }
        });
        ui.add_space(4.0);

        ui.input(|i| {
            for ev in &i.events {
                if let egui::Event::Key {
                    key,
                    physical_key,
                    pressed,
                    ..
                } = ev
                {
                    if matches!(key, egui::Key::SuperLeft | egui::Key::SuperRight)
                        || matches!(
                            physical_key,
                            Some(egui::Key::SuperLeft | egui::Key::SuperRight)
                        )
                    {
                        self.super_key_latched = *pressed;
                    }
                }
            }
            if !i.focused {
                self.super_key_latched = false;
            }
        });

        let mut recorded_binding: Option<(&'static str, RecordMode, String)> = None;
        let mut cancel_recording = false;
        if let Some((target_field, mode)) = self.recording_shortcut {
            if let Some(focused_id) = ui.ctx().memory(|m| m.focused()) {
                ui.ctx().memory_mut(|m| m.surrender_focus(focused_id));
            }

            let current_mods = ui.input(|i| i.modifiers);
            let super_held = self.super_key_latched || ui.input(detect_super_held);
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
                        let merged_mods = egui::Modifiers {
                            alt: modifiers.alt || current_mods.alt,
                            ctrl: modifiers.ctrl || current_mods.ctrl,
                            shift: modifiers.shift || current_mods.shift,
                            mac_cmd: modifiers.mac_cmd || current_mods.mac_cmd,
                            command: modifiers.command || current_mods.command,
                        };
                        let ev_super = super_held
                            || merged_mods.mac_cmd
                            || (merged_mods.command && !merged_mods.ctrl);

                        if key == egui::Key::Escape && merged_mods.is_none() && !ev_super {
                            cancel_recording = true;
                            break;
                        }
                        if is_modifier_only_key(key)
                            || physical_key.is_some_and(is_modifier_only_key)
                        {
                            continue;
                        }
                        if let Some(formatted) =
                            format_key_shortcut(key, physical_key, merged_mods, ev_super)
                        {
                            recorded_binding = Some((target_field, mode, formatted));
                            break;
                        }
                    }
                    egui::Event::Copy => {
                        let mut mods = current_mods;
                        if !super_held {
                            mods.ctrl = true;
                        }
                        if let Some(formatted) =
                            format_key_shortcut(egui::Key::C, Some(egui::Key::C), mods, super_held)
                        {
                            recorded_binding = Some((target_field, mode, formatted));
                            break;
                        }
                    }
                    egui::Event::Cut => {
                        let mut mods = current_mods;
                        if !super_held {
                            mods.ctrl = true;
                        }
                        if let Some(formatted) =
                            format_key_shortcut(egui::Key::X, Some(egui::Key::X), mods, super_held)
                        {
                            recorded_binding = Some((target_field, mode, formatted));
                            break;
                        }
                    }
                    egui::Event::Paste(_) => {
                        let mut mods = current_mods;
                        if !super_held {
                            mods.ctrl = true;
                        }
                        if let Some(formatted) =
                            format_key_shortcut(egui::Key::V, Some(egui::Key::V), mods, super_held)
                        {
                            recorded_binding = Some((target_field, mode, formatted));
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
            .num_columns(2)
            .spacing([GRID_COL_SPACING, 8.0])
            .striped(true)
            .show(ui, |ui| {
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "copy",
                    &tr("Copy Selection:"),
                    &mut settings.shortcuts.copy,
                    &defaults.copy,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "paste",
                    &tr("Paste Clipboard:"),
                    &mut settings.shortcuts.paste,
                    &defaults.paste,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "split_horizontal",
                    &tr("Split Horizontally:"),
                    &mut settings.shortcuts.split_horizontal,
                    &defaults.split_horizontal,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "split_vertical",
                    &tr("Split Vertically:"),
                    &mut settings.shortcuts.split_vertical,
                    &defaults.split_vertical,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "new_tab",
                    &tr("New Tab:"),
                    &mut settings.shortcuts.new_tab,
                    &defaults.new_tab,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "close_pane",
                    &tr("Close Pane:"),
                    &mut settings.shortcuts.close_pane,
                    &defaults.close_pane,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "focus_next_pane",
                    &tr("Focus Next Pane:"),
                    &mut settings.shortcuts.focus_next_pane,
                    &defaults.focus_next_pane,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "focus_prev_pane",
                    &tr("Focus Previous Pane:"),
                    &mut settings.shortcuts.focus_prev_pane,
                    &defaults.focus_prev_pane,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "open_preferences",
                    &tr("Open Preferences:"),
                    &mut settings.shortcuts.open_preferences,
                    &defaults.open_preferences,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "close_preferences",
                    &tr("Close Preferences:"),
                    &mut settings.shortcuts.close_preferences,
                    &defaults.close_preferences,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "toggle_collapse_preferences",
                    &tr("Collapse/Expand Preferences:"),
                    &mut settings.shortcuts.toggle_collapse_preferences,
                    &defaults.toggle_collapse_preferences,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "toggle_fullscreen",
                    &tr("Toggle Fullscreen:"),
                    &mut settings.shortcuts.toggle_fullscreen,
                    &defaults.toggle_fullscreen,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "zoom_in",
                    &tr("Zoom In:"),
                    &mut settings.shortcuts.zoom_in,
                    &defaults.zoom_in,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "zoom_out",
                    &tr("Zoom Out:"),
                    &mut settings.shortcuts.zoom_out,
                    &defaults.zoom_out,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
                changed |= shortcut_row(
                    ui,
                    col2_width,
                    "zoom_reset",
                    &tr("Reset Zoom:"),
                    &mut settings.shortcuts.zoom_reset,
                    &defaults.zoom_reset,
                    &mut self.recording_shortcut,
                    &recorded_binding,
                );
            });

        changed
    }

    fn render_language_tab(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut ThemeSettings,
        col2_width: f32,
    ) -> bool {
        let mut changed = false;

        ui.label(
            egui::RichText::new(tr(
                "Select UI language or load a custom GNU gettext (.po) translation file.",
            ))
                .color(egui::Color32::from_rgb(185, 190, 200)),
        );
        ui.add_space(6.0);

        let discovered_po = discover_po_files();
        let (active_locale, entry_count) = active_catalog_info();

        section_heading(ui, &tr("Localization"));
        egui::Grid::new("prefs_lang_grid")
            .num_columns(2)
            .spacing([GRID_COL_SPACING, 10.0])
            .striped(true)
            .show(ui, |ui| {
                grid_label(ui, &tr("Language (.po):"));
                let selected_label = match settings.language.as_str() {
                    "auto" | "" => tr("Auto (System Locale)"),
                    "en" => "English (en.po)".to_string(),
                    "ja" => "日本語 (ja.po)".to_string(),
                    other => format!("{}.po", other),
                };

                let combo_w = (col2_width - 12.0).max(140.0);
                egui::ComboBox::from_id_salt("language_po_combo")
                    .selected_text(selected_label)
                    .width(combo_w)
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

                grid_label(ui, &tr("Custom .po File Path:"));
                ui.horizontal(|ui| {
                    ui.set_max_width(col2_width);
                    let text_w = (col2_width - 150.0).max(80.0);
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut settings.custom_po_path)
                                .desired_width(text_w)
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

                grid_label(ui, &tr("Active Catalog:"));
                ui.label(format!("{} ({} strings)", active_locale, entry_count));
                ui.end_row();
            });

        changed
    }
}

/// 1列目のラベル幅を全タブ共通で LABEL_COL_WIDTH に完全固定する
fn grid_label(ui: &mut egui::Ui, text: &str) {
    ui.allocate_ui_with_layout(
        egui::vec2(LABEL_COL_WIDTH, 22.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_max_width(LABEL_COL_WIDTH);
            ui.add(egui::Label::new(text).truncate());
        },
    );
}

fn terminator_tab_button(
    ui: &mut egui::Ui,
    active_tab: &mut PrefsTab,
    tab_val: PrefsTab,
    label: &str,
) {
    let selected = *active_tab == tab_val;
    let text_color = if selected {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(175, 180, 190)
    };
    let btn = egui::Button::new(egui::RichText::new(label).color(text_color).strong())
        .fill(if selected {
            egui::Color32::from_rgb(48, 54, 66)
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::NONE)
        .min_size(egui::vec2(110.0, 26.0));

    let resp = ui.add(btn);
    if selected {
        let underline_rect = egui::Rect::from_min_max(
            egui::pos2(resp.rect.min.x + 4.0, resp.rect.max.y - 2.5),
            egui::pos2(resp.rect.max.x - 4.0, resp.rect.max.y),
        );
        ui.painter().rect_filled(
            underline_rect,
            1.0,
            egui::Color32::from_rgb(59, 130, 246),
        );
    }
    if resp.clicked() {
        *active_tab = tab_val;
    }
}

fn section_heading(ui: &mut egui::Ui, title: &str) {
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(title)
            .strong()
            .color(egui::Color32::WHITE),
    );
    ui.add_space(2.0);
}

fn responsive_slider_f32(
    ui: &mut egui::Ui,
    col2_width: f32,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    decimals: usize,
    suffix: &str,
) -> bool {
    let mut changed = false;
    let min_val = *range.start();
    let max_val = *range.end();

    ui.horizontal(|ui| {
        ui.set_max_width(col2_width);
        let reserved_for_controls = 185.0;
        ui.spacing_mut().slider_width = (col2_width - reserved_for_controls).max(60.0);

        let slider = egui::Slider::new(value, range)
            .step_by(step as f64)
            .fixed_decimals(decimals)
            .suffix(suffix);
        if ui.add(slider).changed() {
            changed = true;
        }

        if ui
            .add_enabled(
                *value > min_val,
                egui::Button::new(" − ").min_size(egui::vec2(26.0, 22.0)),
            )
            .on_hover_text(tr("Decrease value"))
            .clicked()
        {
            let next = (*value - step).clamp(min_val, max_val);
            *value = if step > 0.0 {
                ((next / step).round() * step).clamp(min_val, max_val)
            } else {
                next
            };
            changed = true;
        }

        if ui
            .add_enabled(
                *value < max_val,
                egui::Button::new(" + ").min_size(egui::vec2(26.0, 22.0)),
            )
            .on_hover_text(tr("Increase value"))
            .clicked()
        {
            let next = (*value + step).clamp(min_val, max_val);
            *value = if step > 0.0 {
                ((next / step).round() * step).clamp(min_val, max_val)
            } else {
                next
            };
            changed = true;
        }
    });

    changed
}

fn responsive_slider_u32(
    ui: &mut egui::Ui,
    col2_width: f32,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
    button_step: u32,
    suffix: &str,
) -> bool {
    let mut changed = false;
    let min_val = *range.start();
    let max_val = *range.end();

    ui.horizontal(|ui| {
        ui.set_max_width(col2_width);
        let reserved_for_controls = 185.0;
        ui.spacing_mut().slider_width = (col2_width - reserved_for_controls).max(60.0);

        let slider = egui::Slider::new(value, range).suffix(suffix);
        if ui.add(slider).changed() {
            changed = true;
        }

        if ui
            .add_enabled(
                *value > min_val,
                egui::Button::new(" − ").min_size(egui::vec2(26.0, 22.0)),
            )
            .on_hover_text(tr("Decrease value"))
            .clicked()
        {
            *value = value.saturating_sub(button_step).clamp(min_val, max_val);
            changed = true;
        }

        if ui
            .add_enabled(
                *value < max_val,
                egui::Button::new(" + ").min_size(egui::vec2(26.0, 22.0)),
            )
            .on_hover_text(tr("Increase value"))
            .clicked()
        {
            *value = value.saturating_add(button_step).clamp(min_val, max_val);
            changed = true;
        }
    });

    changed
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
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.interact_size = egui::vec2(48.0, 22.0);
    style.spacing.slider_width = 180.0;
    style.spacing.combo_width = 260.0;
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

#[allow(clippy::too_many_arguments)]
fn shortcut_row(
    ui: &mut egui::Ui,
    col2_width: f32,
    field_id: &'static str,
    label: &str,
    value: &mut String,
    default_val: &str,
    recording_shortcut: &mut Option<(&'static str, RecordMode)>,
    recorded_binding: &Option<(&'static str, RecordMode, String)>,
) -> bool {
    let mut changed = false;
    let rec_idx_key = egui::Id::unique(format!("shortcut_rec_target_idx_{}", field_id));
    let mut target_idx: usize = ui
        .ctx()
        .data_mut(|d| d.get_temp::<usize>(rec_idx_key))
        .unwrap_or(0);

    let mut bindings = split_shortcut_bindings(value);
    if bindings.is_empty() {
        bindings.push(String::new());
    }

    if let Some((rec_id, mode, new_sc)) = recorded_binding {
        if *rec_id == field_id {
            match mode {
                RecordMode::Replace => {
                    if target_idx < bindings.len() {
                        bindings[target_idx] = new_sc.clone();
                    } else {
                        bindings.push(new_sc.clone());
                    }
                    let cleaned: Vec<String> = bindings
                        .into_iter()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    *value = cleaned.join(" / ");
                }
                RecordMode::Append => {
                    let mut existing = split_shortcut_bindings(value);
                    if !existing.iter().any(|s| s.eq_ignore_ascii_case(new_sc)) {
                        existing.push(new_sc.clone());
                    }
                    *value = existing.join(" / ");
                }
            }
            *recording_shortcut = None;
            changed = true;
            bindings = split_shortcut_bindings(value);
            if bindings.is_empty() {
                bindings.push(String::new());
            }
        }
    }

    let is_recording_replace = *recording_shortcut == Some((field_id, RecordMode::Replace));
    let is_recording_append = *recording_shortcut == Some((field_id, RecordMode::Append));
    let any_recording_this = is_recording_replace || is_recording_append;

    grid_label(ui, label);

    // 複数行になっても全てのテキストフィールドの幅がピタッと揃うよう共通の text_w を使う
    let text_w = (col2_width - 255.0).max(90.0);

    ui.vertical(|ui| {
        ui.set_max_width(col2_width);
        let mut modified_in_ui = false;
        let mut remove_idx: Option<usize> = None;
        let num_bindings = bindings.len();

        for (idx, binding_str) in bindings.iter_mut().enumerate() {
            let is_rec_this_slot = is_recording_replace && target_idx == idx;

            ui.horizontal(|ui| {
                ui.set_max_width(col2_width);

                let mut display_str = if is_rec_this_slot {
                    String::new()
                } else {
                    binding_str.clone()
                };

                let resp = ui.add(
                    egui::TextEdit::singleline(&mut display_str)
                        .id(egui::Id::unique(format!("sc_field_edit_{}_{}", field_id, idx)))
                        .desired_width(text_w)
                        .font(egui::TextStyle::Monospace)
                        .hint_text(if is_rec_this_slot {
                            tr("Press keys... (Esc)")
                        } else {
                            tr("None")
                        })
                        .interactive(!any_recording_this),
                );
                if resp.changed() && !any_recording_this {
                    *binding_str = display_str;
                    modified_in_ui = true;
                }

                // 録画中もボタンの横幅が変わらないよう固定サイズにする
                let rec_btn_text = if is_rec_this_slot {
                    egui::RichText::new("Esc")
                        .color(egui::Color32::from_rgb(250, 204, 21))
                        .strong()
                } else {
                    egui::RichText::new(tr("Record"))
                };

                if ui
                    .add(egui::Button::new(rec_btn_text).min_size(egui::vec2(62.0, 22.0)))
                    .clicked()
                {
                    if is_rec_this_slot {
                        *recording_shortcut = None;
                    } else {
                        target_idx = idx;
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(rec_idx_key, target_idx));
                        *recording_shortcut = Some((field_id, RecordMode::Replace));
                    }
                }

                if idx == 0 {
                    let add_btn_text = if is_recording_append {
                        egui::RichText::new("Esc")
                            .color(egui::Color32::from_rgb(74, 222, 128))
                            .strong()
                    } else {
                        egui::RichText::new(tr("+ Add"))
                    };

                    if ui
                        .add(egui::Button::new(add_btn_text).min_size(egui::vec2(58.0, 22.0)))
                        .clicked()
                    {
                        if is_recording_append {
                            *recording_shortcut = None;
                        } else {
                            *recording_shortcut = Some((field_id, RecordMode::Append));
                        }
                    }

                    let is_modified = value != default_val;
                    if ui
                        .add_enabled(
                            is_modified,
                            egui::Button::new(tr("Default")).min_size(egui::vec2(64.0, 22.0)),
                        )
                        .on_hover_text(format!("{}: {}", tr("Default"), default_val))
                        .clicked()
                    {
                        *value = default_val.to_string();
                        *recording_shortcut = None;
                        changed = true;
                    }
                }

                if num_bindings > 1 {
                    if ui
                        .add(egui::Button::new("✕").min_size(egui::vec2(24.0, 22.0)))
                        .on_hover_text(tr("Remove this shortcut"))
                        .clicked()
                    {
                        remove_idx = Some(idx);
                    }
                }
            });
        }

        if is_recording_append {
            ui.horizontal(|ui| {
                ui.set_max_width(col2_width);
                let mut placeholder = String::new();
                ui.add(
                    egui::TextEdit::singleline(&mut placeholder)
                        .desired_width(text_w)
                        .font(egui::TextStyle::Monospace)
                        .hint_text(tr("Press keys... (Esc)"))
                        .interactive(false),
                );
                if ui
                    .add(egui::Button::new("✕").min_size(egui::vec2(24.0, 22.0)))
                    .clicked()
                {
                    *recording_shortcut = None;
                }
            });
        }

        if let Some(rem) = remove_idx {
            if rem < bindings.len() {
                bindings.remove(rem);
                modified_in_ui = true;
            }
        }

        if modified_in_ui {
            let cleaned: Vec<String> = bindings
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            *value = cleaned.join(" / ");
            changed = true;
        }
    });

    ui.end_row();
    changed
}

fn color_row(
    ui: &mut egui::Ui,
    col2_width: f32,
    label: &str,
    value: &mut String,
    fallback_preview: egui::Color32,
    fallback_text: &str,
) -> bool {
    let mut changed = false;
    grid_label(ui, label);

    ui.horizontal(|ui| {
        ui.set_max_width(col2_width);
        let text_w = (col2_width - 210.0).max(80.0);
        let resp = ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(text_w)
                .font(egui::TextStyle::Monospace)
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
            ui.add(
                egui::Label::new(
                    egui::RichText::new(tr("Applied")).color(egui::Color32::from_rgb(74, 222, 128)),
                )
                    .truncate(),
            );
        } else if value.trim().is_empty() {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(fallback_text)
                        .color(egui::Color32::from_rgb(156, 163, 175)),
                )
                    .truncate(),
            );
        } else {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("{} ({})", tr("Invalid"), fallback_text))
                        .color(egui::Color32::from_rgb(251, 191, 36)),
                )
                    .truncate(),
            );
        }
    });

    ui.end_row();
    changed
}

#[cfg(test)]
#[path = "test/prefs_ui_tests.rs"]
mod tests;