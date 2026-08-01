use std::path::Path;
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use op_git::Repo;
use op_store::Store;
use op_watch::{Change, Watcher};

const WAIT: Duration = Duration::from_secs(5);
const QUIET: Duration = Duration::from_millis(800);

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

// Named the way the store names files: the id is the number the name starts with.
fn write_task(dir: &Path, number: u64, body: &str) {
    let tasks = dir.join(".plan").join("tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    std::fs::write(
        tasks.join(format!("{number:05}-task-{number}.md")),
        format!("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n\n# {body}\n"),
    )
    .unwrap();
}

fn write_config(dir: &Path, contents: &str) {
    std::fs::create_dir_all(dir.join(".plan")).unwrap();
    std::fs::write(dir.join(".plan").join("config.toml"), contents).unwrap();
}

fn init_repo(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
    write_config(dir, "abbreviation = \"OPP\"\n");
}

fn store(dir: &Path) -> Store {
    Store::discover(dir).unwrap()
}

// Returns as soon as (number, branch) is seen; false on timeout.
fn saw_change(rx: &Receiver<Change>, number: u64, branch: &str) -> bool {
    let deadline = Instant::now() + WAIT;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(Change::Task {
                number: got,
                branch: got_branch,
            }) if got == number && got_branch == branch => return true,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    false
}

fn saw_config(rx: &Receiver<Change>) -> bool {
    let deadline = Instant::now() + WAIT;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(Change::Config) => return true,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    false
}

// Every task change seen over a quiet window.
fn collect(rx: &Receiver<Change>, window: Duration) -> Vec<(u64, String)> {
    let deadline = Instant::now() + window;
    let mut out = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(Change::Task { number, branch }) => out.push((number, branch)),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    out
}

#[test]
fn working_edit_emits_task_changed_for_its_branch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    write_task(path, 1, "Alpha");
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "init"]);

    let repo = Repo::discover(path).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, store(path), tx).unwrap();

    write_task(path, 1, "Alpha edited");

    assert!(
        saw_change(&rx, 1, "main"),
        "editing a task file should push TaskChanged for the checked-out branch"
    );
    watcher.stop();
}

#[test]
fn unrelated_git_activity_emits_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    write_task(path, 1, "Alpha");
    std::fs::write(path.join("code.txt"), "one").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "init"]);

    let repo = Repo::discover(path).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, store(path), tx).unwrap();

    // Churn that leaves every task blob untouched: a non-task file staged (rewrites .git/index) and
    // an empty commit (moves HEAD). Neither must produce a task event.
    std::fs::write(path.join("code.txt"), "two").unwrap();
    git(path, &["add", "code.txt"]);
    git(path, &["commit", "-qm", "code only"]);
    git(path, &["commit", "--allow-empty", "-qm", "noop"]);

    let changes = collect(&rx, QUIET);
    assert!(
        changes.is_empty(),
        "activity that changes no task must emit nothing: {changes:?}"
    );
    watcher.stop();
}

#[test]
fn commit_on_a_worktree_branch_emits_only_the_changed_task() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    write_task(path, 1, "Alpha");
    write_task(path, 2, "Beta");
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "init"]);

    let feat_root = tempfile::tempdir().unwrap();
    let feat = feat_root.path().join("wt");
    git(
        path,
        &[
            "worktree",
            "add",
            "-q",
            feat.to_str().unwrap(),
            "-b",
            "feat",
        ],
    );

    let repo = Repo::discover(path).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, store(path), tx).unwrap();

    write_task(&feat, 1, "Alpha on feat");
    git(&feat, &["add", "."]);
    git(&feat, &["commit", "-qm", "edit alpha"]);

    assert!(
        saw_change(&rx, 1, "feat"),
        "a commit on feat should push TaskChanged for alpha on feat"
    );
    let changes = collect(&rx, QUIET);
    assert!(
        !changes.iter().any(|(number, _)| *number == 2),
        "beta was untouched and must not be reported: {changes:?}"
    );
    watcher.stop();
}

#[test]
fn adding_a_worktree_starts_watching_its_plan() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    write_task(path, 1, "Alpha");
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "init"]);

    let repo = Repo::discover(path).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, store(path), tx).unwrap();

    let feat_root = tempfile::tempdir().unwrap();
    let feat = feat_root.path().join("wt");
    git(
        path,
        &[
            "worktree",
            "add",
            "-q",
            feat.to_str().unwrap(),
            "-b",
            "feat",
        ],
    );
    // Drain the events the worktree creation itself produces before testing the live edit.
    let _ = collect(&rx, QUIET);

    write_task(&feat, 1, "Alpha on the new worktree");

    assert!(
        saw_change(&rx, 1, "feat"),
        "an edit in a worktree added after start should be watched and reported"
    );
    watcher.stop();
}

#[test]
fn switching_branches_reattributes_the_dirty_task() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    write_task(path, 1, "Alpha");
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "init"]);
    // `src` and `other` sit on the same commit, so their .plan trees are identical and the switch
    // produces no working-tree fs change — only HEAD moves.
    git(path, &["branch", "src"]);
    git(path, &["branch", "other"]);

    let work_root = tempfile::tempdir().unwrap();
    let work = work_root.path().join("wt");
    git(
        path,
        &["worktree", "add", "-q", work.to_str().unwrap(), "src"],
    );

    let repo = Repo::discover(path).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, store(path), tx).unwrap();

    // Uncommitted edit on `src` in this worktree, then carry it onto `other`.
    write_task(&work, 1, "Alpha uncommitted");
    assert!(saw_change(&rx, 1, "src"), "the dirty edit lands on src");
    let _ = collect(&rx, QUIET);

    git(&work, &["switch", "-q", "other"]);

    assert!(
        saw_change(&rx, 1, "other"),
        "after switching, the dirty overlay must move to `other`"
    );
    watcher.stop();
}

#[test]
fn editing_the_config_is_reported_as_a_config_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    write_task(path, 1, "Alpha");
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "init"]);

    let repo = Repo::discover(path).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, store(path), tx).unwrap();

    write_config(path, "abbreviation = \"WEB\"\n");

    assert!(
        saw_config(&rx),
        "the store's abbreviation is watched like its tasks are"
    );
    watcher.stop();
}

#[test]
fn removing_the_config_is_reported_too() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    write_task(path, 1, "Alpha");
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "init"]);

    let repo = Repo::discover(path).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, store(path), tx).unwrap();

    std::fs::remove_file(path.join(".plan").join("config.toml")).unwrap();

    assert!(
        saw_config(&rx),
        "a store left with no abbreviation must be reported, not read as unchanged"
    );
    watcher.stop();
}
