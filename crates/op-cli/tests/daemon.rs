use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

struct Daemon {
    home: TempDir,
    root: TempDir,
}

impl Daemon {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".plan/tasks")).unwrap();
        Self { home, root }
    }

    fn home_path(&self) -> &Path {
        self.home.path()
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_oplan"));
        cmd.env("OPLAN_HOME", self.home.path())
            .arg("--root")
            .arg(self.root.path());
        cmd
    }

    fn info_pid(&self) -> Option<u32> {
        self.info_field("pid").map(|n| n as u32)
    }

    fn info_port(&self) -> Option<u16> {
        self.info_field("port").map(|n| n as u16)
    }

    fn info_field(&self, key: &str) -> Option<u64> {
        let text = std::fs::read_to_string(self.home.path().join("daemon.json")).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        value.get(key)?.as_u64()
    }

    fn set_recorded_pid(&self, pid: u32) {
        let path = self.home.path().join("daemon.json");
        let text = std::fs::read_to_string(&path).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["pid"] = serde_json::json!(pid);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.cmd().args(["server", "stop"]).output();
    }
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn parse_pid(text: &str) -> Option<u32> {
    let start = text.find("pid ")? + "pid ".len();
    let rest = &text[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn wait_until(mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "condition not met in time"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn start_ping_stop_roundtrip() {
    let daemon = Daemon::new();

    let start = daemon
        .cmd()
        .args(["server", "start", "--port", "0"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&start.stderr)
    );
    assert!(String::from_utf8_lossy(&start.stdout).contains("started (pid"));

    let pid = daemon.info_pid().expect("daemon.json records a pid");
    assert!(pid_alive(pid), "recorded pid must be alive");

    let ping = daemon.cmd().args(["server", "ping"]).output().unwrap();
    assert!(ping.status.success());
    assert!(String::from_utf8_lossy(&ping.stdout).contains("running (pid"));

    let stop = daemon.cmd().args(["server", "stop"]).output().unwrap();
    assert!(stop.status.success());
    assert!(String::from_utf8_lossy(&stop.stdout).contains("stopped"));

    let down = daemon.cmd().args(["server", "ping"]).output().unwrap();
    assert!(!down.status.success(), "ping must exit non-zero when down");
    assert!(String::from_utf8_lossy(&down.stdout).contains("not running"));

    assert!(!daemon.home_path().join("daemon.json").exists());
    assert!(
        !pid_alive(pid),
        "stop must wait for the daemon process to fully exit"
    );
}

#[test]
fn second_start_is_idempotent() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cmd()
            .args(["server", "start", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );
    let pid = daemon.info_pid().unwrap();

    let again = daemon
        .cmd()
        .args(["server", "start", "--port", "0"])
        .output()
        .unwrap();
    assert!(again.status.success());
    let out = String::from_utf8_lossy(&again.stdout);
    assert!(out.contains("already running"), "{out}");
    assert!(
        out.contains(&pid.to_string()),
        "reports existing pid: {out}"
    );
    assert_eq!(daemon.info_pid().unwrap(), pid, "pid must not change");
}

#[test]
fn ping_never_starts_daemon() {
    let daemon = Daemon::new();
    let ping = daemon.cmd().args(["server", "ping"]).output().unwrap();
    assert!(!ping.status.success());
    assert!(String::from_utf8_lossy(&ping.stdout).contains("not running"));
    assert!(
        !daemon.home_path().join("daemon.json").exists(),
        "ping must not spawn a daemon"
    );
}

#[test]
fn stop_with_nothing_running_is_clean() {
    let daemon = Daemon::new();
    let stop = daemon.cmd().args(["server", "stop"]).output().unwrap();
    assert!(stop.status.success());
    assert!(String::from_utf8_lossy(&stop.stdout).contains("not running"));
}

#[test]
fn concurrent_starts_yield_single_pid() {
    let daemon = Daemon::new();

    let mut handles = Vec::new();
    for _ in 0..5 {
        let home = daemon.home.path().to_path_buf();
        let root = daemon.root.path().to_path_buf();
        handles.push(std::thread::spawn(move || {
            let out = Command::new(env!("CARGO_BIN_EXE_oplan"))
                .env("OPLAN_HOME", &home)
                .arg("--root")
                .arg(&root)
                .args(["server", "start", "--port", "0"])
                .output()
                .unwrap();
            assert!(out.status.success());
            parse_pid(&String::from_utf8_lossy(&out.stdout))
        }));
    }

    let pids: Vec<u32> = handles
        .into_iter()
        .map(|h| h.join().unwrap().expect("each start reports a pid"))
        .collect();
    let first = pids[0];
    assert!(
        pids.iter().all(|&p| p == first),
        "concurrent starts must agree on one pid: {pids:?}"
    );
    assert_eq!(daemon.info_pid().unwrap(), first);
    assert!(pid_alive(first));
}

#[test]
fn crashed_daemon_is_detected_and_replaced() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cmd()
            .args(["server", "start", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );
    let old = daemon.info_pid().unwrap();

    // SIGKILL so the daemon cannot clean up: daemon.json goes stale, the lock frees on exit.
    assert!(
        Command::new("kill")
            .arg("-9")
            .arg(old.to_string())
            .status()
            .unwrap()
            .success()
    );
    wait_until(|| !pid_alive(old));

    let ping = daemon.cmd().args(["server", "ping"]).output().unwrap();
    assert!(!ping.status.success(), "a crashed daemon pings as down");
    assert!(String::from_utf8_lossy(&ping.stdout).contains("not running"));

    let start = daemon
        .cmd()
        .args(["server", "start", "--port", "0"])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stale files must not block a fresh start: {}",
        String::from_utf8_lossy(&start.stderr)
    );
    let new = daemon.info_pid().unwrap();
    assert_ne!(new, old);
    assert!(pid_alive(new));
}

#[test]
fn ping_rejects_when_recorded_pid_mismatches_served_identity() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cmd()
            .args(["server", "start", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );
    let real = daemon.info_pid().unwrap();

    // Point daemon.json at a foreign pid; a recycled port owned by another service
    // is the real-world version of this. The live daemon still serves its own pid.
    daemon.set_recorded_pid(real.wrapping_add(1));
    let ping = daemon.cmd().args(["server", "ping"]).output().unwrap();
    // Restore the true identity before asserting so Drop can always stop the daemon.
    daemon.set_recorded_pid(real);

    assert!(
        !ping.status.success(),
        "identity mismatch must ping as down"
    );
    assert!(String::from_utf8_lossy(&ping.stdout).contains("not running"));
}

#[test]
fn start_ignores_requested_port_when_already_running() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cmd()
            .args(["server", "start", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );
    let running = daemon.info_port().unwrap();
    let requested = if running == 7373 { 7374 } else { 7373 };

    let again = daemon
        .cmd()
        .args(["server", "start", "--port", &requested.to_string()])
        .output()
        .unwrap();
    assert!(again.status.success());
    let out = String::from_utf8_lossy(&again.stdout);
    assert!(
        out.contains(&format!("ignoring requested port {requested}")),
        "{out}"
    );
}

#[test]
fn stop_treats_already_exited_pid_as_success() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cmd()
            .args(["server", "start", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );
    let real = daemon.info_pid().unwrap();

    // A pid that has already exited stands in for the daemon dying between stop's liveness
    // probe and its SIGTERM. The live daemon still holds the lifetime lock, so stop reaches
    // the signal path with a dead recorded pid — which must be treated as success, not an
    // error, since the goal (that process being gone) is already met.
    let dead = {
        let mut short = Command::new("true").spawn().unwrap();
        short.wait().unwrap();
        short.id()
    };
    daemon.set_recorded_pid(dead);

    let stop = daemon.cmd().args(["server", "stop"]).output().unwrap();
    assert!(
        stop.status.success(),
        "stop of an already-exited pid must not error: {}",
        String::from_utf8_lossy(&stop.stderr)
    );

    // stop cleared daemon.json but the real daemon is still up; stop it directly so the
    // test leaks neither the process nor the port.
    let _ = Command::new("kill").arg(real.to_string()).status();
    wait_until(|| !pid_alive(real));
}

#[test]
fn start_rejects_daemon_override() {
    let daemon = Daemon::new();
    let out = daemon
        .cmd()
        .args([
            "--daemon",
            "http://127.0.0.1:1",
            "server",
            "start",
            "--port",
            "0",
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "start must reject a --daemon override"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--daemon"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !daemon.home_path().join("daemon.json").exists(),
        "a rejected start must not spawn a daemon"
    );
}

#[test]
fn stop_honors_daemon_override_url() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cmd()
            .args(["server", "start", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );
    let pid = daemon.info_pid().unwrap();
    let port = daemon.info_port().unwrap();
    let url = format!("http://127.0.0.1:{port}");

    let stop = daemon
        .cmd()
        .args(["--daemon", &url, "server", "stop"])
        .output()
        .unwrap();
    assert!(
        stop.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&stop.stderr)
    );
    assert!(String::from_utf8_lossy(&stop.stdout).contains("stopping (daemon at"));
    wait_until(|| !pid_alive(pid));
}

#[test]
fn foreground_start_refuses_when_a_daemon_holds_the_lock() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cmd()
            .args(["server", "start", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );

    // A second foreground daemon on the same OPLAN_HOME must fail fast on the lifetime lock
    // rather than start a rival server; if the fix regressed it would acquire the lock and
    // block forever, which the deadline below turns into a failure instead of a hang.
    let mut child = daemon
        .cmd()
        .args(["server", "start", "--foreground", "--port", "0"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() >= Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("foreground start hung; it wrongly acquired the held lifetime lock");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        !status.success(),
        "second foreground daemon must fail to lock"
    );
}
