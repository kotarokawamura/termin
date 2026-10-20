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
pub mod my_custom_vte;
pub mod vt100_adapter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RgbColor(pub u8, pub u8, pub u8);

#[derive(Clone, Debug)]
pub struct RenderableCell {
    pub ch: char,
    pub fg: RgbColor,
    pub bg: RgbColor,
    pub bold: bool,
    pub underline: bool,
    pub inverse: bool,
    pub wide: bool,
    pub wide_continuation: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct CursorState {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}

/// Abstract VTE engine boundary.
/// Swap between `Vt100Adapter`, `MyCustomVte`, or `alacritty_terminal` without touching UI code.
pub trait TerminalBackend: Send {
    fn process_bytes(&mut self, bytes: &[u8]);
    fn resize(&mut self, rows: u16, cols: u16);
    fn read_cell(&self, row: u16, col: u16) -> RenderableCell;
    fn size(&self) -> (u16, u16);
    fn cursor(&self) -> CursorState;

    /// Adjust scrollback offset (positive = scroll up into history, negative = scroll down to live).
    fn scroll(&mut self, _delta_rows: isize) {}

    /// Current scrollback offset in rows (0 = viewing live bottom screen).
    fn scroll_offset(&self) -> usize {
        0
    }

    /// Maximum available scrollback offset in rows (total history lines above the live screen).
    fn max_scroll_offset(&self) -> usize {
        0
    }

    /// Set absolute scrollback offset in rows (0 = live bottom, max_scroll_offset = top of history).
    fn set_scroll_offset(&mut self, _offset: usize) {}

    /// Clear scrollback history buffer.
    fn clear_scrollback(&mut self) {}
}
