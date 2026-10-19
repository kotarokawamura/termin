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

const DEFAULT_FG: RgbColor = RgbColor(230, 230, 230);
const DEFAULT_BG: RgbColor = RgbColor(18, 18, 18);

pub struct Vt100Adapter {
    parser: vt100::Parser,
    rows: u16,
    cols: u16,
}

impl Vt100Adapter {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, 2000),
            rows,
            cols,
        }
    }
}

impl TerminalBackend for Vt100Adapter {
    fn process_bytes(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    fn resize(&mut self, new_rows: u16, new_cols: u16) {
        if self.rows == new_rows && self.cols == new_cols {
            return;
        }

        // Skip reflow if the alternate screen is active (e.g., vim, htop)
        if self.parser.screen().alternate_screen() {
            self.rows = new_rows;
            self.cols = new_cols;
            self.parser.screen_mut().set_size(new_rows, new_cols);
            return;
        }

        // 1. Extract logical lines (joining wrapped physical lines) from the current screen
        let logical_lines = extract_logical_lines(self.parser.screen(), self.rows, self.cols);

        // 2. Create a new parser with the target dimensions and replay the logical lines
        //    so that vt100 automatically wraps them to the new column width
        let mut new_parser = vt100::Parser::new(new_rows, new_cols, 2000);
        let mut buf = Vec::new();

        for (i, line) in logical_lines.iter().enumerate() {
            if i > 0 {
                buf.extend_from_slice(b"\r\n");
            }
            serialize_cells_to_ansi(line, &mut buf);
        }

        new_parser.process(&buf);

        self.parser = new_parser;
        self.rows = new_rows;
        self.cols = new_cols;
    }

    fn read_cell(&self, row: u16, col: u16) -> RenderableCell {
        let screen = self.parser.screen();
        if let Some(cell) = screen.cell(row, col) {
            let ch = cell.contents().chars().next().unwrap_or(' ');
            RenderableCell {
                ch,
                fg: map_vt100_color(cell.fgcolor(), DEFAULT_FG),
                bg: map_vt100_color(cell.bgcolor(), DEFAULT_BG),
                bold: cell.bold(),
                inverse: cell.inverse(),
            }
        } else {
            RenderableCell {
                ch: ' ',
                fg: DEFAULT_FG,
                bg: DEFAULT_BG,
                bold: false,
                inverse: false,
            }
        }
    }

    fn size(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    fn cursor(&self) -> CursorState {
        let screen = self.parser.screen();
        let (row, col) = screen.cursor_position();
        CursorState {
            row,
            col,
            visible: !screen.hide_cursor(),
        }
    }
}

#[derive(Clone, PartialEq)]
struct SavedCell {
    contents: String,
    fg: vt100::Color,
    bg: vt100::Color,
    bold: bool,
    inverse: bool,
}

/// Reads cells from the current screen and returns a list of logical lines
/// by joining physical rows that were soft-wrapped at the right margin.
fn extract_logical_lines(
    screen: &vt100::Screen,
    rows: u16,
    cols: u16,
) -> Vec<Vec<SavedCell>> {
    let (cursor_row, cursor_col) = screen.cursor_position();

    // Find the last row containing non-empty content
    let mut last_active_row = cursor_row;
    for r in (0..rows).rev() {
        let has_content = (0..cols).any(|c| {
            screen
                .cell(r, c)
                .map(|cell| !cell.contents().is_empty() && cell.contents() != " ")
                .unwrap_or(false)
        });
        if has_content {
            last_active_row = last_active_row.max(r);
            break;
        }
    }

    let mut logical_lines: Vec<Vec<SavedCell>> = Vec::new();
    let mut current_line: Vec<SavedCell> = Vec::new();

    for r in 0..=last_active_row {
        let mut row_cells = Vec::with_capacity(cols as usize);
        for c in 0..cols {
            if let Some(cell) = screen.cell(r, c) {
                let s = cell.contents();
                row_cells.push(SavedCell {
                    contents: if s.is_empty() { " ".to_string() } else { s.to_string() },
                    fg: cell.fgcolor(),
                    bg: cell.bgcolor(),
                    bold: cell.bold(),
                    inverse: cell.inverse(),
                });
            }
        }

        // Check whether this row was soft-wrapped at the right edge
        let is_wrapped = screen.row_wrapped(r);

        if is_wrapped {
            // Keep trailing spaces on wrapped rows and concatenate with the next row
            current_line.extend(row_cells);
        } else {
            // Trim trailing default-background spaces on hard-newline rows
            let mut end = row_cells.len();
            while end > 0 {
                let cell = &row_cells[end - 1];
                let is_blank = cell.contents == " "
                    && cell.bg == vt100::Color::Default
                    && !cell.inverse;
                // Preserve spaces up to the cursor position on the active cursor row
                if r == cursor_row && (end as u16) <= cursor_col {
                    break;
                }
                if is_blank {
                    end -= 1;
                } else {
                    break;
                }
            }
            current_line.extend(row_cells.into_iter().take(end));
            logical_lines.push(std::mem::take(&mut current_line));
        }
    }

    if !current_line.is_empty() {
        logical_lines.push(current_line);
    }

    logical_lines
}

/// Serializes a slice of cells into an ANSI-escaped byte stream with colors and attributes.
fn serialize_cells_to_ansi(cells: &[SavedCell], out: &mut Vec<u8>) {
    let mut cur_fg = vt100::Color::Default;
    let mut cur_bg = vt100::Color::Default;
    let mut cur_bold = false;
    let mut cur_inverse = false;

    for cell in cells {
        if cell.fg != cur_fg || cell.bg != cur_bg || cell.bold != cur_bold || cell.inverse != cur_inverse {
            out.extend_from_slice(b"\x1b[0");
            if cell.bold {
                out.extend_from_slice(b";1");
            }
            if cell.inverse {
                out.extend_from_slice(b";7");
            }
            match cell.fg {
                vt100::Color::Default => {}
                vt100::Color::Idx(i) => {
                    out.extend_from_slice(format!(";38;5;{}", i).as_bytes());
                }
                vt100::Color::Rgb(r, g, b) => {
                    out.extend_from_slice(format!(";38;2;{};{};{}", r, g, b).as_bytes());
                }
            }
            match cell.bg {
                vt100::Color::Default => {}
                vt100::Color::Idx(i) => {
                    out.extend_from_slice(format!(";48;5;{}", i).as_bytes());
                }
                vt100::Color::Rgb(r, g, b) => {
                    out.extend_from_slice(format!(";48;2;{};{};{}", r, g, b).as_bytes());
                }
            }
            out.push(b'm');

            cur_fg = cell.fg;
            cur_bg = cell.bg;
            cur_bold = cell.bold;
            cur_inverse = cell.inverse;
        }
        out.extend_from_slice(cell.contents.as_bytes());
    }

    if cur_fg != vt100::Color::Default
        || cur_bg != vt100::Color::Default
        || cur_bold
        || cur_inverse
    {
        out.extend_from_slice(b"\x1b[0m");
    }
}

fn map_vt100_color(c: vt100::Color, default_rgb: RgbColor) -> RgbColor {
    match c {
        vt100::Color::Default => default_rgb,
        vt100::Color::Idx(idx) => ansi_idx_to_rgb(idx),
        vt100::Color::Rgb(r, g, b) => RgbColor(r, g, b),
    }
}

fn ansi_idx_to_rgb(idx: u8) -> RgbColor {
    match idx {
        0 => RgbColor(40, 44, 52),
        1 => RgbColor(224, 108, 117),
        2 => RgbColor(152, 195, 121),
        3 => RgbColor(229, 192, 123),
        4 => RgbColor(97, 175, 239),
        5 => RgbColor(198, 120, 221),
        6 => RgbColor(86, 182, 194),
        7 => RgbColor(220, 223, 228),
        8 => RgbColor(92, 99, 112),
        9 => RgbColor(244, 128, 137),
        10 => RgbColor(172, 215, 141),
        11 => RgbColor(249, 212, 143),
        12 => RgbColor(117, 195, 255),
        13 => RgbColor(218, 140, 241),
        14 => RgbColor(106, 202, 214),
        _ => RgbColor(255, 255, 255),
    }
}