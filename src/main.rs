// Copyright (C) [2026/10/19] kotaro,termin
// (ライセンスヘッダ省略)
mod config;
pub mod i18n;
pub mod menus;
mod session;
pub mod ui;
mod vte;

use std::path::PathBuf;
use eframe::egui;
use ui::app::{AppStartupOptions, TerminApp};
use ui::scheduler::PollMode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliOptions {
    pub fps_override: Option<u32>,
    pub poll_mode: PollMode,
    pub open_prefs: bool,
    pub maximize: bool,
    pub show_version: bool,
    pub show_help: bool,
    pub debug_level: u8,
    pub config_path: Option<PathBuf>,
    pub config_json: Option<String>,
    pub startup_command: Option<String>,
}

fn shell_quote_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if arg
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '\'' | '"' | '\\' | '$' | '`' | ';' | '&' | '|'))
    {
        format!("'{}'", arg.replace('\'', "'\\''"))
    } else {
        arg.to_string()
    }
}

fn join_command_args(args: &[String]) -> Option<String> {
    match args.len() {
        0 => None,
        1 => {
            if args[0].is_empty() {
                None
            } else {
                Some(args[0].clone())
            }
        }
        _ => Some(
            args.iter()
                .map(|a| shell_quote_arg(a))
                .collect::<Vec<_>>()
                .join(" "),
        ),
    }
}

pub fn help_text() -> String {
    format!(
        "\
Usage: {bin} [OPTIONS]

Options:
  -h, --help                    show this help message and exit
  -v, --version                 Display program version
  -M, --maximize                Maximize the window
  -m, --maximise                Maximize the window
  -d, --debug                   Enable debugging information (twice for debug server)
  -g, --config CONFIG           Specify a config file
  -j, --config-json CONFIGJSON  Specify configuration via JSON string or file
  -e, --command ...             Execute the command passed after the argument on startup
  -x, --execute ...             Use the rest of the command line as a command to
                                execute inside the terminal, and its arguments
  -f, --fps FPS                 Override target FPS (1..=240)
      --mode MODE               Redraw mode: 'event' (default) or 'poll'
      --event, --event-driven   Use event-driven redraw mode
      --poll, --continuous      Use continuous FPS polling mode
      --prefs                   Open Preferences window on startup",
        bin = env!("CARGO_PKG_NAME")
    )
}

pub fn version_text() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

pub fn parse_cli_args() -> CliOptions {
    parse_cli_args_from(std::env::args().skip(1))
}

pub fn parse_cli_args_from<I, S>(iter: I) -> CliOptions
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut fps_override = None;
    let mut explicit_mode = None;
    let mut open_prefs = false;
    let mut maximize = false;
    let mut show_version = false;
    let mut show_help = false;
    let mut debug_level: u8 = 0;
    let mut config_path = None;
    let mut config_json = None;
    let mut startup_command = None;

    let mut args = iter.into_iter().map(Into::into);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                show_help = true;
            }
            "-v" | "--version" => {
                show_version = true;
            }
            "-M" | "--maximize" | "-m" | "--maximise" => {
                maximize = true;
            }
            "-d" | "--debug" => {
                debug_level = debug_level.saturating_add(1);
            }
            s if s.starts_with("-d") && s.chars().skip(1).all(|c| c == 'd') => {
                let count = (s.len() - 1).min(u8::MAX as usize) as u8;
                debug_level = debug_level.saturating_add(count);
            }
            "-g" | "--config" => {
                if let Some(val) = args.next() {
                    config_path = Some(PathBuf::from(val));
                }
            }
            s if s.starts_with("--config=") => {
                let val = &s["--config=".len()..];
                if !val.is_empty() {
                    config_path = Some(PathBuf::from(val));
                }
            }
            "-j" | "--config-json" => {
                if let Some(val) = args.next() {
                    config_json = Some(val);
                }
            }
            s if s.starts_with("--config-json=") => {
                let val = &s["--config-json=".len()..];
                if !val.is_empty() {
                    config_json = Some(val.to_string());
                }
            }
            "-e" | "--command" | "-x" | "--execute" => {
                let rest: Vec<String> = args.collect();
                startup_command = join_command_args(&rest);
                break;
            }
            s if s.starts_with("--command=") => {
                let val = &s["--command=".len()..];
                if !val.is_empty() {
                    startup_command = Some(val.to_string());
                }
            }
            s if s.starts_with("--execute=") => {
                let val = &s["--execute=".len()..];
                if !val.is_empty() {
                    startup_command = Some(val.to_string());
                }
            }
            "--fps" | "-f" => {
                if let Some(val) = args.next().and_then(|s| s.parse::<u32>().ok()) {
                    fps_override = Some(val.clamp(1, 240));
                }
            }
            "--mode" => {
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
        maximize,
        show_version,
        show_help,
        debug_level,
        config_path,
        config_json,
        startup_command,
    }
}

fn start_debug_server(cli_summary: String) {
    let bind_addr =
        std::env::var("TERMIN_DEBUG_ADDR").unwrap_or_else(|_| "127.0.0.1:0".to_string());
    match std::net::TcpListener::bind(&bind_addr) {
        Ok(listener) => {
            if let Ok(local_addr) = listener.local_addr() {
                eprintln!("[termin][debug] Debug server listening on {local_addr}");
            }
            std::thread::Builder::new()
                .name("termin-debug-server".to_string())
                .spawn(move || {
                    use std::io::Write as _;
                    for stream in listener.incoming().flatten() {
                        let mut stream = stream;
                        let body = format!(
                            "{{\"program\":\"{}\",\"version\":\"{}\",\"pid\":{},\"cli\":{:?}}}\n",
                            env!("CARGO_PKG_NAME"),
                            env!("CARGO_PKG_VERSION"),
                            std::process::id(),
                            cli_summary
                        );
                        let _ = stream.write_all(body.as_bytes());
                        let _ = stream.flush();
                    }
                })
                .ok();
        }
        Err(err) => {
            eprintln!("[termin][debug] Failed to bind debug server on {bind_addr}: {err}");
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let cli = parse_cli_args();

    if cli.show_help {
        println!("{}", help_text());
        return Ok(());
    }

    if cli.show_version {
        println!("{}", version_text());
        return Ok(());
    }

    if cli.debug_level >= 1 {
        eprintln!("[termin][debug] Debug logging enabled (level={})", cli.debug_level);
        eprintln!("[termin][debug] CLI options: {:?}", cli);
    }
    if cli.debug_level >= 2 {
        start_debug_server(format!("{:?}", cli));
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("termin")
            .with_decorations(false)
            .with_active(true)
            .with_maximized(cli.maximize)
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([400.0, 260.0]),
        ..Default::default()
    };

    let startup_options = AppStartupOptions {
        fps_override: cli.fps_override,
        poll_mode: Some(cli.poll_mode),
        open_prefs_on_startup: cli.open_prefs,
        maximize: cli.maximize,
        debug_level: cli.debug_level,
        config_path: cli.config_path,
        config_json: cli.config_json,
        startup_command: cli.startup_command,
    };

    eframe::run_native(
        "termin",
        options,
        Box::new(move |cc| Ok(Box::new(TerminApp::new_with_options(cc, startup_options)))),
    )
}

#[cfg(test)]
#[path = "test/main_tests.rs"]
mod tests;