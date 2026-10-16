use crate::session::SessionDriver;

/// Skeleton driver for `tmux -CC` control mode integration (Priority 4).
#[allow(dead_code)]
pub struct TmuxControlSession {
    pub pane_id: String,
    alive: bool,
}

#[allow(dead_code)]
impl TmuxControlSession {
    pub fn new(pane_id: &str) -> Self {
        Self {
            pane_id: pane_id.to_string(),
            alive: true,
        }
    }
}

impl SessionDriver for TmuxControlSession {
    fn read_output(&mut self) -> Vec<u8> {
        Vec::new()
    }

    fn write_input(&mut self, _data: &[u8]) {
        // Send `send-keys -t %pane_id ...` to the tmux -CC control stream
    }

    fn resize(&mut self, _rows: u16, _cols: u16) {
        // Send `refresh-client -C ...` to the tmux -CC control stream
    }

    fn is_alive(&mut self) -> bool {
        self.alive
    }
}
