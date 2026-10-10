use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use op_backend::{Actor, Backend, BackendError, Op, Signer, Snapshot};
use op_backend_git::{GitBackend, Options};
use op_backend_local::LocalBackend;
use op_task::{Status, Task, Timestamp};
use op_tracker::{
    FORMATS, Format, FormatError, Formats, HistoryQuery, Retired, TaskMergePolicy, Tracker,
    TrackerError,
};

// Format 2 of these tests spells the status `todo` as `backlog`.
fn backlog_for_todo(snapshot: &dyn Snapshot) -> Result<Vec<Op>, BackendError> {
    let mut ops = Vec::new();
    for path in snapshot.list("tasks")? {
        let text = snapshot.read_text(&path)?.unwrap_or_default();
        let migrated = text.replace("status: todo", "status: backlog");
        if migrated != text {
            ops.push(Op::put(path, migrated));
        }
    }
    Ok(ops)
}

const FIRST: Format = Format {
    number: 1,
    released: Some("0.0.1"),
    migrate: None,
};

static RELEASED: Formats = Formats {
    known: &[
        FIRST,
        Format {
            number: 2,
            released: Some("0.0.9"),
            migrate: Some(backlog_for_todo),
        },
    ],
    retired: &[],
};

static UNRELEASED: Formats = Formats {
    known: &[
        FIRST,
        Format {
            number: 2,
            released: None,
            migrate: Some(backlog_for_todo),
        },
    ],
    retired: &[],
};

static RETIRING: Formats = Formats {
    known: &[Format {
        number: 2,
        released: Some("0.0.9"),
        migrate: None,
    }],
    retired: &[Retired {
        number: 1,
        last_release: "0.0.9",
    }],
};

fn actor() -> Actor {
    Actor::new("Ada")
}

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().expect("time")
}

struct Store {
    _dir: tempfile::TempDir,
    backend: Arc<dyn Backend>,
}

impl Store {
    // A format 1 store with one task in `todo`.
    fn first() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let backend: Arc<dyn Backend> = Arc::new(
            LocalBackend::open(
                dir.path(),
                op_backend_local::Options::new(Signer::fixed(Actor::new("filesystem"))),
            )
            .expect("open"),
        );
        let tracker = Tracker::new(Arc::clone(&backend));
        tracker
            .init(&actor(), "OPP".parse().expect("abbreviation"))
            .expect("init");
        tracker
            .create_task(&actor(), &Task::new("Ship it", Status::Todo, stamp()))
            .expect("create");
        Self { _dir: dir, backend }
    }

    fn tracker(&self, formats: &'static Formats) -> Tracker {
        Tracker::new(Arc::clone(&self.backend)).with_formats(formats)
    }

    fn config(&self) -> String {
        self.backend
            .head()
            .expect("head")
            .read_text("config.toml")
            .expect("read")
            .expect("a config")
    }
}

fn status(tracker: &Tracker) -> Status {
    tracker
        .plan()
        .expect("plan")
        .task(1)
        .expect("task")
        .frontmatter
        .status
}

#[test]
fn a_store_starts_in_the_newest_format() {
    let store = Store::first();
    assert_eq!(
        store.config(),
        "format = 1\nrequires = \"0.0.1\"\nabbreviation = \"OPP\"\n"
    );
}

#[test]
fn an_older_store_reads_as_migrated_and_refuses_a_write_until_it_migrates() {
    let store = Store::first();
    let tracker = store.tracker(&RELEASED);

    let plan = tracker.plan().expect("plan");
    assert_eq!(plan.migrated_from(), Some(1));
    assert_eq!(status(&tracker), Status::Backlog);
    let refused = tracker.create_task(&actor(), &Task::new("Next", Status::Todo, stamp()));
    assert!(
        matches!(
            refused,
            Err(TrackerError::Format(FormatError::Unmigrated {
                format: 1,
                current: 2,
                ..
            }))
        ),
        "{refused:?}"
    );

    assert_eq!(tracker.migrate(&actor()).expect("migrate"), Some(1));

    assert_eq!(
        store.config(),
        "format = 2\nrequires = \"0.0.9\"\nabbreviation = \"OPP\"\n"
    );
    assert_eq!(tracker.plan().expect("plan").migrated_from(), None);
    assert_eq!(status(&tracker), Status::Backlog);
    assert_eq!(tracker.migrate(&actor()).expect("migrate again"), None);
    tracker
        .create_task(&actor(), &Task::new("Next", Status::Todo, stamp()))
        .expect("a write after the migration");
}

#[test]
fn the_history_names_the_migration_and_reads_older_revisions_as_migrated() {
    let store = Store::first();
    let tracker = store.tracker(&RELEASED);
    tracker.migrate(&actor()).expect("migrate");

    let entries = tracker.history(&HistoryQuery::default()).expect("history");
    assert_eq!(
        entries[0].revision.message,
        "Migrate the tasks from format 1 to format 2"
    );
    let described = tracker.describe(&entries[0]).expect("describe");
    assert_eq!(
        described.lines(None, None),
        vec!["Migrate the tasks from format 1 to format 2"]
    );
    assert_eq!(
        tracker
            .describe(&entries[1])
            .expect("describe")
            .lines(Some("OPP".parse().expect("abbreviation")), None),
        vec!["OPP-1: create \"Ship it\""]
    );
    let before = tracker
        .task_at(1, &entries[1].revision.id)
        .expect("task at")
        .expect("the task");
    assert!(before.contains("status: backlog"), "{before}");
}

#[test]
fn a_store_of_a_newer_format_names_the_release_it_needs() {
    let store = Store::first();
    store.tracker(&RELEASED).migrate(&actor()).expect("migrate");
    let tracker = store.tracker(&FORMATS);

    let problem = tracker.plan().expect("plan").config().map(drop);
    let Err(TrackerError::Format(problem)) = problem else {
        panic!("{problem:?}");
    };
    assert_eq!(
        problem.to_string(),
        "these tasks use store format 2, and this openplan reads formats up to 1; they need \
         openplan 0.0.9 or newer"
    );
    assert!(matches!(
        tracker.update_task(&actor(), 1, |_| Ok(())),
        Err(TrackerError::Format(FormatError::Newer { .. }))
    ));
}

#[test]
fn a_retired_format_names_the_last_release_that_migrates_it() {
    let store = Store::first();
    let problem = store
        .tracker(&RETIRING)
        .plan()
        .expect("plan")
        .config()
        .map(drop);
    let Err(TrackerError::Format(problem)) = problem else {
        panic!("{problem:?}");
    };
    assert_eq!(
        problem.to_string(),
        "these tasks use store format 1, and this openplan migrates only format 2 and newer; \
         run openplan 0.0.9 on this project once to migrate them, then update"
    );
}

#[test]
fn only_a_released_format_migrates_by_itself() {
    assert!(RELEASED.migrates_by_itself());
    assert!(!UNRELEASED.migrates_by_itself());
    let store = Store::first();
    store
        .tracker(&UNRELEASED)
        .migrate(&actor())
        .expect("an explicit migration");
    assert!(
        store
            .config()
            .contains(&format!("requires = \"{}\"", env!("CARGO_PKG_VERSION")))
    );
}

#[test]
fn every_format_after_the_oldest_migrates_from_the_one_before() {
    for formats in [&FORMATS, &RELEASED, &UNRELEASED, &RETIRING] {
        for (at, format) in formats.known.iter().enumerate() {
            assert_eq!(format.migrate.is_some(), at > 0, "format {}", format.number);
        }
        for pair in formats.known.windows(2) {
            assert_eq!(pair[1].number, pair[0].number + 1);
        }
    }
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Member {
    tracker: Tracker,
    backend: Arc<GitBackend>,
}

fn open(root: &Path, name: &str, formats: &'static Formats, policy: TaskMergePolicy) -> Member {
    let backend = Arc::new(
        GitBackend::open(
            root.join(name),
            Options::new(
                Arc::new(policy.with_formats(formats)),
                Signer::fixed(Actor::new("openplan")),
            ),
        )
        .expect("open"),
    );
    Member {
        tracker: Tracker::new(backend.clone()).with_formats(formats),
        backend,
    }
}

fn join(root: &Path, name: &str, formats: &'static Formats, policy: TaskMergePolicy) -> Member {
    let remote = root.join("remote.git");
    git(
        root,
        &["clone", "--quiet", remote.to_str().expect("utf-8"), name],
    );
    open(root, name, formats, policy)
}

impl Member {
    fn sync(&self) -> Result<op_backend::SyncReport, BackendError> {
        self.backend.remote().expect("a remote").sync()
    }

    fn create(&self, title: &str) {
        self.tracker
            .create_task(&actor(), &Task::new(title, Status::Todo, stamp()))
            .expect("create");
    }
}

// Alice starts the tasks in format 1, and Bob migrates them to format 2 and pushes. Alice then
// writes a task in format 1 before her next sync.
fn diverged(root: &Path, alice_policy: TaskMergePolicy) -> Member {
    git(
        root,
        &["init", "--quiet", "--bare", "-b", "main", "remote.git"],
    );
    let alice = join(root, "alice", &FORMATS, alice_policy);
    alice
        .tracker
        .init(&actor(), "OPP".parse().expect("abbreviation"))
        .expect("init");
    alice.create("Shared");
    alice.sync().expect("sync");
    let bob = join(root, "bob", &RELEASED, TaskMergePolicy::default());
    bob.sync().expect("sync");
    bob.tracker.migrate(&actor()).expect("migrate");
    bob.sync().expect("sync");
    alice.create("Written in format 1");
    alice
}

#[test]
fn a_sync_migrates_the_older_side_before_it_merges() {
    let root = tempfile::tempdir().expect("tempdir");
    drop(diverged(root.path(), TaskMergePolicy::default()));
    let alice = open(root.path(), "alice", &RELEASED, TaskMergePolicy::default());

    alice.sync().expect("sync");

    let plan = alice.tracker.plan().expect("plan");
    assert_eq!(plan.migrated_from(), None);
    let statuses: Vec<Status> = plan
        .numbers()
        .map(|number| plan.task(number).expect("task").frontmatter.status)
        .collect();
    assert_eq!(statuses, vec![Status::Backlog, Status::Backlog]);
}

#[test]
fn a_sync_stops_on_a_format_this_binary_cannot_read_and_says_so() {
    let root = tempfile::tempdir().expect("tempdir");
    let heard: Arc<Mutex<Option<FormatError>>> = Arc::default();
    let reported = Arc::clone(&heard);
    let alice = diverged(
        root.path(),
        TaskMergePolicy::default().on_unreadable(move |problem| {
            *reported.lock().expect("lock") = Some(problem.clone());
        }),
    );

    let stopped = alice.sync();

    assert!(
        matches!(&stopped, Err(BackendError::Sync(reason)) if reason.contains("store format 2")),
        "{stopped:?}"
    );
    assert!(matches!(
        heard.lock().expect("lock").as_ref(),
        Some(FormatError::Newer { format: 2, .. })
    ));
    let plan = alice.tracker.plan().expect("plan");
    assert_eq!(plan.numbers().count(), 2, "the local tasks stay");
}
