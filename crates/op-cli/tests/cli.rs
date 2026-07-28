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
    assert!(stdout.contains("shipit"), "{stdout}");
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
fn create_writes_slug_file_and_prints_id() {
    let dir = store();
    let before = op_task::now();
    let id = create(dir.path(), "Wire the parser");
    assert!(id.starts_with("wire-the-parser-"), "id: {id}");

    let path = dir.path().join(".plan/tasks").join(format!("{id}.md"));
    let contents = std::fs::read_to_string(&path).unwrap();
    let created = frontmatter_value(&contents, "created");
    assert_eq!(
        contents,
        format!("---\nstatus: todo\ncreated: {created}\n---\n# Wire the parser\n")
    );
    assert!(
        created.parse::<Timestamp>().unwrap() >= before,
        "created must come from the clock at creation: {created}"
    );
    assert!(
        !created.contains('.'),
        "a stored timestamp carries whole seconds: {created}"
    );

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
    assert_eq!(view["metadata"]["status"], "todo");

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
        contents.starts_with("---\nstatus: in_progress\ncreated: "),
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
    assert_eq!(tasks[0]["metadata"]["status"], "todo");
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
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nestimate: 3.5\nassignee: milan\n---\n# Task C\n",
    );

    assert!(
        run(dir.path(), &["set", &id, "status", "done"])
            .status
            .success()
    );

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
    let dir = store();
    let id = create(dir.path(), "Task D");
    let path = dir.path().join(".plan/tasks").join(format!("{id}.md"));
    let raw =
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nrank: 7\n---\n# Task D\n\nsome body\n";
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
    let contents = std::fs::read_to_string(&path).unwrap();
    let created = frontmatter_value(&contents, "created");
    assert_eq!(
        contents,
        format!(
            "---\nstatus: todo\ncreated: {created}\n---\n# Ship login\n\nSupport OAuth and email login.\n"
        )
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
fn list_json_carries_an_unreadable_task_rather_than_dropping_it() {
    let dir = store();
    let good = create(dir.path(), "Good one");
    write(
        &dir.path().join(".plan/tasks/broken.md"),
        "this file has no frontmatter\n",
    );
    write(
        &dir.path().join(".plan/tasks/legacy.md"),
        "---\nstatus: in_progress\n---\n# Legacy\n",
    );

    let out = run(dir.path(), &["list", "--json"]);
    assert!(out.status.success());
    let tasks: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    let by_id = |id: &str| {
        tasks
            .iter()
            .find(|t| t["id"] == id)
            .unwrap_or_else(|| panic!("{id} missing from {tasks:?}"))
            .clone()
    };

    assert_eq!(by_id(&good)["metadata"]["status"], "todo");
    // A file with no readable frontmatter says so, instead of borrowing a status it never claimed.
    assert_eq!(by_id("broken")["metadata"]["kind"], "error");
    // One unreadable field costs only itself: the status is still the file's own.
    assert_eq!(by_id("legacy")["metadata"]["status"], "in_progress");
    assert_eq!(by_id("legacy")["metadata"]["created"]["kind"], "missing");
}

#[test]
fn list_filters_by_status_across_an_unreadable_task() {
    let dir = store();
    write(
        &dir.path().join(".plan/tasks/broken.md"),
        "this file has no frontmatter\n",
    );
    let good = create(dir.path(), "Good one");

    let out = run(dir.path(), &["list", "--json", "--status", "todo"]);
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

// A repo whose `alpha` task diverges: `todo`/`# Alpha` on main, `done`/`# Alpha done` on feature.
// The working tree is left on main.
fn diverged_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    write(
        &root.join(".plan/tasks/alpha.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
    );
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "init"]);
    git(root, &["checkout", "-q", "-b", "feature"]);
    write(
        &root.join(".plan/tasks/alpha.md"),
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha done\n",
    );
    git(root, &["commit", "-qam", "edit alpha on feature"]);
    git(root, &["checkout", "-q", "main"]);
    dir
}

#[test]
fn list_all_branches_json_has_one_row_per_task_branch() {
    let dir = diverged_repo();
    let out = run(dir.path(), &["list", "--all-branches", "--json"]);
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
    let dir = diverged_repo();
    let out = run(dir.path(), &["list", "--branch", "feature", "--json"]);
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
    let head = run(dir.path(), &["get", "alpha", "--json"]);
    let view: serde_json::Value = serde_json::from_slice(&head.stdout).unwrap();
    assert_eq!(
        view["metadata"]["status"], "todo",
        "reading a branch must not mutate the worktree"
    );
    let on_disk = std::fs::read_to_string(dir.path().join(".plan/tasks/alpha.md")).unwrap();
    assert!(
        on_disk.contains("status: todo"),
        "working file unchanged: {on_disk}"
    );
}

#[test]
fn list_branch_nonexistent_errors_nonzero() {
    let dir = diverged_repo();
    let out = run(dir.path(), &["list", "--branch", "ghost"]);
    assert!(!out.status.success(), "a missing branch must fail");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("no such branch"),
        "clear message: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn get_branch_prints_that_branchs_version() {
    let dir = diverged_repo();
    let out = run(dir.path(), &["get", "alpha", "--branch", "feature"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    assert!(text.contains("status: done"), "{text}");
    assert!(text.contains("# Alpha done"), "{text}");

    let missing = run(dir.path(), &["get", "ghost", "--branch", "feature"]);
    assert!(
        !missing.status.success(),
        "a missing task on a branch must fail"
    );
}

#[test]
fn show_branches_groups_and_flags_divergence() {
    let dir = diverged_repo();
    let out = run(dir.path(), &["show", "alpha", "--branches"]);
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
    let dir = diverged_repo();
    // There is deliberately no cross-branch write flag; `--branch` is not a valid arg for `set`.
    let out = run(
        dir.path(),
        &["set", "alpha", "--branch", "feature", "status", "done"],
    );
    assert!(
        !out.status.success(),
        "writes must not accept a --branch target"
    );

    // feature's committed version is untouched by the failed attempt.
    let feature = run(
        dir.path(),
        &["get", "alpha", "--branch", "feature", "--json"],
    );
    let view: serde_json::Value = serde_json::from_slice(&feature.stdout).unwrap();
    assert_eq!(view["metadata"]["status"], "done");
}

fn child(root: &Path, title: &str, parent: &str) -> String {
    let out = run(root, &["create", title, "--parent", parent]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout(&out).trim().to_owned()
}

fn tree_ids(root: &Path, id: &str) -> Vec<String> {
    let out = run(root, &["tree", id, "--json"]);
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
    let dir = store();
    let parent = create(dir.path(), "Parent");
    let kid = child(dir.path(), "Kid", &parent);

    assert!(
        run(dir.path(), &["set", &kid, "parent", ""])
            .status
            .success()
    );

    let show = run(dir.path(), &["show", &kid]);
    assert!(stdout(&show).contains("parent: -"), "{}", stdout(&show));
    let path = dir.path().join(".plan/tasks").join(format!("{kid}.md"));
    assert!(
        !std::fs::read_to_string(&path).unwrap().contains("parent"),
        "cleared parent key must drop from the file"
    );
}

#[test]
fn tree_bounds_by_depth_and_reports_json() {
    let dir = store();
    let root = create(dir.path(), "Root");
    let a = child(dir.path(), "A", &root);
    let _grandchild = child(dir.path(), "A1", &a);

    let out = run(dir.path(), &["tree", &root, "--depth", "1", "--json"]);
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
    let dir = store();
    let root = create(dir.path(), "Root");
    let a = child(dir.path(), "A", &root);
    let b = child(dir.path(), "B", &root);
    let c = child(dir.path(), "C", &root);

    // Move C before A, then B after C.
    assert!(
        run(dir.path(), &["move", &c, "--parent", &root, "--before", &a])
            .status
            .success()
    );
    assert!(
        run(dir.path(), &["move", &b, "--parent", &root, "--after", &c])
            .status
            .success()
    );
    assert_eq!(
        tree_ids(dir.path(), &root),
        vec![c.clone(), b.clone(), a.clone()]
    );
}

#[test]
fn move_reparents_across_parents() {
    let dir = store();
    let root = create(dir.path(), "Root");
    let other = create(dir.path(), "Other");
    let kid = child(dir.path(), "Kid", &root);

    assert!(
        run(dir.path(), &["move", &kid, "--parent", &other])
            .status
            .success()
    );
    assert_eq!(tree_ids(dir.path(), &root), Vec::<String>::new());
    assert_eq!(tree_ids(dir.path(), &other), vec![kid]);
}

#[test]
fn move_under_own_descendant_is_refused() {
    let dir = store();
    let a = create(dir.path(), "A");
    let b = child(dir.path(), "B", &a);
    let c = child(dir.path(), "C", &b);

    let out = run(dir.path(), &["move", &a, "--parent", &c]);
    assert!(!out.status.success(), "cycle must be refused");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("descendant"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn move_unranked_siblings_then_reorder_lists_in_new_order() {
    let dir = store();
    let root = create(dir.path(), "Root");
    // Created without explicit order — unranked, so they list by id first.
    let a = child(dir.path(), "A", &root);
    let z = child(dir.path(), "Z", &root);
    assert_eq!(tree_ids(dir.path(), &root), vec![a.clone(), z.clone()]);

    // Reorder Z before A; the group migrates to ranks and the new order sticks.
    assert!(
        run(dir.path(), &["move", &z, "--parent", &root, "--before", &a])
            .status
            .success()
    );
    assert_eq!(tree_ids(dir.path(), &root), vec![z, a]);
}

fn set_rank(root: &Path, id: &str, rank: &str) {
    let path = root.join(".plan/tasks").join(format!("{id}.md"));
    let raw = std::fs::read_to_string(&path).unwrap();
    let patched = raw.replacen("---\n", &format!("---\nrank: {rank}\n"), 1);
    write(&path, &patched);
}

fn rank_of(root: &Path, id: &str) -> Option<String> {
    let path = root.join(".plan/tasks").join(format!("{id}.md"));
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
    let dir = store();
    let root = create(dir.path(), "Root");
    let a = child(dir.path(), "A", &root);
    let b = child(dir.path(), "B", &root);
    let c = child(dir.path(), "C", &root);
    set_rank(dir.path(), &a, "a");
    set_rank(dir.path(), &b, "a0");

    let out = run(dir.path(), &["move", &c, "--parent", &root, "--after", &a]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(tree_ids(dir.path(), &root), vec![a, c, b]);
}

#[test]
fn move_within_a_group_holding_a_malformed_rank_rebalances() {
    let dir = store();
    let root = create(dir.path(), "Root");
    let a = child(dir.path(), "A", &root);
    let b = child(dir.path(), "B", &root);
    set_rank(dir.path(), &a, "NOT-BASE36");

    let out = run(dir.path(), &["move", &b, "--parent", &root, "--before", &a]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(tree_ids(dir.path(), &root), vec![b.clone(), a.clone()]);
    for id in [&a, &b] {
        let rank = rank_of(dir.path(), id).expect("rebalance ranks the whole group");
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
    let dir = store();
    let a = create(dir.path(), "A");
    let b = child(dir.path(), "B", &a);
    let c = child(dir.path(), "C", &b);
    let sibling = child(dir.path(), "Sibling", &c);
    let before = rank_of(dir.path(), &sibling);

    let out = run(dir.path(), &["move", &a, "--parent", &c]);
    assert!(!out.status.success(), "cycle must be refused");
    assert_eq!(
        rank_of(dir.path(), &sibling),
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
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    // `created` predates both commits, so the view's clamp cannot mask which one is reported.
    write(
        &root.join(".plan/tasks/alpha.md"),
        "---\nstatus: todo\ncreated: 2000-01-01T00:00:00Z\n---\n# Alpha\n",
    );
    commit_at(root, 1_000_000_000, "add alpha");
    git(root, &["checkout", "-q", "-b", "feature"]);
    write(
        &root.join(".plan/tasks/alpha.md"),
        "---\nstatus: done\ncreated: 2000-01-01T00:00:00Z\n---\n# Alpha done\n",
    );
    commit_at(root, 2_000_000_000, "edit alpha on feature");
    git(root, &["checkout", "-q", "main"]);

    let out = run(root, &["get", "alpha", "--json"]);
    let view: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();

    // `feature` headlines the task, but the file `get` read is main's — so is its date.
    assert_eq!(view["title"], "Alpha");
    assert_eq!(view["updated"], "2001-09-09T01:46:40Z");
}
