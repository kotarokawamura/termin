mod config;
mod session;
mod ui;
mod vte;

use eframe::egui;
use ui::app::TerminechanApp;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Terminechan")
            .with_decorations(false)
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([400.0, 260.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Terminechan",
        options,
        Box::new(|cc| Ok(Box::new(TerminechanApp::new(cc)))),
    )
}
