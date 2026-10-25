// Copyright (C) [2026/10/19] kotaro,termin
// (ライセンスヘッダ省略)
mod config;
pub mod i18n;
pub mod menus;
mod session;
pub mod ui;
mod vte;

use eframe::egui;
use ui::app::TerminApp;
use ui::scheduler::PollMode;

pub struct CliOptions {
    pub fps_override: Option<u32>,
    pub poll_mode: PollMode,
    pub open_prefs: bool,
}

fn parse_cli_args() -> CliOptions {
    let mut fps_override = None;
    let mut explicit_mode = None;
    let mut open_prefs = false;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fps" | "-f" => {
                if let Some(val) = args.next().and_then(|s| s.parse::<u32>().ok()) {
                    fps_override = Some(val.clamp(1, 240));
                }
            }
            "--mode" | "-m" => {
                if let Some(val) = args.next().and_then(|s| PollMode::from_cli_str(&s)) {
                    explicit_mode = Some(val);
                }
            }
            "--event" | "--event-driven" => {
                explicit_mode = Some(PollMode::EventDriven);
            }
            "--poll" | "--continuous" => {
                explicit_mode = Some(PollMode::ContinuousFps);
            }
            "--prefs" => {
                open_prefs = true;
            }
            s if s.starts_with("--fps=") => {
                if let Ok(val) = s["--fps=".len()..].parse::<u32>() {
                    fps_override = Some(val.clamp(1, 240));
                }
            }
            s if s.starts_with("--mode=") => {
                if let Some(val) = PollMode::from_cli_str(&s["--mode=".len()..]) {
                    explicit_mode = Some(val);
                }
            }
            _ => {}
        }
    }

    // --mode が明示されていればそれに従い、未指定で --fps が指定された場合は ContinuousFps にする
    let poll_mode = explicit_mode.unwrap_or_else(|| {
        if fps_override.is_some() {
            PollMode::ContinuousFps
        } else {
            PollMode::EventDriven
        }
    });

    CliOptions {
        fps_override,
        poll_mode,
        open_prefs,
    }
}

fn main() -> Result<(), eframe::Error> {
    let cli = parse_cli_args();

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
        Box::new(move |cc| {
            Ok(Box::new(TerminApp::new(
                cc,
                cli.fps_override,
                cli.poll_mode,
                cli.open_prefs,
            )))
        }),
    )
}