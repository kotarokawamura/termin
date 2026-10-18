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
use serde::{Deserialize, Serialize};
use std::env;

/// Serializable layout preset structure for saving and restoring workspace splits.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[warn(dead_code)]
pub enum LayoutNode {
    Pane {
        title: String,
        command: Option<String>,
    },
    HorizontalSplit {
        children: Vec<LayoutNode>,
    },
    VerticalSplit {
        children: Vec<LayoutNode>,
    },
}



impl Default for LayoutNode {
    fn default() -> Self {
        Self::Pane {
            title: env::var("SHELL").unwrap_or_else(|_| "Terminal 1".to_string()).to_string(),
            command: None,
        }
    }

}
