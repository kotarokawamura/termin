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
pub mod local_pty;
pub mod tmux_cc;

/// Abstract session driver for shell process IO and terminal multiplexers.
pub trait SessionDriver {
    fn read_output(&mut self) -> Vec<u8>;
    fn write_input(&mut self, data: &[u8]);
    fn resize(&mut self, rows: core::num::NonZeroU16, cols: core::num::NonZeroU16);
    fn is_alive(&mut self) -> bool;
}
