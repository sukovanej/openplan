use std::path::Path;
use std::process::Command;

use op_git::Repo;
use op_index::Index;
use op_store::{Config, Store};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed");
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn task(root: &Path, number: u64, contents: &str) {
    write(
        &root.join(format!(".plan/tasks/{number:05}-task-{number}.md")),
        contents,
    );
}

fn key(number: u64) -> String {
    format!("OPP-{number}")
}

fn init(root: &Path) {
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    write(&root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n");
}

fn commit(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", message]);
}

fn commit_at(root: &Path, message: &str, at: &str) {
    git(root, &["add", "-A"]);
    let status = Command::new("git")
        .current_dir(root)
        .args(["commit", "-qm", message])
        .env("GIT_AUTHOR_DATE", at)
        .env("GIT_COMMITTER_DATE", at)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git commit failed");
}

fn built(root: &Path) -> Index {
    let repo = Repo::discover(root).unwrap();
    let store = Store::discover(root).unwrap();
    let mut index = Index::new(&Config::read(root).unwrap());
    index.rebuild(&repo, &store).unwrap();
    index
}

fn ids(index: &Index, query: &str) -> Vec<String> {
    index
        .search("test", query)
        .into_iter()
        .map(|hit| hit.task.id)
        .collect()
}

fn seeded(root: &Path) {
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship the login page\n\nSupport OAuth.\n",
    );
    task(
        root,
        2,
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\nparent: ./00001-task-1.md\n---\n# Add validation\n",
    );
    commit(root, "two tasks");
}

#[test]
fn the_title_matches_without_regard_to_case() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    seeded(root);

    let index = built(root);
    assert_eq!(ids(&index, "LOGIN"), vec![key(1)]);
    assert_eq!(ids(&index, "login page"), vec![key(1)]);
}

#[test]
fn the_body_matches() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    seeded(root);

    assert_eq!(ids(&built(root), "oauth"), vec![key(1)]);
}

#[test]
fn the_frontmatter_fields_match() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    seeded(root);

    let index = built(root);
    assert_eq!(ids(&index, "done"), vec![key(2)], "the status");
    assert_eq!(
        ids(&index, "OPP-1"),
        vec![key(1), key(2)],
        "the key finds the task itself and the child that names it as a parent"
    );
}

#[test]
fn a_key_finds_its_own_task() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    seeded(root);

    let index = built(root);
    assert_eq!(ids(&index, "opp-2"), vec![key(2)], "case does not matter");
    assert_eq!(
        ids(&index, "OPP-"),
        vec![key(1), key(2)],
        "so does a prefix"
    );
}

#[test]
fn a_query_of_nothing_but_spaces_matches_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    seeded(root);

    let index = built(root);
    assert!(
        ids(&index, "  ").is_empty(),
        "a space is in every title, so this would be the whole store"
    );
    assert_eq!(
        ids(&index, "login page"),
        vec![key(1)],
        "a space inside a query still matches literally"
    );
}

#[test]
fn an_empty_query_matches_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    seeded(root);

    assert!(ids(&built(root), "").is_empty());
}

#[test]
fn a_query_that_matches_nothing_returns_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    seeded(root);

    assert!(ids(&built(root), "kubernetes").is_empty());
}

#[test]
fn hits_changed_at_the_same_time_are_ordered_by_id_as_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    for number in [1, 2, 10] {
        task(
            root,
            number,
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Shared word\n",
        );
    }
    commit(root, "three tasks");

    assert_eq!(
        ids(&built(root), "shared"),
        vec![key(1), key(2), key(10)],
        "one commit dates all three, so the id breaks the tie: 10 sorts after 2, not between 1 and 2"
    );
}

#[test]
fn a_key_hit_leads_a_title_hit_and_a_title_hit_leads_a_body_hit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Name the zeppelin\n",
    );
    task(
        root,
        2,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Fly it\n\nA zeppelin needs a mast.\n",
    );
    task(
        root,
        3,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Land it\n",
    );
    commit(root, "three tasks");

    assert_eq!(
        ids(&built(root), "zeppelin"),
        vec![key(1), key(2)],
        "the title leads the body"
    );
}

#[test]
fn a_key_hit_leads_a_task_that_only_names_that_key() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# A parent\n",
    );
    task(
        root,
        2,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# OPP-3 in the title\n",
    );
    task(
        root,
        3,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: ./00001-task-1.md\n---\n# A child\n",
    );
    commit(root, "three tasks");

    assert_eq!(
        ids(&built(root), "OPP-3"),
        vec![key(3), key(2)],
        "the task the key names leads the task whose title only mentions it"
    );
}

#[test]
fn text_only_a_branch_carries_matches_and_names_that_branch() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Shared\n",
    );
    commit(root, "one task");
    git(root, &["checkout", "-q", "-b", "feature"]);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Shared\n\nWith a zeppelin.\n",
    );
    commit(root, "feature adds a word");
    git(root, &["checkout", "-q", "main"]);

    let index = built(root);
    let hits = index.search("test", "zeppelin");
    assert_eq!(hits.len(), 1, "one row per task, not one per branch");
    assert_eq!(hits[0].task.id, key(1));
    assert_eq!(hits[0].branch, "feature");
}

#[test]
fn a_hit_every_branch_carries_names_the_headline_branch() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Shared\n",
    );
    commit(root, "one task");
    git(root, &["checkout", "-q", "-b", "feature"]);
    task(
        root,
        1,
        "---\nstatus: in_progress\ncreated: 2026-01-01T00:00:00Z\n---\n# Shared\n",
    );
    commit(root, "feature moves it on");

    let index = built(root);
    let hits = index.search("test", "shared");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].branch, hits[0].task.headline);
    assert_eq!(hits[0].branch, "feature");
}

#[test]
fn a_deleted_task_still_on_main_is_found_once() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Doomed\n",
    );
    commit(root, "one task");
    git(root, &["checkout", "-q", "-b", "feature"]);
    std::fs::remove_file(root.join(".plan/tasks/00001-task-1.md")).unwrap();
    commit(root, "feature drops it");
    git(root, &["checkout", "-q", "main"]);

    let hits = built(root).search("test", "doomed");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].branch, "main", "the branch that still carries it");
}

#[test]
fn the_task_changed_last_leads_the_hits_that_matched_the_same_way() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    for number in [1, 2, 3] {
        task(
            root,
            number,
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Shared word\n",
        );
    }
    commit_at(root, "three tasks", "2026-01-01T00:00:00Z");
    task(
        root,
        2,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Shared word again\n",
    );
    commit_at(root, "task 2 moves on", "2026-02-01T00:00:00Z");
    task(
        root,
        3,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Shared word once more\n",
    );
    commit_at(root, "task 3 moves on", "2026-03-01T00:00:00Z");

    assert_eq!(
        ids(&built(root), "shared"),
        vec![key(3), key(2), key(1)],
        "the newest change leads, and the id no longer decides"
    );
}

#[test]
fn a_key_hit_leads_a_title_hit_the_change_time_cannot_overturn() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# A parent\n",
    );
    task(
        root,
        2,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# OPP-1 in the title\n",
    );
    commit_at(root, "two tasks", "2026-01-01T00:00:00Z");
    task(
        root,
        2,
        "---\nstatus: in_progress\ncreated: 2026-01-01T00:00:00Z\n---\n# OPP-1 in the title\n",
    );
    commit_at(root, "task 2 moves on", "2026-03-01T00:00:00Z");

    assert_eq!(
        ids(&built(root), "OPP-1"),
        vec![key(1), key(2)],
        "the task the key names leads, however long ago it changed"
    );
}
