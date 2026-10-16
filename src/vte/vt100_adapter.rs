use crate::vte::{CursorState, RenderableCell, RgbColor, TerminalBackend};

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

    fn resize(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
        self.parser.screen_mut().set_size(rows, cols);
    }

    fn read_cell(&self, row: u16, col: u16) -> RenderableCell {
        let screen = self.parser.screen();
        if let Some(cell) = screen.cell(row, col) {
            let ch = cell.contents().chars().next().unwrap_or(' ');
            RenderableCell {
                ch,
                fg: map_vt100_color(cell.fgcolor(), RgbColor(230, 230, 230)),
                bg: map_vt100_color(cell.bgcolor(), RgbColor(18, 18, 18)),
                bold: cell.bold(),
                inverse: cell.inverse(),
            }
        } else {
            RenderableCell {
                ch: ' ',
                fg: RgbColor(230, 230, 230),
                bg: RgbColor(18, 18, 18),
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
