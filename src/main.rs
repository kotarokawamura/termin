mod config;
mod session;
pub mod ui;
mod vte;

use eframe::egui;
use ui::app::TerminApp;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("termin")
            .with_decorations(false)
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([400.0, 260.0]),
        ..Default::default()
    };

    eframe::run_native(
        "termin",
        options,
        Box::new(|cc| Ok(Box::new(TerminApp::new(cc)))),
    )
}
