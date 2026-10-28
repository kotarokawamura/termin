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

// Unit tests for TerminalPane and Broadcast Groups
use super::*;
use std::sync::{Arc, Mutex};
use crate::ui::state::{InteractionMode, PaneContext};
use crate::ui::workspace::{SplitAxis, Workspace};

#[derive(Clone, Default)]
struct RecordingSession {
    written: Arc<Mutex<Vec<u8>>>,
}

impl SessionDriver for RecordingSession {
    fn write_input(&mut self, data: &[u8]) {
        self.written.lock().unwrap().extend_from_slice(data);
    }
    fn read_output(&mut self) -> Vec<u8> {
        Vec::new()
    }
    fn resize(&mut self, _rows: NonZeroU16, _cols: NonZeroU16) {}
    fn is_alive(&mut self) -> bool {
        true
    }
}

fn make_test_pane_with_recorder() -> (TerminalPane, Arc<Mutex<Vec<u8>>>) {
    let rows = NonZeroU16::new(24).unwrap();
    let cols = NonZeroU16::new(80).unwrap();
    let mut backend = Vt100Adapter::new(rows, cols);
    backend.process_bytes(b"Hello, World!\r\nSecond Line\r\n");
    let written = Arc::new(Mutex::new(Vec::new()));
    let session = RecordingSession {
        written: Arc::clone(&written),
    };
    let pane = TerminalPane {
        id: 1,
        title: "/bin/bash".to_string(),
        editing_title: false,
        broadcast_group: None,
        zoom_scale: 1.0,
        scroll_accum_y: 0.0,
        backend,
        session: Box::new(session),
        selection_start: None,
        selection_end: None,
        abs_selection_start: None,
        abs_selection_end: None,
        select_anchor_pos: None,
        pending_paste: false,
        super_key_latched: false,
        pending_broadcast_input: Vec::new(),
        grid_cache_meta: None,
        row_caches: Vec::new(),
        cached_terminal_mesh: Arc::new(egui::Mesh::default()),
        last_rendered_gen: u64::MAX,
        last_rendered_scroll: usize::MAX,
        last_rendered_sel: None,
        last_max_scroll: 0,
    };
    (pane, written)
}

fn make_test_pane() -> TerminalPane {
    make_test_pane_with_recorder().0
}

fn make_pane_ctx(tile_id: egui_tiles::TileId, is_focused: bool, blocked: bool) -> PaneContext {
    PaneContext {
        tile_id,
        is_focused,
        is_top_right: true,
        window_focused: true,
        popup_open: false,
        mode: if blocked {
            InteractionMode::BlockedByModal
        } else {
            InteractionMode::Normal
        },
        external_zoom_delta: 1.0,
        is_selecting_text: false,
        broadcast_group: None,
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
    let mut shortcuts = ShortcutSettings::default();
    shortcuts.copy = "Ctrl+C".to_string();
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
fn test_ctrl_zoom_shortcuts() {
    let mut pane = make_test_pane();
    let ctx = egui::Context::default();
    let shortcuts = ShortcutSettings::default();

    // Ctrl + = -> ZoomIn
    let mut raw_in = egui::RawInput::default();
    raw_in.events.push(egui::Event::Key {
        key: egui::Key::Equals,
        physical_key: Some(egui::Key::Equals),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    });
    let mut action = None;
    ctx.run_ui(raw_in, |ui| {
        let (_, act) = pane.handle_shortcut_keyboard_input(ui, &shortcuts);
        action = act;
    })
        .textures_delta
        .clear();
    assert_eq!(action, Some(TitleBarAction::ZoomIn));

    // Ctrl + - -> ZoomOut
    let mut raw_out = egui::RawInput::default();
    raw_out.events.push(egui::Event::Key {
        key: egui::Key::Minus,
        physical_key: Some(egui::Key::Minus),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    });
    ctx.run_ui(raw_out, |ui| {
        let (_, act) = pane.handle_shortcut_keyboard_input(ui, &shortcuts);
        action = act;
    })
        .textures_delta
        .clear();
    assert_eq!(action, Some(TitleBarAction::ZoomOut));
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

    // 1. Blocked by modal: selection on the background pane must NOT be triggered.
    let start_pos = egui::pos2(100.0, 80.0);
    let mut raw_press = egui::RawInput::default();
    raw_press.screen_rect = Some(screen);
    raw_press.events.push(egui::Event::PointerMoved(start_pos));
    raw_press.events.push(egui::Event::PointerButton {
        pos: start_pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    let blocked_ctx = make_pane_ctx(tile_id, true, true);
    ctx.run_ui(raw_press, |ui| {
        pane.ui(ui, &blocked_ctx, &menus, &settings);
    })
        .textures_delta
        .clear();

    let mut raw_drag = egui::RawInput::default();
    raw_drag.screen_rect = Some(screen);
    let drag_pos = egui::pos2(240.0, 120.0);
    raw_drag.events.push(egui::Event::PointerMoved(drag_pos));
    ctx.run_ui(raw_drag, |ui| {
        pane.ui(ui, &blocked_ctx, &menus, &settings);
    })
        .textures_delta
        .clear();

    assert!(
        pane.selection_start.is_none() && pane.selection_end.is_none(),
        "Dragging a sub-window titlebar must never trigger text selection on the background panel"
    );

    // 2. While a pane is inactive (is_focused = false), dragging on it must NOT select text.
    let inactive_ctx = make_pane_ctx(tile_id, false, false);
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
        pane.ui(ui, &inactive_ctx, &menus, &settings);
    })
        .textures_delta
        .clear();

    let mut raw_inactive_drag = egui::RawInput::default();
    raw_inactive_drag.screen_rect = Some(screen);
    raw_inactive_drag.events.push(egui::Event::PointerMoved(drag_pos));
    ctx.run_ui(raw_inactive_drag, |ui| {
        pane.ui(ui, &inactive_ctx, &menus, &settings);
    })
        .textures_delta
        .clear();

    assert!(
        pane.selection_start.is_none() && pane.selection_end.is_none(),
        "Inactive background panel must not allow text range selection"
    );

    // 3. When blocked by modal, even the focused pane must ignore clicks and shortcuts.
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
        key: egui::Key::E,
        physical_key: Some(egui::Key::E),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
    });
    let mut out = PaneUiOutput {
        action: None,
        toggle_tile_move: false,
        toggle_broadcast_select: false,
        clicked_or_focused: false,
        broadcast_input: Vec::new(),
    };
    ctx.run_ui(raw_other_open, |ui| {
        out = pane.ui(ui, &blocked_ctx, &menus, &settings);
    })
        .textures_delta
        .clear();

    assert!(
        out.action.is_none()
            && !out.toggle_tile_move
            && !out.toggle_broadcast_select
            && !out.clicked_or_focused,
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
        let (_, act) = pane.handle_shortcut_keyboard_input(ui, &shortcuts);
        action = act;
    })
        .textures_delta
        .clear();
    assert_eq!(action, Some(TitleBarAction::SplitHorizontal));
}

#[test]
fn test_broadcast_groups_numbering_key_forwarding_and_unlinking() {
    let mut ws = Workspace::new();
    let first_id = ws.focused_tile.unwrap();
    ws.split_pane(first_id, SplitAxis::Horizontal);
    let second_id = ws.focused_tile.unwrap();
    ws.split_pane(second_id, SplitAxis::Vertical);
    ws.split_pane(first_id, SplitAxis::Vertical);

    // Collect the 4 pane TileIds and attach RecordingSessions to each
    let pane_ids: Vec<egui_tiles::TileId> = ws
        .tree
        .tiles
        .iter()
        .filter_map(|(id, tile)| matches!(tile, egui_tiles::Tile::Pane(_)).then_some(*id))
        .collect();
    assert_eq!(pane_ids.len(), 4);

    let mut recorders = Vec::new();
    for &pid in &pane_ids {
        let rec = Arc::new(Mutex::new(Vec::new()));
        if let Some(egui_tiles::Tile::Pane(p)) = ws.tree.tiles.get_mut(pid) {
            p.session = Box::new(RecordingSession {
                written: Arc::clone(&rec),
            });
        }
        recorders.push(rec);
    }

    // 1. Link pane 0 and pane 1 -> Group 1
    ws.link_broadcast_panes(pane_ids[0], pane_ids[1]);
    let g0 = match ws.tree.tiles.get(pane_ids[0]) {
        Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
        _ => None,
    };
    let g1 = match ws.tree.tiles.get(pane_ids[1]) {
        Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
        _ => None,
    };
    assert_eq!(g0, Some(1));
    assert_eq!(g1, Some(1));

    // 2. Link pane 2 and pane 3 -> Group 2 (different group number!)
    ws.link_broadcast_panes(pane_ids[2], pane_ids[3]);
    let g2 = match ws.tree.tiles.get(pane_ids[2]) {
        Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
        _ => None,
    };
    let g3 = match ws.tree.tiles.get(pane_ids[3]) {
        Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
        _ => None,
    };
    assert_eq!(g2, Some(2));
    assert_eq!(g3, Some(2));

    // 3. Simulate typing "ls" then Enter into pane 0 (active pane in Group 1)
    let ctx = egui::Context::default();
    let mut settings = ThemeSettings::default();
    settings.apply_fonts(&ctx);
    let menus = MenuManager::new();
    let mut prefs_open = false;
    ws.focused_tile = Some(pane_ids[0]);

    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 800.0));

    let mut raw_text = egui::RawInput::default();
    raw_text.screen_rect = Some(screen);
    raw_text.focused = true;
    raw_text.events.push(egui::Event::Text("ls".to_string()));
    ctx.run_ui(raw_text, |ui| {
        let ctx_clone = ui.ctx().clone();
        ws.ui(
            ui,
            &ctx_clone,
            &mut settings,
            &menus,
            &mut prefs_open,
            false,
            false,
            false,
            1.0,
        );
    })
        .textures_delta
        .clear();

    let mut raw_enter = egui::RawInput::default();
    raw_enter.screen_rect = Some(screen);
    raw_enter.focused = true;
    raw_enter.events.push(egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    ctx.run_ui(raw_enter, |ui| {
        let ctx_clone = ui.ctx().clone();
        ws.ui(
            ui,
            &ctx_clone,
            &mut settings,
            &menus,
            &mut prefs_open,
            false,
            false,
            false,
            1.0,
        );
    })
        .textures_delta
        .clear();

    // Pane 0 and Pane 1 (Group 1) both received "ls\r", while Pane 2 and Pane 3 (Group 2) received nothing
    assert_eq!(&*recorders[0].lock().unwrap(), b"ls\r");
    assert_eq!(&*recorders[1].lock().unwrap(), b"ls\r");
    assert!(recorders[2].lock().unwrap().is_empty());
    assert!(recorders[3].lock().unwrap().is_empty());

    // 4. Unlink pane 0 from Group 1 -> since only 1 pane (pane 1) would remain in Group 1, Group 1 dissolves completely
    ws.leave_broadcast_group(pane_ids[0]);
    let g0_after = match ws.tree.tiles.get(pane_ids[0]) {
        Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
        _ => None,
    };
    let g1_after = match ws.tree.tiles.get(pane_ids[1]) {
        Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
        _ => None,
    };
    let g2_after = match ws.tree.tiles.get(pane_ids[2]) {
        Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
        _ => None,
    };
    assert_eq!(g0_after, None);
    assert_eq!(g1_after, None);
    assert_eq!(g2_after, Some(2));
}