use eframe::egui;
use crate::config::theme::ThemeSettings;

#[derive(Clone, Copy, Debug)]
pub enum TitleBarAction {
    ClosePane,
    MinimizeWindow,
    ToggleMaximizeWindow,
    StartWindowMove,
    SplitHorizontal,
    SplitVertical,
    OpenPreferences,
}

pub struct TitleBarOutput {
    pub action: Option<TitleBarAction>,
    pub start_tile_drag: bool,
    pub clicked: bool,
}

pub fn render_titlebar(
    ui: &mut egui::Ui,
    title: &mut String,
    editing_title: &mut bool,
    is_active: bool,
    is_top_right: bool,
    settings: &ThemeSettings,
) -> TitleBarOutput {
    let mut output = TitleBarOutput {
        action: None,
        start_tile_drag: false,
        clicked: false,
    };

    let bg_color = settings.titlebar_bg(is_active);
    let fg_color = settings.titlebar_fg(is_active);

    let bar_height = 24.0;
    let desired_size = egui::vec2(ui.available_width(), bar_height);
    let (bar_rect, bar_resp) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

    ui.painter().rect_filled(bar_rect, 0.0, bg_color);

    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bar_rect.shrink2(egui::vec2(4.0, 2.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );

    child_ui.spacing_mut().item_spacing = egui::vec2(3.0, 0.0);

    // 1. Left controls: Drag grip [ + ] for pane swap, plus split buttons
    let grip_btn = egui::Button::new(egui::RichText::new(" ✥ ").color(fg_color).size(12.0))
        .frame(false)
        .sense(egui::Sense::drag());
    let grip_resp = child_ui
        .add(grip_btn)
        .on_hover_text("Drag and drop onto another pane to swap/split");

    if grip_resp.drag_started() {
        output.start_tile_drag = true;
    }

    if small_bar_button(&mut child_ui, "[Split-H]", fg_color, "Split Horizontally (Left/Right)").clicked() {
        output.action = Some(TitleBarAction::SplitHorizontal);
    }
    if small_bar_button(&mut child_ui, "[Split-V]", fg_color, "Split Vertically (Top/Bottom)").clicked() {
        output.action = Some(TitleBarAction::SplitVertical);
    }

    // 2. Title text or inline rename entry
    if *editing_title {
        let edit_resp = child_ui.add(
            egui::TextEdit::singleline(title)
                .desired_width(150.0)
                .font(egui::TextStyle::Monospace),
        );
        if edit_resp.lost_focus() || child_ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            *editing_title = false;
        } else {
            edit_resp.request_focus();
        }
    } else {
        child_ui.add(
            egui::Label::new(egui::RichText::new(title.as_str()).color(fg_color).size(12.0))
                .selectable(false),
        );
    }

    // 3. Right-side controls: [Gear] [Edit] ([—] [🗖] if top-right) [×]
    child_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if small_bar_button(ui, " × ", fg_color, "Close this pane").clicked() {
            output.action = Some(TitleBarAction::ClosePane);
        }

        if is_top_right {
            if small_bar_button(ui, " 🗖 ", fg_color, "Maximize / Restore window").clicked() {
                output.action = Some(TitleBarAction::ToggleMaximizeWindow);
            }
            if small_bar_button(ui, " — ", fg_color, "Minimize window").clicked() {
                output.action = Some(TitleBarAction::MinimizeWindow);
            }
        }

        if small_bar_button(ui, " ✎ ", fg_color, "Rename pane title").clicked() {
            *editing_title = !*editing_title;
        }

        if small_bar_button(ui, " ⚙ ", fg_color, "Open CustomDragBar Preferences").clicked() {
            output.action = Some(TitleBarAction::OpenPreferences);
        }
    });

    // 4. Titlebar background interaction (Double-click = Maximize, Drag = Move Window)
    if !*editing_title && !output.start_tile_drag && output.action.is_none() {
        if bar_resp.double_clicked() {
            output.action = Some(TitleBarAction::ToggleMaximizeWindow);
            output.clicked = true;
        } else if bar_resp.drag_started_by(egui::PointerButton::Primary) {
            output.action = Some(TitleBarAction::StartWindowMove);
            output.clicked = true;
        } else if bar_resp.clicked() {
            output.clicked = true;
        }
    }

    output
}

fn small_bar_button(
    ui: &mut egui::Ui,
    label: &str,
    fg_color: egui::Color32,
    tooltip: &str,
) -> egui::Response {
    let btn = egui::Button::new(egui::RichText::new(label).color(fg_color).size(12.0)).frame(false);
    ui.add(btn).on_hover_text(tooltip)
}
