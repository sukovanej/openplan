use std::cmp::Ordering;
use std::path::Path;
use std::process::Command;

use op_git::Repo;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed");
}

fn commit(dir: &Path, file: &str, contents: &str, message: &str) {
    std::fs::write(dir.join(file), contents).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", message]);
}

#[test]
fn containment_answers_which_version_supersedes_which() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    commit(root, "a.txt", "one\n", "first");
    let first = Repo::discover(root).unwrap().branch_commit("main").unwrap();
    git(root, &["checkout", "-qb", "side"]);
    commit(root, "b.txt", "two\n", "side");
    let side = Repo::discover(root).unwrap().branch_commit("side").unwrap();
    git(root, &["checkout", "-q", "main"]);
    commit(root, "c.txt", "three\n", "main");
    let main = Repo::discover(root).unwrap().branch_commit("main").unwrap();

    let found = Repo::discover(root)
        .unwrap()
        .ancestry(&[
            (first.as_str(), side.as_str()),
            (side.as_str(), first.as_str()),
            (side.as_str(), main.as_str()),
            (side.as_str(), side.as_str()),
        ])
        .unwrap();

    assert_eq!(
        found,
        vec![
            Some(Ordering::Less),
            Some(Ordering::Greater),
            // Two branches off one base reach neither, so no version supersedes the other.
            None,
            // A commit supersedes nothing, least of all itself.
            None,
        ]
    );
}
