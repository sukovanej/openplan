use std::sync::atomic::{AtomicU64, Ordering};

use op_store::{Config, Store, StoreError};
use op_task::tag::{Color, Tag};
use op_task::{Abbreviation, Status, Task, Timestamp};

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().unwrap()
}

fn abbreviation() -> Abbreviation {
    "OPP".parse().unwrap()
}

fn plant_store(dir: &std::path::Path, config: Option<&str>) {
    std::fs::create_dir_all(dir.join(".plan/tasks")).unwrap();
    if let Some(config) = config {
        std::fs::write(dir.join(".plan/config.toml"), config).unwrap();
    }
}

fn make_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    plant_store(dir.path(), Some("abbreviation = \"OPP\"\n"));
    let store = Store::discover(dir.path()).unwrap();
    (dir, store)
}

// The daemon owns allocation repo-wide; a store test only needs each number to be fresh.
fn create(store: &Store, task: &Task) -> Result<u64, StoreError> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    store.create(task, NEXT.fetch_add(1, Ordering::Relaxed))
}

// A task file's name pads its id and carries a title slug; a test that plants one by hand
// still has to name it the way the store does.
fn plant(store: &Store, id: u64, contents: &str) -> std::path::PathBuf {
    let path = store
        .tasks_dir()
        .join(op_task::task_filename(id, "Planted"));
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
    assert!(Store::open(dir.path(), abbreviation()).is_err());
    std::fs::create_dir_all(dir.path().join(".plan")).unwrap();
    assert!(Store::open(dir.path(), abbreviation()).is_ok());
}

#[test]
fn open_takes_the_abbreviation_it_is_given_whatever_the_file_says() {
    let dir = tempfile::tempdir().unwrap();
    plant_store(dir.path(), Some("abbreviation = \"WEB\"\n"));

    let store = Store::open(dir.path(), abbreviation()).unwrap();
    assert_eq!(
        store.abbreviation(),
        abbreviation(),
        "a sibling worktree is read under the serving store's abbreviation, not its own"
    );
}

#[test]
fn discover_walks_up_to_store_root() {
    let dir = tempfile::tempdir().unwrap();
    plant_store(dir.path(), Some("abbreviation = \"OPP\"\n"));
    let nested = dir.path().join("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();

    let store = Store::discover(&nested).unwrap();
    assert_eq!(store.plan_dir(), dir.path().join(".plan"));
    assert_eq!(store.abbreviation(), abbreviation());
}

#[test]
fn discover_fails_when_no_store_above() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        Store::discover(dir.path()),
        Err(StoreError::StoreMissing)
    ));
}

#[test]
fn discover_refuses_a_store_that_names_no_abbreviation() {
    let dir = tempfile::tempdir().unwrap();
    plant_store(dir.path(), None);

    let err = Store::discover(dir.path()).expect_err("a store with no id space cannot be opened");
    assert_eq!(
        err.to_string(),
        ".plan/config.toml: 'abbreviation' required",
        "the reader is told which file and which field"
    );
}

#[test]
fn a_config_that_names_no_usable_abbreviation_is_refused() {
    let must_be = ".plan/config.toml: 'abbreviation' must be exactly three uppercase letters";
    for (config, expected) in [
        ("", ".plan/config.toml: 'abbreviation' required"),
        ("other = 1\n", ".plan/config.toml: 'abbreviation' required"),
        ("abbreviation = \"opp\"\n", must_be),
        ("abbreviation = \"OP\"\n", must_be),
        ("abbreviation = \"OPPX\"\n", must_be),
        ("abbreviation = \"OP1\"\n", must_be),
        ("abbreviation = 42\n", must_be),
        ("abbreviation = [\"OPP\"]\n", must_be),
    ] {
        let dir = tempfile::tempdir().unwrap();
        plant_store(dir.path(), Some(config));
        let err = Store::discover(dir.path()).expect_err(&format!("{config:?} must be refused"));
        assert_eq!(err.to_string(), expected, "for config {config:?}");
    }
}

#[test]
fn a_config_that_is_not_toml_is_refused_with_the_parse_error() {
    let dir = tempfile::tempdir().unwrap();
    plant_store(dir.path(), Some("abbreviation = \n"));

    let err = Store::discover(dir.path()).expect_err("a malformed config cannot be read");
    assert!(
        err.to_string().starts_with(".plan/config.toml: "),
        "the file is named before the parser's complaint: {err}"
    );
}

#[test]
fn a_valid_config_reads_back_its_abbreviation() {
    let dir = tempfile::tempdir().unwrap();
    plant_store(dir.path(), Some("abbreviation = \"WEB\"\n"));

    assert_eq!(
        Config::read(dir.path()).unwrap().abbreviation,
        "WEB".parse::<Abbreviation>().unwrap()
    );
}

#[test]
fn a_config_reads_back_its_default_branch_and_lives_without_one() {
    for (config, expected) in [
        ("abbreviation = \"WEB\"\n", None),
        (
            "abbreviation = \"WEB\"\ndefault_branch = \"dev\"\n",
            Some("dev".to_owned()),
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        plant_store(dir.path(), Some(config));
        assert_eq!(
            Config::read(dir.path()).unwrap().default_branch,
            expected,
            "for config {config:?}"
        );
    }
}

#[test]
fn a_default_branch_that_is_not_a_string_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    plant_store(
        dir.path(),
        Some("abbreviation = \"WEB\"\ndefault_branch = 42\n"),
    );

    let err = Config::read(dir.path()).expect_err("a non-string branch name cannot be read");
    assert_eq!(
        err.to_string(),
        ".plan/config.toml: 'default_branch' must be a string"
    );
}

#[test]
fn task_ids_are_numerically_sorted_and_skip_files_no_id_names() {
    let (_dir, store) = make_store();
    for id in [10, 2, 1] {
        plant(
            &store,
            id,
            "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n",
        );
    }
    std::fs::write(store.tasks_dir().join("notes.txt"), "ignored").unwrap();
    std::fs::write(store.tasks_dir().join("ship-login-3d0c.md"), "ignored").unwrap();

    assert_eq!(store.task_ids().unwrap(), vec![1, 2, 10]);
}

#[test]
fn read_parses_status_and_title() {
    let (_dir, store) = make_store();
    plant(
        &store,
        1,
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship it\n",
    );

    let task = store.read(1).unwrap();
    assert_eq!(task.frontmatter.status, Status::Done);
    assert_eq!(task.title().as_deref(), Some("Ship it"));
}

#[test]
fn with_lock_runs_closure_under_existing_file() {
    let (_dir, store) = make_store();
    plant(
        &store,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n",
    );

    let value = store.with_lock(1, || Ok(42)).unwrap();
    assert_eq!(value, 42);
}

#[test]
fn with_lock_does_not_create_missing_task() {
    let (_dir, store) = make_store();

    assert!(store.with_lock(99, || Ok(())).is_err());
    assert!(
        !store.exists(99),
        "with_lock must not create a phantom task file"
    );
}

#[test]
fn create_read_update_delete_roundtrip() {
    let (_dir, store) = make_store();

    let id = create(&store, &Task::new("Wire the parser", Status::Todo, stamp())).unwrap();

    let created = store.read(id).unwrap();
    assert_eq!(created.frontmatter.status, Status::Todo);
    assert_eq!(created.title().as_deref(), Some("Wire the parser"));

    let raw = std::fs::read_to_string(store.task_path(id).unwrap()).unwrap();
    assert_eq!(
        raw,
        created.to_file_string().unwrap(),
        "the file holds the task and nothing else — the id lives only in the filename"
    );

    let updated = store
        .update(id, |task| {
            task.set_status(Status::InProgress);
            Ok(())
        })
        .unwrap();
    assert_eq!(updated.frontmatter.status, Status::InProgress);
    assert_eq!(
        store.read(id).unwrap().frontmatter.status,
        Status::InProgress
    );

    store.delete(id).unwrap();
    assert!(!store.exists(id));
    assert!(matches!(store.read(id), Err(StoreError::NotFound { .. })));
}

#[test]
fn create_gives_distinct_ids_for_same_title() {
    let (_dir, store) = make_store();
    let a = create(&store, &Task::new("Same title", Status::Todo, stamp())).unwrap();
    let b = create(&store, &Task::new("Same title", Status::Todo, stamp())).unwrap();
    assert_ne!(a, b);
    assert!(store.exists(a) && store.exists(b));
}

#[test]
fn create_names_the_file_after_the_allocated_number() {
    let (_dir, store) = make_store();
    let task = Task::new("Wire the parser", Status::Todo, stamp());

    let id = store.create(&task, 7).unwrap();
    assert_eq!(
        id, 7,
        "the id is the number; the title only shapes the file name"
    );

    // The allocator must not hand the same number out twice; if it does, the store refuses rather
    // than let one task's file be overwritten by another's.
    let reused = store.create(&task, 7);
    assert!(
        matches!(&reused, Err(StoreError::IdTaken { id }) if id == "OPP-7"),
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
        1,
        &format!("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n{body}"),
    );

    store
        .update(1, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();

    let raw = std::fs::read_to_string(store.task_path(1).unwrap()).unwrap();
    assert_eq!(
        raw,
        format!("---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n{body}")
    );
}

#[test]
fn missing_task_mutations_are_not_found() {
    let (_dir, store) = make_store();
    assert!(matches!(store.delete(99), Err(StoreError::NotFound { .. })));
    assert!(matches!(
        store.update(99, |_| Ok(())),
        Err(StoreError::NotFound { .. })
    ));
    assert!(matches!(
        store.write(99, &Task::new("x", Status::Todo, stamp())),
        Err(StoreError::NotFound { .. })
    ));
}

#[test]
fn create_and_update_validate_references() {
    let (_dir, store) = make_store();
    let parent = create(&store, &Task::new("Parent", Status::Todo, stamp())).unwrap();

    let mut child = Task::new("Child", Status::Todo, stamp());
    child.set_parent(Some("999".to_owned()));
    let missing = create(&store, &child);
    assert!(
        matches!(&missing, Err(StoreError::Invalid(message)) if message == "parent OPP-999 does not exist"),
        "the store names the task in the key spelling its callers speak: {missing:?}"
    );

    child.set_parent(Some("does-not-exist".to_owned()));
    assert!(
        matches!(create(&store, &child), Err(StoreError::InvalidRef { .. })),
        "a reference that names no task file is refused as such, not as a missing task"
    );

    child.set_parent(Some(parent.to_string()));
    let child = create(&store, &child).unwrap();

    let self_parent = store.update(child, |task| {
        task.set_parent(Some(child.to_string()));
        Ok(())
    });
    assert!(
        matches!(self_parent, Err(StoreError::Invalid(_))),
        "a task may not be its own parent"
    );
    assert_eq!(
        store.read(child).unwrap().frontmatter.parent,
        Some(parent.to_string()),
        "a rejected update must not mutate the file"
    );
}

#[test]
fn reparenting_under_a_descendant_is_refused() {
    let (_dir, store) = make_store();
    let a = create(&store, &Task::new("A", Status::Todo, stamp())).unwrap();

    let mut b = Task::new("B", Status::Todo, stamp());
    b.set_parent(Some(a.to_string()));
    let b = create(&store, &b).unwrap();

    let mut c = Task::new("C", Status::Todo, stamp());
    c.set_parent(Some(b.to_string()));
    let c = create(&store, &c).unwrap();

    // A is an ancestor of C; making C the parent of A would close a cycle.
    let cycle = store.update(a, |task| {
        task.set_parent(Some(c.to_string()));
        Ok(())
    });
    assert!(
        matches!(cycle, Err(StoreError::Invalid(_))),
        "a task may not be reparented under its own descendant"
    );
    assert_eq!(
        store.read(a).unwrap().frontmatter.parent,
        None,
        "a rejected reparent must not mutate the file"
    );
}

#[test]
fn dangling_reference_does_not_block_unrelated_edit() {
    let (_dir, store) = make_store();
    let a = create(&store, &Task::new("A", Status::Todo, stamp())).unwrap();

    let mut b = Task::new("B", Status::Todo, stamp());
    b.set_dependencies(vec![a.to_string()]);
    let b = create(&store, &b).unwrap();

    store.delete(a).unwrap();

    // B still depends on the now-deleted A; changing an unrelated field must still succeed.
    let updated = store
        .update(b, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();
    assert_eq!(updated.frontmatter.status, Status::Done);
    assert_eq!(updated.frontmatter.dependencies, vec![a.to_string()]);
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
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nestimate: 3.5\n---\n# T\n",
    );

    store
        .update(1, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();

    let raw = std::fs::read_to_string(store.task_path(1).unwrap()).unwrap();
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
            std::thread::spawn(move || {
                store
                    .update(id, |task| {
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

    let body = store.read(id).unwrap().body;
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
        .update(id, |task| {
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
        let result = store.update(id, |task| {
            task.set_rank(Some(bad.to_owned()));
            Ok(())
        });
        assert!(
            matches!(result, Err(StoreError::Invalid(_))),
            "rank {bad:?} must be refused"
        );
        assert_eq!(
            store.read(id).unwrap().frontmatter.rank,
            None,
            "a rejected rank must not mutate the file"
        );
    }

    store
        .update(id, |task| {
            task.set_rank(Some("a5".to_owned()));
            Ok(())
        })
        .expect("a base-36 key is accepted");
    assert_eq!(
        store.read(id).unwrap().frontmatter.rank.as_deref(),
        Some("a5")
    );
}

#[test]
fn an_already_persisted_malformed_rank_does_not_block_an_unrelated_edit() {
    // Task files are hand-editable, so a bad rank can predate the write being validated. Blocking
    // an unrelated status change on it would strand the task; `move` rebalances the group instead.
    let (_dir, store) = make_store();
    let id = create(&store, &Task::new("A", Status::Todo, stamp())).unwrap();
    let path = store.task_path(id).unwrap();
    let raw = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        raw.replace("status: todo", "status: todo\nrank: NOPE"),
    )
    .unwrap();

    store
        .update(id, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .expect("an unrelated edit still lands");
    assert_eq!(store.read(id).unwrap().frontmatter.status, Status::Done);
}

#[test]
fn a_status_change_preserves_created() {
    let (_dir, store) = make_store();
    let id = create(&store, &Task::new("A", Status::Todo, stamp())).unwrap();

    store
        .update(id, |task| {
            task.set_status(Status::InReview);
            Ok(())
        })
        .unwrap();

    let task = store.read(id).unwrap();
    assert_eq!(task.frontmatter.status, Status::InReview);
    assert_eq!(task.frontmatter.created, stamp());
}

#[test]
fn a_file_without_created_refuses_to_be_written_and_says_how_to_fix_it() {
    let (_dir, store) = make_store();
    let path = plant(&store, 1, "---\nstatus: in_progress\n---\n# Legacy\n");

    let err = store
        .update(1, |task| {
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

#[test]
fn references_are_written_as_the_target_file() {
    let (_dir, store) = make_store();
    let target = create(
        &store,
        &Task::new("Write the parser", Status::Todo, stamp()),
    )
    .unwrap();
    let target_name = format!("{:0>5}-write-the-parser.md", target);

    let mut child = Task::new("Child", Status::Todo, stamp());
    child.set_parent(Some(target.to_string()));
    child.set_dependencies(vec![target.to_string(), format!("{target}#Design")]);
    let child = create(&store, &child).unwrap();

    let raw = store.read_raw(child).unwrap();
    assert!(raw.contains(&format!("parent: ./{target_name}")), "{raw}");
    assert!(raw.contains(&format!("- ./{target_name}")), "{raw}");
    assert!(raw.contains(&format!("- ./{target_name}#Design")), "{raw}");

    // The reference reads back as the id, so nothing above the store sees a file name.
    let read = store.read(child).unwrap();
    assert_eq!(read.frontmatter.parent, Some(target.to_string()));
    assert_eq!(
        read.frontmatter.dependencies,
        vec![target.to_string(), format!("{target}#Design")]
    );
}

#[test]
fn a_reference_to_a_deleted_task_keeps_its_number() {
    let (_dir, store) = make_store();
    let gone = create(&store, &Task::new("Gone", Status::Todo, stamp())).unwrap();
    let mut task = Task::new("Holder", Status::Todo, stamp());
    task.set_dependencies(vec![gone.to_string()]);
    let holder = create(&store, &task).unwrap();

    store.delete(gone).unwrap();
    store
        .update(holder, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();

    // No file carries the number any more, so there is no name to point at — the number is what
    // survives, and it still reads back as the reference it was.
    let raw = store.read_raw(holder).unwrap();
    assert!(raw.contains(&format!("- '{gone}'")), "{raw}");
    assert_eq!(
        store.read(holder).unwrap().frontmatter.dependencies,
        vec![gone.to_string()]
    );
}

#[test]
fn a_retitled_target_is_pointed_at_by_its_new_name() {
    let (_dir, store) = make_store();
    let target = create(&store, &Task::new("First title", Status::Todo, stamp())).unwrap();
    let mut child = Task::new("Child", Status::Todo, stamp());
    child.set_parent(Some(target.to_string()));
    let child = create(&store, &child).unwrap();

    // Renaming a task's file is how a retitle lands; the digits are what identify it.
    let old = store.task_path(target).unwrap();
    let new = old.with_file_name(format!("{:0>5}-a-better-title.md", target));
    std::fs::rename(&old, &new).unwrap();

    store
        .update(child, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();
    let raw = store.read_raw(child).unwrap();
    assert!(
        raw.contains(&format!("parent: ./{:0>5}-a-better-title.md", target)),
        "{raw}"
    );
}

#[test]
fn a_body_reference_is_written_as_the_target_file() {
    let (_dir, store) = make_store();
    let target = create(
        &store,
        &Task::new("Write the parser", Status::Todo, stamp()),
    )
    .unwrap();
    let target_name = format!("{target:0>5}-write-the-parser.md");

    let mut holder = Task::new("Holder", Status::Todo, stamp());
    holder.append_body(&format!(
        "see [[{target}]] and [[{target}#Design]], but not [[Some Page Title]] or `[[{target}]]`"
    ));
    let holder = create(&store, &holder).unwrap();

    let raw = store.read_raw(holder).unwrap();
    assert!(raw.contains(&format!("[[./{target_name}]]")), "{raw}");
    assert!(
        raw.contains(&format!("[[./{target_name}#Design]]")),
        "{raw}"
    );
    assert!(
        raw.contains("[[Some Page Title]]"),
        "bracketed prose is not a reference: {raw}"
    );
    assert!(
        raw.contains(&format!("`[[{target}]]`")),
        "a quoted reference is prose about the spelling, not a reference: {raw}"
    );
}

#[test]
fn a_body_reference_that_names_no_file_is_written_as_the_key() {
    let (_dir, store) = make_store();
    let mut holder = Task::new("Holder", Status::Todo, stamp());
    holder.append_body("blocked on [[999]] and [[999#Design]]");

    let holder = create(&store, &holder).unwrap();

    // A markdown renderer knows no bare number, so writing one back would leave a reference nothing
    // can resolve even once task 999 arrives.
    let raw = store.read_raw(holder).unwrap();
    assert!(
        raw.contains("blocked on [[OPP-999]] and [[OPP-999#Design]]"),
        "{raw}"
    );
}

#[test]
fn a_write_leaves_a_quoted_reference_alone() {
    let (_dir, store) = make_store();
    let target = create(&store, &Task::new("Target", Status::Todo, stamp())).unwrap();
    let body = format!(
        "# Holder\n\nthe old spelling was `[[{target}]]`, and a fence:\n\n```\nparent: [[{target}]]\n```\n"
    );
    let mut holder = Task::new("Holder", Status::Todo, stamp());
    holder.body = body.clone();
    let holder = create(&store, &holder).unwrap();

    store
        .update(holder, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .unwrap();

    let raw = store.read_raw(holder).unwrap();
    assert!(
        raw.ends_with(&body),
        "an unrelated edit must not rewrite prose about the spelling: {raw}"
    );
}

fn tag_temp_files(store: &Store) -> Vec<String> {
    std::fs::read_dir(store.tags_dir())
        .unwrap()
        .filter_map(|e| e.unwrap().file_name().into_string().ok())
        .filter(|name| !name.ends_with(".md"))
        .collect()
}

fn register(store: &Store, display_name: &str) -> String {
    let tag = Tag::new(display_name, None).unwrap();
    store.create_tag(&tag).unwrap();
    tag.name
}

fn tagged(store: &Store, title: &str, tags: &[&str]) -> Result<u64, StoreError> {
    let mut task = Task::new(title, Status::Todo, stamp());
    task.set_tags(tags.iter().map(|name| (*name).to_owned()).collect());
    create(store, &task)
}

fn tags_of(store: &Store, id: u64) -> Vec<String> {
    store.read(id).unwrap().frontmatter.tags
}

#[test]
fn tag_create_read_update_delete_roundtrip() {
    let (_dir, store) = make_store();

    store
        .create_tag(&Tag::new("Backend", Some(Color::Teal)).unwrap())
        .unwrap();

    let created = store.read_tag("backend").unwrap();
    assert_eq!(created.name, "backend");
    assert_eq!(created.color(), Color::Teal);
    assert_eq!(created.display_name().as_deref(), Some("Backend"));
    assert!(store.tag_exists("backend"));

    let raw = std::fs::read_to_string(store.tags_dir().join("backend.md")).unwrap();
    assert_eq!(
        raw,
        created.to_file_string().unwrap(),
        "the file holds the tag and nothing else — the name lives only in the filename"
    );

    let updated = store
        .update_tag("backend", |tag| {
            tag.set_color(Color::Pink);
            tag.append_body("Server-side work.");
            Ok(())
        })
        .unwrap();
    assert_eq!(updated.color(), Color::Pink);
    assert_eq!(store.read_tag("backend").unwrap(), updated);
    assert!(
        store
            .read_tag("backend")
            .unwrap()
            .body
            .contains("Server-side work.")
    );

    let mut recolored = store.read_tag("backend").unwrap();
    recolored.set_color(Color::Amber);
    store.write_tag(&recolored).unwrap();
    assert_eq!(store.read_tag("backend").unwrap().color(), Color::Amber);

    assert_eq!(
        store.list_tags().unwrap(),
        vec![store.read_tag("backend").unwrap()]
    );

    store.delete_tag("backend", false).unwrap();
    assert!(!store.tag_exists("backend"));
    assert!(matches!(
        store.read_tag("backend"),
        Err(StoreError::TagNotFound { .. })
    ));
    assert!(
        tag_temp_files(&store).is_empty(),
        "no temp file should survive a tag roundtrip: {:?}",
        tag_temp_files(&store)
    );
}

#[test]
fn creating_a_tag_twice_is_refused() {
    let (_dir, store) = make_store();
    let tag = Tag::new("Backend", None).unwrap();
    store.create_tag(&tag).unwrap();

    let again = store.create_tag(&Tag::new("backend", Some(Color::Red)).unwrap());
    assert!(
        matches!(&again, Err(StoreError::TagExists { name }) if name == "backend"),
        "a taken name must be reported, not clobbered: {again:?}"
    );
    assert_eq!(
        store.read_tag("backend").unwrap(),
        tag,
        "the refused create must leave the first tag untouched"
    );
    assert!(
        tag_temp_files(&store).is_empty(),
        "a refused create leaves no temp file: {:?}",
        tag_temp_files(&store)
    );
}

#[test]
fn tag_names_are_normalized_at_the_store_boundary() {
    let (_dir, store) = make_store();
    store
        .create_tag(&Tag::new("Front End", None).unwrap())
        .unwrap();

    assert!(store.tags_dir().join("front-end.md").is_file());
    assert!(store.tag_exists("Front End"));
    assert_eq!(store.read_tag("FRONT_END").unwrap().name, "front-end");

    let refused = store.read_tag("C++");
    assert!(
        matches!(&refused, Err(StoreError::Invalid(message)) if message.contains(op_task::tag::NAME_RULE)),
        "a name the normalizer refuses must say the rule: {refused:?}"
    );
}

#[test]
fn tag_enumeration_skips_files_no_normalized_name_names() {
    let (_dir, store) = make_store();
    register(&store, "backend");
    std::fs::write(store.tags_dir().join("Backend Team.md"), "---\n---\n# x\n").unwrap();
    std::fs::write(store.tags_dir().join("notes.txt"), "ignored").unwrap();

    assert_eq!(
        store
            .list_tags()
            .unwrap()
            .into_iter()
            .map(|tag| tag.name)
            .collect::<Vec<_>>(),
        vec!["backend".to_owned()]
    );
}

#[test]
fn assignment_requires_every_name_in_the_registry() {
    let (_dir, store) = make_store();
    register(&store, "backend");

    let refused = tagged(&store, "Wire the parser", &["backend", "wip"]);
    assert!(
        matches!(&refused, Err(StoreError::Invalid(message))
            if message.contains("tag wip does not exist") && message.contains("openplan tag create")),
        "an unknown tag must be refused with a hint: {refused:?}"
    );

    register(&store, "wip");
    let id = tagged(&store, "Wire the parser", &["wip", "backend", "wip"]).unwrap();
    assert_eq!(tags_of(&store, id), vec!["backend", "wip"]);
}

#[test]
fn a_dangling_tag_blocks_an_unrelated_edit_until_it_is_dropped() {
    let (_dir, store) = make_store();
    plant(
        &store,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- ghost\n---\n# Planted\n",
    );

    let refused = store.update(1, |task| {
        task.set_status(Status::Done);
        Ok(())
    });
    assert!(
        matches!(&refused, Err(StoreError::Invalid(message)) if message.contains("tag ghost does not exist")),
        "validation covers the whole set, not only what the write adds: {refused:?}"
    );

    store
        .update(1, |task| {
            task.set_status(Status::Done);
            task.set_tags(Vec::new());
            Ok(())
        })
        .unwrap();
    assert!(tags_of(&store, 1).is_empty());
    assert_eq!(store.read(1).unwrap().frontmatter.status, Status::Done);
}

#[test]
fn renaming_a_tag_rewrites_the_tasks_that_reference_it() {
    let (_dir, store) = make_store();
    register(&store, "backend");
    register(&store, "wip");
    let both = tagged(&store, "Wire the parser", &["backend", "wip"]).unwrap();
    let neither = tagged(&store, "Ship it", &["wip"]).unwrap();

    let rewritten = store.rename_tag("backend", "Infra Team").unwrap();

    assert_eq!(rewritten, vec![both]);
    assert!(!store.tag_exists("backend"));
    let renamed = store.read_tag("infra-team").unwrap();
    assert_eq!(renamed.display_name().as_deref(), Some("Infra Team"));
    assert_eq!(tags_of(&store, both), vec!["infra-team", "wip"]);
    assert_eq!(tags_of(&store, neither), vec!["wip"]);
}

#[test]
fn renaming_a_tag_keeps_its_color_and_description() {
    let (_dir, store) = make_store();
    let mut tag = Tag::new("Backend", Some(Color::Teal)).unwrap();
    tag.append_body("Server-side work.");
    store.create_tag(&tag).unwrap();

    store.rename_tag("backend", "infra").unwrap();

    let renamed = store.read_tag("infra").unwrap();
    assert_eq!(renamed.color(), Color::Teal);
    assert_eq!(renamed.body, "# infra\n\nServer-side work.\n");
}

#[test]
fn renaming_onto_an_existing_tag_is_refused() {
    let (_dir, store) = make_store();
    register(&store, "backend");
    register(&store, "infra");
    let id = tagged(&store, "Wire the parser", &["backend"]).unwrap();

    let refused = store.rename_tag("backend", "infra");
    assert!(
        matches!(&refused, Err(StoreError::TagExists { name }) if name == "infra"),
        "merging two tags is not a rename: {refused:?}"
    );
    assert!(
        store.tag_exists("backend"),
        "the refused rename keeps the source"
    );
    assert_eq!(tags_of(&store, id), vec!["backend"]);
    assert!(
        tag_temp_files(&store).is_empty(),
        "a refused rename leaves no temp file: {:?}",
        tag_temp_files(&store)
    );
}

#[test]
fn a_rename_rewrites_a_task_that_carries_another_dangling_tag() {
    let (_dir, store) = make_store();
    register(&store, "backend");
    plant(
        &store,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- backend\n- ghost\n---\n# Planted\n",
    );

    store.rename_tag("backend", "infra").unwrap();

    assert_eq!(
        tags_of(&store, 1),
        vec!["ghost", "infra"],
        "a rename is a substitution, so another dangling name must not block it"
    );
}

#[test]
fn a_rename_leaves_the_body_and_the_unknown_frontmatter_of_a_task_alone() {
    let (_dir, store) = make_store();
    register(&store, "backend");
    let body = "# Ship it\n\nSee [[./00009-gone.md]].\n\n## Plan\n- a\n- b\n";
    plant(
        &store,
        1,
        &format!(
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nowner: milan\ntags:\n- backend\n---\n{body}"
        ),
    );

    store.rename_tag("backend", "infra").unwrap();

    let raw = std::fs::read_to_string(store.task_path(1).unwrap()).unwrap();
    assert_eq!(
        raw,
        format!(
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- infra\nowner: milan\n---\n{body}"
        )
    );
}

#[test]
fn deleting_a_referenced_tag_needs_force() {
    let (_dir, store) = make_store();
    register(&store, "backend");
    let id = tagged(&store, "Wire the parser", &["backend"]).unwrap();

    let refused = store.delete_tag("backend", false);
    assert!(
        matches!(&refused, Err(StoreError::TagReferenced { name, count }) if name == "backend" && *count == 1),
        "a referenced tag must name how many tasks hold it: {refused:?}"
    );
    assert!(store.tag_exists("backend"));

    store.delete_tag("backend", true).unwrap();
    assert!(!store.tag_exists("backend"));
    assert_eq!(
        tags_of(&store, id),
        vec!["backend"],
        "a forced delete leaves the reference dangling rather than editing tasks"
    );
}

#[test]
fn deleting_a_missing_tag_is_not_found() {
    let (_dir, store) = make_store();
    assert!(matches!(
        store.delete_tag("backend", true),
        Err(StoreError::TagNotFound { .. })
    ));
    assert!(matches!(
        store.write_tag(&Tag::new("backend", None).unwrap()),
        Err(StoreError::TagNotFound { .. })
    ));
    assert!(matches!(
        store.rename_tag("backend", "infra"),
        Err(StoreError::TagNotFound { .. })
    ));
}

#[test]
fn concurrent_updates_to_one_tag_serialize() {
    let (_dir, store) = make_store();
    register(&store, "backend");

    let threads = 8;
    let handles: Vec<_> = (0..threads)
        .map(|i| {
            let store = store.clone();
            std::thread::spawn(move || {
                store
                    .update_tag("backend", |tag| {
                        tag.append_body(&format!("line {i}"));
                        Ok(())
                    })
                    .unwrap();
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    let body = store.read_tag("backend").unwrap().body;
    for i in 0..threads {
        assert!(
            body.contains(&format!("line {i}\n")),
            "lost update from thread {i}: no interleaving means every append survives\n{body}"
        );
    }
    assert!(
        tag_temp_files(&store).is_empty(),
        "serialized atomic writes leave no partial temp files: {:?}",
        tag_temp_files(&store)
    );
}

#[test]
fn a_tag_file_that_does_not_parse_names_itself() {
    let (_dir, store) = make_store();
    std::fs::create_dir_all(store.tags_dir()).unwrap();
    std::fs::write(
        store.tags_dir().join("backend.md"),
        "---\ncolor: fuchsia\n---\n# Backend\n",
    )
    .unwrap();

    let refused = store.list_tags();
    assert!(
        matches!(&refused, Err(StoreError::TagFile { path, .. }) if path.ends_with("backend.md")),
        "one bad file among many must say which one: {refused:?}"
    );
}
