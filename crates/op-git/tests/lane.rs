use std::path::{Path, PathBuf};
use std::process::Command;

use op_git::{LANE_BRANCH, Rebased, Repo};

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
    lane: PathBuf,
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
    let lane = repo.ensure_lane("main", &driver).unwrap();
    Fixture {
        _dir: dir,
        root,
        repo,
        lane,
        driver,
    }
}

fn lane_task(fixture: &Fixture) -> PathBuf {
    fixture.lane.join(".plan/tasks/00001-t.md")
}

fn trunk_commit(fixture: &Fixture, contents: &str, path: &str, message: &str) {
    std::fs::write(fixture.root.join(path), contents).unwrap();
    git(&fixture.root, &["add", "-A"]);
    git(&fixture.root, &["commit", "-qm", message]);
}

#[test]
fn the_lane_worktree_holds_the_plan_and_no_code() {
    let fixture = fixture();

    assert!(lane_task(&fixture).is_file());
    assert!(!fixture.lane.join("src").exists());
    assert!(fixture.lane.join(".gitattributes").is_file());
    assert_eq!(
        fixture
            .repo
            .worktree_branch(&fixture.lane)
            .unwrap()
            .as_deref(),
        Some(LANE_BRANCH)
    );
}

#[test]
fn ensure_lane_runs_twice_without_complaint() {
    let fixture = fixture();

    let again = fixture.repo.ensure_lane("main", &fixture.driver).unwrap();

    assert_eq!(again, fixture.lane);
}

#[test]
fn a_commit_lands_only_when_something_changed() {
    let fixture = fixture();

    assert!(!fixture.repo.lane_commit("nothing").unwrap());
    std::fs::write(lane_task(&fixture), task("lane alpha", "base beta")).unwrap();
    assert!(fixture.repo.lane_commit("an ambient edit").unwrap());
    assert!(!fixture.repo.lane_commit("nothing again").unwrap());
}

#[test]
fn a_code_only_move_of_main_replays_the_lane_and_materializes_no_code() {
    let fixture = fixture();
    std::fs::write(lane_task(&fixture), task("lane alpha", "base beta")).unwrap();
    fixture.repo.lane_commit("an ambient edit").unwrap();
    trunk_commit(&fixture, "fn main() { }\n", "src/main.rs", "code");

    assert_eq!(fixture.repo.lane_rebase("main").unwrap(), Rebased::Clean);

    assert!(!fixture.lane.join("src").exists());
    let text = std::fs::read_to_string(lane_task(&fixture)).unwrap();
    assert!(text.contains("lane alpha"), "{text}");
}

#[test]
fn edits_to_different_sections_replay_through_the_driver() {
    let fixture = fixture();
    std::fs::write(lane_task(&fixture), task("base alpha", "lane beta")).unwrap();
    fixture.repo.lane_commit("an ambient edit").unwrap();
    trunk_commit(
        &fixture,
        &task("trunk alpha", "base beta"),
        ".plan/tasks/00001-t.md",
        "trunk edit",
    );

    assert_eq!(fixture.repo.lane_rebase("main").unwrap(), Rebased::Clean);

    let text = std::fs::read_to_string(lane_task(&fixture)).unwrap();
    assert!(text.contains("trunk alpha"), "{text}");
    assert!(text.contains("lane beta"), "{text}");
}

#[test]
fn the_same_section_on_both_sides_holds_the_lane_at_the_conflict() {
    let fixture = fixture();
    std::fs::write(lane_task(&fixture), task("lane alpha", "base beta")).unwrap();
    fixture.repo.lane_commit("an ambient edit").unwrap();
    trunk_commit(
        &fixture,
        &task("trunk alpha", "base beta"),
        ".plan/tasks/00001-t.md",
        "trunk edit",
    );

    let blocked = fixture.repo.lane_rebase("main").unwrap();

    assert_eq!(
        blocked,
        Rebased::Blocked {
            paths: vec![".plan/tasks/00001-t.md".to_owned()]
        }
    );
    assert!(fixture.repo.lane_rebase_in_progress());
    let text = std::fs::read_to_string(lane_task(&fixture)).unwrap();
    assert!(text.contains("<<<<<<<"), "{text}");

    fixture.repo.lane_rebase_abort().unwrap();
    assert!(!fixture.repo.lane_rebase_in_progress());
}

#[test]
fn publish_fast_forwards_the_checked_out_trunk_and_moves_its_files() {
    let fixture = fixture();
    std::fs::write(lane_task(&fixture), task("lane alpha", "base beta")).unwrap();
    fixture.repo.lane_commit("an ambient edit").unwrap();
    fixture.repo.lane_rebase("main").unwrap();
    let tip = fixture.repo.branch_commit(LANE_BRANCH).unwrap();

    fixture.repo.fast_forward("main", &tip).unwrap();

    assert_eq!(fixture.repo.branch_commit("main").unwrap(), tip);
    let text = std::fs::read_to_string(fixture.root.join(".plan/tasks/00001-t.md")).unwrap();
    assert!(text.contains("lane alpha"), "{text}");
}

#[test]
fn publish_refuses_when_the_trunk_worktree_has_uncommitted_task_edits() {
    let fixture = fixture();
    std::fs::write(lane_task(&fixture), task("lane alpha", "base beta")).unwrap();
    fixture.repo.lane_commit("an ambient edit").unwrap();
    fixture.repo.lane_rebase("main").unwrap();
    let tip = fixture.repo.branch_commit(LANE_BRANCH).unwrap();
    std::fs::write(
        fixture.root.join(".plan/tasks/00002-x.md"),
        task("stray", "stray"),
    )
    .unwrap();

    let refused = fixture.repo.fast_forward("main", &tip);

    assert!(matches!(
        refused,
        Err(op_git::GitError::WorktreeDirty { .. })
    ));
    assert_ne!(fixture.repo.branch_commit("main").unwrap(), tip);
}

#[test]
fn publish_refuses_a_target_that_is_not_a_fast_forward() {
    let fixture = fixture();
    std::fs::write(lane_task(&fixture), task("lane alpha", "base beta")).unwrap();
    fixture.repo.lane_commit("an ambient edit").unwrap();
    let tip = fixture.repo.branch_commit(LANE_BRANCH).unwrap();
    trunk_commit(&fixture, "fn main() { }\n", "src/main.rs", "code");

    let refused = fixture.repo.fast_forward("main", &tip);

    assert!(matches!(
        refused,
        Err(op_git::GitError::NotFastForward { .. })
    ));
}

#[test]
fn the_backup_push_sends_the_lane_to_a_mirror() {
    let fixture = fixture();
    let mirror = fixture.root.join("../mirror.git");
    git(
        &fixture.root,
        &["init", "-q", "--bare", mirror.to_str().unwrap()],
    );
    std::fs::write(lane_task(&fixture), task("lane alpha", "base beta")).unwrap();
    fixture.repo.lane_commit("an ambient edit").unwrap();

    fixture.repo.push_lane(mirror.to_str().unwrap()).unwrap();

    let there = Repo::discover(&mirror)
        .unwrap()
        .branch_commit(LANE_BRANCH)
        .unwrap();
    assert_eq!(there, fixture.repo.branch_commit(LANE_BRANCH).unwrap());
}
