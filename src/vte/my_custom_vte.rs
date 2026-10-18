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
use crate::vte::{CursorState, RenderableCell, RgbColor, TerminalBackend};

/// Minimal from-scratch VTE state machine for learning escape sequence parsing (Priority 3).
#[allow(dead_code)]
pub struct MyCustomVte {
    rows: u16,
    cols: u16,
    cursor_row: u16,
    cursor_col: u16,
    grid: Vec<Vec<char>>,
}

#[allow(dead_code)]
impl MyCustomVte {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            rows,
            cols,
            cursor_row: 0,
            cursor_col: 0,
            grid: vec![vec![' '; cols as usize]; rows as usize],
        }
    }

    fn newline(&mut self) {
        if self.cursor_row + 1 < self.rows {
            self.cursor_row += 1;
        } else {
            self.grid.remove(0);
            self.grid.push(vec![' '; self.cols as usize]);
        }
    }
}

impl TerminalBackend for MyCustomVte {
    fn process_bytes(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        for ch in text.chars() {
            match ch {
                '\r' => self.cursor_col = 0,
                '\n' => self.newline(),
                '\x08' => {
                    if self.cursor_col > 0 {
                        self.cursor_col -= 1;
                    }
                }
                c if !c.is_control() => {
                    if self.cursor_col >= self.cols {
                        self.cursor_col = 0;
                        self.newline();
                    }
                    self.grid[self.cursor_row as usize][self.cursor_col as usize] = c;
                    self.cursor_col += 1;
                }
                _ => {}
            }
        }
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
        self.grid.resize(rows as usize, vec![' '; cols as usize]);
        for row in &mut self.grid {
            row.resize(cols as usize, ' ');
        }
        self.cursor_row = self.cursor_row.min(rows.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(cols.saturating_sub(1));
    }

    fn read_cell(&self, row: u16, col: u16) -> RenderableCell {
        let ch = self
            .grid
            .get(row as usize)
            .and_then(|r| r.get(col as usize))
            .copied()
            .unwrap_or(' ');

        RenderableCell {
            ch,
            fg: RgbColor(230, 230, 230),
            bg: RgbColor(18, 18, 18),
            bold: false,
            inverse: false,
        }
    }

    fn size(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    fn cursor(&self) -> CursorState {
        CursorState {
            row: self.cursor_row,
            col: self.cursor_col,
            visible: true,
        }
    }
}
