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
mod config;
pub mod i18n;
pub mod menus;
mod session;
pub mod ui;
mod vte;

use eframe::egui;
use ui::app::TerminApp;

fn parse_cli_args() -> (Option<u32>, bool) {
    let mut fps_override = None;
    let mut open_prefs = false;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fps" | "-f" => {
                if let Some(val) = args.next().and_then(|s| s.parse::<u32>().ok()) {
                    fps_override = Some(val.clamp(1, 240));
                }
            }
            "--prefs" => {
                open_prefs = true;
            }
            s if s.starts_with("--fps=") => {
                if let Ok(val) = s["--fps=".len()..].parse::<u32>() {
                    fps_override = Some(val.clamp(1, 240));
                }
            }
            _ => {}
        }
    }
    (fps_override, open_prefs)
}

fn main() -> Result<(), eframe::Error> {
    let (fps_override, open_prefs) = parse_cli_args();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("termin")
            .with_decorations(false)
            .with_active(true)
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([400.0, 260.0]),
        ..Default::default()
    };

    eframe::run_native(
        "termin",
        options,
        Box::new(move |cc| Ok(Box::new(TerminApp::new(cc, fps_override, open_prefs)))),
    )
}