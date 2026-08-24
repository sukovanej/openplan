use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use op_task::Timestamp;

fn frontmatter_value(contents: &str, key: &str) -> String {
    let prefix = format!("{key}: ");
    contents
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("no {key} in {contents}"))
        .to_owned()
}

fn openplan() -> Command {
    Command::new(env!("CARGO_BIN_EXE_openplan"))
}

fn write(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
}

// A project every command can reach: a git repository (the daemon that owns reads and writes
// resolves the target worktree by branch, so it needs one) plus a private OPENPLAN_HOME so each
// test gets its own daemon, auto-started on the first command. OPENPLAN_PORT=0 keeps those daemons
// off a shared port.
struct Project {
    home: tempfile::TempDir,
    root: tempfile::TempDir,
}

impl Project {
    fn new() -> Self {
        let project = Self {
            home: tempfile::tempdir().unwrap(),
            root: tempfile::tempdir().unwrap(),
        };
        git(project.path(), &["init", "-q", "-b", "main"]);
        git(project.path(), &["config", "user.email", "t@example.com"]);
        git(project.path(), &["config", "user.name", "Test"]);
        std::fs::create_dir_all(project.path().join(".plan/tasks")).unwrap();
        write(
            &project.path().join(".plan/config.toml"),
            "abbreviation = \"OPP\"\n",
        );
        // The index walks `refs/heads/*`, so an unborn HEAD has no branch for a read to be scoped
        // to. Giving the store its own birthing commit is what `op-server`'s harness does; indexing
        // a worktree whose branch no commit holds yet is [[OPP-58]]'s to fix.
        git(project.path(), &["add", "."]);
        git(project.path(), &["commit", "-qm", "birth the store"]);
        project
    }

    fn path(&self) -> &Path {
        self.root.path()
    }

    fn cmd(&self) -> Command {
        let mut cmd = openplan();
        cmd.env("OPENPLAN_HOME", self.home.path())
            .env("OPENPLAN_PORT", "0");
        cmd
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self.cmd().args(["server", "stop"]).output();
    }
}

fn run(project: &Project, args: &[&str]) -> Output {
    project
        .cmd()
        .arg("--root")
        .arg(project.path())
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn create(project: &Project, title: &str) -> String {
    let out = run(project, &["create", title]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout(&out).trim().to_owned()
}

// A task file's name carries a title slug its id does not, so a test locates it by the
// number behind the key.
fn task_file(root: &Path, key: &str) -> PathBuf {
    let dir = root.join(".plan/tasks");
    let prefix = format!("{:0>5}-", number(key));
    std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix))
        })
        .unwrap_or_else(|| panic!("no file for task {key} in {}", dir.display()))
}

// The file layer's spelling of an id a command printed.
fn number(key: &str) -> u64 {
    key.strip_prefix("OPP-")
        .unwrap_or_else(|| panic!("not a key: {key}"))
        .parse()
        .unwrap()
}

fn task_body(path: &PathBuf) -> String {
    std::fs::read_to_string(path)
        .unwrap()
        .splitn(3, "---\n")
        .nth(2)
        .unwrap()
        .to_owned()
}

#[test]
fn list_reports_real_status_and_title() {
    let dir = Project::new();
    write(
        &dir.path().join(".plan/tasks/00001-ship-it.md"),
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship it\n",
    );

    let out = run(&dir, &["list"]);

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = stdout(&out);
    assert!(stdout.contains("OPP-1"), "the id is the key: {stdout}");
    assert!(
        stdout.contains("done"),
        "status must be read from the file: {stdout}"
    );
    assert!(
        stdout.contains("Ship it"),
        "title must be read from the body: {stdout}"
    );
}

#[test]
fn search_finds_a_task_by_its_body_and_reports_the_branch() {
    let dir = Project::new();
    write(
        &dir.path().join(".plan/tasks/00001-ship-it.md"),
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship it\n\nIt needs a zeppelin.\n",
    );
    write(
        &dir.path().join(".plan/tasks/00002-paint-it.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Paint it\n",
    );

    let out = run(&dir, &["search", "ZEPPELIN"]);

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = stdout(&out);
    assert!(
        stdout.contains("main"),
        "the branch it matched on: {stdout}"
    );
    assert!(stdout.contains("OPP-1"), "the key: {stdout}");
    assert!(stdout.contains("done"), "the status: {stdout}");
    assert!(stdout.contains("Ship it"), "the title: {stdout}");
    assert!(!stdout.contains("OPP-2"), "and nothing else: {stdout}");
}

#[test]
fn search_reports_no_matches_rather_than_nothing() {
    let dir = Project::new();
    write(
        &dir.path().join(".plan/tasks/00001-ship-it.md"),
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship it\n",
    );

    let out = run(&dir, &["search", "kubernetes"]);

    assert!(out.status.success());
    assert!(stdout(&out).contains("no matching tasks"));
}

#[test]
fn search_json_carries_the_hit_and_its_branch() {
    let dir = Project::new();
    write(
        &dir.path().join(".plan/tasks/00001-ship-it.md"),
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship it\n",
    );

    let out = run(&dir, &["search", "ship", "--json"]);

    assert!(out.status.success());
    let hits: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let hits = hits.as_array().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["id"], "OPP-1");
    assert_eq!(hits[0]["task"]["metadata"]["status"], "done");
    assert_eq!(hits[0]["branch"], "main");
}

#[test]
fn list_discovers_store_from_subdirectory() {
    let dir = Project::new();
    let nested = dir.path().join("crates/thing/src");
    std::fs::create_dir_all(&nested).unwrap();

    let out = dir.cmd().current_dir(&nested).arg("list").output().unwrap();

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout(&out).contains("no tasks yet"));
}

#[test]
fn merge_driver_clean_conflict_and_read_error() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.md");
    let ours = dir.path().join("ours.md");
    let theirs = dir.path().join("theirs.md");
    let content = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n\n## Plan\nhi\n";
    write(&base, content);
    write(&ours, content);
    write(&theirs, content);

    let clean = openplan()
        .arg("merge-driver")
        .args([&base, &ours, &theirs])
        .output()
        .unwrap();
    assert!(
        clean.status.success(),
        "identical inputs should merge cleanly"
    );

    write(
        &theirs,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n\n## Plan\nBYE\n",
    );
    let conflict = openplan()
        .arg("merge-driver")
        .args([&base, &ours, &theirs])
        .output()
        .unwrap();
    assert!(
        !conflict.status.success(),
        "divergent inputs should conflict"
    );

    let missing = dir.path().join("nope.md");
    let read_error = openplan()
        .arg("merge-driver")
        .args([&missing, &missing, &missing])
        .output()
        .unwrap();
    assert!(
        !read_error.status.success(),
        "unreadable inputs must fail, not report a clean merge"
    );
}

#[test]
fn create_names_the_file_after_the_id_and_the_title() {
    let dir = Project::new();
    let before = op_task::now();
    let id = create(&dir, "Wire the parser");
    assert_eq!(id, "OPP-1", "the id printed is the key");

    let path = task_file(dir.path(), &id);
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        "00001-wire-the-parser.md",
        "the file name pads the id for sorting and carries the title for reading"
    );
    let contents = std::fs::read_to_string(&path).unwrap();
    let created = frontmatter_value(&contents, "created");
    assert_eq!(
        contents,
        format!("---\nstatus: backlog\ncreated: {created}\n---\n# Wire the parser\n")
    );
    assert!(
        created.parse::<Timestamp>().unwrap() >= before,
        "created must come from the clock at creation: {created}"
    );
    assert!(
        !created.contains('.'),
        "a stored timestamp carries whole seconds: {created}"
    );

    // The number is allocated, not derived from the title, so the same title yields the next id.
    assert_eq!(create(&dir, "Wire the parser"), "OPP-2");
    assert!(
        dir.home.path().join("daemon.json").is_file(),
        "the first write brought the daemon up on its own"
    );
}

#[test]
fn a_write_from_a_linked_worktree_lands_on_that_worktrees_branch() {
    let dir = Project::new();
    let anchor = create(&dir, "Anchor");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "anchor"]);
    let feature = dir.path().join(".worktrees/feature");
    git(
        dir.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            feature.to_str().unwrap(),
        ],
    );

    // The daemon serves the main checkout, but the write names the caller's branch — so it lands in
    // the worktree that holds that branch, not in the daemon's own.
    let out = dir
        .cmd()
        .arg("--root")
        .arg(&feature)
        .args(["create", "From the worktree"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let id = stdout(&out).trim().to_owned();

    assert!(
        task_file(&feature, &id).is_file(),
        "written on feature: {id}"
    );
    assert!(
        std::fs::read_dir(dir.path().join(".plan/tasks"))
            .unwrap()
            .all(
                |entry| entry.unwrap().file_name() != task_file(&feature, &id).file_name().unwrap()
            ),
        "main's worktree is untouched"
    );
    assert_ne!(id, anchor, "the allocator's floor spans both branches");
}

#[test]
fn removing_the_worktree_that_started_the_daemon_leaves_writes_working() {
    let dir = Project::new();
    // Committed by hand: the daemon must be started by the worktree's write, not by this one.
    write(
        &dir.path().join(".plan/tasks/00001-anchor.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Anchor\n",
    );
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "anchor"]);
    let feature = dir.path().join(".worktrees/feature");
    git(
        dir.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            feature.to_str().unwrap(),
        ],
    );

    // This write starts the daemon. A worktree per task is this repository's normal workflow, so
    // anchoring the daemon's root here would make every later write depend on a directory that
    // exists only until the task is merged.
    let out = dir
        .cmd()
        .arg("--root")
        .arg(&feature)
        .args(["create", "From the worktree"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    git(
        dir.path(),
        &["worktree", "remove", "--force", feature.to_str().unwrap()],
    );
    let out = run(&dir, &["create", "From main"]);
    assert!(
        out.status.success(),
        "the daemon outlives the worktree that started it; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let id = stdout(&out).trim().to_owned();
    assert!(
        task_file(dir.path(), &id).is_file(),
        "written in the main checkout: {id}"
    );
}

// A read is scoped to the caller's branch, so the worktree a command stands in decides the answer —
// including for a task that branch agrees with main about and therefore has no divergence cell of
// its own.
#[test]
fn a_read_from_a_linked_worktree_answers_for_that_worktrees_branch() {
    let dir = Project::new();
    let alpha = create(&dir, "Alpha");
    let shared = create(&dir, "Shared");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "two tasks"]);
    let feature = dir.path().join(".worktrees/feature");
    git(
        dir.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            feature.to_str().unwrap(),
        ],
    );
    let on_feature = |args: &[&str]| {
        dir.cmd()
            .arg("--root")
            .arg(&feature)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(
        on_feature(&["set", &alpha, "status", "done"])
            .status
            .success()
    );

    let moved = on_feature(&["show", &alpha]);
    assert!(
        stdout(&moved).contains("status: done"),
        "the worktree's own branch: {}",
        stdout(&moved)
    );
    assert!(
        stdout(&run(&dir, &["show", &alpha])).contains("status: backlog"),
        "the serve root keeps its own version"
    );

    let agreed = on_feature(&["show", &shared]);
    assert!(
        agreed.status.success(),
        "a task the branch agrees with main about is still the branch's own; stderr: {}",
        String::from_utf8_lossy(&agreed.stderr)
    );
    let listed = stdout(&on_feature(&["list"]));
    assert!(
        listed.contains(&alpha) && listed.contains(&shared),
        "the branch lists everything it carries: {listed}"
    );
}

// A read scoped to one branch answers about that branch, so a task it does not carry fails — but the
// failure says where the task does live rather than claiming there is no such task.
#[test]
fn a_read_names_the_branches_that_hold_a_task_this_one_does_not() {
    let dir = Project::new();
    let feature = dir.path().join(".worktrees/feature");
    git(
        dir.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            feature.to_str().unwrap(),
        ],
    );
    let created = dir
        .cmd()
        .arg("--root")
        .arg(&feature)
        .args(["create", "Only on feature"])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let id = stdout(&created).trim().to_owned();

    let here = run(&dir, &["get", &id]);
    assert!(!here.status.success(), "main does not carry it");
    let stderr = String::from_utf8_lossy(&here.stderr);
    assert!(
        stderr.contains(&id) && stderr.contains("feature") && stderr.contains("dirty"),
        "stderr: {stderr}"
    );

    let there = run(&dir, &["get", &id, "--branch", "feature"]);
    assert!(
        there.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&there.stderr)
    );
    assert!(
        stdout(&there).contains("# Only on feature"),
        "{}",
        stdout(&there)
    );
}

#[test]
fn a_write_with_no_reachable_daemon_fails_explicitly() {
    let dir = Project::new();
    let out = dir
        .cmd()
        .args(["--daemon", "http://127.0.0.1:1", "--root"])
        .arg(dir.path())
        .args(["create", "Ship login"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "an unreachable daemon must not pass");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no openplan daemon at http://127.0.0.1:1"),
        "stderr: {stderr}"
    );
    assert!(stdout(&run(&dir, &["list"])).contains("no tasks yet"));
}

#[test]
fn a_command_outside_a_git_repository_is_refused() {
    // Every read and write resolves its worktree by branch, so a store with no repository has none.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    let out = openplan()
        .env("OPENPLAN_HOME", dir.path().join("home"))
        .env("OPENPLAN_PORT", "0")
        .arg("--root")
        .arg(dir.path())
        .args(["create", "Ship login"])
        .output()
        .unwrap();

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("requires a git repository"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let read = openplan()
        .env("OPENPLAN_HOME", dir.path().join("home"))
        .env("OPENPLAN_PORT", "0")
        .arg("--root")
        .arg(dir.path())
        .arg("list")
        .output()
        .unwrap();
    assert!(
        !read.status.success(),
        "a read has no local fallback either"
    );
}

// One daemon serves every repository on the machine. A write from a repository it does not yet know
// registers that repository and lands, with no setup step and no restart.
#[test]
fn writes_from_two_repositories_land_in_their_own_stores() {
    let first = Project::new();
    let second = Project::new();
    create(&first, "Anchor");

    let out = openplan()
        .env("OPENPLAN_HOME", first.home.path())
        .env("OPENPLAN_PORT", "0")
        .arg("--root")
        .arg(second.path())
        .args(["create", "Ship login"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Each repository has its own id counter, so both first tasks are number one.
    assert_eq!(stdout(&out).trim(), "OPP-1");
    assert_eq!(
        std::fs::read_dir(second.path().join(".plan/tasks"))
            .unwrap()
            .count(),
        1,
        "the write lands in the repository --root names"
    );
    assert_eq!(
        std::fs::read_dir(first.path().join(".plan/tasks"))
            .unwrap()
            .count(),
        1,
        "and nothing leaks into the other one"
    );
}

// A daemon older than the project routes answers /health and falls every unknown path through to
// the SPA. A write cannot learn which project it is talking to there, so it stops and says how to
// fix it rather than write into whatever the daemon happens to serve.
#[test]
fn a_daemon_without_project_routes_asks_for_a_restart() {
    let project = Project::new();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut head = [0u8; 1024];
            let read = std::io::Read::read(&mut stream, &mut head).unwrap_or(0);
            let (kind, body) = match head[..read].starts_with(b"GET /health") {
                true => (
                    "application/json",
                    r#"{"pid":1,"port":1,"version":"0.0.1","started_at":0}"#,
                ),
                false => ("text/html", "<!doctype html>"),
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: \
                 close\r\n\r\n{body}",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        }
    });

    let out = project
        .cmd()
        .arg("--root")
        .arg(project.path())
        .args([
            "--daemon",
            &format!("http://127.0.0.1:{port}"),
            "create",
            "Ship login",
        ])
        .output()
        .unwrap();

    assert!(!out.status.success(), "the write must not be attempted");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("predates") && stderr.contains("openplan server stop"),
        "stderr: {stderr}"
    );
    assert!(
        std::fs::read_dir(project.path().join(".plan/tasks"))
            .unwrap()
            .next()
            .is_none(),
        "nothing was written"
    );
}

// `--daemon` borrows a daemon for one command. Registering there would leave it indexing and
// watching a checkout the caller never asked it to serve, with two daemons then writing one store.
#[test]
fn a_named_daemon_is_not_registered_into_by_a_write() {
    let served = Project::new();
    let other = Project::new();
    create(&served, "Anchor");
    let port = std::fs::read_to_string(served.home.path().join("daemon.json"))
        .map(|text| serde_json::from_str::<serde_json::Value>(&text).unwrap()["port"].as_u64())
        .unwrap()
        .unwrap();

    let out = openplan()
        .env("OPENPLAN_HOME", other.home.path())
        .env("OPENPLAN_PORT", "0")
        .arg("--root")
        .arg(other.path())
        .args([
            "--daemon",
            &format!("http://127.0.0.1:{port}"),
            "create",
            "Ship login",
        ])
        .output()
        .unwrap();

    assert!(!out.status.success(), "the write must be refused");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("openplan project add --daemon"),
        "the refusal names the explicit way in: {stderr}"
    );
    let registry = served.home.path().join("registry.toml");
    let text = std::fs::read_to_string(&registry).unwrap();
    assert_eq!(
        text.matches("[[project]]").count(),
        1,
        "the named daemon keeps the projects it had: {text}"
    );
    assert!(
        std::fs::read_dir(other.path().join(".plan/tasks"))
            .unwrap()
            .next()
            .is_none(),
        "and nothing was written"
    );
}

#[test]
fn only_the_first_write_from_a_repository_reports_a_registration() {
    let project = Project::new();

    let first = run(&project, &["create", "Anchor"]);
    assert!(first.status.success());
    let reported = String::from_utf8_lossy(&first.stderr);
    assert!(
        reported.contains("registered project"),
        "stderr: {reported}"
    );

    let second = run(&project, &["create", "Ship login"]);
    assert!(second.status.success());
    assert!(
        !String::from_utf8_lossy(&second.stderr).contains("registered"),
        "a repository already served is registered once, and said so once"
    );
    assert_eq!(
        stdout(&first).trim(),
        "OPP-1",
        "the id stays the only thing on stdout"
    );
}

#[test]
fn get_show_and_missing_id() {
    let dir = Project::new();
    let id = create(&dir, "Ship it");

    let show = run(&dir, &["show", &id]);
    assert!(show.status.success());
    let text = stdout(&show);
    assert!(text.contains("status: backlog"), "{text}");
    assert!(text.contains("title:  Ship it"), "{text}");

    let get = run(&dir, &["get", &id, "--json"]);
    assert!(get.status.success());
    let view: serde_json::Value = serde_json::from_slice(&get.stdout).unwrap();
    assert_eq!(view["title"], "Ship it");
    assert_eq!(view["metadata"]["status"], "backlog");

    let missing = run(&dir, &["get", "does-not-exist"]);
    assert!(
        !missing.status.success(),
        "get on a missing id must exit non-zero"
    );
    assert!(String::from_utf8_lossy(&missing.stderr).contains("does-not-exist"));
}

#[test]
fn set_updates_only_frontmatter() {
    let dir = Project::new();
    let id = create(&dir, "Ship it");
    let path = task_file(dir.path(), &id);
    let body_before = task_body(&path);

    let set = run(&dir, &["set", &id, "status", "in_progress"]);
    assert!(
        set.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&set.stderr)
    );

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(
        contents.starts_with("---\nstatus: in_progress\ncreated: "),
        "{contents}"
    );
    assert_eq!(
        task_body(&path),
        body_before,
        "body must be byte-for-byte unchanged"
    );

    let bad_status = run(&dir, &["set", &id, "status", "bogus"]);
    assert!(
        !bad_status.status.success(),
        "invalid status must be rejected"
    );

    let bad_parent = run(&dir, &["set", &id, "parent", "ghost"]);
    assert!(
        !bad_parent.status.success(),
        "non-existent parent must be rejected"
    );
}

#[test]
fn delete_removes_the_file() {
    let dir = Project::new();
    let id = create(&dir, "Temporary");
    let path = task_file(dir.path(), &id);
    assert!(path.exists());

    let del = run(&dir, &["delete", &id, "--yes"]);
    assert!(del.status.success());
    assert!(!path.exists(), "delete must remove the file");

    let list = run(&dir, &["list"]);
    assert!(
        !stdout(&list).contains(&id),
        "deleted id must not appear in list"
    );
}

#[test]
fn delete_of_a_missing_id_fails_before_it_prompts() {
    let dir = Project::new();
    // No `--yes`: a typo must be refused outright rather than put to the reader as a question.
    let out = run(&dir, &["delete", "OPP-99"]);

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("no such task: OPP-99"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !stdout(&out).contains("delete OPP-99?"),
        "stdout: {}",
        stdout(&out)
    );
}

#[test]
fn list_json_filters_by_status() {
    let dir = Project::new();
    let todo = create(&dir, "Still to do");
    let done = create(&dir, "Already done");
    for (id, status) in [(&todo, "todo"), (&done, "done")] {
        assert!(run(&dir, &["set", id, "status", status]).status.success());
    }

    let out = run(&dir, &["list", "--json", "--status", "todo"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tasks: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tasks.len(), 1, "only the todo task should match: {tasks:?}");
    assert_eq!(tasks[0]["id"], todo);
    assert_eq!(tasks[0]["metadata"]["status"], "todo");
}

#[test]
fn set_status_survives_a_deleted_dependency() {
    let dir = Project::new();
    let a = create(&dir, "Task A");
    let b_out = run(&dir, &["create", "Task B", "--dependency", &a]);
    assert!(b_out.status.success());
    let b = stdout(&b_out).trim().to_owned();

    assert!(run(&dir, &["delete", &a, "--yes"]).status.success());

    // B still lists A as a dep, but changing B's status is unrelated and must succeed.
    let set = run(&dir, &["set", &b, "status", "done"]);
    assert!(
        set.status.success(),
        "a deleted dep must not block a status change: {}",
        String::from_utf8_lossy(&set.stderr)
    );
    assert!(stdout(&run(&dir, &["show", &b])).contains("status: done"));
}

#[test]
fn set_preserves_unknown_frontmatter_keys() {
    let dir = Project::new();
    let id = create(&dir, "Task C");
    let path = task_file(dir.path(), &id);
    write(
        &path,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nestimate: 3.5\nassignee: milan\n---\n# Task C\n",
    );

    assert!(run(&dir, &["set", &id, "status", "done"]).status.success());

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(
        contents.contains("estimate: 3.5"),
        "estimate dropped: {contents}"
    );
    assert!(
        contents.contains("assignee: milan"),
        "assignee dropped: {contents}"
    );
    assert!(contents.contains("status: done"));
}

// The daemon holds the task parsed, not the bytes it parsed, so `get` renders that state back to
// markdown. Key order, spacing, and keys no field names normalize; the task itself does not change.
#[test]
fn get_renders_the_daemons_state_rather_than_the_file() {
    let dir = Project::new();
    let id = create(&dir, "Task D");
    let path = task_file(dir.path(), &id);
    write(
        &path,
        "---\nassignee: milan\nrank:   '7'\ncreated: 2026-01-01T00:00:00Z\nstatus: todo\n---\n# Task D\n\nsome body\n",
    );

    let out = run(&dir, &["get", &id]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        stdout(&out),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nrank: '7'\n---\n# Task D\n\nsome body\n"
    );
}

// The rendering looks like a task file, so its references have to be ones the store can read: a
// reference is the number the file layer allocates there, where every surface above the store
// speaks the key. Unknown frontmatter keys are a separate matter — `Metadata` does not carry them,
// so the rendering cannot either.
#[test]
fn get_renders_references_the_store_can_read_back() {
    let dir = Project::new();
    let parent = create(&dir, "Parent");
    let kid = child(&dir, "Kid", &parent);
    let dependent = run(&dir, &["create", "Dependent", "--dependency", &kid]);
    assert!(dependent.status.success());
    let dependent = stdout(&dependent).trim().to_owned();

    for id in [&kid, &dependent] {
        let rendered = run(&dir, &["get", id]);
        assert!(rendered.status.success());
        write(&task_file(dir.path(), id), &stdout(&rendered));
    }

    let kid_again = run(&dir, &["show", &kid]);
    assert!(
        stdout(&kid_again).contains(&format!("parent: {parent}")),
        "the parent survives a round-trip through `get`: {}",
        stdout(&kid_again)
    );
    let dependent_again = run(&dir, &["show", &dependent]);
    assert!(
        stdout(&dependent_again).contains(&format!("dependencies: {kid}")),
        "and so do the dependencies: {}",
        stdout(&dependent_again)
    );
    assert!(
        !stdout(&kid_again).contains('!'),
        "no field may come back unreadable: {}",
        stdout(&kid_again)
    );
}

// The rendering is a task file, and `status` and `created` are what make one. A task missing either
// cannot be rendered at all: emitting the rest would look like a task file and destroy the task if
// it were written over one.
#[test]
fn get_refuses_to_render_a_task_missing_a_required_field() {
    let dir = Project::new();
    write(
        &dir.path().join(".plan/tasks/00001-legacy.md"),
        "---\nstatus: todo\n---\n# Legacy\n",
    );

    let out = run(&dir, &["get", "OPP-1"]);
    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    assert!(
        stdout(&out).is_empty(),
        "nothing that reads as a task file: {}",
        stdout(&out)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("created: missing"), "stderr: {stderr}");
    assert!(
        stderr.contains("cannot be rendered as a task file"),
        "stderr: {stderr}"
    );

    // `--json` answers with the state the daemon holds, which is exactly what a broken file needs
    // read, so it keeps working.
    let json = run(&dir, &["get", "OPP-1", "--json"]);
    assert!(json.status.success());
    let view: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(view["metadata"]["created"]["kind"], "missing");
}

// A field that is not required has no canonical form when it fails to parse, so it is left out and
// named on stderr rather than dropped in silence.
#[test]
fn get_reports_a_field_it_cannot_render() {
    let dir = Project::new();
    write(
        &dir.path().join(".plan/tasks/00001-legacy.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nrank: [1, 2]\n---\n# Legacy\n",
    );

    let out = run(&dir, &["get", "OPP-1"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        stdout(&out),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Legacy\n"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("rank:"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn create_with_body_places_content_below_title() {
    let dir = Project::new();
    let out = run(
        &dir,
        &[
            "create",
            "Ship login",
            "--body",
            "Support OAuth and email login.",
        ],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let id = stdout(&out).trim().to_owned();

    let path = task_file(dir.path(), &id);
    let contents = std::fs::read_to_string(&path).unwrap();
    let created = frontmatter_value(&contents, "created");
    assert_eq!(
        contents,
        format!(
            "---\nstatus: backlog\ncreated: {created}\n---\n# Ship login\n\nSupport OAuth and email login.\n"
        )
    );

    let view = run(&dir, &["get", &id, "--json"]);
    let json: serde_json::Value = serde_json::from_slice(&view.stdout).unwrap();
    assert_eq!(json["title"], "Ship login");
    assert_eq!(
        json["body"],
        "# Ship login\n\nSupport OAuth and email login.\n"
    );
}

#[test]
fn create_with_body_file_reads_the_file() {
    let dir = Project::new();
    let notes = dir.path().join("notes.md");
    write(&notes, "## Goals\n- OAuth\n- Email + password\n");

    let out = run(
        &dir,
        &[
            "create",
            "Ship login",
            "--body-file",
            notes.to_str().unwrap(),
        ],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let id = stdout(&out).trim().to_owned();

    let path = task_file(dir.path(), &id);
    assert_eq!(
        task_body(&path),
        "# Ship login\n\n## Goals\n- OAuth\n- Email + password\n"
    );
}

#[test]
fn create_with_body_file_dash_reads_stdin() {
    let dir = Project::new();
    let mut child = dir
        .cmd()
        .arg("--root")
        .arg(dir.path())
        .args(["create", "Ship login", "--body-file", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"## Goals\n- OAuth\n- Email + password\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let id = stdout(&out).trim().to_owned();

    let path = task_file(dir.path(), &id);
    assert_eq!(
        task_body(&path),
        "# Ship login\n\n## Goals\n- OAuth\n- Email + password\n"
    );
}

#[test]
fn create_rejects_body_with_body_file() {
    let dir = Project::new();
    let out = run(
        &dir,
        &[
            "create",
            "Ship login",
            "--body",
            "x",
            "--body-file",
            "notes.md",
        ],
    );
    assert!(
        !out.status.success(),
        "--body and --body-file are mutually exclusive"
    );
    assert!(stdout(&run(&dir, &["list"])).contains("no tasks yet"));
}

#[test]
fn create_rejects_malformed_title() {
    let dir = Project::new();
    assert!(
        !run(&dir, &["create", ""]).status.success(),
        "an empty title must be rejected"
    );
    assert!(
        !run(&dir, &["create", "line one\n# line two"])
            .status
            .success(),
        "a title producing two H1 headings must be rejected"
    );
    assert!(stdout(&run(&dir, &["list"])).contains("no tasks yet"));
}

#[test]
fn list_distinguishes_empty_store_from_empty_filter() {
    let dir = Project::new();
    assert!(stdout(&run(&dir, &["list"])).contains("no tasks yet"));

    create(&dir, "A todo");
    let filtered = stdout(&run(&dir, &["list", "--status", "done"]));
    assert!(filtered.contains("no matching tasks"), "{filtered}");
    assert!(!filtered.contains("no tasks yet"), "{filtered}");
}

#[test]
fn list_json_carries_an_unreadable_task_rather_than_dropping_it() {
    let dir = Project::new();
    let good = create(&dir, "Good one");
    write(
        &dir.path().join(".plan/tasks/00002-broken.md"),
        "this file has no frontmatter\n",
    );
    write(
        &dir.path().join(".plan/tasks/00003-legacy.md"),
        "---\nstatus: in_progress\n---\n# Legacy\n",
    );

    let out = run(&dir, &["list", "--json"]);
    assert!(out.status.success());
    let tasks: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    let by_id = |id: &str| {
        tasks
            .iter()
            .find(|t| t["id"] == id)
            .unwrap_or_else(|| panic!("{id} missing from {tasks:?}"))
            .clone()
    };

    assert_eq!(by_id(&good)["metadata"]["status"], "backlog");
    // A file with no readable frontmatter says so, instead of borrowing a status it never claimed.
    assert_eq!(by_id("OPP-2")["metadata"]["kind"], "error");
    // One unreadable field costs only itself: the status is still the file's own.
    assert_eq!(by_id("OPP-3")["metadata"]["status"], "in_progress");
    assert_eq!(by_id("OPP-3")["metadata"]["created"]["kind"], "missing");
}

#[test]
fn list_filters_by_status_across_an_unreadable_task() {
    let dir = Project::new();
    write(
        &dir.path().join(".plan/tasks/00001-broken.md"),
        "this file has no frontmatter\n",
    );
    let good = create(&dir, "Good one");

    let out = run(&dir, &["list", "--json", "--status", "backlog"]);
    let tasks: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();

    // A task with no readable status matches no status filter, rather than matching the default.
    assert_eq!(tasks.len(), 1, "{tasks:?}");
    assert_eq!(tasks[0]["id"], good);
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed");
}

impl Project {
    // A project whose `alpha` task diverges: `todo`/`# Alpha` on main, `done`/`# Alpha done` on
    // feature. The working tree is left on main.
    fn diverged() -> Self {
        let project = Project::new();
        let root = project.path();
        write(
            &root.join(".plan/tasks/00001-alpha.md"),
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
        );
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "init"]);
        git(root, &["checkout", "-q", "-b", "feature"]);
        write(
            &root.join(".plan/tasks/00001-alpha.md"),
            "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha done\n",
        );
        git(root, &["commit", "-qam", "edit alpha on feature"]);
        git(root, &["checkout", "-q", "main"]);
        project
    }
}

#[test]
fn list_all_branches_json_has_one_row_per_task_branch() {
    let dir = Project::diverged();
    let out = run(&dir, &["list", "--all-branches", "--json"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let matrix: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let cells = matrix["cells"].as_array().unwrap();
    assert_eq!(
        cells.len(),
        2,
        "alpha on main + alpha on feature: {cells:?}"
    );
    let branches: Vec<&str> = cells
        .iter()
        .map(|c| c["branch"].as_str().unwrap())
        .collect();
    assert!(
        branches.contains(&"main") && branches.contains(&"feature"),
        "{branches:?}"
    );
}

#[test]
fn list_branch_reads_other_branch_without_checking_it_out() {
    let dir = Project::diverged();
    let out = run(&dir, &["list", "--branch", "feature", "--json"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let tasks: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["metadata"]["status"], "done", "feature's version");
    assert_eq!(tasks[0]["title"], "Alpha done");

    // The current worktree is untouched: still on main, alpha still `todo`.
    let head = run(&dir, &["get", "OPP-1", "--json"]);
    let view: serde_json::Value = serde_json::from_slice(&head.stdout).unwrap();
    assert_eq!(
        view["metadata"]["status"], "todo",
        "reading a branch must not mutate the worktree"
    );
    let on_disk = std::fs::read_to_string(dir.path().join(".plan/tasks/00001-alpha.md")).unwrap();
    assert!(
        on_disk.contains("status: todo"),
        "working file unchanged: {on_disk}"
    );
}

#[test]
fn list_branch_nonexistent_errors_nonzero() {
    let dir = Project::diverged();
    let out = run(&dir, &["list", "--branch", "ghost"]);
    assert!(!out.status.success(), "a missing branch must fail");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("no such branch"),
        "clear message: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn get_branch_prints_that_branchs_version() {
    let dir = Project::diverged();
    let out = run(&dir, &["get", "OPP-1", "--branch", "feature"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    assert!(text.contains("status: done"), "{text}");
    assert!(text.contains("# Alpha done"), "{text}");

    let missing = run(&dir, &["get", "OPP-99", "--branch", "feature"]);
    assert!(
        !missing.status.success(),
        "a missing task on a branch must fail"
    );
}

#[test]
fn show_branches_groups_and_flags_divergence() {
    let dir = Project::diverged();
    let out = run(&dir, &["show", "OPP-1", "--branches"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    assert!(
        text.contains("divergent"),
        "diverging versions flagged: {text}"
    );
    assert!(text.contains("main"), "{text}");
    assert!(text.contains("feature"), "{text}");
}

#[test]
fn set_rejects_a_cross_branch_write() {
    let dir = Project::diverged();
    // There is deliberately no cross-branch write flag; `--branch` is not a valid arg for `set`.
    let out = run(
        &dir,
        &["set", "OPP-1", "--branch", "feature", "status", "done"],
    );
    assert!(
        !out.status.success(),
        "writes must not accept a --branch target"
    );

    // feature's committed version is untouched by the failed attempt.
    let feature = run(&dir, &["get", "OPP-1", "--branch", "feature", "--json"]);
    let view: serde_json::Value = serde_json::from_slice(&feature.stdout).unwrap();
    assert_eq!(view["metadata"]["status"], "done");
}

fn child(project: &Project, title: &str, parent: &str) -> String {
    let out = run(project, &["create", title, "--parent", parent]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout(&out).trim().to_owned()
}

fn tree_ids(project: &Project, id: &str) -> Vec<String> {
    let out = run(project, &["tree", id, "--json"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    tree["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn set_parent_empty_clears_to_top_level() {
    let dir = Project::new();
    let parent = create(&dir, "Parent");
    let kid = child(&dir, "Kid", &parent);

    assert!(run(&dir, &["set", &kid, "parent", ""]).status.success());

    let show = run(&dir, &["show", &kid]);
    assert!(stdout(&show).contains("parent: -"), "{}", stdout(&show));
    let path = task_file(dir.path(), &kid);
    assert!(
        !std::fs::read_to_string(&path).unwrap().contains("parent"),
        "cleared parent key must drop from the file"
    );
}

#[test]
fn tree_bounds_by_depth_and_reports_json() {
    let dir = Project::new();
    let root = create(&dir, "Root");
    let a = child(&dir, "A", &root);
    let _grandchild = child(&dir, "A1", &a);

    let out = run(&dir, &["tree", &root, "--depth", "1", "--json"]);
    assert!(out.status.success());
    let tree: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tree["children"][0]["id"], a.as_str());
    assert!(
        tree["children"][0]["children"]
            .as_array()
            .unwrap()
            .is_empty(),
        "depth 1 must not expand grandchildren"
    );
}

#[test]
fn move_reorders_siblings_via_before_and_after() {
    let dir = Project::new();
    let root = create(&dir, "Root");
    let a = child(&dir, "A", &root);
    let b = child(&dir, "B", &root);
    let c = child(&dir, "C", &root);

    // Move C before A, then B after C.
    assert!(
        run(&dir, &["move", &c, "--parent", &root, "--before", &a])
            .status
            .success()
    );
    assert!(
        run(&dir, &["move", &b, "--parent", &root, "--after", &c])
            .status
            .success()
    );
    assert_eq!(tree_ids(&dir, &root), vec![c.clone(), b.clone(), a.clone()]);
}

#[test]
fn move_reparents_across_parents() {
    let dir = Project::new();
    let root = create(&dir, "Root");
    let other = create(&dir, "Other");
    let kid = child(&dir, "Kid", &root);

    assert!(
        run(&dir, &["move", &kid, "--parent", &other])
            .status
            .success()
    );
    assert_eq!(tree_ids(&dir, &root), Vec::<String>::new());
    assert_eq!(tree_ids(&dir, &other), vec![kid]);
}

#[test]
fn move_under_own_descendant_is_refused() {
    let dir = Project::new();
    let a = create(&dir, "A");
    let b = child(&dir, "B", &a);
    let c = child(&dir, "C", &b);

    let out = run(&dir, &["move", &a, "--parent", &c]);
    assert!(!out.status.success(), "cycle must be refused");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("descendant"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn move_unranked_siblings_then_reorder_lists_in_new_order() {
    let dir = Project::new();
    let root = create(&dir, "Root");
    // Created without explicit order — unranked, so they list by id first.
    let a = child(&dir, "A", &root);
    let z = child(&dir, "Z", &root);
    assert_eq!(tree_ids(&dir, &root), vec![a.clone(), z.clone()]);

    // Reorder Z before A; the group migrates to ranks and the new order sticks.
    assert!(
        run(&dir, &["move", &z, "--parent", &root, "--before", &a])
            .status
            .success()
    );
    assert_eq!(tree_ids(&dir, &root), vec![z, a]);
}

fn set_rank(project: &Project, id: &str, rank: &str) {
    let path = task_file(project.path(), id);
    let raw = std::fs::read_to_string(&path).unwrap();
    let patched = raw.replacen("---\n", &format!("---\nrank: {rank}\n"), 1);
    write(&path, &patched);
}

fn rank_of(project: &Project, id: &str) -> Option<String> {
    let path = task_file(project.path(), id);
    std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("rank: "))
        .map(|value| value.trim().to_owned())
}

#[test]
fn move_between_neighbours_naming_the_same_point_rebalances() {
    // Hand-edited frontmatter can give two siblings ranks that differ as text but name one point
    // (`a` and `a0`). There is no key between them, so the group has to be rebalanced rather than
    // searched forever for a gap that cannot exist.
    let dir = Project::new();
    let root = create(&dir, "Root");
    let a = child(&dir, "A", &root);
    let b = child(&dir, "B", &root);
    let c = child(&dir, "C", &root);
    set_rank(&dir, &a, "a");
    set_rank(&dir, &b, "a0");

    let out = run(&dir, &["move", &c, "--parent", &root, "--after", &a]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(tree_ids(&dir, &root), vec![a, c, b]);
}

#[test]
fn move_within_a_group_holding_a_malformed_rank_rebalances() {
    let dir = Project::new();
    let root = create(&dir, "Root");
    let a = child(&dir, "A", &root);
    let b = child(&dir, "B", &root);
    set_rank(&dir, &a, "NOT-BASE36");

    let out = run(&dir, &["move", &b, "--parent", &root, "--before", &a]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(tree_ids(&dir, &root), vec![b.clone(), a.clone()]);
    for id in [&a, &b] {
        let rank = rank_of(&dir, id).expect("rebalance ranks the whole group");
        assert!(
            rank.bytes()
                .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase()),
            "{id} kept a malformed rank: {rank}"
        );
    }
}

#[test]
fn a_refused_move_leaves_sibling_ranks_untouched() {
    // The rebalance path rewrites every sibling, so the moved task is written first: its write is
    // the one that fails the cycle check, and a refused command must not have edited anything.
    let dir = Project::new();
    let a = create(&dir, "A");
    let b = child(&dir, "B", &a);
    let c = child(&dir, "C", &b);
    let sibling = child(&dir, "Sibling", &c);
    let before = rank_of(&dir, &sibling);

    let out = run(&dir, &["move", &a, "--parent", &c]);
    assert!(!out.status.success(), "cycle must be refused");
    assert_eq!(
        rank_of(&dir, &sibling),
        before,
        "a refused move must not rewrite the target group"
    );
}

fn commit_at(root: &Path, seconds: i64, message: &str) {
    let date = format!("@{seconds} +0000");
    git(root, &["add", "-A"]);
    let status = Command::new("git")
        .current_dir(root)
        .args(["commit", "-qm", message])
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git commit failed");
}

#[test]
fn get_json_dates_the_checked_out_branch_not_the_headline() {
    let dir = Project::new();
    let root = dir.path();
    // `created` predates both commits, so the view's clamp cannot mask which one is reported.
    write(
        &root.join(".plan/tasks/00001-alpha.md"),
        "---\nstatus: todo\ncreated: 2000-01-01T00:00:00Z\n---\n# Alpha\n",
    );
    commit_at(root, 1_000_000_000, "add alpha");
    git(root, &["checkout", "-q", "-b", "feature"]);
    write(
        &root.join(".plan/tasks/00001-alpha.md"),
        "---\nstatus: done\ncreated: 2000-01-01T00:00:00Z\n---\n# Alpha done\n",
    );
    commit_at(root, 2_000_000_000, "edit alpha on feature");
    git(root, &["checkout", "-q", "main"]);

    let out = run(&dir, &["get", "OPP-1", "--json"]);
    let view: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();

    // `feature` headlines the task, but the file `get` read is main's — so is its date.
    assert_eq!(view["title"], "Alpha");
    assert_eq!(view["updated"], "2001-09-09T01:46:40Z");
}

#[test]
fn set_on_a_task_without_created_explains_what_to_add() {
    let dir = Project::new();
    write(
        &dir.path().join(".plan/tasks/00002-legacy.md"),
        "---\nstatus: todo\n---\n# Legacy\n",
    );

    let out = run(&dir, &["set", "OPP-2", "status", "done"]);

    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("created:"), "{err}");
    assert!(err.contains("git log --diff-filter=A"), "{err}");
}

// A store `lint` can read: `.plan/config.toml` plus `.plan/tasks/`, no git and no daemon — lint
// never starts one, so every case here drives the built binary directly against a bare directory.
fn lint_store() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    write(
        &dir.path().join(".plan/config.toml"),
        "abbreviation = \"OPP\"\n",
    );
    dir
}

fn run_lint(root: &Path, args: &[&str]) -> Output {
    openplan()
        .arg("--root")
        .arg(root)
        .arg("lint")
        .args(args)
        .output()
        .unwrap()
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

const VALID: &str = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Title\n";

#[test]
fn lint_clean_project_exits_zero() {
    let dir = lint_store();
    write(&dir.path().join(".plan/tasks/00001-clean.md"), VALID);

    let out = run_lint(dir.path(), &[]);
    assert!(
        out.status.success(),
        "a clean store must lint clean; output: {}",
        combined(&out)
    );
}

#[test]
fn lint_seeded_defect_exits_nonzero_and_prints_it() {
    let dir = lint_store();
    write(&dir.path().join(".plan/tasks/00001-clean.md"), VALID);
    write(
        &dir.path().join(".plan/tasks/00002-seeded-defect.md"),
        "---\nstatus: bogus\ncreated: 2026-01-01T00:00:00Z\n---\n# Seeded defect\n",
    );

    let out = run_lint(dir.path(), &[]);
    assert!(
        !out.status.success(),
        "a defect must fail the run; output: {}",
        combined(&out)
    );
    let report = combined(&out);
    assert!(
        report.contains("seeded-defect"),
        "the diagnostic must name the offending file: {report}"
    );
}

#[test]
fn lint_json_carries_severity_error() {
    let dir = lint_store();
    write(
        &dir.path().join(".plan/tasks/00001-seeded-defect.md"),
        "---\nstatus: bogus\ncreated: 2026-01-01T00:00:00Z\n---\n# Seeded defect\n",
    );

    let out = run_lint(dir.path(), &["--json"]);
    assert!(
        !out.status.success(),
        "a defect must fail even under --json"
    );
    let diagnostics: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|err| panic!("--json must emit a diagnostics array: {err}"));
    let first = diagnostics
        .first()
        .unwrap_or_else(|| panic!("expected at least one diagnostic: {diagnostics:?}"));
    assert_eq!(
        first["severity"], "error",
        "every diagnostic carries the severity field from day one: {first}"
    );
    assert!(
        first.get("code").is_some() && first.get("path").is_some(),
        "a diagnostic names its code and file: {first}"
    );
}

#[test]
fn lint_targets_filter_output_and_ignore_breakage_elsewhere() {
    let dir = lint_store();
    write(
        &dir.path().join(".plan/tasks/00001-alpha-broken.md"),
        "---\nstatus: bogus\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
    );
    write(
        &dir.path().join(".plan/tasks/00002-beta-broken.md"),
        "---\nstatus: bogus\ncreated: 2026-01-01T00:00:00Z\n---\n# Beta\n",
    );
    write(&dir.path().join(".plan/tasks/00003-gamma-clean.md"), VALID);

    // Targeting the one clean task: breakage in the others is off the commit's path, so the run
    // passes and says nothing about them.
    let clean = run_lint(dir.path(), &["OPP-3"]);
    assert!(
        clean.status.success(),
        "breakage outside the targets must not fail the run; output: {}",
        combined(&clean)
    );
    let clean_report = combined(&clean);
    assert!(
        !clean_report.contains("alpha-broken") && !clean_report.contains("beta-broken"),
        "output must be filtered to the targets: {clean_report}"
    );

    // Targeting a broken task reports only that task, not its equally broken neighbour.
    let alpha = run_lint(dir.path(), &["OPP-1"]);
    assert!(
        !alpha.status.success(),
        "a targeted defect must fail the run"
    );
    let alpha_report = combined(&alpha);
    assert!(
        alpha_report.contains("alpha-broken"),
        "the targeted task's diagnostic must show: {alpha_report}"
    );
    assert!(
        !alpha_report.contains("beta-broken"),
        "a non-targeted task must not leak into the output: {alpha_report}"
    );
}

#[test]
fn lint_fix_rewrites_then_reports_clean() {
    let dir = lint_store();
    write(&dir.path().join(".plan/tasks/00001-root.md"), VALID);
    let child = dir.path().join(".plan/tasks/00002-child.md");
    write(
        &child,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: ./00001-wrong-slug.md\n---\n# Child\n",
    );

    let fixed = run_lint(dir.path(), &["--fix"]);
    assert!(
        fixed.status.success(),
        "--fix must repair the derivable defect and re-check clean; output: {}",
        combined(&fixed)
    );

    let after = std::fs::read_to_string(&child).unwrap();
    assert!(
        after.contains("parent: ./00001-root.md"),
        "the stale slug must be canonicalized to the target's path: {after}"
    );
    assert!(
        !after.contains("wrong-slug"),
        "the stale slug must be gone: {after}"
    );

    let relint = run_lint(dir.path(), &[]);
    assert!(
        relint.status.success(),
        "a plain lint after --fix must be clean; output: {}",
        combined(&relint)
    );
}

#[test]
fn lint_lints_this_repos_own_plan_cleanly() {
    // The workspace root holds `.plan/`; CARGO_MANIFEST_DIR is this crate under `crates/`.
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();

    let out = run_lint(workspace, &[]);
    assert!(
        out.status.success(),
        "the binary CI and `mise run lint` run must lint the real store clean; output: {}",
        combined(&out)
    );
}

// The documented way to use openplan is to stand in a repository and type `openplan create`. `--root`
// then defaults to `.`, and the daemon runs in its own home, so a `.` that reaches the daemon names
// a directory the caller never chose. Every other test passes an absolute `--root`, which is why
// this one exists.
#[test]
fn a_write_from_inside_the_repository_needs_no_root_flag() {
    let project = Project::new();
    let out = project
        .cmd()
        .current_dir(project.path())
        .args(["create", "Ship login page"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let id = stdout(&out).trim().to_owned();
    assert_eq!(id, "OPP-1");
    assert!(task_file(project.path(), &id).is_file());
}

// A daemon whose home sits inside a git repository must not answer for that repository. Before this
// was refused, a write from another checkout registered the home's repository and landed the task
// there, reporting success — the wrong repository, silently.
#[test]
fn a_home_inside_a_repository_never_becomes_the_project_written_to() {
    let project = Project::new();
    let elsewhere = tempfile::tempdir().unwrap();
    git(elsewhere.path(), &["init", "-q", "-b", "main"]);
    git(elsewhere.path(), &["config", "user.email", "t@example.com"]);
    git(elsewhere.path(), &["config", "user.name", "Test"]);
    std::fs::create_dir_all(elsewhere.path().join(".plan/tasks")).unwrap();
    write(
        &elsewhere.path().join(".plan/config.toml"),
        "abbreviation = \"ELS\"\n",
    );
    // The daemon's home is a directory of that second repository, so its own `.` is a servable
    // checkout.
    let home = elsewhere.path().join("openplanhome");
    std::fs::create_dir_all(&home).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_openplan"))
        .env("OPENPLAN_HOME", &home)
        .env("OPENPLAN_PORT", "0")
        .current_dir(project.path())
        .args(["create", "Ship login page"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        stdout(&out).trim(),
        "OPP-1",
        "the key of the repo we stand in"
    );
    assert!(task_file(project.path(), "OPP-1").is_file());
    assert_eq!(
        std::fs::read_dir(elsewhere.path().join(".plan/tasks"))
            .unwrap()
            .count(),
        0,
        "nothing may be written to the repository the daemon's home happens to sit in"
    );

    let _ = Command::new(env!("CARGO_BIN_EXE_openplan"))
        .env("OPENPLAN_HOME", &home)
        .env("OPENPLAN_PORT", "0")
        .args(["server", "stop"])
        .output();
}

// Reads resolve through branches, and a repository whose first commit is still to come has none —
// not even the one HEAD points at. [[OPP-58]] is what makes these reads work; until then the
// refusal has to name the commit that is missing rather than deny a branch the caller is standing
// on.
#[test]
fn a_read_before_the_first_commit_names_the_missing_commit() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "t@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    write(
        &dir.path().join(".plan/config.toml"),
        "abbreviation = \"OPP\"\n",
    );
    let run_there = |args: &[&str]| {
        openplan()
            .env("OPENPLAN_HOME", home.path())
            .env("OPENPLAN_PORT", "0")
            .arg("--root")
            .arg(dir.path())
            .args(args)
            .output()
            .unwrap()
    };

    let created = run_there(&["create", "First task"]);
    assert!(
        created.status.success(),
        "a write still lands: {}",
        String::from_utf8_lossy(&created.stderr)
    );

    let listed = run_there(&["list"]);
    assert!(!listed.status.success());
    let stderr = String::from_utf8_lossy(&listed.stderr);
    assert!(
        stderr.contains("no commits yet"),
        "the refusal names the commit, not the branch: {stderr}"
    );
    assert!(
        !stderr.contains("no such branch"),
        "main is checked out; denying it by name misleads: {stderr}"
    );

    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "birth the store"]);
    assert!(
        run_there(&["list"]).status.success(),
        "and the commit is all it needed"
    );
    let _ = run_there(&["server", "stop"]);
}

fn create_tag(project: &Project, name: &str) -> String {
    let out = run(project, &["tag", "create", name]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    stdout(&out).trim().to_owned()
}

fn tag_file(root: &Path, name: &str) -> PathBuf {
    root.join(".plan/tags").join(format!("{name}.md"))
}

fn tags_of(root: &Path, key: &str) -> Vec<String> {
    std::fs::read_to_string(task_file(root, key))
        .unwrap()
        .lines()
        .skip_while(|line| *line != "tags:")
        .skip(1)
        .map_while(|line| line.strip_prefix("- ").map(str::to_owned))
        .collect()
}

#[test]
fn tag_create_normalizes_the_name_and_writes_the_file() {
    let dir = Project::new();

    let out = run(&dir, &["tag", "create", "Front End", "--desc", "the SPA"]);

    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(
        stdout(&out).trim(),
        "front-end",
        "create prints the name the tag is registered under"
    );
    let contents = std::fs::read_to_string(tag_file(dir.path(), "front-end")).unwrap();
    assert!(contents.contains("# Front End"), "contents: {contents}");
    assert!(contents.contains("the SPA"), "contents: {contents}");
    assert!(
        contents.contains("color: "),
        "a create always materializes the color: {contents}"
    );
}

#[test]
fn tag_create_refuses_a_name_it_cannot_normalize() {
    let dir = Project::new();

    let out = run(&dir, &["tag", "create", "C++"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("lowercase letters"),
        "the refusal carries the naming rule: {}",
        stderr(&out)
    );
}

#[test]
fn tag_create_refuses_a_second_tag_of_the_same_name() {
    let dir = Project::new();
    create_tag(&dir, "backend");

    let out = run(&dir, &["tag", "create", "Backend"]);

    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("backend"),
        "the refusal names the tag: {}",
        stderr(&out)
    );
}

#[test]
fn tag_create_refuses_a_color_outside_the_palette() {
    let dir = Project::new();

    let out = run(&dir, &["tag", "create", "backend", "--color", "turquoise"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("teal"),
        "the refusal lists the palette: {}",
        stderr(&out)
    );
    assert!(
        !tag_file(dir.path(), "backend").exists(),
        "a refused color registers nothing"
    );
}

#[test]
fn tag_colors_lists_the_palette() {
    let dir = Project::new();

    let out = run(&dir, &["tag", "colors"]);

    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let names: Vec<String> = stdout(&out).lines().map(str::to_owned).collect();
    assert_eq!(names.len(), 12, "the palette is closed: {names:?}");
    assert!(names.contains(&"teal".to_owned()), "{names:?}");

    let elsewhere = tempfile::tempdir().unwrap();
    let outside = dir
        .cmd()
        .arg("--root")
        .arg(elsewhere.path())
        .arg("tag")
        .arg("colors")
        .output()
        .unwrap();
    assert!(
        outside.status.success(),
        "the palette needs no repository and no daemon: {}",
        stderr(&outside)
    );
    assert_eq!(stdout(&outside), stdout(&out));
}

#[test]
fn tag_list_and_show_report_the_registry() {
    let dir = Project::new();
    assert!(
        run(&dir, &["tag", "create", "Backend", "--color", "teal"])
            .status
            .success()
    );
    assert!(
        run(&dir, &["tag", "create", "wip", "--desc", "in flight"])
            .status
            .success()
    );

    let listed = stdout(&run(&dir, &["tag", "list"]));
    assert!(listed.contains("backend"), "{listed}");
    assert!(listed.contains("teal"), "{listed}");
    assert!(listed.contains("in flight"), "{listed}");

    let shown = stdout(&run(&dir, &["tag", "show", "Backend"]));
    assert!(
        shown.contains("name:    backend"),
        "show takes any spelling that normalizes: {shown}"
    );
    assert!(shown.contains("display: Backend"), "{shown}");
    assert!(shown.contains("color:   teal"), "{shown}");

    let out = run(&dir, &["tag", "show", "wip", "--json"]);
    let tag: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tag["name"], "wip");
    assert_eq!(tag["description"], "in flight");
}

#[test]
fn tag_set_recolors_and_redescribes() {
    let dir = Project::new();
    create_tag(&dir, "backend");

    assert!(
        run(&dir, &["tag", "set", "backend", "color", "pink"])
            .status
            .success()
    );
    assert!(
        run(&dir, &["tag", "set", "backend", "desc", "server work"])
            .status
            .success()
    );
    let shown = stdout(&run(&dir, &["tag", "show", "backend"]));
    assert!(shown.contains("color:   pink"), "{shown}");
    assert!(shown.contains("desc:    server work"), "{shown}");

    assert!(
        run(&dir, &["tag", "set", "backend", "desc", ""])
            .status
            .success()
    );
    let shown = stdout(&run(&dir, &["tag", "show", "backend"]));
    assert!(
        shown.contains("desc:    -"),
        "an empty value clears the description: {shown}"
    );
}

#[test]
fn tag_set_rejects_an_unknown_field() {
    let dir = Project::new();
    create_tag(&dir, "backend");

    let out = run(&dir, &["tag", "set", "backend", "colour", "pink"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("expected color | desc"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn tag_rename_moves_the_file_and_rewrites_the_tasks_that_carry_it() {
    let dir = Project::new();
    create_tag(&dir, "backend");
    let out = run(&dir, &["create", "Wire the parser", "--tag", "backend"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let id = stdout(&out).trim().to_owned();

    let renamed = run(&dir, &["tag", "rename", "backend", "Infra"]);

    assert!(renamed.status.success(), "stderr: {}", stderr(&renamed));
    assert_eq!(stdout(&renamed).trim(), "infra");
    assert!(!tag_file(dir.path(), "backend").exists());
    assert!(tag_file(dir.path(), "infra").exists());
    assert_eq!(tags_of(dir.path(), &id), vec!["infra".to_owned()]);
}

#[test]
fn tag_delete_refuses_a_referenced_tag_until_it_is_forced() {
    let dir = Project::new();
    create_tag(&dir, "backend");
    assert!(
        run(&dir, &["create", "Wire the parser", "--tag", "backend"])
            .status
            .success()
    );

    let refused = run(&dir, &["tag", "delete", "backend", "--yes"]);
    assert!(!refused.status.success(), "stdout: {}", stdout(&refused));
    assert!(
        stderr(&refused).contains("--force"),
        "the refusal says how to override it: {}",
        stderr(&refused)
    );
    assert!(tag_file(dir.path(), "backend").exists());

    let forced = run(&dir, &["tag", "delete", "backend", "--force", "--yes"]);
    assert!(forced.status.success(), "stderr: {}", stderr(&forced));
    assert!(!tag_file(dir.path(), "backend").exists());
    assert!(
        stderr(&forced).contains("1 task still carries backend"),
        "a forced delete says what it left behind: {}",
        stderr(&forced)
    );
}

#[test]
fn tag_names_reach_the_store_as_the_identity_they_normalize_to() {
    let dir = Project::new();
    assert_eq!(create_tag(&dir, "Front End"), "front-end");

    let out = run(&dir, &["create", "Wire the parser", "--tag", "Front End"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let id = stdout(&out).trim().to_owned();
    assert_eq!(tags_of(dir.path(), &id), vec!["front-end".to_owned()]);

    assert!(
        run(&dir, &["set", &id, "tags", "FRONT_END"])
            .status
            .success()
    );
    assert_eq!(tags_of(dir.path(), &id), vec!["front-end".to_owned()]);

    let shown = stdout(&run(&dir, &["tag", "show", "Front_End"]));
    assert!(shown.contains("name:    front-end"), "{shown}");
}

#[test]
fn a_name_no_tag_can_have_is_refused_with_the_rule() {
    let dir = Project::new();

    for args in [
        vec!["create", "Wire the parser", "--tag", "C++"],
        vec!["tag", "show", ""],
        vec!["tag", "delete", ""],
    ] {
        let out = run(&dir, &args);
        assert!(!out.status.success(), "{args:?} stdout: {}", stdout(&out));
        assert!(
            stderr(&out).contains("lowercase letters"),
            "{args:?} answers with the naming rule: {}",
            stderr(&out)
        );
        assert!(
            !stderr(&out).contains("openplan server stop"),
            "{args:?} must not blame the daemon: {}",
            stderr(&out)
        );
    }
}

#[test]
fn tag_delete_of_an_unknown_name_fails_before_it_prompts() {
    let dir = Project::new();

    // No `--yes`: a typo must be refused outright rather than put to the reader as a question.
    let out = run(&dir, &["tag", "delete", "backend"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("no such tag: backend"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        !stdout(&out).contains("delete tag backend?"),
        "stdout: {}",
        stdout(&out)
    );
}

#[test]
fn create_with_tags_writes_a_sorted_set_and_leaves_the_body_alone() {
    let dir = Project::new();
    create_tag(&dir, "backend");
    create_tag(&dir, "wip");

    let out = run(
        &dir,
        &[
            "create",
            "Wire the parser",
            "--tag",
            "wip",
            "--tag",
            "backend",
            "--tag",
            "wip",
            "--body",
            "## Goals\n- Parse it\n",
        ],
    );

    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let id = stdout(&out).trim().to_owned();
    assert_eq!(
        tags_of(dir.path(), &id),
        vec!["backend".to_owned(), "wip".to_owned()],
        "the set is sorted and deduplicated"
    );
    assert_eq!(
        task_body(&task_file(dir.path(), &id)),
        "# Wire the parser\n\n## Goals\n- Parse it\n"
    );
}

#[test]
fn create_with_an_unknown_tag_names_the_command_that_registers_it() {
    let dir = Project::new();

    let out = run(&dir, &["create", "Wire the parser", "--tag", "wip"]);

    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("openplan tag create"),
        "the refusal says how to register the tag: {}",
        stderr(&out)
    );
    assert!(
        !stdout(&run(&dir, &["list"])).contains("Wire the parser"),
        "a refused write creates no task"
    );
}

#[test]
fn set_tags_replaces_the_whole_set_and_an_empty_value_clears_it() {
    let dir = Project::new();
    create_tag(&dir, "backend");
    create_tag(&dir, "wip");
    let id = create(&dir, "Wire the parser");

    assert!(
        run(&dir, &["set", &id, "tags", "wip, backend"])
            .status
            .success()
    );
    assert_eq!(
        tags_of(dir.path(), &id),
        vec!["backend".to_owned(), "wip".to_owned()]
    );

    assert!(run(&dir, &["set", &id, "tags", "wip"]).status.success());
    assert_eq!(tags_of(dir.path(), &id), vec!["wip".to_owned()]);

    assert!(run(&dir, &["set", &id, "tags", ""]).status.success());
    assert!(
        tags_of(dir.path(), &id).is_empty(),
        "an empty value clears the set and omits the field"
    );
    assert!(
        !std::fs::read_to_string(task_file(dir.path(), &id))
            .unwrap()
            .contains("tags:")
    );
    assert!(stdout(&run(&dir, &["show", &id])).contains("tags: -"));
}

#[test]
fn set_tags_refuses_a_name_this_branch_does_not_register() {
    let dir = Project::new();
    create_tag(&dir, "backend");
    let id = create(&dir, "Wire the parser");

    let out = run(&dir, &["set", &id, "tags", "backend, wip"]);

    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("openplan tag create"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        tags_of(dir.path(), &id).is_empty(),
        "a refused set leaves the task alone"
    );
}
