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
use crate::config::theme::ThemeSettings;
use crate::i18n::tr;
use crate::menus::{
    MenuManager,
    types::{MenuAction, MenuContext},
};
use crate::ui::state::PaneContext;

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
    pub toggle_tile_move: bool,
    pub clicked: bool,
    pub context_menu_open: bool,
}

pub fn render_titlebar(
    ui: &mut egui::Ui,
    ctx: &PaneContext,
    menus: &MenuManager,
    title: &mut String,
    editing_title: &mut bool,
    settings: &ThemeSettings,
) -> TitleBarOutput {
    let mut output = TitleBarOutput {
        action: None,
        toggle_tile_move: false,
        clicked: false,
        context_menu_open: false,
    };

    if ctx.is_blocked() && *editing_title {
        *editing_title = false;
    }

    let can_interact = ctx.can_interact_titlebar();
    let visual_active = ctx.is_visually_active();
    let is_moving_self = ctx.is_moving_self();
    let any_moving = ctx.is_any_moving();

    let bar_height = 24.0;
    let desired_size = egui::vec2(ui.available_width(), bar_height);

    let bar_sense = if *editing_title || !can_interact {
        egui::Sense::hover()
    } else {
        egui::Sense::click_and_drag()
    };
    let (bar_rect, bar_resp) = ui.allocate_exact_size(desired_size, bar_sense);

    let bg_shape_idx = ui.painter().add(egui::Shape::Noop);
    let fg_color = settings.titlebar_fg(visual_active);

    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bar_rect.shrink2(egui::vec2(4.0, 2.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );

    child_ui.spacing_mut().item_spacing = egui::vec2(3.0, 0.0);

    let btn_sense = if can_interact {
        egui::Sense::CLICK
    } else {
        egui::Sense::hover()
    };

    // 1. Left controls: Clickable [ ✥ ] button to enter/exit pane move mode
    let grip_color = if is_moving_self {
        egui::Color32::from_rgb(250, 204, 21)
    } else {
        fg_color
    };
    let grip_btn = egui::Button::new(egui::RichText::new(" ✥ ").color(grip_color).size(12.0))
        .frame(is_moving_self)
        .sense(btn_sense);
    let grip_resp = child_ui
        .add(grip_btn)
        .on_hover_text(tr("Click to enter pane move mode, then click target pane"));

    if can_interact && !*editing_title && grip_resp.clicked() {
        output.toggle_tile_move = true;
        output.clicked = true;
    }

    let split_h_resp = small_bar_button(
        &mut child_ui,
        &tr("[Split-H]"),
        fg_color,
        btn_sense,
        &tr("Split Horizontally (Left/Right)"),
    );
    if can_interact && !any_moving && split_h_resp.clicked() {
        output.action = Some(TitleBarAction::SplitHorizontal);
        output.clicked = true;
    }

    let split_v_resp = small_bar_button(
        &mut child_ui,
        &tr("[Split-V]"),
        fg_color,
        btn_sense,
        &tr("Split Vertically (Top/Bottom)"),
    );
    if can_interact && !any_moving && split_v_resp.clicked() {
        output.action = Some(TitleBarAction::SplitVertical);
        output.clicked = true;
    }

    let new_tab_resp = small_bar_button(
        &mut child_ui,
        &tr("[+Tab]"),
        fg_color,
        btn_sense,
        &tr("Add a new tab in this pane"),
    );
    if can_interact && !any_moving && new_tab_resp.clicked() {
        output.action = Some(TitleBarAction::NewTab);
        output.clicked = true;
    }

    let left_controls_right_x = child_ui.cursor().min.x;

    // 2. Right-side controls
    let mut just_started_editing = false;
    let mut right_controls_left_x = bar_rect.max.x;
    let mut title_label_rect = egui::Rect::NOTHING;

    child_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if small_bar_button(ui, " × ", fg_color, btn_sense, &tr("Close this pane")).clicked()
            && can_interact
            && !any_moving
        {
            output.action = Some(TitleBarAction::ClosePane);
            output.clicked = true;
        }

        if ctx.is_top_right {
            if small_bar_button(
                ui,
                " 🗖 ",
                fg_color,
                btn_sense,
                &tr("Maximize / Restore window"),
            )
                .clicked()
                && can_interact
                && !any_moving
            {
                output.action = Some(TitleBarAction::ToggleMaximizeWindow);
                output.clicked = true;
            }
            if small_bar_button(ui, " — ", fg_color, btn_sense, &tr("Minimize window")).clicked()
                && can_interact
                && !any_moving
            {
                output.action = Some(TitleBarAction::MinimizeWindow);
                output.clicked = true;
            }
        }

        let edit_btn_resp =
            small_bar_button(ui, " ✎ ", fg_color, btn_sense, &tr("Rename pane title"));
        if can_interact && !any_moving && edit_btn_resp.clicked() {
            *editing_title = !*editing_title;
            if *editing_title {
                just_started_editing = true;
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
            && can_interact
            && !any_moving
        {
            output.action = Some(TitleBarAction::OpenPreferences);
            output.clicked = true;
        }

        right_controls_left_x = ui.cursor().max.x;

        // 3. Title text or inline rename entry
        let avail_left = (left_controls_right_x + 4.0).min(bar_rect.max.x);
        let avail_right = (right_controls_left_x - 4.0).max(avail_left);
        let middle_rect = egui::Rect::from_min_max(
            egui::pos2(avail_left, bar_rect.min.y + 1.0),
            egui::pos2(avail_right, bar_rect.max.y - 1.0),
        );

        let edit_id = egui::Id::new(("pane_title_edit", ctx.tile_id));
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
            title_label_rect = label_rect;
            let mut label_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(label_rect)
                    .layout(egui::Layout::centered_and_justified(
                        egui::Direction::LeftToRight,
                    )),
            );
            // Sense::hover() にすることで、タイトル文字上のドラッグが bar_resp から奪われないようにする
            let lbl_resp = label_ui.add(
                egui::Label::new(
                    egui::RichText::new(title.as_str())
                        .color(fg_color)
                        .size(12.0),
                )
                    .truncate()
                    .sense(egui::Sense::hover())
                    .selectable(false),
            );
            #[cfg(test)]
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::new("title_lbl_rect"), lbl_resp.rect));
        }
    });

    // 4. Context menu
    if !ctx.is_blocked() && !any_moving {
        if let Some((_ctx, menu_action)) =
            menus.show(&bar_resp, MenuContext::Titlebar(ctx.tile_id), &settings.shortcuts)
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

    if !ctx.is_blocked() && (bar_resp.context_menu_opened() || egui::Popup::is_any_open(ui.ctx())) {
        output.context_menu_open = true;
    }

    // 5. Titlebar middle area interaction (Window move, double-click, focus)
    let pointer_pos = ui.input(|i| i.pointer.interact_pos().or_else(|| i.pointer.latest_pos()));
    let press_origin = ui.input(|i| i.pointer.press_origin());

    let pos_in_middle = |pos: egui::Pos2| -> bool {
        bar_rect.contains(pos)
            && pos.x >= left_controls_right_x
            && pos.x <= right_controls_left_x
    };

    let in_middle_now = pointer_pos.is_some_and(pos_in_middle);
    let pressed_in_middle = press_origin.is_some_and(pos_in_middle) || in_middle_now;

    if can_interact && !*editing_title && !any_moving && !output.toggle_tile_move && output.action.is_none() {
        if bar_resp.double_clicked() {
            if pointer_pos.is_some_and(|p| title_label_rect.contains(p)) {
                *editing_title = true;
            } else {
                output.action = Some(TitleBarAction::ToggleMaximizeWindow);
            }
            output.clicked = true;
        } else if pressed_in_middle
            && (bar_resp.drag_started_by(egui::PointerButton::Primary)
            || bar_resp.dragged_by(egui::PointerButton::Primary)
            || (in_middle_now
            && ui.input(|i| {
            i.pointer.button_down(egui::PointerButton::Primary)
                && i.pointer.delta().length_sq() > 0.0
        })))
        {
            output.action = Some(TitleBarAction::StartWindowMove);
            output.clicked = true;
        } else if (in_middle_now && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)))
            || bar_resp.clicked()
        {
            output.clicked = true;
        }
    }

    let effective_active = visual_active && !output.context_menu_open;
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