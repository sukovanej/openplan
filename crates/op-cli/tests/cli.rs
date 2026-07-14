use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn oplan() -> Command {
    Command::new(env!("CARGO_BIN_EXE_oplan"))
}

fn write(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
}

fn store() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    dir
}

fn run(root: &Path, args: &[&str]) -> Output {
    oplan().arg("--root").arg(root).args(args).output().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn create(root: &Path, title: &str) -> String {
    let out = run(root, &["create", title]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout(&out).trim().to_owned()
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
        &tasks.join("shipit.md"),
        "---\nstatus: done\n---\n# Ship it\n",
    );

    let out = oplan()
        .arg("--root")
        .arg(dir.path())
        .arg("list")
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("shipit"), "{stdout}");
    assert!(
        stdout.contains("Done"),
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
    let content = "---\nstatus: todo\n---\n# T\n\n## Plan\nhi\n";
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

    write(&theirs, "---\nstatus: todo\n---\n# T\n\n## Plan\nBYE\n");
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
fn create_writes_slug_file_and_prints_id() {
    let dir = store();
    let id = create(dir.path(), "Wire the parser");
    assert!(id.starts_with("wire-the-parser-"), "id: {id}");

    let path = dir.path().join(".plan/tasks").join(format!("{id}.md"));
    let contents = std::fs::read_to_string(&path).unwrap();
    assert_eq!(contents, "---\nstatus: todo\n---\n# Wire the parser\n");

    let second = create(dir.path(), "Wire the parser");
    assert_ne!(id, second, "same title must yield a distinct id");
}

#[test]
fn get_show_and_missing_id() {
    let dir = store();
    let id = create(dir.path(), "Ship it");

    let show = run(dir.path(), &["show", &id]);
    assert!(show.status.success());
    let text = stdout(&show);
    assert!(text.contains("status: todo"), "{text}");
    assert!(text.contains("title:  Ship it"), "{text}");

    let get = run(dir.path(), &["get", &id, "--json"]);
    assert!(get.status.success());
    let view: serde_json::Value = serde_json::from_slice(&get.stdout).unwrap();
    assert_eq!(view["title"], "Ship it");
    assert_eq!(view["status"], "todo");

    let missing = run(dir.path(), &["get", "does-not-exist"]);
    assert!(
        !missing.status.success(),
        "get on a missing id must exit non-zero"
    );
    assert!(String::from_utf8_lossy(&missing.stderr).contains("does-not-exist"));
}

#[test]
fn set_updates_only_frontmatter() {
    let dir = store();
    let id = create(dir.path(), "Ship it");
    let path = dir.path().join(".plan/tasks").join(format!("{id}.md"));
    let body_before = task_body(&path);

    let set = run(dir.path(), &["set", &id, "status", "in_progress"]);
    assert!(
        set.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&set.stderr)
    );

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(
        contents.starts_with("---\nstatus: in_progress\n---\n"),
        "{contents}"
    );
    assert_eq!(
        task_body(&path),
        body_before,
        "body must be byte-for-byte unchanged"
    );

    let bad_status = run(dir.path(), &["set", &id, "status", "bogus"]);
    assert!(
        !bad_status.status.success(),
        "invalid status must be rejected"
    );

    let bad_parent = run(dir.path(), &["set", &id, "parent", "ghost"]);
    assert!(
        !bad_parent.status.success(),
        "non-existent parent must be rejected"
    );
}

#[test]
fn delete_removes_the_file() {
    let dir = store();
    let id = create(dir.path(), "Temporary");
    let path = dir.path().join(".plan/tasks").join(format!("{id}.md"));
    assert!(path.exists());

    let del = run(dir.path(), &["delete", &id, "--yes"]);
    assert!(del.status.success());
    assert!(!path.exists(), "delete must remove the file");

    let list = run(dir.path(), &["list"]);
    assert!(
        !stdout(&list).contains(&id),
        "deleted id must not appear in list"
    );
}

#[test]
fn list_json_filters_by_status() {
    let dir = store();
    let todo = create(dir.path(), "Still to do");
    let done = create(dir.path(), "Already done");
    assert!(
        run(dir.path(), &["set", &done, "status", "done"])
            .status
            .success()
    );

    let out = run(dir.path(), &["list", "--json", "--status", "todo"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tasks: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tasks.len(), 1, "only the todo task should match: {tasks:?}");
    assert_eq!(tasks[0]["id"], todo);
    assert_eq!(tasks[0]["status"], "todo");
}

#[test]
fn set_status_survives_a_deleted_dependency() {
    let dir = store();
    let a = create(dir.path(), "Task A");
    let b_out = run(dir.path(), &["create", "Task B", "--dep", &a]);
    assert!(b_out.status.success());
    let b = stdout(&b_out).trim().to_owned();

    assert!(run(dir.path(), &["delete", &a, "--yes"]).status.success());

    // B still lists A as a dep, but changing B's status is unrelated and must succeed.
    let set = run(dir.path(), &["set", &b, "status", "done"]);
    assert!(
        set.status.success(),
        "a deleted dep must not block a status change: {}",
        String::from_utf8_lossy(&set.stderr)
    );
    assert!(stdout(&run(dir.path(), &["show", &b])).contains("status: done"));
}

#[test]
fn set_preserves_unknown_frontmatter_keys() {
    let dir = store();
    let id = create(dir.path(), "Task C");
    let path = dir.path().join(".plan/tasks").join(format!("{id}.md"));
    write(
        &path,
        "---\nstatus: todo\nrank: 3.5\nassignee: milan\n---\n# Task C\n",
    );

    assert!(
        run(dir.path(), &["set", &id, "status", "done"])
            .status
            .success()
    );

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("rank: 3.5"), "rank dropped: {contents}");
    assert!(
        contents.contains("assignee: milan"),
        "assignee dropped: {contents}"
    );
    assert!(contents.contains("status: done"));
}

#[test]
fn get_prints_the_file_verbatim() {
    let dir = store();
    let id = create(dir.path(), "Task D");
    let path = dir.path().join(".plan/tasks").join(format!("{id}.md"));
    let raw = "---\nstatus: todo\nrank: 7\n---\n# Task D\n\nsome body\n";
    write(&path, raw);

    let out = run(dir.path(), &["get", &id]);
    assert!(out.status.success());
    assert_eq!(
        stdout(&out),
        raw,
        "get must print the on-disk bytes, not a re-serialization"
    );
}

#[test]
fn create_with_body_places_content_below_title() {
    let dir = store();
    let out = run(
        dir.path(),
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

    let path = dir.path().join(".plan/tasks").join(format!("{id}.md"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "---\nstatus: todo\n---\n# Ship login\n\nSupport OAuth and email login.\n"
    );

    let view = run(dir.path(), &["get", &id, "--json"]);
    let json: serde_json::Value = serde_json::from_slice(&view.stdout).unwrap();
    assert_eq!(json["title"], "Ship login");
    assert_eq!(
        json["body"],
        "# Ship login\n\nSupport OAuth and email login.\n"
    );
}

#[test]
fn create_with_body_file_reads_the_file() {
    let dir = store();
    let notes = dir.path().join("notes.md");
    write(&notes, "## Goals\n- OAuth\n- Email + password\n");

    let out = run(
        dir.path(),
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

    let path = dir.path().join(".plan/tasks").join(format!("{id}.md"));
    assert_eq!(
        task_body(&path),
        "# Ship login\n\n## Goals\n- OAuth\n- Email + password\n"
    );
}

#[test]
fn create_with_body_file_dash_reads_stdin() {
    let dir = store();
    let mut child = oplan()
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

    let path = dir.path().join(".plan/tasks").join(format!("{id}.md"));
    assert_eq!(
        task_body(&path),
        "# Ship login\n\n## Goals\n- OAuth\n- Email + password\n"
    );
}

#[test]
fn create_rejects_body_with_body_file() {
    let dir = store();
    let out = run(
        dir.path(),
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
    assert!(stdout(&run(dir.path(), &["list"])).contains("no tasks yet"));
}

#[test]
fn create_rejects_malformed_title() {
    let dir = store();
    assert!(
        !run(dir.path(), &["create", ""]).status.success(),
        "an empty title must be rejected"
    );
    assert!(
        !run(dir.path(), &["create", "line one\n# line two"])
            .status
            .success(),
        "a title producing two H1 headings must be rejected"
    );
    assert!(stdout(&run(dir.path(), &["list"])).contains("no tasks yet"));
}

#[test]
fn list_distinguishes_empty_store_from_empty_filter() {
    let dir = store();
    assert!(stdout(&run(dir.path(), &["list"])).contains("no tasks yet"));

    create(dir.path(), "A todo");
    let filtered = stdout(&run(dir.path(), &["list", "--status", "done"]));
    assert!(filtered.contains("no matching tasks"), "{filtered}");
    assert!(!filtered.contains("no tasks yet"), "{filtered}");
}

#[test]
fn list_json_reports_unreadable_tasks_on_stderr() {
    let dir = store();
    let good = create(dir.path(), "Good one");
    write(
        &dir.path().join(".plan/tasks/broken.md"),
        "this file has no frontmatter\n",
    );

    let out = run(dir.path(), &["list", "--json"]);
    assert!(out.status.success());
    let tasks: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tasks.len(), 1, "only the readable task appears: {tasks:?}");
    assert_eq!(tasks[0]["id"], good);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("broken"),
        "the unreadable task must be reported on stderr"
    );
}
