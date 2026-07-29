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

// Named the way the store names files (§3.1): the id is the number, the slug is decoration, and
// the pair must still be read as one task.
fn task(root: &Path, id: &str, status: &str) {
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    let number: u64 = id.parse().unwrap();
    std::fs::write(
        root.join(format!(".plan/tasks/{number:05}-task-{number}.md")),
        format!("---\nstatus: {status}\ncreated: 2026-01-01T00:00:00Z\n---\n# Task {number}\n"),
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
    task(root, "1", "todo");
    git(root, &["add", "-A"]);
    // A rebase or amend rewrites the commit date and leaves the author date alone, so only author
    // time keeps an untouched task from reading as freshly edited.
    commit_at(root, 1_000_000_000, 2_000_000_000, "add a");

    let times = Repo::discover(root)
        .unwrap()
        .task_change_times("main", &ids("1"))
        .unwrap();

    assert_eq!(times.get("1"), Some(&Ok(at(1_000_000_000))));
}

#[test]
fn the_newest_change_wins_and_untouched_tasks_keep_their_own_date() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    task(root, "1", "todo");
    task(root, "2", "todo");
    git(root, &["add", "-A"]);
    commit_at(root, 1_000_000_000, 1_000_000_000, "add a and b");
    task(root, "1", "done");
    commit_at(root, 1_000_000_500, 1_000_000_500, "finish a");

    let times = Repo::discover(root)
        .unwrap()
        .task_change_times("main", &HashSet::from(["1".to_owned(), "2".to_owned()]))
        .unwrap();

    assert_eq!(times.get("1"), Some(&Ok(at(1_000_000_500))));
    assert_eq!(times.get("2"), Some(&Ok(at(1_000_000_000))));
}

#[test]
fn a_merge_does_not_restamp_what_the_merged_branch_changed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    task(root, "1", "todo");
    git(root, &["add", "-A"]);
    commit_at(root, 1_600_000_000, 1_600_000_000, "add a");
    git(root, &["checkout", "-q", "-b", "feature"]);
    task(root, "1", "done");
    commit_at(root, 1_700_000_000, 1_700_000_000, "finish a on feature");
    git(root, &["checkout", "-q", "main"]);
    merge_at(root, 1_790_000_000, "feature");

    let times = Repo::discover(root)
        .unwrap()
        .task_change_times("main", &ids("1"))
        .unwrap();

    // The merge introduced no version of its own, so the task is still dated by the edit itself.
    assert_eq!(times.get("1"), Some(&Ok(at(1_700_000_000))));
}

fn merge_at(dir: &Path, seconds: i64, branch: &str) {
    let date = format!("@{seconds} +0000");
    let status = Command::new("git")
        .current_dir(dir)
        .args(["merge", "-q", "--no-ff", "-m", "merge", branch])
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git merge failed");
}

// Git stores an author date as a bare i64 and validates nothing, so a broken clock or an importer
// can leave one no calendar can express.
const UNUSABLE_AUTHOR_SECONDS: i64 = 253_402_300_800;

#[test]
fn a_commit_with_an_unusable_author_date_costs_only_the_tasks_it_changed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    task(root, "1", "todo");
    task(root, "2", "todo");
    git(root, &["add", "-A"]);
    commit_at(root, 1_000_000_000, 1_000_000_000, "add a and b");
    task(root, "2", "done");
    git(root, &["add", "-A"]);
    commit_at(root, UNUSABLE_AUTHOR_SECONDS, 2_000_000_000, "edit b");

    let times = Repo::discover(root)
        .unwrap()
        .task_change_times("main", &HashSet::from(["1".to_owned(), "2".to_owned()]))
        .expect("one undatable commit must not sink the whole walk");

    assert_eq!(times.get("1"), Some(&Ok(at(1_000_000_000))));
    let why = times.get("2").unwrap().as_ref().unwrap_err();
    assert!(why.contains("unusable author date"), "{why}");
}

#[test]
fn an_unusable_author_date_on_a_commit_touching_no_task_costs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    task(root, "1", "todo");
    git(root, &["add", "-A"]);
    commit_at(root, 1_000_000_000, 1_000_000_000, "add a");
    std::fs::write(root.join("README.md"), "hello\n").unwrap();
    git(root, &["add", "-A"]);
    commit_at(root, UNUSABLE_AUTHOR_SECONDS, 2_000_000_000, "unrelated");

    let times = Repo::discover(root)
        .unwrap()
        .task_change_times("main", &ids("1"))
        .expect("a commit that changes no task must not be dated at all");

    assert_eq!(times.get("1"), Some(&Ok(at(1_000_000_000))));
}
