use std::path::Path;
use std::process::Command;

use op_api::Status;
use op_git::Repo;
use op_index::Index;
use op_store::Store;

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

fn init(root: &Path) {
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
}

fn built(root: &Path) -> Index {
    let repo = Repo::discover(root).unwrap();
    let store = Store::open(root).unwrap();
    let mut index = Index::new();
    index.rebuild(&repo, &store).unwrap();
    index
}

#[test]
fn matrix_rows_per_task_branch_with_blob_dedup() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);

    // `shared` is byte-identical on both branches (feature forks before `only-main` is added),
    // so it is one blob shared by two branches — it must parse exactly once.
    write(
        &root.join(".plan/tasks/shared.md"),
        "---\nstatus: todo\n---\n# Shared\n",
    );
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "add shared"]);
    git(root, &["branch", "feature"]);
    write(
        &root.join(".plan/tasks/only-main.md"),
        "---\nstatus: done\n---\n# Only main\n",
    );
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "add only-main"]);

    let index = built(root);
    let cells = &index.matrix().cells;

    assert_eq!(cells.len(), 3, "shared×2 branches + only-main×1: {cells:?}");
    assert!(
        index.cached_versions() < cells.len(),
        "identical blob parsed once: {} cached < {} rows",
        index.cached_versions(),
        cells.len()
    );

    let shared: Vec<_> = cells.iter().filter(|c| c.task.id == "shared").collect();
    assert_eq!(shared.len(), 2);
    assert_eq!(
        shared[0].blob_oid, shared[1].blob_oid,
        "identical versions share a blob OID"
    );
    assert!(shared.iter().all(|c| !c.dirty));
}

#[test]
fn working_tree_overlay_marks_dirty_in_a_linked_worktree() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    write(
        &root.join(".plan/tasks/t.md"),
        "---\nstatus: todo\n---\n# T\n",
    );
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "add t"]);
    git(root, &["branch", "feature"]);

    let wt = tempfile::tempdir().unwrap();
    let wt_path = wt.path().join("feature");
    git(
        root,
        &[
            "worktree",
            "add",
            "-q",
            wt_path.to_str().unwrap(),
            "feature",
        ],
    );
    // Uncommitted edit in the feature worktree — the effective state must overlay this.
    write(
        &wt_path.join(".plan/tasks/t.md"),
        "---\nstatus: in_progress\n---\n# T edited\n",
    );

    let repo = Repo::discover(root).unwrap();
    let index = built(root);
    let cells = &index.matrix().cells;

    let feature = cells
        .iter()
        .find(|c| c.task.id == "t" && c.branch == "feature")
        .expect("feature cell");
    assert!(feature.dirty, "uncommitted worktree edit is dirty");
    assert_eq!(feature.task.status, Status::InProgress);
    assert_eq!(feature.task.title, "T edited");

    let main = cells
        .iter()
        .find(|c| c.branch == "main")
        .expect("main cell");
    assert!(!main.dirty);
    assert_eq!(main.task.status, Status::Todo);

    let raw = index
        .effective_raw(&repo, "t", "feature")
        .unwrap()
        .expect("effective feature version");
    assert!(
        raw.contains("in_progress"),
        "dirty read is the working copy: {raw}"
    );
}

#[test]
fn unparseable_frontmatter_does_not_abort_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    // Markers in the body: frontmatter still parses.
    write(
        &root.join(".plan/tasks/body.md"),
        "---\nstatus: todo\n---\n# Body\n\n## Plan\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> other\n",
    );
    // Markers straddling the frontmatter fence: parsing fails, so the fallback summary is used.
    write(
        &root.join(".plan/tasks/head.md"),
        "---\n<<<<<<< HEAD\nstatus: todo\n=======\nstatus: done\n>>>>>>> other\n---\n# Head\n",
    );
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "broken frontmatter"]);

    let index = built(root);
    let cells = &index.matrix().cells;
    assert_eq!(cells.len(), 2, "rebuild kept both cells: {cells:?}");

    let body = cells.iter().find(|c| c.task.id == "body").unwrap();
    assert_eq!(body.task.title, "Body");

    let head = cells.iter().find(|c| c.task.id == "head").unwrap();
    assert_eq!(head.task.title, "Head", "best-effort title survives");
}

#[test]
fn no_worktree_uses_committed_blobs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    write(
        &root.join(".plan/tasks/a.md"),
        "---\nstatus: todo\n---\n# A\n",
    );
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "add a"]);
    // Diverge `a` on feature without ever checking it out.
    git(root, &["checkout", "-q", "-b", "feature"]);
    write(
        &root.join(".plan/tasks/a.md"),
        "---\nstatus: done\n---\n# A done\n",
    );
    git(root, &["commit", "-qam", "edit a"]);
    git(root, &["checkout", "-q", "main"]);

    let index = built(root);
    let feature = index
        .matrix()
        .cells
        .iter()
        .find(|c| c.branch == "feature")
        .expect("feature cell from the object DB, no checkout");
    assert!(!feature.dirty);
    assert_eq!(feature.task.status, Status::Done);
    assert_eq!(feature.task.title, "A done");
}
