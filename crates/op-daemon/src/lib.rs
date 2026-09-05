mod control;
mod home;
mod serve;

use std::time::{SystemTime, UNIX_EPOCH};

pub use control::{Control, Started, StopOutcome};
pub use home::Home;
pub use op_api::DaemonInfo;
pub use serve::{SERVE_ARG, serve, serve_if_requested, serve_request};

pub const DEFAULT_PORT: u16 = 7373;

// The port the daemon binds unless told otherwise. A write brings the daemon up itself, with no
// `--port` to carry, so the override has to be reachable from the environment too.
pub fn default_port() -> u16 {
    std::env::var("OPENPLAN_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

pub fn base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// A live daemon answers /health with its own identity; requiring the pid to match daemon.json
// rejects a stale record whose port was recycled by an unrelated service.
pub fn serves(client: &op_client::Client, info: &DaemonInfo) -> bool {
    client
        .health(&base_url(info.port))
        .is_some_and(|live| live.pid == info.pid)
}

pub fn serving(home: &Home, client: &op_client::Client) -> Option<DaemonInfo> {
    let info = home.read_info()?;
    serves(client, &info).then_some(info)
}
