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
use egui::{Ui};
use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
use x11rb::connection::{Connection as _, RequestConnection as _};
use x11rb::protocol::xinput::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, InputFocus, MapState, PropMode,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use crate::config::prefs_ui::PreferencesWindow;
use crate::config::theme::ThemeSettings;
use crate::menus::MenuManager;
use crate::ui::scheduler::{PollMode, RepaintScheduler};
use crate::ui::workspace::Workspace;

const RESIZE_CORNER_SIZE: f32 = 16.0;

struct X11PinchListener {
    conn: RustConnection,
    xwindow: u32,
    root: u32,
    net_active_window_atom: u32,
    startup_focus_remaining: u8,
    last_scale: f32,
    last_poll: std::time::Instant,
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
        let mut root = 0;
        if let Ok(cookie) = conn.query_tree(xwindow) {
            if let Ok(tree) = cookie.reply() {
                root = tree.root;
                for &child in &tree.children {
                    let _ = conn.xinput_xi_select_events(child, &event_masks);
                }
            }
        }

        let wm_hints: [u32; 9] = [1u32 | 2u32, 1, 1, 0, 0, 0, 0, 0, 0];
        let _ = conn.change_property32(
            PropMode::REPLACE,
            xwindow,
            AtomEnum::WM_HINTS,
            AtomEnum::WM_HINTS,
            &wm_hints,
        );

        let net_active_window_atom = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.atom)
            .unwrap_or(0);

        conn.flush().ok()?;

        Some(Self {
            conn,
            xwindow,
            root,
            net_active_window_atom,
            startup_focus_remaining: 15,
            last_scale: 1.0,
            last_poll: std::time::Instant::now(),
        })
    }

    fn ensure_startup_focus(&mut self, egui_focused: bool) {
        if self.startup_focus_remaining == 0 {
            return;
        }
        if egui_focused {
            self.startup_focus_remaining = 0;
            return;
        }
        let viewable = self
            .conn
            .get_window_attributes(self.xwindow)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|attrs| attrs.map_state == MapState::VIEWABLE);
        if !viewable {
            return;
        }
        self.startup_focus_remaining = self.startup_focus_remaining.saturating_sub(1);
        if self.net_active_window_atom != 0 && self.root != 0 {
            let ev = ClientMessageEvent::new(
                32,
                self.xwindow,
                self.net_active_window_atom,
                [2u32, x11rb::CURRENT_TIME, 0, 0, 0],
            );
            let _ = self.conn.send_event(
                false,
                self.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                ev,
            );
        }
        let _ = self.conn.set_input_focus(
            InputFocus::PARENT,
            self.xwindow,
            x11rb::CURRENT_TIME,
        );
        let _ = self.conn.flush();
    }

    fn poll_zoom_delta(&mut self) -> f32 {

        let now = std::time::Instant::now();
        if now.duration_since(self.last_poll) < std::time::Duration::from_millis(33) {
            return 1.0;
        }
        self.last_poll = now;

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

#[derive(Clone, Debug, Default)]
pub struct AppStartupOptions {
    pub fps_override: Option<u32>,
    pub poll_mode: Option<PollMode>,
    pub open_prefs_on_startup: bool,
    pub maximize: bool,
    pub debug_level: u8,
    pub config_path: Option<std::path::PathBuf>,
    pub config_json: Option<String>,
    pub startup_command: Option<String>,
}

pub struct TerminApp {
    pub settings: ThemeSettings,
    pub workspace: Workspace,
    pub prefs_window: PreferencesWindow,
    pub menu_manager: MenuManager,
    pub scheduler: RepaintScheduler,
    x11_pinch_listener: Option<X11PinchListener>,
    x11_pinch_init_attempted: bool,
    startup_focus_frames: u8,
    current_zoom_delta: f32,
    last_pty_poll: std::time::Instant,
}

impl TerminApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        fps_override: Option<u32>,
        poll_mode: PollMode,
        open_prefs_on_startup: bool,
    ) -> Self {
        Self::new_with_options(
            cc,
            AppStartupOptions {
                fps_override,
                poll_mode: Some(poll_mode),
                open_prefs_on_startup,
                ..Default::default()
            },
        )
    }

    pub fn new_with_options(
        cc: &eframe::CreationContext<'_>,
        options: AppStartupOptions,
    ) -> Self {
        if let Some(ref custom_path) = options.config_path {
            ThemeSettings::set_custom_config_path(Some(custom_path.clone()));
        }

        let mut settings = ThemeSettings::load();
        if let Some(ref json_override) = options.config_json {
            if let Err(err) = settings.apply_config_json(json_override) {
                eprintln!("[termin] Warning: failed to apply --config-json: {err}");
            }
        }
        if let Some(fps) = options.fps_override {
            settings.target_fps = fps.clamp(1, 240);
        }

        let poll_mode = options.poll_mode.unwrap_or(PollMode::EventDriven);

        if options.debug_level >= 1 {
            eprintln!(
                "[termin][debug] Initialized settings from {:?} (target_fps={}, poll_mode={:?}, maximize={}, startup_command={:?})",
                ThemeSettings::config_path(),
                settings.target_fps,
                poll_mode,
                options.maximize,
                options.startup_command,
            );
        }

        cc.egui_ctx.options_mut(|egui_opts| {
            egui_opts.quit_shortcuts.clear();
        });

        let scheduler = RepaintScheduler::new(poll_mode);
        let wakeup_ctx = scheduler.pty_wakeup_ctx(&cc.egui_ctx);

        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        settings.apply_fonts(&cc.egui_ctx);
        if options.maximize {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Focus);

        let x11_pinch_listener = cc
            .window_handle()
            .ok()
            .and_then(|h| X11PinchListener::try_new(h.as_raw()));
        let x11_pinch_init_attempted = x11_pinch_listener.is_some();

        let mut prefs_window = PreferencesWindow::default();
        prefs_window.open = options.open_prefs_on_startup;

        Self {
            settings,
            workspace: Workspace::new_with_wakeup_and_command(
                wakeup_ctx,
                options.startup_command.as_deref(),
            ),
            prefs_window,
            menu_manager: MenuManager::new(),
            scheduler,
            x11_pinch_listener,
            x11_pinch_init_attempted,
            startup_focus_frames: 10,
            current_zoom_delta: 1.0,
            last_pty_poll: std::time::Instant::now(),
        }
    }

    fn handle_custom_resize_frame(&self, ctx: &egui::Context) {
        if self.prefs_window.any_open()
            || egui::Popup::is_any_open(ctx)
            || !ctx.input(|i| i.focused)
        {
            return;
        }
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
    fn logic(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        if !self.x11_pinch_init_attempted {
            self.x11_pinch_init_attempted = true;
            if let Ok(handle) = frame.window_handle() {
                self.x11_pinch_listener = X11PinchListener::try_new(handle.as_raw());
            }
        }

        let (window_focused, is_minimized) = ctx.input(|i| {
            (i.focused, i.viewport().minimized.unwrap_or(false))
        });

        if self.startup_focus_frames > 0 {
            if window_focused {
                self.startup_focus_frames = 0;
            } else {
                self.startup_focus_frames -= 1;
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.request_repaint();
            }
        }

        // 非フォーカス時は X11 ソケットのポーリング（システムコール）を行わない
        self.current_zoom_delta = if window_focused || self.startup_focus_frames > 0 {
            self.x11_pinch_listener
                .as_mut()
                .map(|l| {
                    l.ensure_startup_focus(window_focused);
                    l.poll_zoom_delta()
                })
                .unwrap_or(1.0)
        } else {
            1.0
        };

        if (self.current_zoom_delta - 1.0).abs() > 0.001 {
            ctx.request_repaint();
        }

        let now = std::time::Instant::now();
        let fps_interval = std::time::Duration::from_millis(
            (1000 / self.settings.effective_fps().clamp(1, 240) as u64).max(4),
        );

        let elapsed = now.duration_since(self.last_pty_poll);
        let pty_updated = if elapsed >= fps_interval {
            self.last_pty_poll = now;
            self.workspace.poll_sessions()
        } else {

            ctx.request_repaint_after(fps_interval - elapsed);
            false
        };

        if !is_minimized {
            self.scheduler.schedule_next_frame(
                ctx,
                pty_updated,
                self.settings.effective_fps(),
            );
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut Frame) {
        let ctx = ui.ctx().clone();

        self.handle_custom_resize_frame(&ctx);

        let sep_color = self.settings.separator_color();
        if ui.style().visuals.widgets.noninteractive.bg_stroke.color != sep_color {
            ctx.all_styles_mut(|style| {
                style.visuals.widgets.noninteractive.bg_stroke.color = sep_color;
            });
        }

        let prefs_was_open = self.prefs_window.any_open();
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

        let other_screen_open = prefs_was_open || self.prefs_window.any_open();
        let prefs_capturing_pointer = self.prefs_window.is_capturing_pointer(&ctx);
        let prefs_dragging = self.prefs_window.subwindow_dragging;

        egui::CentralPanel::default().frame(panel_frame).show(ui, |ui: &mut Ui| {
            self.workspace.ui(
                ui,
                &ctx,
                &mut self.settings,
                &self.menu_manager,
                &mut self.prefs_window.open,
                other_screen_open,
                prefs_capturing_pointer,
                prefs_dragging,
                self.current_zoom_delta,
            );
        });
    }
}