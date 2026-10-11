use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use op_backend::{Actor, Backend, BackendError, MemorySnapshot, MergePolicy, Op, Signer, Snapshot};
use op_backend_git::{GitBackend, Options};
use op_backend_local::LocalBackend;
use op_task::{Status, Task, Timestamp};
use op_tracker::{
    HistoryQuery, Retired, STORE_VERSIONS, StoreVersion, StoreVersions, TaskMergePolicy, Tracker,
    TrackerError, VersionError,
};

// Store version 3 of these tests spells the status `todo` as `backlog`.
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

fn nothing(_: &dyn Snapshot) -> Result<Vec<Op>, BackendError> {
    Ok(Vec::new())
}

// The store version that the built-in table writes.
const TWO: StoreVersion = StoreVersion {
    number: 2,
    released: Some("0.0.1"),
    migrate: None,
};

const THREE: StoreVersion = StoreVersion {
    number: 3,
    released: Some("0.0.9"),
    migrate: Some(backlog_for_todo),
};

static RELEASED: StoreVersions = StoreVersions {
    known: &[TWO, THREE],
    retired: &[],
};

static UNRELEASED: StoreVersions = StoreVersions {
    known: &[
        TWO,
        StoreVersion {
            released: None,
            ..THREE
        },
    ],
    retired: &[],
};

// A canary build: 3 is released, 4 is not yet.
static CANARY: StoreVersions = StoreVersions {
    known: &[
        TWO,
        THREE,
        StoreVersion {
            number: 4,
            released: None,
            migrate: Some(nothing),
        },
    ],
    retired: &[],
};

static RETIRING: StoreVersions = StoreVersions {
    known: &[StoreVersion {
        migrate: None,
        ..THREE
    }],
    retired: &[Retired {
        number: 2,
        last_release: "0.0.9",
    }],
};

fn actor() -> Actor {
    Actor::new("Ada")
}

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().expect("time")
}

fn todo(title: &str) -> Task {
    Task::new(title, Status::Todo, stamp())
}

struct Store {
    _dir: tempfile::TempDir,
    backend: Arc<dyn Backend>,
}

impl Store {
    // A store of version 2 with one task in `todo`.
    fn new() -> Self {
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
            .create_task(&actor(), &todo("Ship it"))
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

fn status(tracker: &Tracker, number: u64) -> Status {
    tracker
        .plan()
        .expect("plan")
        .task(number)
        .expect("task")
        .frontmatter
        .status
}

fn messages(tracker: &Tracker, count: usize) -> Vec<String> {
    tracker
        .history(&HistoryQuery::default())
        .expect("history")
        .into_iter()
        .take(count)
        .map(|entry| entry.revision.message)
        .collect()
}

fn problem_of(tracker: &Tracker) -> VersionError {
    match tracker.plan().expect("plan").config().map(drop) {
        Err(TrackerError::Version(problem)) => problem,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_store_starts_in_the_newest_store_version() {
    assert_eq!(
        Store::new().config(),
        "abbreviation = \"OPP\"\nversion = 2\n"
    );
}

#[test]
fn a_store_from_before_the_store_versions_gets_the_version_key_at_the_first_write() {
    let store = Store::new();
    store
        .backend
        .commit(&actor(), &mut |_| {
            Ok(op_backend::Edit::new(
                "Write the config of an older openplan",
                vec![Op::put("config.toml", "abbreviation = \"OPP\"\n")],
            ))
        })
        .expect("commit");
    let tracker = Tracker::new(Arc::clone(&store.backend));
    assert_eq!(tracker.plan().expect("plan").migrated_from(), Some(1));
    assert_eq!(
        store.config(),
        "abbreviation = \"OPP\"\n",
        "a read writes nothing"
    );

    tracker
        .update_task(&actor(), 1, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .expect("a write migrates the store first");

    assert_eq!(store.config(), "abbreviation = \"OPP\"\nversion = 2\n");
    assert_eq!(
        messages(&tracker, 2),
        vec![
            "OPP-1: status → done",
            "Migrate the tasks from store version 1 to 2"
        ]
    );
}

#[test]
fn an_older_store_reads_as_migrated_and_migrates_before_a_write() {
    let store = Store::new();
    let tracker = store.tracker(&RELEASED);

    assert_eq!(tracker.plan().expect("plan").migrated_from(), Some(2));
    assert_eq!(status(&tracker, 1), Status::Backlog);

    tracker
        .create_task(&actor(), &todo("Next"))
        .expect("a write migrates the store first");

    assert_eq!(store.config(), "abbreviation = \"OPP\"\nversion = 3\n");
    assert_eq!(tracker.plan().expect("plan").migrated_from(), None);
    assert_eq!(
        messages(&tracker, 2),
        vec![
            "OPP-2: create \"Next\"",
            "Migrate the tasks from store version 2 to 3"
        ]
    );
    assert_eq!(tracker.migrate(&actor()).expect("migrate again"), None);
}

#[test]
fn an_older_store_refuses_a_write_until_someone_migrates_it_to_an_unreleased_version() {
    let store = Store::new();
    let tracker = store.tracker(&UNRELEASED);

    let refused = tracker.create_task(&actor(), &todo("Next"));
    assert!(
        matches!(
            refused,
            Err(TrackerError::Version(VersionError::Unmigrated {
                stored: 2,
                current: 3
            }))
        ),
        "{refused:?}"
    );
    assert_eq!(store.config(), "abbreviation = \"OPP\"\nversion = 2\n");

    assert_eq!(tracker.migrate(&actor()).expect("migrate"), Some(2));
    tracker
        .create_task(&actor(), &todo("Next"))
        .expect("a write after the migration");
}

#[test]
fn a_canary_migrates_by_itself_only_as_far_as_a_release_reads() {
    let store = Store::new();
    let tracker = store.tracker(&CANARY);

    let refused = tracker.create_task(&actor(), &todo("Next"));

    assert!(
        matches!(
            refused,
            Err(TrackerError::Version(VersionError::Unmigrated {
                stored: 3,
                current: 4
            }))
        ),
        "{refused:?}"
    );
    assert_eq!(store.config(), "abbreviation = \"OPP\"\nversion = 3\n");
    assert_eq!(
        messages(&tracker, 1),
        vec!["Migrate the tasks from store version 2 to 3"]
    );
}

#[test]
fn the_history_names_the_migration_and_reads_older_revisions_as_migrated() {
    let store = Store::new();
    let tracker = store.tracker(&RELEASED);
    tracker.migrate(&actor()).expect("migrate");

    let entries = tracker.history(&HistoryQuery::default()).expect("history");
    let described = tracker.describe(&entries[0]).expect("describe");
    assert_eq!(
        described.lines(None, None),
        vec!["Migrate the tasks from store version 2 to 3"]
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
fn a_store_of_a_newer_store_version_needs_a_newer_openplan() {
    let store = Store::new();
    store.tracker(&RELEASED).migrate(&actor()).expect("migrate");
    let tracker = store.tracker(&STORE_VERSIONS);

    assert_eq!(
        problem_of(&tracker).to_string(),
        "these tasks use store version 3, and this openplan reads store versions up to 2. They \
         need a newer openplan"
    );
    assert!(matches!(
        tracker.update_task(&actor(), 1, |_| Ok(())),
        Err(TrackerError::Version(VersionError::Newer { .. }))
    ));
}

#[test]
fn a_retired_store_version_names_the_last_release_that_migrates_it() {
    assert_eq!(
        problem_of(&Store::new().tracker(&RETIRING)).to_string(),
        "these tasks use store version 2, and this openplan migrates only store version 3 and \
         newer. Run openplan 0.0.9 on this project once to migrate them, then update"
    );
}

#[test]
fn a_daemon_migrates_by_itself_up_to_the_newest_released_store_version() {
    assert_eq!(STORE_VERSIONS.newest_released(), 2);
    assert_eq!(RELEASED.newest_released(), 3);
    assert_eq!(UNRELEASED.newest_released(), 2);
    assert_eq!(CANARY.newest_released(), 3);
}

// Two daemons that migrate one store before they sync must write the same bytes, whichever build
// each of them runs.
#[test]
fn a_migration_depends_on_the_store_alone() {
    let (released, unreleased) = (Store::new(), Store::new());
    released
        .tracker(&RELEASED)
        .migrate(&actor())
        .expect("migrate");
    unreleased
        .tracker(&UNRELEASED)
        .migrate(&actor())
        .expect("an explicit migration");
    let task = |store: &Store| {
        store
            .tracker(&RELEASED)
            .plan()
            .expect("plan")
            .raw(1)
            .expect("raw")
    };
    assert_eq!(released.config(), unreleased.config());
    assert_eq!(task(&released), task(&unreleased));
}

#[test]
fn every_store_version_after_the_oldest_migrates_from_the_one_before() {
    for versions in [&STORE_VERSIONS, &RELEASED, &UNRELEASED, &CANARY, &RETIRING] {
        for (at, known) in versions.known.iter().enumerate() {
            assert_eq!(
                known.migrate.is_some(),
                at > 0,
                "store version {}",
                known.number
            );
        }
        for pair in versions.known.windows(2) {
            assert_eq!(pair[1].number, pair[0].number + 1);
        }
    }
}

fn store(version: u32) -> MemorySnapshot {
    MemorySnapshot::new(
        None,
        BTreeMap::from([(
            "config.toml".to_owned(),
            format!("abbreviation = \"OPP\"\nversion = {version}\n").into_bytes(),
        )]),
    )
}

#[test]
fn a_merge_base_older_than_every_migration_stops_the_sync_and_does_not_panic() {
    let policy = TaskMergePolicy::default().with_store_versions(&RETIRING);

    let stopped = policy.align(&store(1), &store(3), &store(3));

    assert!(
        matches!(&stopped, Err(BackendError::Sync(reason)) if reason.contains("store version 1")),
        "{stopped:?}"
    );
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

fn join(root: &Path, name: &str, versions: &'static StoreVersions) -> Member {
    let remote = root.join("remote.git");
    git(
        root,
        &["clone", "--quiet", remote.to_str().expect("utf-8"), name],
    );
    open(root, name, versions, TaskMergePolicy::default())
}

impl Member {
    fn sync(&self) -> Result<op_backend::SyncReport, BackendError> {
        self.backend.remote().expect("a remote").sync()
    }

    fn create(&self, title: &str) {
        self.tracker
            .create_task(&actor(), &todo(title))
            .expect("create");
    }

    fn statuses(&self) -> Vec<Status> {
        let plan = self.tracker.plan().expect("plan");
        plan.numbers()
            .map(|number| plan.task(number).expect("task").frontmatter.status)
            .collect()
    }
}

// Every revision of a test lands in the same second, so the history does not put the merge first.
fn merge_of(member: &Member) -> op_backend::LogEntry {
    member
        .tracker
        .history(&HistoryQuery::default())
        .expect("history")
        .into_iter()
        .find(|entry| entry.revision.parents.len() == 2)
        .expect("a merge")
}

// Alice starts the tasks in store version 2 with one task in `todo`, and Bob joins with a binary
// that knows store version 3.
fn team(root: &Path, alice_policy: TaskMergePolicy) -> (Member, Member) {
    git(
        root,
        &["init", "--quiet", "--bare", "-b", "main", "remote.git"],
    );
    let remote = root.join("remote.git");
    git(
        root,
        &["clone", "--quiet", remote.to_str().expect("utf-8"), "alice"],
    );
    let alice = open(root, "alice", &STORE_VERSIONS, alice_policy);
    alice
        .tracker
        .init(&actor(), "OPP".parse().expect("abbreviation"))
        .expect("init");
    alice.create("Shared");
    alice.sync().expect("sync");
    let bob = join(root, "bob", &RELEASED);
    bob.sync().expect("sync");
    (alice, bob)
}

#[test]
fn a_sync_migrates_our_older_side_before_it_merges() {
    let root = tempfile::tempdir().expect("tempdir");
    let (alice, bob) = team(root.path(), TaskMergePolicy::default());
    bob.create("Written in store version 3");
    bob.sync().expect("sync");
    alice.create("Written in store version 2");
    drop(alice);
    let alice = open(root.path(), "alice", &RELEASED, TaskMergePolicy::default());

    alice.sync().expect("sync");

    assert_eq!(alice.tracker.plan().expect("plan").migrated_from(), None);
    // Bob's task was written in store version 3 and keeps `todo`. Alice's task moved to OPP-3.
    assert_eq!(
        alice.statuses(),
        vec![Status::Backlog, Status::Todo, Status::Backlog]
    );
    let merge = merge_of(&alice);
    assert_eq!(
        alice.tracker.describe(&merge).expect("describe").version,
        None,
        "Bob's revision shows the migration already"
    );
}

#[test]
fn a_sync_migrates_their_older_side_before_it_merges() {
    let root = tempfile::tempdir().expect("tempdir");
    let (alice, bob) = team(root.path(), TaskMergePolicy::default());
    alice.create("Written in store version 2");
    alice.sync().expect("sync");
    bob.create("Written in store version 3");

    bob.sync().expect("sync");

    // Alice's task was written in store version 2 and moves to `backlog`. Bob's task moved to OPP-3.
    assert_eq!(
        bob.statuses(),
        vec![Status::Backlog, Status::Backlog, Status::Todo]
    );
    assert_eq!(bob.tracker.plan().expect("plan").migrated_from(), None);
}

#[test]
fn two_sides_that_migrate_by_themselves_merge_without_a_conflict() {
    let root = tempfile::tempdir().expect("tempdir");
    let (alice, bob) = team(root.path(), TaskMergePolicy::default());
    drop(alice);
    let alice = open(root.path(), "alice", &RELEASED, TaskMergePolicy::default());
    alice
        .tracker
        .update_task(&actor(), 1, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .expect("update");
    alice.sync().expect("sync");
    bob.tracker
        .create_tag(
            &actor(),
            &op_task::tag::Tag::new("Backend", None).expect("tag"),
        )
        .expect("create tag");

    let report = bob.sync().expect("sync");

    assert!(report.merged);
    let history = bob
        .tracker
        .history(&HistoryQuery::default())
        .expect("history");
    assert!(
        history.iter().all(|entry| entry.revision.parents.len() < 2),
        "the history hides a merge that changes nothing of its own"
    );
    let plan = bob.tracker.plan().expect("plan");
    assert_eq!(plan.config().expect("config").version, 3);
    assert_eq!(bob.statuses(), vec![Status::Done]);
}

#[test]
fn a_sync_stops_on_a_store_version_this_binary_cannot_read_and_says_so() {
    let root = tempfile::tempdir().expect("tempdir");
    let heard: Arc<Mutex<Option<VersionError>>> = Arc::default();
    let reported = Arc::clone(&heard);
    let (alice, bob) = team(
        root.path(),
        TaskMergePolicy::default().on_unreadable(move |problem| {
            *reported.lock().expect("lock") = Some(problem.clone());
        }),
    );
    bob.tracker.migrate(&actor()).expect("migrate");
    bob.sync().expect("sync");
    alice.create("Written before the sync");

    let stopped = alice.sync();

    assert!(
        matches!(&stopped, Err(BackendError::Sync(reason)) if reason.contains("store version 3")),
        "{stopped:?}"
    );
    assert!(matches!(
        heard.lock().expect("lock").as_ref(),
        Some(VersionError::Newer { .. })
    ));
    assert_eq!(alice.statuses().len(), 2, "the local tasks stay");
}
