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
use eframe::{egui, Frame};
use egui::Ui;
use crate::config::prefs_ui::PreferencesWindow;
use crate::config::theme::ThemeSettings;
use crate::menus::MenuManager;
use crate::ui::workspace::Workspace;

const RESIZE_CORNER_SIZE: f32 = 16.0;

pub struct TerminApp {
    pub settings: ThemeSettings,
    pub workspace: Workspace,
    pub prefs_window: PreferencesWindow,
    pub menu_manager: MenuManager,
}

impl TerminApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = ThemeSettings::load();
        // Apply saved font family and sizes on startup
        settings.apply_fonts(&cc.egui_ctx);

        Self {
            settings,
            workspace: Workspace::new(),
            prefs_window: PreferencesWindow::default(),
            menu_manager: MenuManager::new(),
        }
    }

    /// Handle custom borderless window edge and corner resizing.
    fn handle_custom_resize_frame(&self, ctx: &egui::Context) {
        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if is_maximized {
            return;
        }

        let border = self.settings.effective_border_width() + 2.0;
        let screen_rect = ctx.content_rect();
        let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };

        let on_left = pos.x <= screen_rect.min.x + border;
        let on_right = pos.x >= screen_rect.max.x - border;
        let on_top = pos.y <= screen_rect.min.y + border;
        let on_bottom = pos.y >= screen_rect.max.y - border;

        if !(on_left || on_right || on_top || on_bottom) {
            return;
        }

        let in_left_corner = pos.x <= screen_rect.min.x + RESIZE_CORNER_SIZE;
        let in_right_corner = pos.x >= screen_rect.max.x - RESIZE_CORNER_SIZE;
        let in_top_corner = pos.y <= screen_rect.min.y + RESIZE_CORNER_SIZE;
        let in_bottom_corner = pos.y >= screen_rect.max.y - RESIZE_CORNER_SIZE;

        let dir_and_cursor = if in_top_corner && in_left_corner {
            Some((egui::ResizeDirection::NorthWest, egui::CursorIcon::ResizeNorthWest))
        } else if in_top_corner && in_right_corner {
            Some((egui::ResizeDirection::NorthEast, egui::CursorIcon::ResizeNorthEast))
        } else if in_bottom_corner && in_left_corner {
            Some((egui::ResizeDirection::SouthWest, egui::CursorIcon::ResizeSouthWest))
        } else if in_bottom_corner && in_right_corner {
            Some((egui::ResizeDirection::SouthEast, egui::CursorIcon::ResizeSouthEast))
        } else if on_top {
            Some((egui::ResizeDirection::North, egui::CursorIcon::ResizeNorth))
        } else if on_bottom {
            Some((egui::ResizeDirection::South, egui::CursorIcon::ResizeSouth))
        } else if on_left {
            Some((egui::ResizeDirection::West, egui::CursorIcon::ResizeWest))
        } else if on_right {
            Some((egui::ResizeDirection::East, egui::CursorIcon::ResizeEast))
        } else {
            None
        };

        if let Some((direction, cursor)) = dir_and_cursor {
            ctx.set_cursor_icon(cursor);
            let primary_pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
            if primary_pressed {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
            }
        }
    }
}

impl eframe::App for TerminApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut Frame) {
        let ctx = ui.ctx().clone();

        // Poll active PTY sessions and request repaint if new output arrived
        if self.workspace.poll_sessions() {
            ctx.request_repaint();
        }

        // 33FPS
        ctx.request_repaint_after(std::time::Duration::from_millis(30));

        self.handle_custom_resize_frame(&ctx);

        // Apply separator color to egui style
        ctx.all_styles_mut(|style| {
            style.visuals.widgets.noninteractive.bg_stroke.color = self.settings.separator_color();
        });

        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let border_w = if is_maximized {
            0.0_f32
        } else {
            self.settings.effective_border_width()
        };

        let frame = egui::Frame::new()
            .fill(self.settings.frame_bg_color())
            .inner_margin(border_w)
            .stroke(egui::Stroke::new(
                if is_maximized { 0.0_f32 } else { 1.0_f32 },
                self.settings.frame_border_color(),
            ));

        egui::CentralPanel::default().frame(frame).show(ui, |ui: &mut Ui| {
            self.workspace.ui(
                ui,
                &ctx,
                &self.settings,
                &self.menu_manager,
                &mut self.prefs_window.open,
            );
        });

        // Preferences modal / floating window
        self.prefs_window.ui(&ctx, &mut self.settings);
    }
}