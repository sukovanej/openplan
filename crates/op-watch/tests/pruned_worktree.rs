use std::path::{Component, Path};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use op_api::ChangeEvent;
use op_git::Repo;
use op_watch::{Watcher, watch_paths};

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
    let number: u64 = id.parse().unwrap();
    std::fs::write(
        tasks.join(format!("{number:05}-task-{number}.md")),
        format!("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n\n# {body}\n"),
    )
    .unwrap();
}

fn init_repo(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
}

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

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_test_writer()
        .try_init()
        .ok();
}

fn linked_worktree(main: &Path) -> std::path::PathBuf {
    let linked = main.join("wt");
    git(main, &["worktree", "add", "-q", "-b", "side", "wt"]);
    linked
}

#[test]
fn watch_paths_from_a_linked_worktree_are_absolute_and_dotdot_free() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path();
    init_repo(main);
    write_task(main, "1", "Alpha");
    git(main, &["add", "."]);
    git(main, &["commit", "-qm", "init"]);
    let linked = linked_worktree(main);

    let repo = Repo::discover(&linked).unwrap();
    let paths = watch_paths(&repo, &repo.worktrees().unwrap());
    assert!(!paths.is_empty(), "a live repo must yield watch paths");

    for (path, _) in &paths {
        assert!(path.is_absolute(), "{} is not absolute", path.display());
        assert!(
            !path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir)),
            "{} still carries a . or .. component; notify's fsevent backend spins forever \
             resolving one that no longer exists",
            path.display()
        );
    }
}

// The daemon that hung: started against a linked worktree, then the worktree (and its
// `.git/worktrees/<name>` admin dir) is pruned out from under it. Its own git dir is gone, so it has
// nothing left to report — the requirements are that it neither wedges nor invents changes.
#[test]
fn pruning_its_own_worktree_stops_the_watcher_without_a_deletion_flood() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path();
    init_repo(main);
    write_task(main, "1", "Alpha");
    write_task(main, "2", "Beta");
    git(main, &["add", "."]);
    git(main, &["commit", "-qm", "init"]);
    let linked = linked_worktree(main);
    init_tracing();

    let repo = Repo::discover(&linked).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, tx).unwrap();

    git(main, &["worktree", "remove", "--force", "wt"]);
    assert!(!linked.exists(), "the linked worktree should be gone");

    let invented = collect(&rx, QUIET);
    assert!(
        invented.is_empty(),
        "no task changed, yet the watcher reported {invented:?} — a repo that resolves nothing \
         reads as every task deleted"
    );

    // Bounded, because the bug this guards was an unwatch that never returned, and `stop` joins the
    // worker thread.
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::spawn(move || {
        watcher.stop();
        let _ = done_tx.send(());
    });
    assert!(
        done_rx.recv_timeout(WAIT).is_ok(),
        "the watcher wedged instead of shutting down"
    );
}

// The everyday case: a daemon on the main checkout, watching the linked worktrees an agent creates
// and removes. Pruning one must leave the watcher reporting on the ones that remain.
#[test]
fn pruning_another_worktree_leaves_the_watcher_working() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path();
    init_repo(main);
    write_task(main, "1", "Alpha");
    git(main, &["add", "."]);
    git(main, &["commit", "-qm", "init"]);
    linked_worktree(main);
    init_tracing();

    let repo = Repo::discover(main).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, tx).unwrap();

    git(main, &["worktree", "remove", "--force", "wt"]);
    let _ = collect(&rx, QUIET);

    write_task(main, "1", "Alpha edited");
    assert!(
        saw_change(&rx, "1", "main"),
        "an edit after an unrelated worktree was pruned should still be reported"
    );
    watcher.stop();
}
