use std::path::{Component, Path};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use op_api::ChangeEvent;
use op_git::Repo;
use op_watch::{Watcher, watch_paths};

const WAIT: Duration = Duration::from_secs(5);

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
// `.git/worktrees/<name>` admin dir) is pruned out from under it.
#[test]
fn watcher_survives_its_own_worktree_being_pruned() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path();
    init_repo(main);
    write_task(main, "1", "Alpha");
    git(main, &["add", "."]);
    git(main, &["commit", "-qm", "init"]);
    let linked = linked_worktree(main);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_test_writer()
        .try_init()
        .ok();

    let repo = Repo::discover(&linked).unwrap();
    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::start(repo, tx).unwrap();

    git(main, &["worktree", "remove", "--force", "wt"]);
    assert!(!linked.exists(), "the linked worktree should be gone");

    write_task(main, "1", "Alpha edited");
    assert!(
        saw_change(&rx, "1", "main"),
        "the watcher must keep reporting changes after its own worktree is pruned"
    );
    watcher.stop();
}
