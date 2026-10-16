pub mod local_pty;
pub mod tmux_cc;

/// Abstract session driver for shell process IO and terminal multiplexers.
pub trait SessionDriver {
    fn read_output(&mut self) -> Vec<u8>;
    fn write_input(&mut self, data: &[u8]);
    fn resize(&mut self, rows: u16, cols: u16);
    fn is_alive(&mut self) -> bool;
}
