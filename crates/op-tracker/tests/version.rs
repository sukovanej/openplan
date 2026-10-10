use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use op_backend::{Actor, Backend, BackendError, Op, Signer, Snapshot};
use op_backend_git::{GitBackend, Options};
use op_backend_local::LocalBackend;
use op_task::{Status, Task, Timestamp};
use op_tracker::{
    HistoryQuery, Retired, STORE_VERSIONS, StoreVersion, StoreVersions, TaskMergePolicy, Tracker,
    TrackerError, VersionError,
};
use semver::Version;

// Store version 0.0.2 of these tests spells the status `todo` as `backlog`.
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

const FIRST: StoreVersion = StoreVersion {
    version: Version::new(0, 0, 1),
    released: Some("0.0.1"),
    migrate: None,
};

static RELEASED: StoreVersions = StoreVersions {
    known: &[
        FIRST,
        StoreVersion {
            version: Version::new(0, 0, 2),
            released: Some("0.0.9"),
            migrate: Some(backlog_for_todo),
        },
    ],
    retired: &[],
};

static UNRELEASED: StoreVersions = StoreVersions {
    known: &[
        FIRST,
        StoreVersion {
            version: Version::new(0, 0, 2),
            released: None,
            migrate: Some(backlog_for_todo),
        },
    ],
    retired: &[],
};

static RETIRING: StoreVersions = StoreVersions {
    known: &[StoreVersion {
        version: Version::new(0, 0, 2),
        released: Some("0.0.9"),
        migrate: None,
    }],
    retired: &[Retired {
        version: Version::new(0, 0, 1),
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
    // A store of version 0.0.1 with one task in `todo`.
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

    fn tracker(&self, versions: &'static StoreVersions) -> Tracker {
        Tracker::new(Arc::clone(&self.backend)).with_store_versions(versions)
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
fn a_store_starts_in_the_newest_store_version() {
    let store = Store::first();
    assert_eq!(
        store.config(),
        "version = \"0.0.1\"
abbreviation = \"OPP\"\n"
    );
}

#[test]
fn an_older_store_reads_as_migrated_and_refuses_a_write_until_it_migrates() {
    let store = Store::first();
    let tracker = store.tracker(&RELEASED);

    let plan = tracker.plan().expect("plan");
    assert_eq!(plan.migrated_from(), Some(&Version::new(0, 0, 1)));
    assert_eq!(status(&tracker), Status::Backlog);
    let refused = tracker.create_task(&actor(), &Task::new("Next", Status::Todo, stamp()));
    assert!(
        matches!(
            refused,
            Err(TrackerError::Version(VersionError::Unmigrated { .. }))
        ),
        "{refused:?}"
    );

    assert_eq!(
        tracker.migrate(&actor()).expect("migrate"),
        Some(Version::new(0, 0, 1))
    );

    assert_eq!(
        store.config(),
        "version = \"0.0.2\"
abbreviation = \"OPP\"\n"
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
        "Migrate the tasks from store version 0.0.1 to 0.0.2"
    );
    let described = tracker.describe(&entries[0]).expect("describe");
    assert_eq!(
        described.lines(None, None),
        vec!["Migrate the tasks from store version 0.0.1 to 0.0.2"]
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
fn a_store_of_a_newer_store_version_names_the_release_it_needs() {
    let store = Store::first();
    store.tracker(&RELEASED).migrate(&actor()).expect("migrate");
    let tracker = store.tracker(&STORE_VERSIONS);

    let problem = tracker.plan().expect("plan").config().map(drop);
    let Err(TrackerError::Version(problem)) = problem else {
        panic!("{problem:?}");
    };
    assert_eq!(
        problem.to_string(),
        "these tasks use store version 0.0.2, and this openplan reads store versions up to 0.0.1; they need a \
         newer openplan"
    );
    assert!(matches!(
        tracker.update_task(&actor(), 1, |_| Ok(())),
        Err(TrackerError::Version(VersionError::Newer { .. }))
    ));
}

#[test]
fn a_retired_store_version_names_the_last_release_that_migrates_it() {
    let store = Store::first();
    let problem = store
        .tracker(&RETIRING)
        .plan()
        .expect("plan")
        .config()
        .map(drop);
    let Err(TrackerError::Version(problem)) = problem else {
        panic!("{problem:?}");
    };
    assert_eq!(
        problem.to_string(),
        "these tasks use store version 0.0.1, and this openplan migrates only store version 0.0.2 and newer; \
         run openplan 0.0.9 on this project once to migrate them, then update"
    );
}

#[test]
fn only_a_released_store_version_migrates_by_itself() {
    assert!(RELEASED.migrates_by_itself());
    assert!(!UNRELEASED.migrates_by_itself());
}

// Two daemons that migrate one store before they sync must write the same bytes, whichever build
// each of them runs.
#[test]
fn a_migration_depends_on_the_store_alone() {
    let (released, unreleased) = (Store::first(), Store::first());
    released
        .tracker(&RELEASED)
        .migrate(&actor())
        .expect("migrate");
    unreleased
        .tracker(&UNRELEASED)
        .migrate(&actor())
        .expect("an explicit migration");
    let task = |store: &Store| {
        let plan = store.tracker(&RELEASED).plan().expect("plan");
        plan.raw(1).expect("raw")
    };
    assert_eq!(released.config(), unreleased.config());
    assert_eq!(task(&released), task(&unreleased));
}

#[test]
fn every_store_version_after_the_oldest_migrates_from_the_one_before() {
    for versions in [&STORE_VERSIONS, &RELEASED, &UNRELEASED, &RETIRING] {
        for (at, known) in versions.known.iter().enumerate() {
            assert_eq!(
                known.migrate.is_some(),
                at > 0,
                "store version {}",
                known.version
            );
        }
        for pair in versions.known.windows(2) {
            assert!(pair[0].version < pair[1].version);
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

fn open(
    root: &Path,
    name: &str,
    versions: &'static StoreVersions,
    policy: TaskMergePolicy,
) -> Member {
    let backend = Arc::new(
        GitBackend::open(
            root.join(name),
            Options::new(
                Arc::new(policy.with_store_versions(versions)),
                Signer::fixed(Actor::new("openplan")),
            ),
        )
        .expect("open"),
    );
    Member {
        tracker: Tracker::new(backend.clone()).with_store_versions(versions),
        backend,
    }
}

fn join(
    root: &Path,
    name: &str,
    versions: &'static StoreVersions,
    policy: TaskMergePolicy,
) -> Member {
    let remote = root.join("remote.git");
    git(
        root,
        &["clone", "--quiet", remote.to_str().expect("utf-8"), name],
    );
    open(root, name, versions, policy)
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

// Alice starts the tasks in store version 0.0.1, and Bob migrates them to 0.0.2 and pushes.
// Alice then writes a task in 0.0.1 before her next sync.
fn diverged(root: &Path, alice_policy: TaskMergePolicy) -> Member {
    git(
        root,
        &["init", "--quiet", "--bare", "-b", "main", "remote.git"],
    );
    let alice = join(root, "alice", &STORE_VERSIONS, alice_policy);
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
    alice.create("Written in store version 0.0.1");
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
fn a_sync_stops_on_a_store_version_this_binary_cannot_read_and_says_so() {
    let root = tempfile::tempdir().expect("tempdir");
    let heard: Arc<Mutex<Option<VersionError>>> = Arc::default();
    let reported = Arc::clone(&heard);
    let alice = diverged(
        root.path(),
        TaskMergePolicy::default().on_unreadable(move |problem| {
            *reported.lock().expect("lock") = Some(problem.clone());
        }),
    );

    let stopped = alice.sync();

    assert!(
        matches!(&stopped, Err(BackendError::Sync(reason)) if reason.contains("store version 0.0.2")),
        "{stopped:?}"
    );
    assert!(matches!(
        heard.lock().expect("lock").as_ref(),
        Some(VersionError::Newer { .. })
    ));
    let plan = alice.tracker.plan().expect("plan");
    assert_eq!(plan.numbers().count(), 2, "the local tasks stay");
}
