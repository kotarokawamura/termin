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
use crate::vte::{CursorState, RenderableCell, RgbColor};
use std::num::NonZeroU16;

const DEFAULT_FG: RgbColor = RgbColor(230, 230, 230);
const DEFAULT_BG: RgbColor = RgbColor(18, 18, 18);
const SCROLLBACK_LEN: usize = 5000;

pub struct Vt100Adapter {
    parser: vt100::Parser,
    rows: NonZeroU16,
    cols: NonZeroU16,
    max_scrollback: usize,
    generation: u64,
}

impl Vt100Adapter {
    pub fn new(rows: core::num::NonZeroU16, cols: core::num::NonZeroU16) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, SCROLLBACK_LEN),
            rows,
            cols,
            max_scrollback: 0,
            generation: 0,
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    fn refresh_max_scrollback(&mut self) {
        let current = self.parser.screen().scrollback();
        self.parser.screen_mut().set_scrollback(usize::MAX);
        self.max_scrollback = self.parser.screen().scrollback();
        self.parser
            .screen_mut()
            .set_scrollback(current.min(self.max_scrollback));
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.parser.process(bytes);
        self.generation = self.generation.wrapping_add(1);
        let cursor_at_bottom = self.parser.screen().cursor_position().0 + 1 >= self.rows.get();
        if cursor_at_bottom || bytes.contains(&b'\n') || bytes.contains(&0x1b) {
            self.refresh_max_scrollback();
        }
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.process_bytes(bytes);
    }

    pub fn resize(&mut self, new_rows: core::num::NonZeroU16, new_cols: core::num::NonZeroU16) {
        if self.rows.get() == new_rows.get() && self.cols.get() == new_cols.get() {
            return;
        }

        // Skip reflow if the alternate screen is active (e.g., vim, htop)
        if self.parser.screen().alternate_screen() {
            self.rows = new_rows;
            self.cols = new_cols;
            self.parser.screen_mut().set_size(new_rows, new_cols);
            self.refresh_max_scrollback();
            self.generation = self.generation.wrapping_add(1);
            return;
        }

        let prev_scrollback = self.parser.screen().scrollback();
        self.refresh_max_scrollback();

        // 1. Extract logical lines (including scrollback history + live screen, joining wrapped rows)
        let logical_lines = extract_all_logical_lines(
            self.parser.screen_mut(),
            self.rows,
            self.cols,
            self.max_scrollback,
        );

        // 2. Create a new parser with the target dimensions and replay the logical lines
        //    so that vt100 automatically wraps them to the new column width
        let mut new_parser = vt100::Parser::new(new_rows, new_cols, SCROLLBACK_LEN);
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
        self.refresh_max_scrollback();
        if prev_scrollback > 0 {
            self.parser
                .screen_mut()
                .set_scrollback(prev_scrollback.min(self.max_scrollback));
        }
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn read_cell(&self, row: u16, col: u16) -> RenderableCell {
        let screen = self.parser.screen();
        if let Some(cell) = screen.cell(row, col) {
            let wide_continuation = cell.is_wide_continuation();
            let ch = if wide_continuation {
                ' '
            } else {
                cell.contents().chars().next().unwrap_or(' ')
            };
            RenderableCell {
                ch,
                fg: map_vt100_color(cell.fgcolor(), DEFAULT_FG),
                bg: map_vt100_color(cell.bgcolor(), DEFAULT_BG),
                bold: cell.bold(),
                underline: cell.underline(),
                inverse: cell.inverse(),
                wide: cell.is_wide(),
                wide_continuation,
            }
        } else {
            RenderableCell {
                ch: ' ',
                fg: DEFAULT_FG,
                bg: DEFAULT_BG,
                bold: false,
                underline: false,
                inverse: false,
                wide: false,
                wide_continuation: false,
            }
        }
    }

    pub fn size(&self) -> (NonZeroU16, NonZeroU16) {
        (self.rows, self.cols)
    }

    pub fn cursor(&self) -> CursorState {
        let screen = self.parser.screen();
        let (row, col) = screen.cursor_position();
        CursorState {
            row,
            col,
            visible: !screen.hide_cursor() && screen.scrollback() == 0,
        }
    }

    pub fn scroll(&mut self, delta_rows: isize) {
        let current = self.parser.screen().scrollback();
        let next = if delta_rows >= 0 {
            current.saturating_add(delta_rows as usize)
        } else {
            current.saturating_sub((-delta_rows) as usize)
        };
        self.parser
            .screen_mut()
            .set_scrollback(next.min(self.max_scrollback));
    }

    pub fn scroll_offset(&self) -> usize {
        self.parser.screen().scrollback()
    }

    pub fn max_scroll_offset(&self) -> usize {
        self.max_scrollback
    }

    pub fn set_scroll_offset(&mut self, offset: usize) {
        self.parser
            .screen_mut()
            .set_scrollback(offset.min(self.max_scrollback));
    }

    pub fn clear_scrollback(&mut self) {
        self.parser.screen_mut().set_scrollback(0);
        let logical_lines = extract_all_logical_lines(self.parser.screen_mut(), self.rows, self.cols, 0);
        let mut new_parser = vt100::Parser::new(self.rows, self.cols, SCROLLBACK_LEN);
        let mut buf = Vec::new();
        for (i, line) in logical_lines.iter().enumerate() {
            if i > 0 {
                buf.extend_from_slice(b"\r\n");
            }
            serialize_cells_to_ansi(line, &mut buf);
        }
        new_parser.process(&buf);
        self.parser = new_parser;
        self.max_scrollback = 0;
        self.generation = self.generation.wrapping_add(1);
    }
}

#[derive(Clone, PartialEq)]
struct SavedCell {
    contents: String,
    fg: vt100::Color,
    bg: vt100::Color,
    bold: bool,
    underline: bool,
    inverse: bool,
    wide_continuation: bool,
}

fn append_physical_row(
    screen: &vt100::Screen,
    r: u16,
    cols: NonZeroU16,
    preserve_up_to_col: Option<u16>,
    current_line: &mut Vec<SavedCell>,
    logical_lines: &mut Vec<Vec<SavedCell>>,
) {
    let mut row_cells = Vec::with_capacity(cols.get() as usize);
    for c in 0..cols.get() {
        if let Some(cell) = screen.cell(r, c) {
            let is_cont = cell.is_wide_continuation();
            let s = cell.contents();
            row_cells.push(SavedCell {
                contents: if is_cont {
                    String::new()
                } else if s.is_empty() {
                    " ".to_string()
                } else {
                    s.to_string()
                },
                fg: cell.fgcolor(),
                bg: cell.bgcolor(),
                bold: cell.bold(),
                underline: cell.underline(),
                inverse: cell.inverse(),
                wide_continuation: is_cont,
            });
        }
    }

    let is_wrapped = screen.row_wrapped(r);

    if is_wrapped {
        current_line.extend(row_cells.into_iter().filter(|c| !c.wide_continuation));
    } else {
        let mut end = row_cells.len();
        while end > 0 {
            let cell = &row_cells[end - 1];
            let is_blank = !cell.wide_continuation
                && cell.contents == " "
                && cell.bg == vt100::Color::Default
                && !cell.inverse;
            if let Some(min_col) = preserve_up_to_col {
                if (end as u16) <= min_col {
                    break;
                }
            }
            if is_blank {
                end -= 1;
            } else {
                break;
            }
        }
        current_line.extend(
            row_cells
                .into_iter()
                .take(end)
                .filter(|c| !c.wide_continuation),
        );
        logical_lines.push(std::mem::take(current_line));
    }
}

/// Reads cells from scrollback history and the live screen, returning a list of logical lines
/// by joining physical rows that were soft-wrapped at the right margin.
fn extract_all_logical_lines(
    screen: &mut vt100::Screen,
    rows: NonZeroU16,
    cols: NonZeroU16,
    max_scrollback: usize,
) -> Vec<Vec<SavedCell>> {
    screen.set_scrollback(0);
    let (cursor_row, cursor_col) = screen.cursor_position();

    // Find the last row containing non-empty content on the live screen
    let mut last_active_row = cursor_row;
    for r in (0..rows.get()).rev() {
        let has_content = (0..cols.get()).any(|c| {
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

    // 1. Extract scrollback history rows from oldest (max_scrollback) to newest (1)
    for offset in (1..=max_scrollback).rev() {
        screen.set_scrollback(offset);
        append_physical_row(
            screen,
            0,
            cols,
            None,
            &mut current_line,
            &mut logical_lines,
        );
    }

    // 2. Extract live screen rows (0..=last_active_row)
    screen.set_scrollback(0);
    for r in 0..=last_active_row {
        let preserve_col = if r == cursor_row {
            Some(cursor_col)
        } else {
            None
        };
        append_physical_row(
            screen,
            r,
            cols,
            preserve_col,
            &mut current_line,
            &mut logical_lines,
        );
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
    let mut cur_underline = false;
    let mut cur_inverse = false;

    for cell in cells {
        if cell.wide_continuation {
            continue;
        }
        if cell.fg != cur_fg
            || cell.bg != cur_bg
            || cell.bold != cur_bold
            || cell.underline != cur_underline
            || cell.inverse != cur_inverse
        {
            out.extend_from_slice(b"\x1b[0");
            if cell.bold {
                out.extend_from_slice(b";1");
            }
            if cell.underline {
                out.extend_from_slice(b";4");
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
            cur_underline = cell.underline;
            cur_inverse = cell.inverse;
        }
        out.extend_from_slice(cell.contents.as_bytes());
    }

    if cur_fg != vt100::Color::Default
        || cur_bg != vt100::Color::Default
        || cur_bold
        || cur_underline
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

/// Converts a full 8-bit xterm-256color index (0..=255) into `RgbColor`.
fn ansi_idx_to_rgb(idx: u8) -> RgbColor {
    match idx {
        // 16 standard + bright ANSI colors
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
        15 => RgbColor(255, 255, 255),
        // 6x6x6 216-color cube (indices 16..=231)
        16..=231 => {
            let cube = idx - 16;
            let r_idx = cube / 36;
            let g_idx = (cube % 36) / 6;
            let b_idx = cube % 6;
            let to_val = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            RgbColor(to_val(r_idx), to_val(g_idx), to_val(b_idx))
        }
        // 24-step grayscale ramp (indices 232..=255)
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            RgbColor(gray, gray, gray)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_256_color_mapping() {
        assert_eq!(ansi_idx_to_rgb(15), RgbColor(255, 255, 255));
        // Index 16 is (0, 0, 0) in the 6x6x6 cube
        assert_eq!(ansi_idx_to_rgb(16), RgbColor(0, 0, 0));
        // Index 196 is pure red (5, 0, 0) -> (255, 0, 0)
        assert_eq!(ansi_idx_to_rgb(196), RgbColor(255, 0, 0));
        // Index 232 is darkest grayscale (8, 8, 8)
        assert_eq!(ansi_idx_to_rgb(232), RgbColor(8, 8, 8));
        // Index 255 is lightest grayscale (238, 238, 238)
        assert_eq!(ansi_idx_to_rgb(255), RgbColor(238, 238, 238));
    }

    fn nz(v: u16) -> NonZeroU16 {
        NonZeroU16::new(v).unwrap()
    }

    #[test]
    fn test_wide_char_resize_does_not_insert_extra_spaces() {
        let mut adapter = Vt100Adapter::new(nz(10), nz(40));
        adapter.process_bytes("日本語テスト".as_bytes());
        adapter.resize(nz(10), nz(50));
        adapter.resize(nz(10), nz(40));

        assert_eq!(adapter.read_cell(0, 0).ch, '日');
        assert!(adapter.read_cell(0, 1).wide_continuation);
        assert_eq!(adapter.read_cell(0, 2).ch, '本');
        assert!(adapter.read_cell(0, 3).wide_continuation);
        assert_eq!(adapter.read_cell(0, 4).ch, '語');
    }

    #[test]
    fn test_scrollback_preserved_across_resize() {
        let mut adapter = Vt100Adapter::new(nz(5), nz(40));
        for i in 1..=20 {
            adapter.process_bytes(format!("line {:02}\r\n", i).as_bytes());
        }
        assert!(adapter.max_scroll_offset() > 0);
        let initial_max = adapter.max_scroll_offset();

        adapter.resize(nz(5), nz(50));
        assert_eq!(adapter.max_scroll_offset(), initial_max);

        adapter.set_scroll_offset(initial_max);
        assert_eq!(adapter.scroll_offset(), initial_max);
        assert_eq!(adapter.read_cell(0, 0).ch, 'l');
        assert_eq!(adapter.read_cell(0, 5).ch, '0');
        assert_eq!(adapter.read_cell(0, 6).ch, '1');
    }
}
