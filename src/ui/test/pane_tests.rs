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

// Unit tests for TerminalPane
use super::*;

struct DummySession;
impl SessionDriver for DummySession {
    fn write_input(&mut self, _data: &[u8]) {}
    fn read_output(&mut self) -> Vec<u8> {
        Vec::new()
    }
    fn resize(&mut self, _rows: u16, _cols: u16) {}
    fn is_alive(&mut self) -> bool {
        true
    }
}

fn make_test_pane() -> TerminalPane {
    let mut backend = Vt100Adapter::new(24, 80);
    backend.process_bytes(b"Hello, World!\r\nSecond Line\r\n");
    TerminalPane {
        id: 1,
        title: "/bin/bash".to_string(),
        editing_title: false,
        zoom_scale: 1.0,
        scroll_accum_y: 0.0,
        backend,
        session: Box::new(DummySession),
        selection_start: None,
        selection_end: None,
        select_anchor_pos: None,
        pending_paste: false,
    }
}

#[test]
fn test_copy_selection_range_and_single_char() {
    let mut pane = make_test_pane();

    // Single-char selection (start == end)
    pane.selection_start = Some((0, 0));
    pane.selection_end = Some((0, 0));
    assert_eq!(pane.extract_selected_text().as_deref(), Some("H"));

    // Range selection on row 0 ("Hello")
    pane.selection_start = Some((0, 0));
    pane.selection_end = Some((0, 4));
    assert_eq!(pane.extract_selected_text().as_deref(), Some("Hello"));

    // Verify Event::Copy (emitted by egui-winit on Ctrl+Shift+C / Ctrl+C) copies the selection
    let ctx = egui::Context::default();
    let shortcuts = ShortcutSettings::default();
    let mut raw = egui::RawInput::default();
    raw.events.push(egui::Event::Copy);
    let mut out = ctx.run_ui(raw, |ui| {
        pane.handle_shortcut_keyboard_input(ui, &shortcuts);
    });
    out.textures_delta.clear();
    let copied = out.platform_output.commands.iter().find_map(|cmd| {
        if let egui::OutputCommand::CopyText(t) = cmd {
            Some(t.clone())
        } else {
            None
        }
    });
    assert_eq!(copied.as_deref(), Some("Hello"));
}

#[test]
fn test_super_zoom_shortcuts() {
    let mut pane = make_test_pane();
    let ctx = egui::Context::default();
    let shortcuts = ShortcutSettings::default();

    // Super + = -> ZoomIn
    let mut raw_in = egui::RawInput::default();
    raw_in.events.push(egui::Event::Key {
        key: egui::Key::SuperLeft,
        physical_key: Some(egui::Key::SuperLeft),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    raw_in.events.push(egui::Event::Key {
        key: egui::Key::Equals,
        physical_key: Some(egui::Key::Equals),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    let mut action = None;
    ctx.run_ui(raw_in, |ui| {
        action = pane.handle_shortcut_keyboard_input(ui, &shortcuts);
    })
        .textures_delta
        .clear();
    assert_eq!(action, Some(TitleBarAction::ZoomIn));

    // Super + - -> ZoomOut
    let mut raw_out = egui::RawInput::default();
    raw_out.events.push(egui::Event::Key {
        key: egui::Key::Minus,
        physical_key: Some(egui::Key::Minus),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    ctx.run_ui(raw_out, |ui| {
        action = pane.handle_shortcut_keyboard_input(ui, &shortcuts);
    })
        .textures_delta
        .clear();
    assert_eq!(action, Some(TitleBarAction::ZoomOut));
}

#[test]
fn test_cursor_glyph_vertical_alignment() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    settings.apply_fonts(&ctx);
    let font_id = egui::FontId::monospace(14.0);
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(800.0, 600.0),
    ));
    ctx.run_ui(raw, |ui| {
        let ref_galley = ui.painter().layout_no_wrap(
            "hk~$日".to_string(),
            font_id.clone(),
            egui::Color32::WHITE,
        );
        let ink_min_y = ref_galley.mesh_bounds.min.y;
        let ink_max_y = ref_galley.mesh_bounds.max.y;
        let ink_h = (ink_max_y - ink_min_y).max(14.0 * 0.85);
        let char_h = (ink_h + (14.0_f32 * 0.28).max(4.0)).round().max(8.0);
        let ink_center_y = (ink_min_y + ink_max_y) * 0.5;
        let glyph_y_offset = (char_h * 0.5) - ink_center_y;

        let prompt_galley = ui.painter().layout_no_wrap(
            "kotaro@debian:~$".to_string(),
            font_id.clone(),
            egui::Color32::WHITE,
        );
        let rendered_prompt_center_y =
            glyph_y_offset + prompt_galley.mesh_bounds.center().y;
        let cursor_center_y = char_h * 0.5;
        assert!(
            (rendered_prompt_center_y - cursor_center_y).abs() < 0.5,
            "Cursor vertical center ({cursor_center_y}) must match rendered prompt ink center ({rendered_prompt_center_y})"
        );
    })
        .textures_delta
        .clear();
}

#[test]
fn test_subwindow_drag_does_not_select_background_and_inactive_pane_blocked() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    settings.apply_fonts(&ctx);
    let menus = MenuManager::new();
    let tile_id = egui_tiles::TileId::from_u64(1);
    let mut pane = make_test_pane();

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));

    // 1. Simulate dragging a sub-window titlebar while pointer is captured by sub-window:
    //    selection on the background pane must NOT be triggered.
    let mut raw_press = egui::RawInput::default();
    raw_press.screen_rect = Some(screen);
    let start_pos = egui::pos2(100.0, 80.0);
    raw_press.events.push(egui::Event::PointerMoved(start_pos));
    raw_press.events.push(egui::Event::PointerButton {
        pos: start_pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    ctx.run_ui(raw_press, |ui| {
        pane.ui(
            ui,
            tile_id,
            &menus,
            &settings,
            true,
            true,
            false,
            true, // prefs_capturing_pointer = true (dragging subwindow titlebar)
            false,
            true, // workspace_dragging = true
            1.0,
        );
    })
        .textures_delta
        .clear();

    let mut raw_drag = egui::RawInput::default();
    raw_drag.screen_rect = Some(screen);
    let drag_pos = egui::pos2(240.0, 120.0);
    raw_drag.events.push(egui::Event::PointerMoved(drag_pos));
    ctx.run_ui(raw_drag, |ui| {
        pane.ui(
            ui,
            tile_id,
            &menus,
            &settings,
            true,
            true,
            false,
            true,
            false,
            true,
            1.0,
        );
    })
        .textures_delta
        .clear();

    assert!(
        pane.selection_start.is_none() && pane.selection_end.is_none(),
        "Dragging a sub-window titlebar must never trigger text selection on the background panel"
    );

    // Release mouse button
    let mut raw_release = egui::RawInput::default();
    raw_release.screen_rect = Some(screen);
    raw_release.events.push(egui::Event::PointerButton {
        pos: drag_pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    ctx.run_ui(raw_release, |ui| {
        pane.ui(
            ui,
            tile_id,
            &menus,
            &settings,
            true,
            true,
            false,
            false,
            false,
            false,
            1.0,
        );
    })
        .textures_delta
        .clear();

    // 2. While a pane is inactive (is_focused = false), dragging on it must NOT select text.
    let mut raw_inactive_press = egui::RawInput::default();
    raw_inactive_press.screen_rect = Some(screen);
    raw_inactive_press.events.push(egui::Event::PointerMoved(start_pos));
    raw_inactive_press.events.push(egui::Event::PointerButton {
        pos: start_pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    ctx.run_ui(raw_inactive_press, |ui| {
        pane.ui(
            ui,
            tile_id,
            &menus,
            &settings,
            false, // inactive pane
            true,
            false,
            false,
            false,
            false,
            1.0,
        );
    })
        .textures_delta
        .clear();

    let mut raw_inactive_drag = egui::RawInput::default();
    raw_inactive_drag.screen_rect = Some(screen);
    raw_inactive_drag.events.push(egui::Event::PointerMoved(drag_pos));
    ctx.run_ui(raw_inactive_drag, |ui| {
        pane.ui(
            ui,
            tile_id,
            &menus,
            &settings,
            false, // still inactive during drag
            true,
            false,
            false,
            false,
            false,
            1.0,
        );
    })
        .textures_delta
        .clear();

    assert!(
        pane.selection_start.is_none() && pane.selection_end.is_none(),
        "Inactive background panel must not allow text range selection"
    );

    // 3. When another screen is open (other_screen_open = true), even the focused pane
    //    must ignore clicks, drags, and keyboard shortcuts.
    let mut raw_other_open = egui::RawInput::default();
    raw_other_open.screen_rect = Some(screen);
    raw_other_open.events.push(egui::Event::PointerMoved(start_pos));
    raw_other_open.events.push(egui::Event::PointerButton {
        pos: start_pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    raw_other_open.events.push(egui::Event::Key {
        key: egui::Key::H,
        physical_key: Some(egui::Key::H),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
    });
    let mut out = PaneUiOutput {
        action: None,
        start_tile_drag: false,
        clicked_or_focused: false,
    };
    ctx.run_ui(raw_other_open, |ui| {
        out = pane.ui(
            ui,
            tile_id,
            &menus,
            &settings,
            true, // focused pane
            true,
            true, // other_screen_open = true
            false,
            false,
            false,
            1.0,
        );
    })
        .textures_delta
        .clear();

    assert!(
        out.action.is_none() && !out.start_tile_drag && !out.clicked_or_focused,
        "Pane must not respond to clicks or shortcuts while another screen is open"
    );
}

#[test]
fn test_custom_shortcut_configuration() {
    let mut pane = make_test_pane();
    let ctx = egui::Context::default();
    let mut shortcuts = ShortcutSettings::default();
    shortcuts.split_horizontal = "Alt+H".to_string();

    let mut raw = egui::RawInput::default();
    raw.events.push(egui::Event::Key {
        key: egui::Key::H,
        physical_key: Some(egui::Key::H),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::ALT,
    });
    let mut action = None;
    ctx.run_ui(raw, |ui| {
        action = pane.handle_shortcut_keyboard_input(ui, &shortcuts);
    })
        .textures_delta
        .clear();
    assert_eq!(action, Some(TitleBarAction::SplitHorizontal));
}