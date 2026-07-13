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

#[test]
fn lists_local_branches_and_detects_no_op_in_progress() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    git(path, &["init", "-q"]);
    git(path, &["config", "user.email", "t@example.com"]);
    git(path, &["config", "user.name", "Test"]);
    std::fs::write(path.join("f.txt"), "x").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "init"]);
    git(path, &["branch", "feature"]);

    let repo = Repo::discover(path).unwrap();
    let branches = repo.local_branches().unwrap();

    assert!(branches.contains(&"feature".to_string()), "{branches:?}");
    assert!(
        branches.len() >= 2,
        "expected default + feature: {branches:?}"
    );
    assert!(!repo.op_in_progress());
}
