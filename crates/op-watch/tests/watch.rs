use std::path::Path;
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use op_api::ChangeEvent;
use op_git::Repo;
use op_watch::Watcher;

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

fn write_task(dir: &Path, id: &str, body: &str) {
    let tasks = dir.join(".plan").join("tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    std::fs::write(
        tasks.join(format!("{id}.md")),
        format!("---\nstatus: todo\n---\n\n# {body}\n"),
    )
    .unwrap();
}

fn init_repo(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
}

// Returns as soon as (id, branch) is seen; false on timeout.
fn saw_change(rx: &Receiver<ChangeEvent>, id: &str, branch: &str) -> bool {
    let deadline = Instant::now() + WAIT;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(ChangeEvent::TaskChanged {
                id: got_id,
                branch: got_branch,
            }) if got_id == id && got_branch == branch => return true,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    false
}

// Every TaskChanged seen over a quiet window.
fn collect(rx: &Receiver<ChangeEvent>, window: Duration) -> Vec<(String, String)> {
    let deadline = Instant::now() + window;
    let mut out = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(ChangeEvent::TaskChanged { id, branch }) => out.push((id, branch)),
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
    write_task(path, "alpha", "Alpha");
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "init"]);

    let repo = Repo::discover(path).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, tx).unwrap();

    write_task(path, "alpha", "Alpha edited");

    assert!(
        saw_change(&rx, "alpha", "main"),
        "editing a task file should push TaskChanged for the checked-out branch"
    );
    watcher.stop();
}

#[test]
fn commit_on_a_worktree_branch_emits_only_the_changed_task() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    write_task(path, "alpha", "Alpha");
    write_task(path, "beta", "Beta");
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
    let watcher = Watcher::start(repo, tx).unwrap();

    write_task(&feat, "alpha", "Alpha on feat");
    git(&feat, &["add", "."]);
    git(&feat, &["commit", "-qm", "edit alpha"]);

    assert!(
        saw_change(&rx, "alpha", "feat"),
        "a commit on feat should push TaskChanged for alpha on feat"
    );
    let changes = collect(&rx, QUIET);
    assert!(
        !changes.iter().any(|(id, _)| id == "beta"),
        "beta was untouched and must not be reported: {changes:?}"
    );
    watcher.stop();
}

#[test]
fn adding_a_worktree_starts_watching_its_plan() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    write_task(path, "alpha", "Alpha");
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "init"]);

    let repo = Repo::discover(path).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, tx).unwrap();

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

    write_task(&feat, "alpha", "Alpha on the new worktree");

    assert!(
        saw_change(&rx, "alpha", "feat"),
        "an edit in a worktree added after start should be watched and reported"
    );
    watcher.stop();
}

#[test]
fn switching_branches_reattributes_the_dirty_task() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    write_task(path, "alpha", "Alpha");
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
    let watcher = Watcher::start(repo, tx).unwrap();

    // Uncommitted edit on `src` in this worktree, then carry it onto `other`.
    write_task(&work, "alpha", "Alpha uncommitted");
    assert!(
        saw_change(&rx, "alpha", "src"),
        "the dirty edit lands on src"
    );
    let _ = collect(&rx, QUIET);

    git(&work, &["switch", "-q", "other"]);

    assert!(
        saw_change(&rx, "alpha", "other"),
        "after switching, the dirty overlay must move to `other`"
    );
    watcher.stop();
}
