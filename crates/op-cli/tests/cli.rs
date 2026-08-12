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

fn oplan() -> Command {
    Command::new(env!("CARGO_BIN_EXE_oplan"))
}

fn write(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
}

// A project every write can reach: a git repository (the daemon that owns writes resolves the target
// worktree by branch, so it needs one) plus a private OPLAN_HOME so each test gets its own daemon,
// auto-started on the first write. OPLAN_PORT=0 keeps those daemons off a shared port.
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
        project
    }

    fn path(&self) -> &Path {
        self.root.path()
    }

    fn cmd(&self) -> Command {
        let mut cmd = oplan();
        cmd.env("OPLAN_HOME", self.home.path())
            .env("OPLAN_PORT", "0");
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
    let dir = tempfile::tempdir().unwrap();
    let tasks = dir.path().join(".plan/tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    write(
        &dir.path().join(".plan/config.toml"),
        "abbreviation = \"OPP\"\n",
    );
    write(
        &tasks.join("00001-ship-it.md"),
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship it\n",
    );

    let out = oplan()
        .arg("--root")
        .arg(dir.path())
        .arg("list")
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
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
fn list_discovers_store_from_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    write(
        &dir.path().join(".plan/config.toml"),
        "abbreviation = \"OPP\"\n",
    );
    let nested = dir.path().join("crates/thing/src");
    std::fs::create_dir_all(&nested).unwrap();

    let out = oplan().current_dir(&nested).arg("list").output().unwrap();

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("no tasks yet"));
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

    let clean = oplan()
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
    let conflict = oplan()
        .arg("merge-driver")
        .args([&base, &ours, &theirs])
        .output()
        .unwrap();
    assert!(
        !conflict.status.success(),
        "divergent inputs should conflict"
    );

    let missing = dir.path().join("nope.md");
    let read_error = oplan()
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
        stderr.contains("no oplan daemon at http://127.0.0.1:1"),
        "stderr: {stderr}"
    );
    assert!(stdout(&run(&dir, &["list"])).contains("no tasks yet"));
}

#[test]
fn a_write_outside_a_git_repository_is_refused() {
    // Writes resolve their target worktree by branch, so a store with no repository has none.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    let out = oplan()
        .env("OPLAN_HOME", dir.path().join("home"))
        .env("OPLAN_PORT", "0")
        .arg("--root")
        .arg(dir.path())
        .args(["create", "Ship login"])
        .output()
        .unwrap();

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("require a git repository"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// One daemon serves every repository on the machine. A write from a repository it does not yet know
// registers that repository and lands, with no setup step and no restart.
#[test]
fn writes_from_two_repositories_land_in_their_own_stores() {
    let first = Project::new();
    let second = Project::new();
    create(&first, "Anchor");

    let out = oplan()
        .env("OPLAN_HOME", first.home.path())
        .env("OPLAN_PORT", "0")
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
        stderr.contains("did not list its projects") && stderr.contains("oplan server stop"),
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

    let out = oplan()
        .env("OPLAN_HOME", other.home.path())
        .env("OPLAN_PORT", "0")
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
        stderr.contains("oplan project add --daemon"),
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
fn delete_of_a_missing_id_fails_before_touching_the_daemon() {
    let dir = Project::new();
    let out = run(&dir, &["delete", "OPP-99", "--yes"]);

    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("no such task: OPP-99"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !dir.home.path().join("daemon.json").exists(),
        "a local read settles this; no daemon should have been started"
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

#[test]
fn get_prints_the_file_verbatim() {
    let dir = Project::new();
    let id = create(&dir, "Task D");
    let path = task_file(dir.path(), &id);
    let raw =
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nrank: 7\n---\n# Task D\n\nsome body\n";
    write(&path, raw);

    let out = run(&dir, &["get", &id]);
    assert!(out.status.success());
    assert_eq!(
        stdout(&out),
        raw,
        "get must print the on-disk bytes, not a re-serialization"
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
    oplan()
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
