use std::process::ExitCode;

const USAGE: &str = "\
A desktop window on the openplan daemon's web UI.

Usage: openplan-gui [OPTIONS]

Options:
  -h, --help     Print this help
  -V, --version  Print the version";

fn main() -> ExitCode {
    if let Some(code) = op_daemon::serve_if_requested(std::env::args()) {
        return code;
    }
    match std::env::args().nth(1).as_deref() {
        Some("-h" | "--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            println!("openplan-gui {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => openplan_gui::run(),
    }
}
