use std::path::{Path, PathBuf};
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
        // `openplan serve` requires a git repository, so the daemon's root must be one.
        git(root.path(), &["init", "-q", "-b", "main"]);
        git(root.path(), &["config", "user.email", "t@example.com"]);
        git(root.path(), &["config", "user.name", "Test"]);
        std::fs::create_dir_all(root.path().join(".plan/tasks")).unwrap();
        std::fs::write(
            root.path().join(".plan/config.toml"),
            "abbreviation = \"OPP\"\n",
        )
        .unwrap();
        Self { home, root }
    }

    fn home_path(&self) -> &Path {
        self.home.path()
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_openplan"));
        // A write starts the daemon itself, with no `--port` to carry, so without this it would
        // reach for the real 7373 — the developer's own daemon, or another test's.
        cmd.env("OPENPLAN_HOME", self.home.path())
            .env("OPENPLAN_PORT", "0")
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

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed");
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

// `openplan project list` prints one line per project — name, abbreviation, root — and indents the
// reason a demoted one is not served under it.
fn projects(daemon: &Daemon) -> Vec<(String, String)> {
    let out = daemon.cmd().args(["project", "list"]).output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    if text.starts_with("no projects registered") {
        return Vec::new();
    }
    text.lines()
        .filter(|line| !line.starts_with('!'))
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some((fields.next()?.to_owned(), fields.nth(1)?.to_owned()))
        })
        .collect()
}

// The registry's own order, which is the order the projects were registered in.
fn registry_names(daemon: &Daemon) -> Vec<String> {
    std::fs::read_to_string(daemon.home_path().join("registry.toml"))
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("name = "))
        .map(|name| name.trim_matches('"').to_owned())
        .collect()
}

// No CLI command reports which routes the daemon answers, so the test asks it over HTTP.
fn http_get(port: u16, path: &str) -> String {
    use std::io::{Read as _, Write as _};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
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
fn foreground_start_reports_lock_conflict_on_both_channels() {
    let daemon = Daemon::new();
    // A detached daemon holds the lock for the rest of the test.
    assert!(
        daemon
            .cmd()
            .args(["server", "start", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );
    wait_until(|| daemon.info_port().is_some());

    // RUST_LOG=off silences tracing; the fatal startup error must still reach stderr.
    let silent = daemon
        .cmd()
        .env("RUST_LOG", "off")
        .args(["server", "start", "--foreground", "--port", "0"])
        .output()
        .unwrap();
    assert!(
        !silent.status.success(),
        "a lock conflict must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&silent.stderr);
    assert!(
        stderr.contains("error:") && stderr.contains("already holds"),
        "stderr: {stderr}"
    );

    // With logging live, the same failure is a tracing-formatted ERROR line (on stdout).
    let logged = daemon
        .cmd()
        .env("RUST_LOG", "info")
        .args(["server", "start", "--foreground", "--port", "0"])
        .output()
        .unwrap();
    assert!(
        !logged.status.success(),
        "a lock conflict must exit non-zero"
    );
    let stdout = String::from_utf8_lossy(&logged.stdout);
    assert!(
        stdout.contains("daemon exited with error") && stdout.contains("already holds"),
        "stdout: {stdout}"
    );
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
            let out = Command::new(env!("CARGO_BIN_EXE_openplan"))
                .env("OPENPLAN_HOME", &home)
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
fn restart_rebinds_a_fresh_daemon_while_running() {
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

    let restart = daemon
        .cmd()
        .args(["server", "restart", "--port", "0"])
        .output()
        .unwrap();
    assert!(
        restart.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&restart.stderr)
    );
    assert!(String::from_utf8_lossy(&restart.stdout).contains("started (pid"));

    let new = daemon.info_pid().unwrap();
    assert_ne!(new, old, "restart must spawn a fresh daemon");
    assert!(!pid_alive(old), "restart must stop the old daemon");
    assert!(pid_alive(new), "the new daemon must be alive");

    let ping = daemon.cmd().args(["server", "ping"]).output().unwrap();
    assert!(ping.status.success(), "the new daemon must be healthy");
}

#[test]
fn restart_rebinds_the_same_fixed_port() {
    let daemon = Daemon::new();
    let port = free_port();
    let port_arg = port.to_string();

    assert!(
        daemon
            .cmd()
            .args(["server", "start", "--port", &port_arg])
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(daemon.info_port().unwrap(), port);
    let old = daemon.info_pid().unwrap();

    let restart = daemon
        .cmd()
        .args(["server", "restart", "--port", &port_arg])
        .output()
        .unwrap();
    assert!(
        restart.status.success(),
        "restart must rebind the requested port: {}",
        String::from_utf8_lossy(&restart.stderr)
    );

    let new = daemon.info_pid().unwrap();
    assert_ne!(new, old, "restart must spawn a fresh daemon");
    assert_eq!(
        daemon.info_port().unwrap(),
        port,
        "the fresh daemon must rebind the same requested port"
    );
    assert!(pid_alive(new), "the new daemon must be alive");
}

#[test]
fn restart_with_nothing_running_just_starts() {
    let daemon = Daemon::new();
    let restart = daemon
        .cmd()
        .args(["server", "restart", "--port", "0"])
        .output()
        .unwrap();
    assert!(
        restart.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&restart.stderr)
    );
    let out = String::from_utf8_lossy(&restart.stdout);
    assert!(out.contains("started (pid"), "{out}");
    assert!(
        !out.contains("not running"),
        "restart with nothing running should read as a plain start, not report a stop: {out}"
    );

    let pid = daemon.info_pid().expect("restart started a daemon");
    assert!(pid_alive(pid));
}

#[test]
fn restart_rejects_daemon_override() {
    let daemon = Daemon::new();
    let out = daemon
        .cmd()
        .args([
            "--daemon",
            "http://127.0.0.1:1",
            "server",
            "restart",
            "--port",
            "0",
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "restart must reject a --daemon override"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--daemon"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !daemon.home_path().join("daemon.json").exists(),
        "a rejected restart must not spawn a daemon"
    );
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

    // A second foreground daemon on the same OPENPLAN_HOME must fail fast on the lifetime lock
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

fn updated_of(daemon: &Daemon, id: &str) -> serde_json::Value {
    let out = daemon.cmd().args(["get", id, "--json"]).output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let view: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    view["updated"].clone()
}

// A read is answered from the daemon's index whether the daemon was already up or the read itself
// brought it up, so the two cannot disagree about a task's date.
#[test]
fn a_read_dates_a_task_the_same_whoever_started_the_daemon() {
    let daemon = Daemon::new();
    let created = daemon
        .cmd()
        .args(["create", "Dated task"])
        .output()
        .unwrap();
    assert!(created.status.success());
    let id = String::from_utf8(created.stdout).unwrap().trim().to_owned();
    git(daemon.root.path(), &["add", "-A"]);
    git(daemon.root.path(), &["commit", "-qm", "add the task"]);

    let started_by_the_write = updated_of(&daemon, &id);
    assert!(started_by_the_write.is_string(), "{started_by_the_write}");

    assert!(
        daemon
            .cmd()
            .args(["server", "restart", "--port", "0"])
            .output()
            .unwrap()
            .status
            .success()
    );

    assert_eq!(updated_of(&daemon, &id), started_by_the_write);
}

// Reads have no local fallback, so a daemon that cannot be reached stops the command instead of
// answering from the files in front of it.
#[test]
fn a_read_with_no_reachable_daemon_fails_explicitly() {
    let daemon = Daemon::new();
    let out = daemon
        .cmd()
        .args(["--daemon", "http://127.0.0.1:1", "list"])
        .output()
        .unwrap();

    assert!(!out.status.success(), "an unreachable daemon must not pass");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no openplan daemon at http://127.0.0.1:1"),
        "stderr: {stderr}"
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).is_empty(),
        "no task data may be printed: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

fn commit_at(dir: &Path, seconds: i64, message: &str) {
    let date = format!("@{seconds} +0000");
    git(dir, &["add", "-A"]);
    let status = Command::new("git")
        .current_dir(dir)
        .args(["commit", "-qm", message])
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git commit failed");
}

fn task_repo(seconds: i64) -> TempDir {
    task_repo_keyed(seconds, "OPP")
}

fn task_repo_keyed(seconds: i64, abbreviation: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "t@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    std::fs::write(
        dir.path().join(".plan/config.toml"),
        format!("abbreviation = \"{abbreviation}\"\n"),
    )
    .unwrap();
    // The number names the file, so both repos can hold the same id on purpose.
    std::fs::write(
        dir.path().join(".plan/tasks/00001-shared.md"),
        "---\nstatus: todo\ncreated: 2001-01-01T00:00:00Z\n---\n# Shared\n",
    )
    .unwrap();
    commit_at(dir.path(), seconds, "add the task");
    dir
}

// One daemon serves every repository on the machine, so a read has to name which one it is asking
// about. Two repositories can hold a task of the same number, and the answer must be the caller's.
#[test]
fn a_read_is_answered_for_the_repository_the_caller_stands_in() {
    let daemon = Daemon::new();
    let theirs = task_repo(1_000_000_000);
    let ours = task_repo(1_500_000_000);

    let mut add = Command::new(env!("CARGO_BIN_EXE_openplan"));
    add.env("OPENPLAN_HOME", daemon.home_path())
        .env("OPENPLAN_PORT", "0");
    assert!(
        add.args(["--root"])
            .arg(theirs.path())
            .args(["project", "add"])
            .output()
            .unwrap()
            .status
            .success()
    );

    let mut read = Command::new(env!("CARGO_BIN_EXE_openplan"));
    read.env("OPENPLAN_HOME", daemon.home_path())
        .arg("--root")
        .arg(ours.path());
    let out = read.args(["get", "OPP-1", "--json"]).output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let view: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();

    // Both repositories hold task `1`, dated differently. The daemon was already serving the other
    // one; answering from it would date this read by a file the caller never named.
    assert_eq!(view["updated"], "2017-07-14T02:40:00Z");
}

// `--root` says which directory a command works in, as `git -C` does. It registers nothing: the
// first write and `openplan project add` are the only ways into the registry.
#[test]
fn a_start_registers_nothing_and_the_first_write_registers() {
    let daemon = Daemon::new();
    let registry = daemon.home_path().join("registry.toml");
    assert!(!registry.exists(), "a fresh OPENPLAN_HOME has no registry");

    assert!(
        daemon
            .cmd()
            .args(["server", "start", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );
    let ping = daemon.cmd().args(["server", "ping"]).output().unwrap();
    assert!(ping.status.success(), "the daemon serves zero projects");
    assert!(!registry.exists(), "starting is not registering");

    assert!(
        daemon
            .cmd()
            .args(["create", "First task"])
            .status()
            .unwrap()
            .success()
    );

    let seeded = std::fs::read_to_string(&registry).unwrap();
    assert_eq!(
        seeded.matches("[[project]]").count(),
        1,
        "one entry for the repository written to: {seeded}"
    );
    let root = daemon.root.path().canonicalize().unwrap();
    assert!(
        seeded.contains(root.to_str().unwrap()),
        "the entry names the serve root: {seeded}"
    );

    assert!(
        daemon
            .cmd()
            .args(["create", "Second task"])
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(
        std::fs::read_to_string(&registry).unwrap(),
        seeded,
        "a repository already served must not be registered again"
    );
}

// The entry is matched by its path, so a name the user chose survives a restart, and the CLI keeps
// resolving the repository to it.
#[test]
fn a_renamed_project_survives_a_restart() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cmd()
            .args(["create", "First task"])
            .status()
            .unwrap()
            .success()
    );

    let (name, root) = projects(&daemon).remove(0);
    let renamed = daemon
        .cmd()
        .args(["project", "rename", &name, "chosen"])
        .output()
        .unwrap();
    assert!(
        renamed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&renamed.stderr)
    );

    assert!(
        daemon
            .cmd()
            .args(["server", "restart", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(projects(&daemon), vec![("chosen".to_owned(), root.clone())]);

    let write = daemon
        .cmd()
        .args(["create", "Second task"])
        .output()
        .unwrap();
    assert!(
        write.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&write.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&write.stderr).contains("registered"),
        "the repository is still the same project, under its new name"
    );
    assert_eq!(projects(&daemon), vec![("chosen".to_owned(), root)]);
}

#[test]
fn project_add_registers_a_second_repository_and_remove_leaves_its_files() {
    let daemon = Daemon::new();
    let second = task_repo(1_000_000_000);
    assert!(
        daemon
            .cmd()
            .args(["create", "First repo"])
            .status()
            .unwrap()
            .success()
    );

    let added = daemon
        .cmd()
        .args(["project", "add"])
        .arg(second.path())
        .output()
        .unwrap();
    assert!(
        added.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&added.stderr)
    );

    let listed = projects(&daemon);
    let first_root = daemon.root.path().canonicalize().unwrap();
    let second_root = second.path().canonicalize().unwrap();
    assert!(
        listed
            .iter()
            .any(|(_, root)| root == first_root.to_str().unwrap()),
        "both repositories are listed: {listed:?}"
    );
    let (name, _) = listed
        .iter()
        .find(|(_, root)| root == second_root.to_str().unwrap())
        .unwrap_or_else(|| panic!("the added repository is listed: {listed:?}"))
        .clone();

    let removed = daemon
        .cmd()
        .args(["project", "remove", &name])
        .output()
        .unwrap();
    assert!(
        removed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(
        String::from_utf8_lossy(&removed.stdout).contains("files stay on disk"),
        "removal says the checkout is untouched: {}",
        String::from_utf8_lossy(&removed.stdout)
    );
    assert!(
        second.path().join(".plan/tasks/00001-shared.md").exists(),
        "removing a project must not touch its files"
    );
}

// `--root` says which directory a command works in. `server start` runs in the daemon's own home
// and serves the whole registry, so the flag decides nothing there: every project keeps answering
// through its own prefix, and none of them captures a route.
#[test]
fn root_on_an_explicit_start_names_no_favoured_project() {
    let daemon = Daemon::new();
    let second = task_repo_keyed(1_000_000_000, "BBB");
    assert!(
        daemon
            .cmd()
            .args(["create", "First repo"])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        daemon
            .cmd()
            .args(["project", "add"])
            .arg(second.path())
            .status()
            .unwrap()
            .success()
    );

    let mut restart = Command::new(env!("CARGO_BIN_EXE_openplan"));
    assert!(
        restart
            .env("OPENPLAN_HOME", daemon.home_path())
            .env("OPENPLAN_PORT", "0")
            .arg("--root")
            .arg(second.path())
            .args(["server", "restart", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );

    let port = daemon.info_port().unwrap();
    for name in registry_names(&daemon) {
        let answered = http_get(port, &format!("/api/projects/{name}/config"));
        assert!(
            answered.contains("200 OK"),
            "{name} answers its own routes: {answered}"
        );
    }
    let dropped = http_get(port, "/api/config");
    assert!(
        dropped.contains("404 Not Found"),
        "no project answers an unprefixed spelling: {dropped}"
    );
}

// The registry's order is the order the projects were registered in, so a rename must leave the
// entry where it is rather than move it to the end.
#[test]
fn a_rename_keeps_the_entry_in_place() {
    let daemon = Daemon::new();
    let second = task_repo(1_000_000_000);
    assert!(
        daemon
            .cmd()
            .args(["create", "First repo"])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        daemon
            .cmd()
            .args(["project", "add"])
            .arg(second.path())
            .status()
            .unwrap()
            .success()
    );

    let before = registry_names(&daemon);
    assert_eq!(before.len(), 2, "{before:?}");

    assert!(
        daemon
            .cmd()
            .args(["project", "rename", &before[0], "chosen"])
            .status()
            .unwrap()
            .success()
    );

    assert_eq!(
        registry_names(&daemon),
        vec!["chosen".to_owned(), before[1].clone()],
        "the renamed entry keeps its place"
    );
    drop(second);
}

// An entry the daemon could not open at startup has no live project. Removing it must still work, or
// the registry the daemon says it owns could only be repaired by hand.
#[test]
fn a_registry_entry_that_cannot_be_opened_can_still_be_removed() {
    let daemon = Daemon::new();
    let second = task_repo(1_000_000_000);
    assert!(
        daemon
            .cmd()
            .args(["create", "First repo"])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        daemon
            .cmd()
            .args(["project", "add"])
            .arg(second.path())
            .status()
            .unwrap()
            .success()
    );
    let (gone, _) = projects(&daemon)
        .into_iter()
        .find(|(_, root)| root == second.path().canonicalize().unwrap().to_str().unwrap())
        .expect("the added repository is listed");

    assert!(
        daemon
            .cmd()
            .args(["server", "stop"])
            .status()
            .unwrap()
            .success()
    );
    std::fs::remove_dir_all(second.path()).unwrap();
    assert!(
        daemon
            .cmd()
            .args(["server", "start", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );

    assert!(
        !projects(&daemon).iter().any(|(name, _)| name == &gone),
        "an entry that cannot be opened is not served"
    );
    let removed = daemon
        .cmd()
        .args(["project", "remove", &gone])
        .output()
        .unwrap();
    assert!(
        removed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&removed.stderr)
    );
    let text = std::fs::read_to_string(daemon.home_path().join("registry.toml")).unwrap();
    assert_eq!(
        text.matches("[[project]]").count(),
        1,
        "the entry is gone from the file: {text}"
    );
}

// A demoted project stays registered and stays listed. Its reason is what answers the question "why
// does my UI not show project X".
#[test]
fn project_list_marks_a_demoted_project_with_its_reason() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cmd()
            .args(["create", "Anchor"])
            .status()
            .unwrap()
            .success()
    );

    std::fs::write(
        daemon.root.path().join(".plan/config.toml"),
        "abbreviation = \"not valid\"\n",
    )
    .unwrap();

    let mut listed = String::new();
    wait_until(|| {
        let out = daemon.cmd().args(["project", "list"]).output().unwrap();
        listed = String::from_utf8(out.stdout).unwrap();
        listed.contains('!')
    });
    assert!(
        listed.contains("three uppercase letters"),
        "the reason names the broken config: {listed}"
    );
}

// Two writes from one repository can be the first one. Registration is idempotent by repository, so
// both land and the registry holds one entry.
#[test]
fn concurrent_first_writes_register_one_project() {
    let daemon = Daemon::new();
    let handles: Vec<_> = (0..4)
        .map(|n| {
            let home = daemon.home.path().to_path_buf();
            let root = daemon.root.path().to_path_buf();
            std::thread::spawn(move || {
                Command::new(env!("CARGO_BIN_EXE_openplan"))
                    .env("OPENPLAN_HOME", &home)
                    .env("OPENPLAN_PORT", "0")
                    .arg("--root")
                    .arg(&root)
                    .args(["create", &format!("Task {n}")])
                    .output()
                    .unwrap()
            })
        })
        .collect();
    for handle in handles {
        let out = handle.join().unwrap();
        assert!(
            out.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let text = std::fs::read_to_string(daemon.home_path().join("registry.toml")).unwrap();
    assert_eq!(
        text.matches("[[project]]").count(),
        1,
        "one repository is one project: {text}"
    );
    assert_eq!(
        std::fs::read_dir(daemon.root.path().join(".plan/tasks"))
            .unwrap()
            .count(),
        4,
        "every write landed, each under its own id"
    );
}

// `Store::discover` walks past the git root, so a `.plan` above the checkout makes the store root and
// the serve root two different directories. The registry has to record the one that holds the repo.
#[test]
fn a_store_above_the_git_root_registers_the_checkout() {
    let home = tempfile::tempdir().unwrap();
    let outer = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(outer.path().join(".plan/tasks")).unwrap();
    std::fs::write(
        outer.path().join(".plan/config.toml"),
        "abbreviation = \"OPP\"\n",
    )
    .unwrap();
    let inner = outer.path().join("repo");
    std::fs::create_dir_all(&inner).unwrap();
    git(&inner, &["init", "-q", "-b", "main"]);
    git(&inner, &["config", "user.email", "t@example.com"]);
    git(&inner, &["config", "user.name", "Test"]);
    git(&inner, &["commit", "-q", "--allow-empty", "-m", "init"]);

    let mut add = Command::new(env!("CARGO_BIN_EXE_openplan"));
    let out = add
        .env("OPENPLAN_HOME", home.path())
        .env("OPENPLAN_PORT", "0")
        .arg("--root")
        .arg(&inner)
        .args(["project", "add"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let text = std::fs::read_to_string(home.path().join("registry.toml")).unwrap();
    assert!(
        text.contains(inner.canonicalize().unwrap().to_str().unwrap()),
        "the entry names the checkout, not the .plan parent: {text}"
    );

    let mut stop = Command::new(env!("CARGO_BIN_EXE_openplan"));
    let _ = stop
        .env("OPENPLAN_HOME", home.path())
        .args(["server", "stop"])
        .output();
}

// `openplan open` hands the URL to a launcher command. A stub in $BROWSER records the arguments a
// real browser would have received, then runs `tail` — `exit 0`, a failing exit, or a sleep that
// stands in for a browser which does not return until the user closes it.
fn browser_stub(dir: &Path, name: &str, tail: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;

    let script = dir.join(name);
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s' \"$*\" > '{}'\n{tail}\n",
            record_path(dir, name).display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn record_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.args"))
}

// The launcher outlives the command that starts it, so the record can land after `openplan open`
// has already returned.
fn launched(dir: &Path, name: &str) -> String {
    wait_until(|| record_path(dir, name).exists());
    std::fs::read_to_string(record_path(dir, name)).unwrap()
}

#[test]
fn open_starts_a_daemon_and_launches_the_browser_at_the_bound_port() {
    let daemon = Daemon::new();
    let stub = tempfile::tempdir().unwrap();
    let script = browser_stub(stub.path(), "browser", "exit 0");

    let out = daemon
        .cmd()
        .env("BROWSER", &script)
        .arg("open")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let port = daemon
        .info_port()
        .expect("open must start a daemon and record its port");
    let url = format!("http://127.0.0.1:{port}/");
    assert_eq!(launched(stub.path(), "browser"), url);
    assert!(String::from_utf8_lossy(&out.stdout).contains(&url));
}

// The UI lists the projects the daemon serves. Opening it from a repository the daemon does not
// serve yet must show that repository, so `open` registers it as a first write does.
#[test]
fn open_registers_the_repository_the_caller_stands_in() {
    let daemon = Daemon::new();
    let stub = tempfile::tempdir().unwrap();
    let script = browser_stub(stub.path(), "browser", "exit 0");

    let out = daemon
        .cmd()
        .env("BROWSER", &script)
        .arg("open")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let registry = std::fs::read_to_string(daemon.home_path().join("registry.toml"))
        .expect("open must register the caller's repository");
    let root = daemon.root.path().canonicalize().unwrap();
    assert!(
        registry.contains(root.to_str().unwrap()),
        "the entry names the serve root: {registry}"
    );
}

#[test]
fn open_places_the_url_where_browser_spells_it() {
    let daemon = Daemon::new();
    let stub = tempfile::tempdir().unwrap();
    let script = browser_stub(stub.path(), "browser", "exit 0");

    let out = daemon
        .cmd()
        .env("BROWSER", format!("{} %s --new-window", script.display()))
        .arg("open")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let port = daemon.info_port().unwrap();
    assert_eq!(
        launched(stub.path(), "browser"),
        format!("http://127.0.0.1:{port}/ --new-window")
    );
}

// $BROWSER lists candidates in order of preference, so a name this machine does not have must not
// stop the command.
#[test]
fn open_skips_a_browser_candidate_that_is_not_installed() {
    let daemon = Daemon::new();
    let stub = tempfile::tempdir().unwrap();
    let script = browser_stub(stub.path(), "second", "exit 0");
    let missing = stub.path().join("no-such-browser");

    let out = daemon
        .cmd()
        .env(
            "BROWSER",
            format!("{}:{}", missing.display(), script.display()),
        )
        .arg("open")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let port = daemon.info_port().unwrap();
    assert_eq!(
        launched(stub.path(), "second"),
        format!("http://127.0.0.1:{port}/")
    );
}

// A launcher that is the browser itself runs until the user closes the window. The command must
// hand it the URL and return, not hold the terminal for the life of the browser.
#[test]
fn open_returns_while_the_browser_keeps_running() {
    let daemon = Daemon::new();
    let stub = tempfile::tempdir().unwrap();
    let script = browser_stub(stub.path(), "browser", "sleep 30");

    let start = Instant::now();
    let out = daemon
        .cmd()
        .env("BROWSER", &script)
        .arg("open")
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "open waited {elapsed:?} for a browser that stays open"
    );

    let port = daemon.info_port().unwrap();
    assert_eq!(
        launched(stub.path(), "browser"),
        format!("http://127.0.0.1:{port}/")
    );
}

#[test]
fn open_honors_the_daemon_override_and_starts_no_local_daemon() {
    let serving = Daemon::new();
    assert!(
        serving
            .cmd()
            .args(["server", "start", "--port", "0"])
            .status()
            .unwrap()
            .success()
    );
    let url = format!("http://127.0.0.1:{}", serving.info_port().unwrap());

    let caller = Daemon::new();
    let stub = tempfile::tempdir().unwrap();
    let script = browser_stub(stub.path(), "browser", "exit 0");
    let out = caller
        .cmd()
        .env("BROWSER", &script)
        .args(["--daemon", &url, "open"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert_eq!(launched(stub.path(), "browser"), format!("{url}/"));
    assert!(
        !caller.home_path().join("daemon.json").exists(),
        "--daemon must not start a machine daemon"
    );
    assert!(
        !serving.home_path().join("registry.toml").exists(),
        "--daemon must not register the caller's repository on a borrowed daemon"
    );
}

#[test]
fn open_fails_when_the_launcher_fails() {
    let daemon = Daemon::new();
    let stub = tempfile::tempdir().unwrap();
    let script = browser_stub(stub.path(), "browser", "exit 3");

    let out = daemon
        .cmd()
        .env("BROWSER", &script)
        .arg("open")
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "a launcher that fails must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("did not open"), "stderr: {stderr}");
    assert!(
        String::from_utf8_lossy(&out.stdout).is_empty(),
        "no URL may be printed as a fallback: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}
