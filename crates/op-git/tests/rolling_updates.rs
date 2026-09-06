use std::path::{Path, PathBuf};
use std::process::Command;

use op_git::{ROLLING_UPDATES_BRANCH, Rebased, Repo};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed");
}

fn task(alpha: &str, beta: &str) -> String {
    format!(
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n\n## Alpha\n\n{alpha}\n\n## Beta\n\n{beta}\n"
    )
}

// A driver that merges the way the real one does for these fixtures: it takes both sides when the
// hunks differ and conflicts when they overlap.
fn driver_script(dir: &Path) -> PathBuf {
    let path = dir.join("driver.sh");
    std::fs::write(
        &path,
        "#!/bin/sh\nexec git merge-file -L ours -L base -L theirs \"$2\" \"$1\" \"$3\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    repo: Repo,
    rolling: PathBuf,
    driver: String,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    std::fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.email", "t@example.com"]);
    git(&root, &["config", "user.name", "Test"]);
    std::fs::write(
        root.join(".plan/tasks/00001-t.md"),
        task("base alpha", "base beta"),
    )
    .unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);

    let driver = driver_script(dir.path()).to_string_lossy().into_owned();
    let repo = Repo::discover(&root).unwrap();
    let rolling = repo.ensure_rolling_updates("main", &driver).unwrap();
    Fixture {
        _dir: dir,
        root,
        repo,
        rolling,
        driver,
    }
}

fn rolling_task(fixture: &Fixture) -> PathBuf {
    fixture.rolling.join(".plan/tasks/00001-t.md")
}

fn commit_on_main(fixture: &Fixture, contents: &str, path: &str, message: &str) {
    std::fs::write(fixture.root.join(path), contents).unwrap();
    git(&fixture.root, &["add", "-A"]);
    git(&fixture.root, &["commit", "-qm", message]);
}

#[test]
fn the_rolling_updates_worktree_holds_the_plan_and_no_code() {
    let fixture = fixture();

    assert!(rolling_task(&fixture).is_file());
    assert!(!fixture.rolling.join("src").exists());
    assert!(fixture.rolling.join(".gitattributes").is_file());
    assert_eq!(
        fixture
            .repo
            .worktree_branch(&fixture.rolling)
            .unwrap()
            .as_deref(),
        Some(ROLLING_UPDATES_BRANCH)
    );
}

#[test]
fn ensure_rolling_updates_runs_twice_without_complaint() {
    let fixture = fixture();

    let again = fixture
        .repo
        .ensure_rolling_updates("main", &fixture.driver)
        .unwrap();

    assert_eq!(again, fixture.rolling);
}

#[test]
fn a_commit_lands_only_when_something_changed() {
    let fixture = fixture();

    assert!(!fixture.repo.rolling_updates_commit("nothing").unwrap());
    std::fs::write(rolling_task(&fixture), task("rolling alpha", "base beta")).unwrap();
    assert!(
        fixture
            .repo
            .rolling_updates_commit("an edit for later")
            .unwrap()
    );
    assert!(
        !fixture
            .repo
            .rolling_updates_commit("nothing again")
            .unwrap()
    );
}

#[test]
fn a_code_only_move_of_main_replays_the_rolling_updates_branch_and_materializes_no_code() {
    let fixture = fixture();
    std::fs::write(rolling_task(&fixture), task("rolling alpha", "base beta")).unwrap();
    fixture
        .repo
        .rolling_updates_commit("an edit for later")
        .unwrap();
    commit_on_main(&fixture, "fn main() { }\n", "src/main.rs", "code");

    assert_eq!(
        fixture.repo.rolling_updates_rebase("main").unwrap(),
        Rebased::Clean
    );

    assert!(!fixture.rolling.join("src").exists());
    let text = std::fs::read_to_string(rolling_task(&fixture)).unwrap();
    assert!(text.contains("rolling alpha"), "{text}");
}

#[test]
fn edits_to_different_sections_replay_through_the_driver() {
    let fixture = fixture();
    std::fs::write(rolling_task(&fixture), task("base alpha", "rolling beta")).unwrap();
    fixture
        .repo
        .rolling_updates_commit("an edit for later")
        .unwrap();
    commit_on_main(
        &fixture,
        &task("main alpha", "base beta"),
        ".plan/tasks/00001-t.md",
        "an edit on main",
    );

    assert_eq!(
        fixture.repo.rolling_updates_rebase("main").unwrap(),
        Rebased::Clean
    );

    let text = std::fs::read_to_string(rolling_task(&fixture)).unwrap();
    assert!(text.contains("main alpha"), "{text}");
    assert!(text.contains("rolling beta"), "{text}");
}

#[test]
fn the_same_section_on_both_sides_holds_the_rolling_updates_branch_at_the_conflict() {
    let fixture = fixture();
    std::fs::write(rolling_task(&fixture), task("rolling alpha", "base beta")).unwrap();
    fixture
        .repo
        .rolling_updates_commit("an edit for later")
        .unwrap();
    commit_on_main(
        &fixture,
        &task("main alpha", "base beta"),
        ".plan/tasks/00001-t.md",
        "an edit on main",
    );

    let blocked = fixture.repo.rolling_updates_rebase("main").unwrap();

    assert_eq!(
        blocked,
        Rebased::Blocked {
            paths: vec![".plan/tasks/00001-t.md".to_owned()]
        }
    );
    assert!(fixture.repo.rolling_updates_rebase_in_progress());
    let text = std::fs::read_to_string(rolling_task(&fixture)).unwrap();
    assert!(text.contains("<<<<<<<"), "{text}");

    fixture.repo.rolling_updates_rebase_abort().unwrap();
    assert!(!fixture.repo.rolling_updates_rebase_in_progress());
}

fn bare_remote(fixture: &Fixture) -> PathBuf {
    let remote = fixture.root.parent().unwrap().join("remote.git");
    git(
        fixture.root.parent().unwrap(),
        &["init", "-q", "--bare", remote.to_str().unwrap()],
    );
    git(
        &fixture.root,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    remote
}

fn remote_commit(remote: &Path, branch: &str) -> Option<String> {
    let out = Command::new("git")
        .current_dir(remote)
        .args(["rev-parse", branch])
        .output()
        .unwrap();
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

#[test]
fn the_remote_branch_carries_the_person_and_a_config_key_overrides_it() {
    let fixture = fixture();

    assert_eq!(
        fixture.repo.rolling_updates_remote_branch(),
        format!("{ROLLING_UPDATES_BRANCH}-t")
    );

    git(
        &fixture.root,
        &["config", "openplan.rollingUpdatesBranch", "tasks/milan"],
    );
    assert_eq!(fixture.repo.rolling_updates_remote_branch(), "tasks/milan");
}

#[test]
fn the_publish_remote_follows_the_default_branch_and_falls_back_to_origin() {
    let fixture = fixture();

    assert_eq!(fixture.repo.rolling_updates_remote("main"), "origin");

    git(&fixture.root, &["config", "branch.main.remote", "upstream"]);
    assert_eq!(fixture.repo.rolling_updates_remote("main"), "upstream");
}

#[test]
fn publish_pushes_the_branch_to_the_remote_and_leaves_main_alone() {
    let fixture = fixture();
    let remote = bare_remote(&fixture);
    let main = fixture.repo.branch_commit("main").unwrap();
    std::fs::write(rolling_task(&fixture), task("rolling alpha", "base beta")).unwrap();
    fixture
        .repo
        .rolling_updates_commit("an edit for later")
        .unwrap();
    let tip = fixture.repo.branch_commit(ROLLING_UPDATES_BRANCH).unwrap();
    let branch = fixture.repo.rolling_updates_remote_branch();

    fixture
        .repo
        .push_rolling_updates("origin", &branch)
        .unwrap();

    assert_eq!(
        remote_commit(&remote, &branch).as_deref(),
        Some(tip.as_str())
    );
    assert_eq!(fixture.repo.branch_commit("main").unwrap(), main);
    assert_eq!(remote_commit(&remote, "main"), None);
}

#[test]
fn a_rebase_rewrites_the_branch_and_the_next_push_replaces_it() {
    let fixture = fixture();
    let remote = bare_remote(&fixture);
    let branch = fixture.repo.rolling_updates_remote_branch();
    std::fs::write(rolling_task(&fixture), task("rolling alpha", "base beta")).unwrap();
    fixture
        .repo
        .rolling_updates_commit("an edit for later")
        .unwrap();
    fixture
        .repo
        .push_rolling_updates("origin", &branch)
        .unwrap();
    let first = remote_commit(&remote, &branch).unwrap();

    commit_on_main(&fixture, "fn main() { }\n", "src/main.rs", "code");
    fixture.repo.rolling_updates_rebase("main").unwrap();
    fixture
        .repo
        .push_rolling_updates("origin", &branch)
        .unwrap();

    let second = remote_commit(&remote, &branch).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        second,
        fixture.repo.branch_commit(ROLLING_UPDATES_BRANCH).unwrap()
    );
}
