use std::sync::atomic::{AtomicU64, Ordering};

use op_store::{Store, StoreError};
use op_task::{Status, Task, Timestamp};

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().unwrap()
}

fn make_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    let store = Store::open(dir.path()).unwrap();
    (dir, store)
}

// The daemon owns allocation repo-wide; a store test only needs each number to be fresh.
fn create(store: &Store, task: &Task) -> Result<String, StoreError> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    store.create(task, NEXT.fetch_add(1, Ordering::Relaxed))
}

// A task file's name pads its id and carries a title slug (§3.1); a test that plants one by hand
// still has to name it the way the store does.
fn plant(store: &Store, id: &str, contents: &str) -> std::path::PathBuf {
    let path = store
        .tasks_dir()
        .join(op_store::task_filename(id.parse().unwrap(), "Planted"));
    std::fs::write(&path, contents).unwrap();
    path
}

fn temp_files(store: &Store) -> Vec<String> {
    std::fs::read_dir(store.tasks_dir())
        .unwrap()
        .filter_map(|e| e.unwrap().file_name().into_string().ok())
        .filter(|name| !name.ends_with(".md"))
        .collect()
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
fn task_ids_are_numerically_sorted_and_skip_files_no_id_names() {
    let (_dir, store) = make_store();
    for id in ["10", "2", "1"] {
        plant(
            &store,
            id,
            "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n",
        );
    }
    std::fs::write(store.tasks_dir().join("notes.txt"), "ignored").unwrap();
    std::fs::write(store.tasks_dir().join("ship-login-3d0c.md"), "ignored").unwrap();

    assert_eq!(store.task_ids().unwrap(), vec!["1", "2", "10"]);
}

#[test]
fn a_string_that_is_not_an_id_names_no_task() {
    let (_dir, store) = make_store();
    std::fs::write(store.tasks_dir().join("ship-login-3d0c.md"), "# Old\n").unwrap();

    for not_an_id in ["ship-login-3d0c", "042", "+7", "", "../escape"] {
        assert!(
            matches!(
                store.task_path(not_an_id),
                Err(StoreError::InvalidId { .. })
            ),
            "{not_an_id:?} must not name a file"
        );
        assert!(!store.exists(not_an_id));
        assert!(matches!(
            store.read_raw(not_an_id),
            Err(StoreError::InvalidId { .. })
        ));
        assert!(matches!(
            store.delete(not_an_id),
            Err(StoreError::InvalidId { .. })
        ));
    }
}

#[test]
fn read_parses_status_and_title() {
    let (_dir, store) = make_store();
    plant(
        &store,
        "1",
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship it\n",
    );

    let task = store.read("1").unwrap();
    assert_eq!(task.frontmatter.status, Status::Done);
    assert_eq!(task.title().as_deref(), Some("Ship it"));
}

#[test]
fn with_lock_runs_closure_under_existing_file() {
    let (_dir, store) = make_store();
    plant(
        &store,
        "1",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n",
    );

    let value = store.with_lock("1", || Ok(42)).unwrap();
    assert_eq!(value, 42);
}

#[test]
fn with_lock_does_not_create_missing_task() {
    let (_dir, store) = make_store();

    assert!(store.with_lock("99", || Ok(())).is_err());
    assert!(
        !store.exists("99"),
        "with_lock must not create a phantom task file"
    );
}

#[test]
fn create_read_update_delete_roundtrip() {
    let (_dir, store) = make_store();

    let id = create(&store, &Task::new("Wire the parser", Status::Todo, stamp())).unwrap();
    assert!(op_task::parse_id(&id).is_some(), "numeric id: {id}");

    let created = store.read(&id).unwrap();
    assert_eq!(created.frontmatter.status, Status::Todo);
    assert_eq!(created.title().as_deref(), Some("Wire the parser"));

    let raw = std::fs::read_to_string(store.task_path(&id).unwrap()).unwrap();
    assert_eq!(
        raw,
        created.to_file_string().unwrap(),
        "the file holds the task and nothing else — the id lives only in the filename"
    );

    let updated = store
        .update(&id, |task| {
            task.set_status(Status::InProgress);
            Ok(())
        })
        .unwrap();
    assert_eq!(updated.frontmatter.status, Status::InProgress);
    assert_eq!(
        store.read(&id).unwrap().frontmatter.status,
        Status::InProgress
    );

    store.delete(&id).unwrap();
    assert!(!store.exists(&id));
    assert!(matches!(store.read(&id), Err(StoreError::NotFound { .. })));
}

#[test]
fn create_gives_distinct_ids_for_same_title() {
    let (_dir, store) = make_store();
    let a = create(&store, &Task::new("Same title", Status::Todo, stamp())).unwrap();
    let b = create(&store, &Task::new("Same title", Status::Todo, stamp())).unwrap();
    assert_ne!(a, b);
    assert!(store.exists(&a) && store.exists(&b));
}

#[test]
fn create_names_the_file_after_the_allocated_number() {
    let (_dir, store) = make_store();
    let task = Task::new("Wire the parser", Status::Todo, stamp());

    let id = store.create(&task, 7).unwrap();
    assert_eq!(
        id, "7",
        "the id is the number; the title only shapes the file name"
    );

    // The allocator must not hand the same number out twice; if it does, the store refuses rather
    // than let one task's file be overwritten by another's.
    let reused = store.create(&task, 7);
    assert!(
        matches!(&reused, Err(StoreError::IdTaken { id }) if id == "7"),
        "a taken id must be reported, not clobbered: {reused:?}"
    );
    assert!(
        temp_files(&store).is_empty(),
        "a refused create leaves no temp file: {:?}",
        temp_files(&store)
    );
}

#[test]
fn update_preserves_body_byte_for_byte() {
    let (_dir, store) = make_store();
    let body = "# Ship it\n\n## Plan\n- a\n- b\n";
    plant(
        &store,
        "1",
        &format!("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n{body}"),
    );

    store
        .update("1", |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();

    let raw = std::fs::read_to_string(store.task_path("1").unwrap()).unwrap();
    assert_eq!(
        raw,
        format!("---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n{body}")
    );
}

#[test]
fn missing_task_mutations_are_not_found() {
    let (_dir, store) = make_store();
    assert!(matches!(
        store.delete("99"),
        Err(StoreError::NotFound { .. })
    ));
    assert!(matches!(
        store.update("99", |_| Ok(())),
        Err(StoreError::NotFound { .. })
    ));
    assert!(matches!(
        store.write("99", &Task::new("x", Status::Todo, stamp())),
        Err(StoreError::NotFound { .. })
    ));
}

#[test]
fn create_and_update_validate_references() {
    let (_dir, store) = make_store();
    let parent = create(&store, &Task::new("Parent", Status::Todo, stamp())).unwrap();

    let mut child = Task::new("Child", Status::Todo, stamp());
    child.set_parent(Some("999".to_owned()));
    assert!(matches!(
        create(&store, &child),
        Err(StoreError::Invalid(_))
    ));

    child.set_parent(Some("does-not-exist".to_owned()));
    assert!(
        matches!(create(&store, &child), Err(StoreError::InvalidId { .. })),
        "a reference that is not an id is refused as such, not as a missing task"
    );

    child.set_parent(Some(parent.clone()));
    let child = create(&store, &child).unwrap();

    let self_parent = store.update(&child, |task| {
        task.set_parent(Some(child.clone()));
        Ok(())
    });
    assert!(
        matches!(self_parent, Err(StoreError::Invalid(_))),
        "a task may not be its own parent"
    );
    assert_eq!(
        store.read(&child).unwrap().frontmatter.parent.as_deref(),
        Some(parent.as_str()),
        "a rejected update must not mutate the file"
    );
}

#[test]
fn reparenting_under_a_descendant_is_refused() {
    let (_dir, store) = make_store();
    let a = create(&store, &Task::new("A", Status::Todo, stamp())).unwrap();

    let mut b = Task::new("B", Status::Todo, stamp());
    b.set_parent(Some(a.clone()));
    let b = create(&store, &b).unwrap();

    let mut c = Task::new("C", Status::Todo, stamp());
    c.set_parent(Some(b.clone()));
    let c = create(&store, &c).unwrap();

    // A is an ancestor of C; making C the parent of A would close a cycle.
    let cycle = store.update(&a, |task| {
        task.set_parent(Some(c.clone()));
        Ok(())
    });
    assert!(
        matches!(cycle, Err(StoreError::Invalid(_))),
        "a task may not be reparented under its own descendant"
    );
    assert_eq!(
        store.read(&a).unwrap().frontmatter.parent,
        None,
        "a rejected reparent must not mutate the file"
    );
}

#[test]
fn dangling_reference_does_not_block_unrelated_edit() {
    let (_dir, store) = make_store();
    let a = create(&store, &Task::new("A", Status::Todo, stamp())).unwrap();

    let mut b = Task::new("B", Status::Todo, stamp());
    b.set_deps(vec![a.clone()]);
    let b = create(&store, &b).unwrap();

    store.delete(&a).unwrap();

    // B still depends on the now-deleted A; changing an unrelated field must still succeed.
    let updated = store
        .update(&b, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();
    assert_eq!(updated.frontmatter.status, Status::Done);
    assert_eq!(updated.frontmatter.deps, vec![a]);
}

#[test]
fn create_rejects_malformed_title() {
    let (_dir, store) = make_store();
    let mut bad = Task::new("placeholder", Status::Todo, stamp());

    for body in ["no heading at all\n", "# \n", "# One\n# Two\n"] {
        bad.body = body.to_owned();
        assert!(
            matches!(create(&store, &bad), Err(StoreError::Invalid(_))),
            "body {body:?} must be rejected as a malformed title"
        );
    }
    assert!(
        temp_files(&store).is_empty(),
        "a rejected create leaves no temp file: {:?}",
        temp_files(&store)
    );
}

#[test]
fn update_preserves_unknown_frontmatter_keys() {
    let (_dir, store) = make_store();
    plant(
        &store,
        "1",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nestimate: 3.5\n---\n# T\n",
    );

    store
        .update("1", |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();

    let raw = std::fs::read_to_string(store.task_path("1").unwrap()).unwrap();
    assert!(
        raw.contains("estimate: 3.5"),
        "estimate must survive update: {raw}"
    );
    assert!(raw.contains("status: done"));
}

#[test]
fn concurrent_updates_to_one_task_serialize() {
    let (_dir, store) = make_store();
    let id = create(&store, &Task::new("Contended", Status::Todo, stamp())).unwrap();

    let threads = 8;
    let handles: Vec<_> = (0..threads)
        .map(|i| {
            let store = store.clone();
            let id = id.clone();
            std::thread::spawn(move || {
                store
                    .update(&id, |task| {
                        task.body.push_str(&format!("line {i}\n"));
                        Ok(())
                    })
                    .unwrap();
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    let body = store.read(&id).unwrap().body;
    for i in 0..threads {
        assert!(
            body.contains(&format!("line {i}\n")),
            "lost update from thread {i}: no interleaving means every append survives\n{body}"
        );
    }
    assert!(
        temp_files(&store).is_empty(),
        "serialized atomic writes leave no partial temp files: {:?}",
        temp_files(&store)
    );
}

#[test]
fn happy_path_leaves_no_temp_files() {
    let (_dir, store) = make_store();
    let id = create(&store, &Task::new("Clean", Status::Todo, stamp())).unwrap();
    store
        .update(&id, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();
    assert!(
        temp_files(&store).is_empty(),
        "no temp file should survive create+update: {:?}",
        temp_files(&store)
    );
}

#[test]
fn a_malformed_rank_is_refused() {
    let (_dir, store) = make_store();
    let id = create(&store, &Task::new("A", Status::Todo, stamp())).unwrap();

    for bad in ["A", "1.5", "", "a b"] {
        let result = store.update(&id, |task| {
            task.set_rank(Some(bad.to_owned()));
            Ok(())
        });
        assert!(
            matches!(result, Err(StoreError::Invalid(_))),
            "rank {bad:?} must be refused"
        );
        assert_eq!(
            store.read(&id).unwrap().frontmatter.rank,
            None,
            "a rejected rank must not mutate the file"
        );
    }

    store
        .update(&id, |task| {
            task.set_rank(Some("a5".to_owned()));
            Ok(())
        })
        .expect("a base-36 key is accepted");
    assert_eq!(
        store.read(&id).unwrap().frontmatter.rank.as_deref(),
        Some("a5")
    );
}

#[test]
fn an_already_persisted_malformed_rank_does_not_block_an_unrelated_edit() {
    // Task files are hand-editable, so a bad rank can predate the write being validated. Blocking
    // an unrelated status change on it would strand the task; `move` rebalances the group instead.
    let (_dir, store) = make_store();
    let id = create(&store, &Task::new("A", Status::Todo, stamp())).unwrap();
    let path = store.task_path(&id).unwrap();
    let raw = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        raw.replace("status: todo", "status: todo\nrank: NOPE"),
    )
    .unwrap();

    store
        .update(&id, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .expect("an unrelated edit still lands");
    assert_eq!(store.read(&id).unwrap().frontmatter.status, Status::Done);
}

#[test]
fn a_status_change_preserves_created() {
    let (_dir, store) = make_store();
    let id = create(&store, &Task::new("A", Status::Todo, stamp())).unwrap();

    store
        .update(&id, |task| {
            task.set_status(Status::InReview);
            Ok(())
        })
        .unwrap();

    let task = store.read(&id).unwrap();
    assert_eq!(task.frontmatter.status, Status::InReview);
    assert_eq!(task.frontmatter.created, stamp());
}

#[test]
fn a_file_without_created_refuses_to_be_written_and_says_how_to_fix_it() {
    let (_dir, store) = make_store();
    let path = plant(&store, "1", "---\nstatus: in_progress\n---\n# Legacy\n");

    let err = store
        .update("1", |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .expect_err("a write must not invent the date the task was created");

    assert!(matches!(err, StoreError::MissingCreated { .. }), "{err:?}");
    let message = err.to_string();
    // The reader is told which file, which field, and the two ways to get the value.
    assert!(message.contains(&path.display().to_string()), "{message}");
    assert!(message.contains("created:"), "{message}");
    assert!(message.contains("--diff-filter=A"), "{message}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "---\nstatus: in_progress\n---\n# Legacy\n",
        "a refused write leaves the file alone"
    );
}
