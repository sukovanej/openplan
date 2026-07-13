use op_store::Store;
use op_task::Status;

fn make_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    let store = Store::open(dir.path()).unwrap();
    (dir, store)
}

#[test]
fn open_requires_plan_at_root() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Store::open(dir.path()).is_err());
    std::fs::create_dir_all(dir.path().join(".plan")).unwrap();
    assert!(Store::open(dir.path()).is_ok());
}

#[test]
fn discover_walks_up_to_store_root() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    let nested = dir.path().join("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();

    let store = Store::discover(&nested).unwrap();
    assert_eq!(store.plan_dir(), dir.path().join(".plan"));
}

#[test]
fn discover_fails_when_no_store_above() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Store::discover(dir.path()).is_err());
}

#[test]
fn task_ids_are_sorted_stems() {
    let (_dir, store) = make_store();
    std::fs::write(store.task_path("beta"), "---\nstatus: done\n---\n# B\n").unwrap();
    std::fs::write(store.task_path("alpha"), "---\nstatus: todo\n---\n# A\n").unwrap();
    std::fs::write(store.tasks_dir().join("notes.txt"), "ignored").unwrap();

    assert_eq!(store.task_ids().unwrap(), vec!["alpha", "beta"]);
}

#[test]
fn read_parses_status_and_title() {
    let (_dir, store) = make_store();
    std::fs::write(store.task_path("t"), "---\nstatus: done\n---\n# Ship it\n").unwrap();

    let task = store.read("t").unwrap();
    assert_eq!(task.frontmatter.status, Status::Done);
    assert_eq!(task.title().as_deref(), Some("Ship it"));
}

#[test]
fn with_lock_runs_closure_under_existing_file() {
    let (_dir, store) = make_store();
    std::fs::write(store.task_path("t"), "---\nstatus: todo\n---\n# T\n").unwrap();

    let value = store.with_lock("t", || Ok(42)).unwrap();
    assert_eq!(value, 42);
}

#[test]
fn with_lock_does_not_create_missing_task() {
    let (_dir, store) = make_store();

    assert!(store.with_lock("ghost", || Ok(())).is_err());
    assert!(
        !store.task_path("ghost").exists(),
        "with_lock must not create a phantom task file"
    );
}
