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
use egui_tiles::TileId;
use crate::config::theme::ThemeSettings;
use crate::i18n::tr;
use crate::menus::{
    MenuManager,
    types::{MenuAction, MenuContext},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleBarAction {
    ClosePane,
    MinimizeWindow,
    ToggleMaximizeWindow,
    StartWindowMove,
    SplitHorizontal,
    SplitVertical,
    NewTab,
    FocusNextPane,
    FocusPrevPane,
    SaveLayout,
    LoadLayout,
    OpenPreferences,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

pub struct TitleBarOutput {
    pub action: Option<TitleBarAction>,
    pub start_tile_drag: bool,
    pub clicked: bool,
    pub context_menu_open: bool,
    pub is_dragging: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn render_titlebar(
    ui: &mut egui::Ui,
    tile_id: TileId,
    menus: &MenuManager,
    title: &mut String,
    editing_title: &mut bool,
    is_active: bool,
    is_top_right: bool,
    other_screen_open: bool,
    settings: &ThemeSettings,
) -> TitleBarOutput {
    let mut output = TitleBarOutput {
        action: None,
        start_tile_drag: false,
        clicked: false,
        context_menu_open: false,
        is_dragging: false,
    };

    if other_screen_open && *editing_title {
        *editing_title = false;
    }

    let popup_open = egui::Popup::is_any_open(ui.ctx());
    let any_drag_in_progress = ui.ctx().dragged_id().is_some();
    let is_effectively_active =
        is_active && !other_screen_open && !popup_open && !any_drag_in_progress;

    let bar_height = 24.0;
    let desired_size = egui::vec2(ui.available_width(), bar_height);
    let bar_sense = if *editing_title || other_screen_open || popup_open {
        egui::Sense::hover()
    } else if is_effectively_active {
        egui::Sense::click_and_drag()
    } else {
        egui::Sense::CLICK
    };
    let (bar_rect, bar_resp) = ui.allocate_exact_size(desired_size, bar_sense);

    // Reserve background shape slot so we can color it after detecting any drag/menu state this frame
    let bg_shape_idx = ui.painter().add(egui::Shape::Noop);

    let fg_color = settings.titlebar_fg(is_effectively_active);

    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bar_rect.shrink2(egui::vec2(4.0, 2.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );

    child_ui.spacing_mut().item_spacing = egui::vec2(3.0, 0.0);

    let btn_sense = if other_screen_open || popup_open {
        egui::Sense::hover()
    } else {
        egui::Sense::CLICK
    };

    // 1. Left controls: Drag grip [ ✥ ] for pane swap, plus split/tab buttons
    let grip_sense = if other_screen_open || popup_open {
        egui::Sense::hover()
    } else if is_effectively_active && !*editing_title {
        egui::Sense::DRAG
    } else {
        egui::Sense::CLICK
    };
    let grip_btn = egui::Button::new(egui::RichText::new(" ✥ ").color(fg_color).size(12.0))
        .frame(false)
        .sense(grip_sense);
    let grip_resp = child_ui
        .add(grip_btn)
        .on_hover_text(tr("Drag and drop onto another pane to swap/split"));

    if is_effectively_active && !*editing_title {
        if grip_resp.drag_started() {
            output.start_tile_drag = true;
            output.is_dragging = true;
            output.clicked = true;
        } else if grip_resp.dragged() {
            output.is_dragging = true;
        }
    } else if !other_screen_open && !popup_open && grip_resp.clicked() {
        output.clicked = true;
    }

    let split_h_resp = small_bar_button(
        &mut child_ui,
        &tr("[Split-H]"),
        fg_color,
        btn_sense,
        &tr("Split Horizontally (Left/Right)"),
    );
    if !other_screen_open && !popup_open && split_h_resp.clicked() {
        if is_effectively_active {
            output.action = Some(TitleBarAction::SplitHorizontal);
        }
        output.clicked = true;
    }

    let split_v_resp = small_bar_button(
        &mut child_ui,
        &tr("[Split-V]"),
        fg_color,
        btn_sense,
        &tr("Split Vertically (Top/Bottom)"),
    );
    if !other_screen_open && !popup_open && split_v_resp.clicked() {
        if is_effectively_active {
            output.action = Some(TitleBarAction::SplitVertical);
        }
        output.clicked = true;
    }

    let new_tab_resp = small_bar_button(
        &mut child_ui,
        &tr("[+Tab]"),
        fg_color,
        btn_sense,
        &tr("Add a new tab in this pane"),
    );
    if !other_screen_open && !popup_open && new_tab_resp.clicked() {
        if is_effectively_active {
            output.action = Some(TitleBarAction::NewTab);
        }
        output.clicked = true;
    }

    let left_controls_right_x = child_ui.cursor().min.x;

    // 2. Right-side controls laid out before the middle title/TextEdit so button IDs & positions remain stable
    let mut just_started_editing = false;
    child_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if small_bar_button(ui, " × ", fg_color, btn_sense, &tr("Close this pane")).clicked()
            && !other_screen_open
            && !popup_open
        {
            if is_effectively_active {
                output.action = Some(TitleBarAction::ClosePane);
            }
            output.clicked = true;
        }

        if is_top_right {
            if small_bar_button(
                ui,
                " 🗖 ",
                fg_color,
                btn_sense,
                &tr("Maximize / Restore window"),
            )
            .clicked()
                && !other_screen_open
                && !popup_open
            {
                if is_effectively_active {
                    output.action = Some(TitleBarAction::ToggleMaximizeWindow);
                }
                output.clicked = true;
            }
            if small_bar_button(ui, " — ", fg_color, btn_sense, &tr("Minimize window")).clicked()
                && !other_screen_open
                && !popup_open
            {
                if is_effectively_active {
                    output.action = Some(TitleBarAction::MinimizeWindow);
                }
                output.clicked = true;
            }
        }

        let edit_btn_resp =
            small_bar_button(ui, " ✎ ", fg_color, btn_sense, &tr("Rename pane title"));
        if !other_screen_open && !popup_open && edit_btn_resp.clicked() {
            if is_effectively_active {
                *editing_title = !*editing_title;
                if *editing_title {
                    just_started_editing = true;
                }
            }
            output.clicked = true;
        }
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("edit_btn_rect"), edit_btn_resp.rect));

        if small_bar_button(
            ui,
            " ⚙ ",
            fg_color,
            btn_sense,
            &tr("Open CustomDragBar Preferences"),
        )
        .clicked()
            && !other_screen_open
            && !popup_open
        {
            if is_effectively_active {
                output.action = Some(TitleBarAction::OpenPreferences);
            }
            output.clicked = true;
        }

        let right_controls_left_x = ui.cursor().max.x;

        // 3. Title text or inline rename entry centered in the titlebar (clamped between left & right controls)
        let avail_left = (left_controls_right_x + 4.0).min(bar_rect.max.x);
        let avail_right = (right_controls_left_x - 4.0).max(avail_left);
        let middle_rect = egui::Rect::from_min_max(
            egui::pos2(avail_left, bar_rect.min.y + 1.0),
            egui::pos2(avail_right, bar_rect.max.y - 1.0),
        );

        let edit_id = egui::Id::new(("pane_title_edit", tile_id));
        if *editing_title {
            let edit_w = middle_rect.width().min(180.0).max(40.0);
            let ideal_min_x = (bar_rect.center().x - edit_w * 0.5).clamp(
                middle_rect.min.x,
                (middle_rect.max.x - edit_w).max(middle_rect.min.x),
            );
            let edit_rect = egui::Rect::from_min_size(
                egui::pos2(ideal_min_x, middle_rect.min.y),
                egui::vec2(edit_w, middle_rect.height()),
            );
            let mut edit_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(edit_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let edit_resp = edit_ui.add(
                egui::TextEdit::singleline(title)
                    .id(edit_id)
                    .desired_width(edit_w)
                    .horizontal_align(egui::Align::Center)
                    .font(egui::TextStyle::Monospace),
            );
            #[cfg(test)]
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::new("edit_text_rect"), edit_resp.rect));

            if just_started_editing || (!edit_resp.has_focus() && !edit_resp.lost_focus()) {
                edit_resp.request_focus();
            } else if edit_resp.lost_focus()
                || ui.input(|i| {
                    i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)
                })
            {
                *editing_title = false;
            }
        } else {
            let font_id = egui::FontId::proportional(12.0);
            let galley = ui
                .painter()
                .layout_no_wrap(title.clone(), font_id, fg_color);
            let label_w = (galley.size().x + 6.0).min(middle_rect.width()).max(16.0);
            let ideal_min_x = (bar_rect.center().x - label_w * 0.5).clamp(
                middle_rect.min.x,
                (middle_rect.max.x - label_w).max(middle_rect.min.x),
            );
            let label_rect = egui::Rect::from_min_size(
                egui::pos2(ideal_min_x, middle_rect.min.y),
                egui::vec2(label_w, middle_rect.height()),
            );
            let mut label_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(label_rect)
                    .layout(egui::Layout::centered_and_justified(
                        egui::Direction::LeftToRight,
                    )),
            );
            let lbl_resp = label_ui.add(
                egui::Label::new(
                    egui::RichText::new(title.as_str())
                        .color(fg_color)
                        .size(12.0),
                )
                .truncate()
                .sense(btn_sense)
                .selectable(false),
            );
            #[cfg(test)]
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::new("title_lbl_rect"), lbl_resp.rect));
            if !other_screen_open && !popup_open {
                if is_effectively_active && lbl_resp.double_clicked() {
                    *editing_title = true;
                    output.clicked = true;
                } else if lbl_resp.clicked() {
                    output.clicked = true;
                }
            }
        }
    });

    // 4. Context menu for the titlebar area (disabled when another window/screen is open)
    if !other_screen_open && (is_active || popup_open) {
        if let Some((_ctx, menu_action)) =
            menus.show(&bar_resp, MenuContext::Titlebar(tile_id), &settings.shortcuts)
        {
            output.clicked = true;
            match menu_action {
                MenuAction::NewTab => output.action = Some(TitleBarAction::NewTab),
                MenuAction::SplitHorizontal => {
                    output.action = Some(TitleBarAction::SplitHorizontal)
                }
                MenuAction::SplitVertical => output.action = Some(TitleBarAction::SplitVertical),
                MenuAction::SaveLayout => output.action = Some(TitleBarAction::SaveLayout),
                MenuAction::LoadLayout => output.action = Some(TitleBarAction::LoadLayout),
                MenuAction::ClosePane => output.action = Some(TitleBarAction::ClosePane),
                MenuAction::OpenSettings => output.action = Some(TitleBarAction::OpenPreferences),
                MenuAction::ToggleFullscreen => {
                    output.action = Some(TitleBarAction::ToggleMaximizeWindow)
                }
                _ => {}
            }
        }
    }

    if !other_screen_open && (bar_resp.context_menu_opened() || egui::Popup::is_any_open(ui.ctx())) {
        output.context_menu_open = true;
    }

    // 5. Titlebar background interaction (Double-click = Maximize, Drag = Move Window — ONLY when active!)
    if !other_screen_open && !popup_open && !*editing_title && !output.start_tile_drag && output.action.is_none() {
        if is_effectively_active {
            if bar_resp.double_clicked() {
                output.action = Some(TitleBarAction::ToggleMaximizeWindow);
                output.clicked = true;
            } else if bar_resp.drag_started_by(egui::PointerButton::Primary) {
                output.action = Some(TitleBarAction::StartWindowMove);
                output.is_dragging = true;
                output.clicked = true;
            } else if bar_resp.dragged_by(egui::PointerButton::Primary) {
                output.is_dragging = true;
            } else if bar_resp.clicked() {
                output.clicked = true;
            }
        } else if bar_resp.clicked() {
            output.clicked = true;
        }
    }

    if ui.ctx().dragged_id().is_some() {
        output.is_dragging = true;
    }

    let effective_active =
        is_active && !other_screen_open && !output.context_menu_open && !output.is_dragging;
    let bg_color = settings.titlebar_bg(effective_active);
    ui.painter().set(
        bg_shape_idx,
        egui::Shape::rect_filled(bar_rect, 0.0, bg_color),
    );

    output
}

fn small_bar_button(
    ui: &mut egui::Ui,
    label: &str,
    fg_color: egui::Color32,
    sense: egui::Sense,
    tooltip: &str,
) -> egui::Response {
    let btn = egui::Button::new(egui::RichText::new(label).color(fg_color).size(12.0))
        .frame(false)
        .sense(sense);
    ui.add(btn).on_hover_text(tooltip)
}

#[cfg(test)]
#[path = "test/titlebar_tests.rs"]
mod tests;