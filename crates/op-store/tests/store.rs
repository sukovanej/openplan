use op_store::{Store, StoreError};
use op_task::{Status, Task};

fn make_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    let store = Store::open(dir.path()).unwrap();
    (dir, store)
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

#[test]
fn create_read_update_delete_roundtrip() {
    let (_dir, store) = make_store();

    let id = store
        .create(&Task::new("Wire the parser", Status::Todo))
        .unwrap();
    assert!(id.starts_with("wire-the-parser-"), "slug id: {id}");

    let created = store.read(&id).unwrap();
    assert_eq!(created.frontmatter.status, Status::Todo);
    assert_eq!(created.title().as_deref(), Some("Wire the parser"));

    let raw = std::fs::read_to_string(store.task_path(&id)).unwrap();
    assert!(!raw.contains(&id), "id must never be written into the file");

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
    let a = store
        .create(&Task::new("Same title", Status::Todo))
        .unwrap();
    let b = store
        .create(&Task::new("Same title", Status::Todo))
        .unwrap();
    assert_ne!(a, b);
    assert!(store.exists(&a) && store.exists(&b));
}

#[test]
fn update_preserves_body_byte_for_byte() {
    let (_dir, store) = make_store();
    let body = "# Ship it\n\n## Plan\n- a\n- b\n";
    std::fs::write(
        store.task_path("t"),
        format!("---\nstatus: todo\n---\n{body}"),
    )
    .unwrap();

    store
        .update("t", |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();

    let raw = std::fs::read_to_string(store.task_path("t")).unwrap();
    assert_eq!(raw, format!("---\nstatus: done\n---\n{body}"));
}

#[test]
fn missing_task_mutations_are_not_found() {
    let (_dir, store) = make_store();
    assert!(matches!(
        store.delete("ghost"),
        Err(StoreError::NotFound { .. })
    ));
    assert!(matches!(
        store.update("ghost", |_| Ok(())),
        Err(StoreError::NotFound { .. })
    ));
    assert!(matches!(
        store.write("ghost", &Task::new("x", Status::Todo)),
        Err(StoreError::NotFound { .. })
    ));
}

#[test]
fn create_and_update_validate_references() {
    let (_dir, store) = make_store();
    let parent = store.create(&Task::new("Parent", Status::Todo)).unwrap();

    let mut child = Task::new("Child", Status::Todo);
    child.set_parent(Some("does-not-exist".to_owned()));
    assert!(matches!(store.create(&child), Err(StoreError::Invalid(_))));

    child.set_parent(Some(parent.clone()));
    let child = store.create(&child).unwrap();

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
    let a = store.create(&Task::new("A", Status::Todo)).unwrap();

    let mut b = Task::new("B", Status::Todo);
    b.set_parent(Some(a.clone()));
    let b = store.create(&b).unwrap();

    let mut c = Task::new("C", Status::Todo);
    c.set_parent(Some(b.clone()));
    let c = store.create(&c).unwrap();

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
    let a = store.create(&Task::new("A", Status::Todo)).unwrap();

    let mut b = Task::new("B", Status::Todo);
    b.set_deps(vec![a.clone()]);
    let b = store.create(&b).unwrap();

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
    let mut bad = Task::new("placeholder", Status::Todo);

    for body in ["no heading at all\n", "# \n", "# One\n# Two\n"] {
        bad.body = body.to_owned();
        assert!(
            matches!(store.create(&bad), Err(StoreError::Invalid(_))),
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
    std::fs::write(
        store.task_path("t"),
        "---\nstatus: todo\nestimate: 3.5\n---\n# T\n",
    )
    .unwrap();

    store
        .update("t", |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();

    let raw = std::fs::read_to_string(store.task_path("t")).unwrap();
    assert!(
        raw.contains("estimate: 3.5"),
        "estimate must survive update: {raw}"
    );
    assert!(raw.contains("status: done"));
}

#[test]
fn concurrent_updates_to_one_task_serialize() {
    let (_dir, store) = make_store();
    let id = store.create(&Task::new("Contended", Status::Todo)).unwrap();

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
    let id = store.create(&Task::new("Clean", Status::Todo)).unwrap();
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
    let id = store.create(&Task::new("A", Status::Todo)).unwrap();

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
    let id = store.create(&Task::new("A", Status::Todo)).unwrap();
    let path = store.tasks_dir().join(format!("{id}.md"));
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
