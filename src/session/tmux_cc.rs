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

    fn resize(&mut self, _rows: core::num::NonZeroU16, _cols: core::num::NonZeroU16) {
        // Send `refresh-client -C ...` to the tmux -CC control stream
    }

    fn is_alive(&mut self) -> bool {
        self.alive
    }
}
