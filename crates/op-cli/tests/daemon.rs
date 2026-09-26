mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use common::{Home, git_repo, ok, stderr, stdout, task_count, wait_until, write};
use tempfile::TempDir;

// A local project no daemon serves yet, and the home of the daemon a test starts for it. The
// project is `.plan/config.toml` in a directory outside any git repository, so the first command
// that reaches the daemon registers it.
struct Daemon {
    home: Home,
    root: TempDir,
}

impl Daemon {
    fn new() -> Self {
        Self {
            home: Home::new(),
            root: task_store("OPP", None),
        }
    }

    fn home_path(&self) -> &Path {
        self.home.path()
    }

    fn root_path(&self) -> &Path {
        self.root.path()
    }

    fn cmd(&self) -> Command {
        let mut cmd = self.home.cmd();
        cmd.arg("--root").arg(self.root.path());
        cmd
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }

    fn start(&self) {
        ok(self.run(&["server", "start", "--port", "0"]));
    }

    fn info_pid(&self) -> Option<u32> {
        self.home.pid()
    }

    fn info_port(&self) -> Option<u16> {
        self.home.port()
    }

    fn set_recorded_pid(&self, pid: u32) {
        let path = self.home_path().join("daemon.json");
        let text = std::fs::read_to_string(&path).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["pid"] = serde_json::json!(pid);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    }
}

// `.plan/` with a config, and one task when `task` names its title. Outside any git repository, so
// no daemon serves it until a command registers it.
fn task_store(abbreviation: &str, task: Option<&str>) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(
        &dir.path().join(".plan/config.toml"),
        &format!("abbreviation = \"{abbreviation}\"\n"),
    );
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    if let Some(title) = task {
        write(
            &dir.path().join(".plan/tasks/00001-shared.md"),
            &format!("---\nstatus: todo\ncreated: 2001-01-01T00:00:00Z\n---\n# {title}\n"),
        );
    }
    dir
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stderr(Stdio::null())
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

// `openplan project list` prints one line per project — name, abbreviation, backend, root — and
// indents the reason a demoted one is not served under it.
fn projects(daemon: &Daemon) -> Vec<(String, String)> {
    let text = ok(daemon.run(&["project", "list"]));
    if text.starts_with("no projects registered") {
        return Vec::new();
    }
    text.lines()
        .filter(|line| !line.starts_with('!'))
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some((fields.next()?.to_owned(), fields.nth(2)?.to_owned()))
        })
        .collect()
}

// The registry's own order, which is the order the projects were registered in.
fn registry_names(daemon: &Daemon) -> Vec<String> {
    daemon
        .home
        .registry()
        .lines()
        .filter_map(|line| line.strip_prefix("name = "))
        .map(|name| name.trim_matches('"').to_owned())
        .collect()
}

fn canonical(path: &Path) -> String {
    path.canonicalize().unwrap().to_str().unwrap().to_owned()
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

fn soon(cond: impl FnMut() -> bool) {
    wait_until(Duration::from_secs(5), cond);
}

#[test]
fn a_cargo_build_does_not_update_itself_and_ping_says_why() {
    let daemon = Daemon::new();
    daemon.start();

    let mut ping = String::new();
    soon(|| {
        ping = ok(daemon.run(&["server", "ping"]));
        ping.contains("no automatic updates")
    });
    assert!(ping.contains("cache directory"), "{ping}");

    ok(daemon.run(&["server", "stop"]));
}

#[test]
fn ping_says_when_automatic_updates_are_off() {
    let daemon = Daemon::new();
    daemon.start();

    ok(daemon.run(&["update", "--auto", "off"]));

    let ping = ok(daemon.run(&["server", "ping"]));
    assert!(ping.contains("automatic updates: off"), "{ping}");
    ok(daemon.run(&["server", "stop"]));
}

#[test]
fn start_ping_stop_roundtrip() {
    let daemon = Daemon::new();

    let started = ok(daemon.run(&["server", "start", "--port", "0"]));
    assert!(started.contains("started (pid"), "{started}");

    let pid = daemon.info_pid().expect("daemon.json records a pid");
    assert!(pid_alive(pid), "recorded pid must be alive");

    let ping = ok(daemon.run(&["server", "ping"]));
    assert!(ping.contains("running (pid"), "{ping}");

    let stop = ok(daemon.run(&["server", "stop"]));
    assert!(stop.contains("stopped"), "{stop}");

    let down = daemon.run(&["server", "ping"]);
    assert!(!down.status.success(), "ping must exit non-zero when down");
    assert!(stdout(&down).contains("not running"));

    assert!(!daemon.home_path().join("daemon.json").exists());
    assert!(
        !pid_alive(pid),
        "stop must wait for the daemon process to fully exit"
    );
}

#[test]
fn second_start_is_idempotent() {
    let daemon = Daemon::new();
    daemon.start();
    let pid = daemon.info_pid().unwrap();

    let again = ok(daemon.run(&["server", "start", "--port", "0"]));

    assert!(again.contains("already running"), "{again}");
    assert!(
        again.contains(&pid.to_string()),
        "reports existing pid: {again}"
    );
    assert_eq!(daemon.info_pid().unwrap(), pid, "pid must not change");
}

#[test]
fn foreground_start_reports_lock_conflict_on_both_channels() {
    let daemon = Daemon::new();
    // A detached daemon holds the lock for the rest of the test.
    daemon.start();
    soon(|| daemon.info_port().is_some());

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
    assert!(
        stderr(&silent).contains("error:") && stderr(&silent).contains("already holds"),
        "stderr: {}",
        stderr(&silent)
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
    assert!(
        stdout(&logged).contains("daemon exited with error")
            && stdout(&logged).contains("already holds"),
        "stdout: {}",
        stdout(&logged)
    );
}

#[test]
fn ping_never_starts_daemon() {
    let daemon = Daemon::new();

    let ping = daemon.run(&["server", "ping"]);

    assert!(!ping.status.success());
    assert!(stdout(&ping).contains("not running"));
    assert!(
        !daemon.home_path().join("daemon.json").exists(),
        "ping must not spawn a daemon"
    );
}

#[test]
fn stop_with_nothing_running_is_clean() {
    let daemon = Daemon::new();

    let stop = ok(daemon.run(&["server", "stop"]));

    assert!(stop.contains("not running"), "{stop}");
}

#[test]
fn concurrent_starts_yield_single_pid() {
    let daemon = Daemon::new();

    let handles: Vec<_> = (0..5)
        .map(|_| {
            let mut cmd = daemon.cmd();
            std::thread::spawn(move || {
                let out = cmd
                    .args(["server", "start", "--port", "0"])
                    .output()
                    .unwrap();
                assert!(out.status.success(), "stderr: {}", stderr(&out));
                parse_pid(&stdout(&out))
            })
        })
        .collect();

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
    daemon.start();
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
    soon(|| !pid_alive(old));

    let ping = daemon.run(&["server", "ping"]);
    assert!(!ping.status.success(), "a crashed daemon pings as down");
    assert!(stdout(&ping).contains("not running"));

    let start = daemon.run(&["server", "start", "--port", "0"]);
    assert!(
        start.status.success(),
        "stale files must not block a fresh start: {}",
        stderr(&start)
    );
    let new = daemon.info_pid().unwrap();
    assert_ne!(new, old);
    assert!(pid_alive(new));
}

#[test]
fn ping_rejects_when_recorded_pid_mismatches_served_identity() {
    let daemon = Daemon::new();
    daemon.start();
    let real = daemon.info_pid().unwrap();

    // Point daemon.json at a foreign pid; a recycled port owned by another service is the
    // real-world version of this. The live daemon still serves its own pid.
    daemon.set_recorded_pid(real.wrapping_add(1));
    let ping = daemon.run(&["server", "ping"]);
    // Restore the true identity before asserting so Drop can always stop the daemon.
    daemon.set_recorded_pid(real);

    assert!(
        !ping.status.success(),
        "identity mismatch must ping as down"
    );
    assert!(stdout(&ping).contains("not running"));
}

#[test]
fn start_ignores_requested_port_when_already_running() {
    let daemon = Daemon::new();
    daemon.start();
    let running = daemon.info_port().unwrap();
    let requested = if running == 7373 { 7374 } else { 7373 };

    let again = ok(daemon.run(&["server", "start", "--port", &requested.to_string()]));

    assert!(
        again.contains(&format!("ignoring requested port {requested}")),
        "{again}"
    );
}

#[test]
fn stop_treats_already_exited_pid_as_success() {
    let daemon = Daemon::new();
    daemon.start();
    let real = daemon.info_pid().unwrap();

    // A pid that has already exited stands in for the daemon dying between stop's liveness probe
    // and its SIGTERM. The live daemon still holds the lifetime lock, so stop reaches the signal
    // path with a dead recorded pid, which must be treated as success, since the goal (that
    // process being gone) is already met.
    let dead = {
        let mut short = Command::new("true").spawn().unwrap();
        short.wait().unwrap();
        short.id()
    };
    daemon.set_recorded_pid(dead);

    let stop = daemon.run(&["server", "stop"]);
    assert!(
        stop.status.success(),
        "stop of an already-exited pid must not error: {}",
        stderr(&stop)
    );

    // stop cleared daemon.json but the real daemon is still up; stop it directly so the test leaks
    // neither the process nor the port.
    let _ = Command::new("kill").arg(real.to_string()).status();
    soon(|| !pid_alive(real));
}

#[test]
fn start_rejects_daemon_override() {
    let daemon = Daemon::new();

    let out = daemon.run(&[
        "--daemon",
        "http://127.0.0.1:1",
        "server",
        "start",
        "--port",
        "0",
    ]);

    assert!(
        !out.status.success(),
        "start must reject a --daemon override"
    );
    assert!(
        stderr(&out).contains("--daemon"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        !daemon.home_path().join("daemon.json").exists(),
        "a rejected start must not spawn a daemon"
    );
}

#[test]
fn stop_honors_daemon_override_url() {
    let daemon = Daemon::new();
    daemon.start();
    let pid = daemon.info_pid().unwrap();
    let url = format!("http://127.0.0.1:{}", daemon.info_port().unwrap());

    let stop = ok(daemon.run(&["--daemon", &url, "server", "stop"]));

    assert!(stop.contains("stopping (daemon at"), "{stop}");
    soon(|| !pid_alive(pid));
}

#[test]
fn ping_honors_daemon_override_url() {
    let daemon = Daemon::new();
    daemon.start();
    let url = format!("http://127.0.0.1:{}", daemon.info_port().unwrap());

    let up = ok(daemon.run(&["--daemon", &url, "server", "ping"]));
    assert!(up.contains(&format!("running (daemon at {url})")), "{up}");

    let down = daemon.run(&["--daemon", "http://127.0.0.1:1", "server", "ping"]);
    assert!(!down.status.success());
    assert!(
        stdout(&down).contains("not running (no openplan daemon at http://127.0.0.1:1)"),
        "{}",
        stdout(&down)
    );
}

#[test]
fn restart_rebinds_a_fresh_daemon_while_running() {
    let daemon = Daemon::new();
    daemon.start();
    let old = daemon.info_pid().unwrap();

    let restart = ok(daemon.run(&["server", "restart", "--port", "0"]));

    assert!(restart.contains("started (pid"), "{restart}");
    let new = daemon.info_pid().unwrap();
    assert_ne!(new, old, "restart must spawn a fresh daemon");
    assert!(!pid_alive(old), "restart must stop the old daemon");
    assert!(pid_alive(new), "the new daemon must be alive");
    ok(daemon.run(&["server", "ping"]));
}

#[test]
fn restart_rebinds_the_same_fixed_port() {
    let daemon = Daemon::new();
    let port = free_port();
    let port_arg = port.to_string();

    ok(daemon.run(&["server", "start", "--port", &port_arg]));
    assert_eq!(daemon.info_port().unwrap(), port);
    let old = daemon.info_pid().unwrap();

    let restart = daemon.run(&["server", "restart", "--port", &port_arg]);
    assert!(
        restart.status.success(),
        "restart must rebind the requested port: {}",
        stderr(&restart)
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

    let restart = ok(daemon.run(&["server", "restart", "--port", "0"]));

    assert!(restart.contains("started (pid"), "{restart}");
    assert!(
        !restart.contains("not running"),
        "restart with nothing running should read as a plain start, not report a stop: {restart}"
    );
    assert!(pid_alive(
        daemon.info_pid().expect("restart started a daemon")
    ));
}

#[test]
fn restart_rejects_daemon_override() {
    let daemon = Daemon::new();

    let out = daemon.run(&[
        "--daemon",
        "http://127.0.0.1:1",
        "server",
        "restart",
        "--port",
        "0",
    ]);

    assert!(
        !out.status.success(),
        "restart must reject a --daemon override"
    );
    assert!(
        stderr(&out).contains("--daemon"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        !daemon.home_path().join("daemon.json").exists(),
        "a rejected restart must not spawn a daemon"
    );
}

#[test]
fn foreground_start_refuses_when_a_daemon_holds_the_lock() {
    let daemon = Daemon::new();
    daemon.start();

    // A second foreground daemon on the same OPENPLAN_HOME must fail fast on the lifetime lock
    // rather than start a rival server; if that regressed it would acquire the lock and block
    // forever, which the deadline below turns into a failure instead of a hang.
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

#[test]
fn openapi_prints_the_spec_without_a_daemon() {
    let daemon = Daemon::new();

    let spec: serde_json::Value =
        serde_json::from_str(&ok(daemon.run(&["server", "openapi"]))).expect("the spec is JSON");

    assert!(
        spec["openapi"].as_str().unwrap().starts_with("3.1"),
        "{}",
        spec["openapi"]
    );
    let paths = spec["paths"].as_object().unwrap();
    for route in [
        "/api/projects/{project}/tasks/{id}/file",
        "/api/projects/{project}/tasks/{id}/history",
        "/api/projects/{project}/sync",
    ] {
        assert!(paths.contains_key(route), "{route} is missing");
    }
    assert!(!daemon.home_path().join("daemon.json").exists());
}

fn updated_of(daemon: &Daemon, id: &str) -> serde_json::Value {
    let view: serde_json::Value =
        serde_json::from_str(&ok(daemon.run(&["get", id, "--json"]))).unwrap();
    view["updated"].clone()
}

// A read is answered from the daemon's index whether the daemon was already up or the read itself
// brought it up, so the two cannot disagree about a task's date.
#[test]
fn a_read_dates_a_task_the_same_whoever_started_the_daemon() {
    let daemon = Daemon::new();
    let id = ok(daemon.run(&["create", "Dated task"])).trim().to_owned();

    let started_by_the_write = updated_of(&daemon, &id);
    assert!(started_by_the_write.is_string(), "{started_by_the_write}");

    ok(daemon.run(&["server", "restart", "--port", "0"]));

    assert_eq!(updated_of(&daemon, &id), started_by_the_write);
}

// Reads have no local fallback, so a daemon that cannot be reached stops the command instead of
// answering from the files in front of it.
#[test]
fn a_read_with_no_reachable_daemon_fails_explicitly() {
    let daemon = Daemon::new();

    let out = daemon.run(&["--daemon", "http://127.0.0.1:1", "list"]);

    assert!(!out.status.success(), "an unreachable daemon must not pass");
    assert!(
        stderr(&out).contains("no openplan daemon at http://127.0.0.1:1"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        stdout(&out).is_empty(),
        "no task data may be printed: {}",
        stdout(&out)
    );
}

// One daemon serves every project on the machine, so a read has to name which one it is asking
// about. Two projects can hold a task of the same number, and the answer must be the caller's.
#[test]
fn a_read_is_answered_for_the_project_the_caller_stands_in() {
    let daemon = Daemon::new();
    let theirs = task_store("OPP", Some("Theirs"));
    let ours = task_store("OPP", Some("Ours"));
    ok(daemon.home.run(theirs.path(), &["project", "add"]));

    let view: serde_json::Value = serde_json::from_str(&ok(daemon
        .home
        .run(ours.path(), &["get", "OPP-1", "--json"])))
    .unwrap();

    assert_eq!(view["title"], "Ours");
}

// `--root` says which directory a command works in, as `git -C` does. It registers nothing: the
// first command that needs a project and `openplan project add` are the ways into the registry.
#[test]
fn a_start_registers_nothing_and_the_first_write_registers() {
    let daemon = Daemon::new();
    let registry = daemon.home_path().join("registry.toml");
    assert!(!registry.exists(), "a fresh OPENPLAN_HOME has no registry");

    daemon.start();
    ok(daemon.run(&["server", "ping"]));
    assert!(!registry.exists(), "starting is not registering");

    let first = daemon.run(&["create", "First task"]);
    assert!(first.status.success(), "stderr: {}", stderr(&first));
    assert!(
        stderr(&first).contains("registered project"),
        "stderr: {}",
        stderr(&first)
    );

    let seeded = daemon.home.registry();
    assert_eq!(
        seeded.matches("[[project]]").count(),
        1,
        "one entry for the project written to: {seeded}"
    );
    assert!(
        seeded.contains(&canonical(daemon.root_path())),
        "the entry names the project root: {seeded}"
    );
    assert!(
        seeded.contains("backend = \"local\""),
        "the entry keeps where the tasks live: {seeded}"
    );

    ok(daemon.run(&["create", "Second task"]));
    assert_eq!(
        daemon.home.registry(),
        seeded,
        "a project already served must not be registered again"
    );
}

// The entry is matched by its path, so a name the user chose survives a restart, and the CLI keeps
// resolving the project to it.
#[test]
fn a_renamed_project_survives_a_restart() {
    let daemon = Daemon::new();
    ok(daemon.run(&["create", "First task"]));

    let (name, root) = projects(&daemon).remove(0);
    let renamed = ok(daemon.run(&["project", "rename", &name, "chosen"]));
    assert!(renamed.contains("renamed"), "{renamed}");

    ok(daemon.run(&["server", "restart", "--port", "0"]));
    assert_eq!(projects(&daemon), vec![("chosen".to_owned(), root.clone())]);

    let write = daemon.run(&["create", "Second task"]);
    assert!(write.status.success(), "stderr: {}", stderr(&write));
    assert!(
        !stderr(&write).contains("registered"),
        "the project is still the same one, under its new name"
    );
    assert_eq!(projects(&daemon), vec![("chosen".to_owned(), root)]);
}

#[test]
fn project_rename_refuses_a_taken_or_unusable_name() {
    let daemon = Daemon::new();
    let second = task_store("BBB", None);
    ok(daemon.run(&["create", "First task"]));
    ok(daemon.run(&["project", "add", second.path().to_str().unwrap()]));
    let names = registry_names(&daemon);

    let taken = daemon.run(&["project", "rename", &names[0], &names[1]]);
    assert!(!taken.status.success());
    assert!(
        stderr(&taken).contains("already taken"),
        "{}",
        stderr(&taken)
    );

    let unusable = daemon.run(&["project", "rename", &names[0], "Not Usable"]);
    assert!(!unusable.status.success());
    assert!(
        stderr(&unusable).contains("lowercase letters"),
        "{}",
        stderr(&unusable)
    );
    assert_eq!(registry_names(&daemon), names, "nothing was renamed");
}

#[test]
fn project_add_registers_a_second_project_and_remove_leaves_its_files() {
    let daemon = Daemon::new();
    let second = task_store("OPP", Some("Shared"));
    ok(daemon.run(&["create", "First project"]));

    let added = ok(daemon.run(&["project", "add", second.path().to_str().unwrap()]));
    assert!(added.contains("registered"), "{added}");
    let again = ok(daemon.run(&["project", "add", second.path().to_str().unwrap()]));
    assert!(again.contains("already serves"), "{again}");

    let listed = projects(&daemon);
    assert!(
        listed
            .iter()
            .any(|(_, root)| *root == canonical(daemon.root_path())),
        "both projects are listed: {listed:?}"
    );
    let (name, _) = listed
        .iter()
        .find(|(_, root)| *root == canonical(second.path()))
        .unwrap_or_else(|| panic!("the added project is listed: {listed:?}"))
        .clone();

    let removed = ok(daemon.run(&["project", "remove", &name]));
    assert!(
        removed.contains("files stay on disk"),
        "removal says the files are untouched: {removed}"
    );
    assert!(
        second.path().join(".plan/tasks/00001-shared.md").exists(),
        "removing a project must not touch its files"
    );
    assert!(!projects(&daemon).iter().any(|(listed, _)| *listed == name));
}

#[test]
fn project_add_refuses_a_directory_with_no_tasks() {
    let daemon = Daemon::new();
    let empty = tempfile::tempdir().unwrap();

    let out = daemon.run(&["project", "add", empty.path().to_str().unwrap()]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("openplan init --abbreviation"),
        "{}",
        stderr(&out)
    );
    assert!(daemon.home.registry().is_empty(), "nothing was registered");
}

#[test]
fn project_list_says_when_nothing_is_registered() {
    let daemon = Daemon::new();

    let listed = ok(daemon.run(&["project", "list"]));

    assert!(listed.contains("no projects registered"), "{listed}");
}

// `--root` says which directory a command works in. `server start` runs in the daemon's own home
// and serves the whole registry, so the flag decides nothing there: every project keeps answering
// through its own prefix, and none of them captures a route.
#[test]
fn root_on_an_explicit_start_names_no_favoured_project() {
    let daemon = Daemon::new();
    let second = task_store("BBB", Some("Other"));
    ok(daemon.run(&["create", "First project"]));
    ok(daemon.run(&["project", "add", second.path().to_str().unwrap()]));

    ok(daemon
        .home
        .run(second.path(), &["server", "restart", "--port", "0"]));

    let port = daemon.info_port().unwrap();
    for name in registry_names(&daemon) {
        let answered = http_get(port, &format!("/api/projects/{name}/board"));
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
    let second = task_store("OPP", Some("Shared"));
    ok(daemon.run(&["create", "First project"]));
    ok(daemon.run(&["project", "add", second.path().to_str().unwrap()]));

    let before = registry_names(&daemon);
    assert_eq!(before.len(), 2, "{before:?}");

    ok(daemon.run(&["project", "rename", &before[0], "chosen"]));

    assert_eq!(
        registry_names(&daemon),
        vec!["chosen".to_owned(), before[1].clone()],
        "the renamed entry keeps its place"
    );
}

// An entry the daemon could not open at startup has no live project. Removing it must still work,
// or the registry the daemon says it owns could only be repaired by hand.
#[test]
fn a_registry_entry_that_cannot_be_opened_can_still_be_removed() {
    let daemon = Daemon::new();
    let second = task_store("OPP", Some("Shared"));
    ok(daemon.run(&["create", "First project"]));
    ok(daemon.run(&["project", "add", second.path().to_str().unwrap()]));
    let gone_root = canonical(second.path());
    let (gone, _) = projects(&daemon)
        .into_iter()
        .find(|(_, root)| *root == gone_root)
        .expect("the added project is listed");

    ok(daemon.run(&["server", "stop"]));
    drop(second);
    daemon.start();

    assert!(
        !projects(&daemon).iter().any(|(name, _)| name == &gone),
        "an entry that cannot be opened is not served"
    );
    ok(daemon.run(&["project", "remove", &gone]));
    let text = daemon.home.registry();
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
    ok(daemon.run(&["create", "Anchor"]));

    write(
        &daemon.root_path().join(".plan/config.toml"),
        "abbreviation = \"not valid\"\n",
    );

    let mut listed = String::new();
    wait_until(Duration::from_secs(15), || {
        listed = ok(daemon.run(&["project", "list"]));
        listed.contains('!')
    });
    assert!(
        listed.contains("three uppercase letters"),
        "the reason names the broken config: {listed}"
    );
}

// Two writes from one project can be the first one. Registration is idempotent by project, so both
// land and the registry holds one entry.
#[test]
fn concurrent_first_writes_register_one_project() {
    let daemon = Daemon::new();
    let handles: Vec<_> = (0..4)
        .map(|n| {
            let mut cmd = daemon.cmd();
            std::thread::spawn(move || cmd.args(["create", &format!("Task {n}")]).output().unwrap())
        })
        .collect();
    for handle in handles {
        ok(handle.join().unwrap());
    }

    let text = daemon.home.registry();
    assert_eq!(
        text.matches("[[project]]").count(),
        1,
        "one project is one entry: {text}"
    );
    assert_eq!(
        task_count(daemon.root_path()),
        4,
        "every write landed, each under its own id"
    );
}

// A local store is found in any ancestor, a git checkout included, so a checkout inside a directory
// that keeps its tasks locally answers from that directory.
#[test]
fn a_local_store_above_a_checkout_answers_for_it() {
    let home = Home::new();
    let outer = tempfile::tempdir().unwrap();
    ok(home.run(outer.path(), &["init", "--abbreviation", "OUT"]));
    let inner = outer.path().join("repo");
    git_repo(&inner);

    let created = home.run(&inner, &["create", "From the checkout"]);

    assert_eq!(ok(created).trim(), "OUT-1");
    assert_eq!(task_count(outer.path()), 1, "the task lands in the store");
    let registry = home.registry();
    assert_eq!(registry.matches("[[project]]").count(), 1, "{registry}");
    assert!(
        registry.contains(&canonical(outer.path())),
        "the entry names the directory that holds the tasks: {registry}"
    );
}

// `openplan open` hands the URL to a launcher command. A stub in $BROWSER records the arguments a
// real browser would have received, then runs `tail`: `exit 0`, a failing exit, or a sleep that
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
    soon(|| record_path(dir, name).exists());
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

    let printed = ok(out);
    let port = daemon
        .info_port()
        .expect("open must start a daemon and record its port");
    let url = format!("http://127.0.0.1:{port}/");
    assert_eq!(launched(stub.path(), "browser"), url);
    assert!(printed.contains(&url), "{printed}");
}

// The UI lists the projects the daemon serves. Opening it from a project the daemon does not serve
// yet must show that project, so `open` registers it as a first write does.
#[test]
fn open_registers_the_project_the_caller_stands_in() {
    let daemon = Daemon::new();
    let stub = tempfile::tempdir().unwrap();
    let script = browser_stub(stub.path(), "browser", "exit 0");

    ok(daemon
        .cmd()
        .env("BROWSER", &script)
        .arg("open")
        .output()
        .unwrap());

    let registry = daemon.home.registry();
    assert!(
        registry.contains(&canonical(daemon.root_path())),
        "open must register the caller's project: {registry}"
    );
}

#[test]
fn open_places_the_url_where_browser_spells_it() {
    let daemon = Daemon::new();
    let stub = tempfile::tempdir().unwrap();
    let script = browser_stub(stub.path(), "browser", "exit 0");

    ok(daemon
        .cmd()
        .env("BROWSER", format!("{} %s --new-window", script.display()))
        .arg("open")
        .output()
        .unwrap());

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

    ok(daemon
        .cmd()
        .env(
            "BROWSER",
            format!("{}:{}", missing.display(), script.display()),
        )
        .arg("open")
        .output()
        .unwrap());

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

    ok(out);
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
    serving.start();
    let url = format!("http://127.0.0.1:{}", serving.info_port().unwrap());

    let caller = Daemon::new();
    let stub = tempfile::tempdir().unwrap();
    let script = browser_stub(stub.path(), "browser", "exit 0");
    ok(caller
        .cmd()
        .env("BROWSER", &script)
        .args(["--daemon", &url, "open"])
        .output()
        .unwrap());

    assert_eq!(launched(stub.path(), "browser"), format!("{url}/"));
    assert!(
        !caller.home_path().join("daemon.json").exists(),
        "--daemon must not start a machine daemon"
    );
    assert!(
        !serving.home_path().join("registry.toml").exists(),
        "--daemon must not register the caller's project on a borrowed daemon"
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
    assert!(
        stderr(&out).contains("did not open"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        stdout(&out).is_empty(),
        "no URL may be printed as a fallback: {}",
        stdout(&out)
    );
}
