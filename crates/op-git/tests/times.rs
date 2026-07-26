use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use jiff::Timestamp;
use op_git::Repo;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed");
}

fn commit_at(dir: &Path, authored: i64, committed: i64, message: &str) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(["commit", "-qam", message])
        .env("GIT_AUTHOR_DATE", format!("@{authored} +0000"))
        .env("GIT_COMMITTER_DATE", format!("@{committed} +0000"))
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git commit failed");
}

fn task(root: &Path, id: &str, status: &str) {
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::write(
        root.join(format!(".plan/tasks/{id}.md")),
        format!("---\nstatus: {status}\ncreated: 2026-01-01T00:00:00Z\n---\n# {id}\n"),
    )
    .unwrap();
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(seconds).unwrap()
}

fn ids(id: &str) -> HashSet<String> {
    HashSet::from([id.to_owned()])
}

#[test]
fn a_change_is_dated_by_author_time_not_commit_time() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    task(root, "a", "todo");
    git(root, &["add", "-A"]);
    // A rebase or amend rewrites the commit date and leaves the author date alone, so only author
    // time keeps an untouched task from reading as freshly edited.
    commit_at(root, 1_000_000_000, 2_000_000_000, "add a");

    let times = Repo::discover(root)
        .unwrap()
        .task_change_times("main", &ids("a"))
        .unwrap();

    assert_eq!(times.get("a"), Some(&at(1_000_000_000)));
}

#[test]
fn the_newest_change_wins_and_untouched_tasks_keep_their_own_date() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    task(root, "a", "todo");
    task(root, "b", "todo");
    git(root, &["add", "-A"]);
    commit_at(root, 1_000_000_000, 1_000_000_000, "add a and b");
    task(root, "a", "done");
    commit_at(root, 1_000_000_500, 1_000_000_500, "finish a");

    let times = Repo::discover(root)
        .unwrap()
        .task_change_times("main", &HashSet::from(["a".to_owned(), "b".to_owned()]))
        .unwrap();

    assert_eq!(times.get("a"), Some(&at(1_000_000_500)));
    assert_eq!(times.get("b"), Some(&at(1_000_000_000)));
}
