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
pub mod vt100_adapter;

use std::num::NonZeroU16;
pub use vt100_adapter::Vt100Adapter;

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

