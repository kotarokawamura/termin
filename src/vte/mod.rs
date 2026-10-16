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
    pub inverse: bool,
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
}
