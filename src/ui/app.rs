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
use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
use x11rb::connection::{Connection as _, RequestConnection as _};
use x11rb::protocol::xinput::{self, ConnectionExt as _};
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;
use crate::config::prefs_ui::PreferencesWindow;
use crate::config::theme::ThemeSettings;
use crate::menus::MenuManager;
use crate::ui::workspace::Workspace;

const RESIZE_CORNER_SIZE: f32 = 16.0;

struct X11PinchListener {
    conn: RustConnection,
    last_scale: f32,
}

impl X11PinchListener {
    fn try_new(raw_window_handle: RawWindowHandle) -> Option<Self> {
        let xwindow: u32 = match raw_window_handle {
            RawWindowHandle::Xlib(h) => h.window as u32,
            RawWindowHandle::Xcb(h) => h.window.get(),
            _ => return None,
        };
        if xwindow == 0 {
            return None;
        }

        let (conn, _screen_num) = RustConnection::connect(None).ok()?;
        conn.extension_information(xinput::X11_EXTENSION_NAME)
            .ok()??;
        let ver = conn.xinput_xi_query_version(2, 4).ok()?.reply().ok()?;
        if (ver.major_version, ver.minor_version) < (2, 4) {
            return None;
        }

        // XInput 2.4: GesturePinchBegin (27), GesturePinchUpdate (28), GesturePinchEnd (29)
        let pinch_mask = xinput::XIEventMask::from((1u32 << 27) | (1u32 << 28) | (1u32 << 29));
        let event_masks = [xinput::EventMask {
            deviceid: xinput::Device::ALL_MASTER.into(),
            mask: vec![pinch_mask],
        }];

        conn.xinput_xi_select_events(xwindow, &event_masks).ok()?;
        if let Ok(cookie) = conn.query_tree(xwindow) {
            if let Ok(tree) = cookie.reply() {
                for &child in &tree.children {
                    let _ = conn.xinput_xi_select_events(child, &event_masks);
                }
            }
        }
        conn.flush().ok()?;

        Some(Self {
            conn,
            last_scale: 1.0,
        })
    }

    fn poll_zoom_delta(&mut self) -> f32 {
        let mut zoom_delta = 1.0_f32;
        while let Ok(Some(ev)) = self.conn.poll_for_event() {
            match ev {
                x11rb::protocol::Event::XinputGesturePinchBegin(pinch) => {
                    let s = (pinch.scale as f32) / 65536.0;
                    self.last_scale = if s > 0.05 { s } else { 1.0 };
                }
                x11rb::protocol::Event::XinputGesturePinchUpdate(pinch) => {
                    let s = (pinch.scale as f32) / 65536.0;
                    if s > 0.05 && self.last_scale > 0.05 {
                        zoom_delta *= s / self.last_scale;
                        self.last_scale = s;
                    }
                }
                x11rb::protocol::Event::XinputGesturePinchEnd(_) => {
                    self.last_scale = 1.0;
                }
                _ => {}
            }
        }
        zoom_delta
    }
}

pub struct TerminApp {
    pub settings: ThemeSettings,
    pub workspace: Workspace,
    pub prefs_window: PreferencesWindow,
    pub menu_manager: MenuManager,
    x11_pinch_listener: Option<X11PinchListener>,
    x11_pinch_init_attempted: bool,
}

impl TerminApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = ThemeSettings::load();
        // Disable egui's global GUI zoom so Ctrl/Super + =/- only zooms the focused terminal pane
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        // Apply saved font family and sizes on startup
        settings.apply_fonts(&cc.egui_ctx);

        let x11_pinch_listener = cc
            .window_handle()
            .ok()
            .and_then(|h| X11PinchListener::try_new(h.as_raw()));
        let x11_pinch_init_attempted = x11_pinch_listener.is_some();

        Self {
            settings,
            workspace: Workspace::new(),
            prefs_window: PreferencesWindow::default(),
            menu_manager: MenuManager::new(),
            x11_pinch_listener,
            x11_pinch_init_attempted,
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
    fn ui(&mut self, ui: &mut Ui, frame: &mut Frame) {
        let ctx = ui.ctx().clone();

        if !self.x11_pinch_init_attempted {
            self.x11_pinch_init_attempted = true;
            if let Ok(handle) = frame.window_handle() {
                self.x11_pinch_listener = X11PinchListener::try_new(handle.as_raw());
            }
        }

        let external_zoom_delta = self
            .x11_pinch_listener
            .as_mut()
            .map(|l| l.poll_zoom_delta())
            .unwrap_or(1.0);
        if (external_zoom_delta - 1.0).abs() > 0.001 {
            ctx.request_repaint();
        }

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

        // Preferences modal / floating window rendered before workspace so wants_keyboard_input is accurate
        self.prefs_window.ui(&ctx, &mut self.settings);
        if self.prefs_window.save_layout_requested {
            self.prefs_window.save_layout_requested = false;
            self.workspace.save_layout();
        }
        if self.prefs_window.load_layout_requested {
            self.prefs_window.load_layout_requested = false;
            self.workspace.load_layout();
        }

        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let border_w = if is_maximized {
            0.0_f32
        } else {
            self.settings.effective_border_width()
        };

        let panel_frame = egui::Frame::new()
            .fill(self.settings.frame_bg_color())
            .inner_margin(border_w)
            .stroke(egui::Stroke::new(
                if is_maximized { 0.0_f32 } else { 1.0_f32 },
                self.settings.frame_border_color(),
            ));

        egui::CentralPanel::default().frame(panel_frame).show(ui, |ui: &mut Ui| {
            self.workspace.ui(
                ui,
                &ctx,
                &mut self.settings,
                &self.menu_manager,
                &mut self.prefs_window.open,
                external_zoom_delta,
            );
        });
    }
}
