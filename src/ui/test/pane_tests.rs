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

#[test]
fn test_broadcast_ui_right_click_select_ctrl_multiselect_and_self_unlink() {
    let mut ws = Workspace::new();
    let first_id = ws.focused_tile.unwrap();
    ws.split_pane(first_id, SplitAxis::Horizontal);
    let second_id = ws.focused_tile.unwrap();
    ws.split_pane(second_id, SplitAxis::Vertical);
    ws.split_pane(first_id, SplitAxis::Vertical);

    let pane_ids: Vec<egui_tiles::TileId> = ws
        .tree
        .tiles
        .iter()
        .filter_map(|(id, tile)| matches!(tile, egui_tiles::Tile::Pane(_)).then_some(*id))
        .collect();
    assert_eq!(pane_ids.len(), 4);

    let ctx = egui::Context::default();
    let mut settings = ThemeSettings::default();
    settings.apply_fonts(&ctx);
    let menus = MenuManager::new();
    let mut prefs_open = false;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 800.0));

    let mut run_frame = |ws: &mut Workspace, events: Vec<egui::Event>| {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(screen);
        raw.focused = true;
        raw.events = events;
        ctx.run_ui(raw, |ui| {
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
    };

    // Frame 0: initial layout so rects are recorded
    run_frame(&mut ws, Vec::new());

    let grip0: egui::Rect = ctx
        .data(|d| d.get_temp(egui::Id::new(("grip_btn_rect", pane_ids[0]))))
        .unwrap();
    let pane0_center = ws.recorded_rects[&pane_ids[0]].center();
    let pane1_center = ws.recorded_rects[&pane_ids[1]].center();
    let pane2_center = ws.recorded_rects[&pane_ids[2]].center();

    let mut click_at = |ws: &mut Workspace,
                        pos: egui::Pos2,
                        button: egui::PointerButton,
                        modifiers: egui::Modifiers| {
        run_frame(
            ws,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed: true,
                    modifiers,
                },
            ],
        );
        run_frame(
            ws,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed: false,
                    modifiers,
                },
            ],
        );
    };

    // 1. Right-click grip button on pane 0 -> enters broadcast selection mode
    click_at(
        &mut ws,
        grip0.center(),
        egui::PointerButton::Secondary,
        egui::Modifiers::NONE,
    );
    assert_eq!(ws.selecting_broadcast_tile, Some(pane_ids[0]));

    // 2. Ctrl + Right-click on pane 1 -> links pane 0 and pane 1 into Group 1, and stays in broadcast selection mode!
    let ctrl_mods = egui::Modifiers {
        ctrl: true,
        command: true,
        ..egui::Modifiers::NONE
    };
    click_at(
        &mut ws,
        pane1_center,
        egui::PointerButton::Secondary,
        ctrl_mods,
    );
    assert_eq!(
        ws.selecting_broadcast_tile,
        Some(pane_ids[0]),
        "Holding Ctrl/Command while selecting must keep broadcast selection mode active"
    );

    // 3. Right-click on pane 2 (without Ctrl) -> adds pane 2 to Group 1 and finishes selection mode!
    click_at(
        &mut ws,
        pane2_center,
        egui::PointerButton::Secondary,
        egui::Modifiers::NONE,
    );
    assert_eq!(ws.selecting_broadcast_tile, None);

    for &pid in &pane_ids[0..3] {
        let g = match ws.tree.tiles.get(pid) {
            Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
            _ => None,
        };
        assert_eq!(g, Some(1));
    }

    // 4. Right-click grip button on pane 0 (already in Group 1) -> enters broadcast selection mode (does NOT immediately unlink!)
    click_at(
        &mut ws,
        grip0.center(),
        egui::PointerButton::Secondary,
        egui::Modifiers::NONE,
    );
    assert_eq!(ws.selecting_broadcast_tile, Some(pane_ids[0]));
    let g0_still = match ws.tree.tiles.get(pane_ids[0]) {
        Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
        _ => None,
    };
    assert_eq!(
        g0_still,
        Some(1),
        "Right-clicking grip on a grouped pane must enter broadcast selection mode without immediately unlinking"
    );

    // 5. Select self (pane 0) -> unlinks pane 0 from Group 1 and exits selection mode, while pane 1 and pane 2 stay in Group 1
    click_at(
        &mut ws,
        pane0_center,
        egui::PointerButton::Primary,
        egui::Modifiers::NONE,
    );
    assert_eq!(ws.selecting_broadcast_tile, None);
    let g0_unlinked = match ws.tree.tiles.get(pane_ids[0]) {
        Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
        _ => None,
    };
    let g1_rem = match ws.tree.tiles.get(pane_ids[1]) {
        Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
        _ => None,
    };
    let g2_rem = match ws.tree.tiles.get(pane_ids[2]) {
        Some(egui_tiles::Tile::Pane(p)) => p.broadcast_group,
        _ => None,
    };
    assert_eq!(g0_unlinked, None);
    assert_eq!(g1_rem, Some(1));
    assert_eq!(g2_rem, Some(1));
}

#[test]
fn test_workspace_tree_ops_split_tabs_swap_remove_focus_and_top_right() {
    let mut ws = Workspace::new();
    assert_eq!(ws.pane_count(), 1);
    let initial_id = ws.focused_tile.unwrap();
    if let Some(egui_tiles::Tile::Pane(pane)) = ws.tree.tiles.get_mut(initial_id) {
        pane.title = "Pane1".to_string();
    }

    // Split horizontally -> 2 panes, focus moves to new pane p2
    ws.split_pane(initial_id, SplitAxis::Horizontal);
    assert_eq!(ws.pane_count(), 2);
    let p2 = ws.focused_tile.unwrap();
    if let Some(egui_tiles::Tile::Pane(pane)) = ws.tree.tiles.get_mut(p2) {
        pane.title = "Pane2".to_string();
    }

    // Find the TileId of Pane1 (which was moved into the new Horizontal container)
    let p1 = ws
        .tree
        .tiles
        .iter()
        .find_map(|(id, t)| match t {
            egui_tiles::Tile::Pane(p) if p.title == "Pane1" => Some(*id),
            _ => None,
        })
        .unwrap();

    // Add tab on p2 -> creates Tabs container with p2 (moved) and p3 (active)
    ws.add_tab(p2);
    assert_eq!(ws.pane_count(), 3);
    let p3 = ws.focused_tile.unwrap();
    if let Some(egui_tiles::Tile::Pane(pane)) = ws.tree.tiles.get_mut(p3) {
        pane.title = "Pane3".to_string();
    }

    // Add another tab into the same Tabs container -> 4 panes
    ws.add_tab(p3);
    assert_eq!(ws.pane_count(), 4);
    let p4 = ws.focused_tile.unwrap();

    // Focus adjacent panes (+1 and -1 wrap around)
    ws.focus_adjacent_pane(1);
    let next_f = ws.focused_tile.unwrap();
    assert_ne!(next_f, p4);
    ws.focus_adjacent_pane(-1);
    assert_eq!(ws.focused_tile, Some(p4));

    // Swap panes p1 and p4 -> p1's content ("Pane1") moves to p4's slot and focus becomes p4
    ws.swap_panes(p1, p4);
    assert_eq!(ws.focused_tile, Some(p4));
    let swapped_title = match ws.tree.tiles.get(p4) {
        Some(egui_tiles::Tile::Pane(pane)) => pane.title.clone(),
        _ => String::new(),
    };
    assert_eq!(swapped_title, "Pane1");

    // Top-right tile calculation from recorded_rects
    ws.recorded_rects.clear();
    ws.recorded_rects.insert(
        p1,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(400.0, 300.0)),
    );
    ws.recorded_rects.insert(
        p4,
        egui::Rect::from_min_max(egui::pos2(400.0, 0.0), egui::pos2(800.0, 300.0)),
    );
    ws.update_top_right_tile();
    assert_eq!(ws.top_right_tile, Some(p4));

    // Remove focused pane p4 -> focus falls back to a valid remaining pane
    ws.remove_pane(p4);
    assert_eq!(ws.pane_count(), 3);
    assert!(ws.focused_tile.is_some());
    assert_ne!(ws.focused_tile, Some(p4));
}

#[test]
fn test_pane_zoom_clamping_and_paste_forwarding() {
    let (mut pane, written) = make_test_pane_with_recorder();
    pane.broadcast_group = Some(1);
    assert!((pane.zoom_scale - 1.0).abs() < f32::EPSILON);

    for _ in 0..30 {
        pane.zoom_in();
    }
    assert!((pane.zoom_scale - 3.5).abs() < f32::EPSILON);

    for _ in 0..40 {
        pane.zoom_out();
    }
    assert!((pane.zoom_scale - 0.4).abs() < f32::EPSILON);

    pane.zoom_reset();
    assert!((pane.zoom_scale - 1.0).abs() < f32::EPSILON);

    // 1. Unrequested Event::Paste (e.g., raw Ctrl+V when paste is Ctrl+Shift+V) sends 0x16 (^V)
    let ctx = egui::Context::default();
    let mut raw_ctrl_v = egui::RawInput::default();
    raw_ctrl_v
        .events
        .push(egui::Event::Paste("ignored".to_string()));
    ctx.run_ui(raw_ctrl_v, |ui| {
        pane.handle_keyboard_input(ui, false);
    })
    .textures_delta
    .clear();
    assert_eq!(&*written.lock().unwrap(), &[0x16]);

    // 2. Requested paste (pending_paste = true) writes the pasted text and records broadcast input
    written.lock().unwrap().clear();
    pane.pending_broadcast_input.clear();
    pane.pending_paste = true;

    let mut raw_paste = egui::RawInput::default();
    raw_paste
        .events
        .push(egui::Event::Paste("echo pasted".to_string()));
    ctx.run_ui(raw_paste, |ui| {
        pane.handle_keyboard_input(ui, false);
    })
    .textures_delta
    .clear();

    assert_eq!(&*written.lock().unwrap(), b"echo pasted");
    assert_eq!(pane.pending_broadcast_input, b"echo pasted");
}
