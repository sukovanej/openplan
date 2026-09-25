use std::sync::Arc;

use op_backend::{
    Actor, Backend, BackendError, BackendEvent, BackendExt as _, ChangeKind, Edit, LogQuery, Op,
    Origin,
};

pub trait Subject {
    // Every call opens the same storage again, the way a restarted daemon would.
    fn open(&self) -> Arc<dyn Backend>;
}

#[macro_export]
macro_rules! suite {
    ($fresh:expr) => {
        $crate::suite!(@tests $fresh;
            empty_store_has_no_revision,
            commit_writes_and_reads_back,
            commit_without_change_makes_no_revision,
            remove_reports_the_removed_path,
            aborted_write_leaves_the_head,
            old_revision_stays_readable,
            read_at_reads_one_document_of_a_revision,
            log_filters_by_prefix_newest_first,
            log_pages_with_before_and_limit,
            changes_between_revisions,
            changes_run_both_ways_and_skip_a_reverted_document,
            large_documents_read_back,
            invalid_paths_are_refused,
            author_and_message_round_trip,
            commit_announces_the_head_move,
            concurrent_writes_lose_nothing,
            history_survives_a_reopen,
            list_names_direct_children_only,
            paging_from_an_unknown_revision_is_refused
        );
    };
    (@tests $fresh:expr; $($name:ident),*) => {
        $(
            #[test]
            fn $name() {
                let subject = ($fresh)();
                $crate::$name(&subject);
            }
        )*
    };
}

fn author() -> Actor {
    Actor::new("Ada Lovelace").with_email("ada@example.com")
}

fn put(backend: &dyn Backend, path: &str, text: &str) -> op_backend::Committed {
    let (path, text) = (path.to_owned(), text.to_owned());
    backend
        .commit(&author(), &mut |_| {
            Ok(Edit::new(
                format!("Write {path}"),
                vec![Op::put(&path, text.as_bytes())],
            ))
        })
        .expect("commit")
        .expect("a revision")
}

fn text(backend: &dyn Backend, path: &str) -> Option<String> {
    backend.head().expect("head").read_text(path).expect("read")
}

pub fn empty_store_has_no_revision(subject: &impl Subject) {
    let backend = subject.open();
    let head = backend.head().expect("head");
    assert_eq!(head.revision(), None);
    assert_eq!(head.files().expect("files"), Vec::<String>::new());
    assert!(backend.log(&LogQuery::default()).expect("log").is_empty());
}

pub fn commit_writes_and_reads_back(subject: &impl Subject) {
    let backend = subject.open();
    let committed = put(&*backend, "tasks/00001-a.md", "one");
    assert_eq!(
        committed.changes,
        vec![op_backend::Change::new(
            "tasks/00001-a.md",
            ChangeKind::Added
        )]
    );
    let head = backend.head().expect("head");
    assert_eq!(head.revision(), Some(&committed.revision.id));
    assert_eq!(text(&*backend, "tasks/00001-a.md").as_deref(), Some("one"));
    assert_eq!(head.files().expect("files"), vec!["tasks/00001-a.md"]);
}

pub fn commit_without_change_makes_no_revision(subject: &impl Subject) {
    let backend = subject.open();
    let first = put(&*backend, "config.toml", "x");
    let none = backend
        .commit(&author(), &mut |_| Ok(Edit::nothing()))
        .expect("commit");
    assert_eq!(none, None);
    let same = backend
        .commit(&author(), &mut |_| {
            Ok(Edit::new("Same", vec![Op::put("config.toml", "x")]))
        })
        .expect("commit");
    assert_eq!(same, None);
    let missing = backend
        .commit(&author(), &mut |_| {
            Ok(Edit::new("Gone", vec![Op::remove("tags/none.md")]))
        })
        .expect("commit");
    assert_eq!(missing, None);
    assert_eq!(
        backend.head().expect("head").revision(),
        Some(&first.revision.id)
    );
}

pub fn remove_reports_the_removed_path(subject: &impl Subject) {
    let backend = subject.open();
    put(&*backend, "tags/bug.md", "bug");
    let removed = backend
        .commit(&author(), &mut |_| {
            Ok(Edit::new("Drop", vec![Op::remove("tags/bug.md")]))
        })
        .expect("commit")
        .expect("a revision");
    assert_eq!(
        removed.changes,
        vec![op_backend::Change::new("tags/bug.md", ChangeKind::Removed)]
    );
    assert_eq!(text(&*backend, "tags/bug.md"), None);
}

pub fn aborted_write_leaves_the_head(subject: &impl Subject) {
    let backend = subject.open();
    let first = put(&*backend, "a.md", "a");
    let refused: Result<(Option<op_backend::Committed>, ()), Refusal> =
        backend.transact(&author(), |_| Err(Refusal::No));
    assert_eq!(refused, Err(Refusal::No));
    assert_eq!(
        backend.head().expect("head").revision(),
        Some(&first.revision.id)
    );
}

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    No,
    Backend(String),
}

impl From<BackendError> for Refusal {
    fn from(err: BackendError) -> Self {
        Self::Backend(err.to_string())
    }
}

pub fn old_revision_stays_readable(subject: &impl Subject) {
    let backend = subject.open();
    let first = put(&*backend, "a.md", "old");
    put(&*backend, "a.md", "new");
    let old = backend.at(&first.revision.id).expect("at");
    assert_eq!(old.read_text("a.md").expect("read").as_deref(), Some("old"));
    assert_eq!(old.revision(), Some(&first.revision.id));
    assert_eq!(text(&*backend, "a.md").as_deref(), Some("new"));
}

pub fn read_at_reads_one_document_of_a_revision(subject: &impl Subject) {
    let backend = subject.open();
    let first = put(&*backend, "a.md", "old");
    let second = put(&*backend, "a.md", "new");
    let removed = backend
        .commit(&author(), &mut |_| {
            Ok(Edit::new("Drop", vec![Op::remove("a.md")]))
        })
        .expect("commit")
        .expect("a revision");
    let read = |revision: &op_backend::RevisionId, path: &str| {
        backend
            .read_at(revision, path)
            .expect("read_at")
            .map(|bytes| String::from_utf8(bytes).expect("text"))
    };
    assert_eq!(read(&first.revision.id, "a.md").as_deref(), Some("old"));
    assert_eq!(read(&second.revision.id, "a.md").as_deref(), Some("new"));
    assert_eq!(read(&removed.revision.id, "a.md"), None);
    assert_eq!(read(&first.revision.id, "b.md"), None);
    let unknown = backend.read_at(&op_backend::RevisionId::new("999999"), "a.md");
    assert!(
        matches!(unknown, Err(BackendError::UnknownRevision(_))),
        "{unknown:?}"
    );
}

pub fn log_filters_by_prefix_newest_first(subject: &impl Subject) {
    let backend = subject.open();
    let one = put(&*backend, "tasks/00001-a.md", "1");
    put(&*backend, "tasks/00002-b.md", "2");
    let three = put(&*backend, "tasks/00001-a.md", "1b");
    let log = backend.log(&LogQuery::under("tasks/00001-")).expect("log");
    let ids: Vec<_> = log.iter().map(|entry| entry.revision.id.clone()).collect();
    assert_eq!(ids, vec![three.revision.id, one.revision.id]);
    assert_eq!(
        log[0].changes,
        vec![op_backend::Change::new(
            "tasks/00001-a.md",
            ChangeKind::Modified
        )]
    );
    assert_eq!(backend.log(&LogQuery::default()).expect("log").len(), 3);
}

pub fn log_pages_with_before_and_limit(subject: &impl Subject) {
    let backend = subject.open();
    let revisions: Vec<_> = (0..5)
        .map(|n| put(&*backend, "a.md", &n.to_string()).revision.id)
        .collect();
    let page = backend
        .log(&LogQuery {
            prefix: String::new(),
            before: Some(revisions[3].clone()),
            limit: Some(2),
        })
        .expect("log");
    let ids: Vec<_> = page.into_iter().map(|entry| entry.revision.id).collect();
    assert_eq!(ids, vec![revisions[2].clone(), revisions[1].clone()]);
}

pub fn changes_between_revisions(subject: &impl Subject) {
    let backend = subject.open();
    let first = put(&*backend, "a.md", "a");
    put(&*backend, "b.md", "b");
    let last = backend
        .commit(&author(), &mut |_| {
            Ok(Edit::new(
                "Swap",
                vec![Op::remove("a.md"), Op::put("b.md", "b2")],
            ))
        })
        .expect("commit")
        .expect("a revision");
    let changes = backend
        .changes(Some(&first.revision.id), &last.revision.id)
        .expect("changes");
    assert_eq!(
        changes,
        vec![
            op_backend::Change::new("a.md", ChangeKind::Removed),
            op_backend::Change::new("b.md", ChangeKind::Added),
        ]
    );
    let from_nothing = backend.changes(None, &first.revision.id).expect("changes");
    assert_eq!(
        from_nothing,
        vec![op_backend::Change::new("a.md", ChangeKind::Added)]
    );
}

pub fn changes_run_both_ways_and_skip_a_reverted_document(subject: &impl Subject) {
    let backend = subject.open();
    let first = put(&*backend, "tasks/00001-a.md", "one");
    put(&*backend, "tasks/00001-a.md", "two");
    put(&*backend, "tags/ui.md", "tag");
    let last = put(&*backend, "tasks/00001-a.md", "one");
    let forward = backend
        .changes(Some(&first.revision.id), &last.revision.id)
        .expect("changes");
    assert_eq!(
        forward,
        vec![op_backend::Change::new("tags/ui.md", ChangeKind::Added)]
    );
    let backward = backend
        .changes(Some(&last.revision.id), &first.revision.id)
        .expect("changes");
    assert_eq!(
        backward,
        vec![op_backend::Change::new("tags/ui.md", ChangeKind::Removed)]
    );
}

pub fn large_documents_read_back(subject: &impl Subject) {
    let backend = subject.open();
    let large = "a line of an embedded asset\n".repeat(20_000);
    let first = put(&*backend, "assets/large.txt", &large);
    put(&*backend, "assets/large.txt", "small now");
    put(&*backend, "assets/other.txt", &large);
    let old = backend
        .at(&first.revision.id)
        .expect("revision")
        .read_text("assets/large.txt")
        .expect("read");
    assert_eq!(old.as_deref(), Some(large.as_str()));
    let reopened = subject.open();
    assert_eq!(
        text(&*reopened, "assets/large.txt").as_deref(),
        Some("small now")
    );
    assert_eq!(
        text(&*reopened, "assets/other.txt").as_deref(),
        Some(large.as_str())
    );
}

pub fn invalid_paths_are_refused(subject: &impl Subject) {
    let backend = subject.open();
    for path in [
        "",
        "/a.md",
        "a//b.md",
        "../a.md",
        ".history",
        "tasks/.tmp",
        "a\\b",
    ] {
        let result = backend.commit(&author(), &mut |_| {
            Ok(Edit::new("Bad", vec![Op::put(path, "x")]))
        });
        assert!(
            matches!(result, Err(BackendError::InvalidPath(_))),
            "{path:?} was accepted"
        );
    }
    assert_eq!(backend.head().expect("head").revision(), None);
}

pub fn author_and_message_round_trip(subject: &impl Subject) {
    let backend = subject.open();
    let actor = Actor::new("Grace Hopper")
        .with_email("grace@example.com")
        .via("claude");
    let committed = backend
        .commit(&actor, &mut |_| {
            Ok(Edit::new(
                "OPP-7: status → done",
                vec![Op::put("a.md", "a")],
            ))
        })
        .expect("commit")
        .expect("a revision");
    let entry = backend.log(&LogQuery::default()).expect("log").remove(0);
    assert_eq!(entry.revision, committed.revision);
    assert_eq!(entry.revision.author, actor);
    assert_eq!(entry.revision.message, "OPP-7: status → done");
    let reopened = subject.open();
    let entry = reopened.log(&LogQuery::default()).expect("log").remove(0);
    assert_eq!(entry.revision.author, actor);
}

pub fn commit_announces_the_head_move(subject: &impl Subject) {
    let backend = subject.open();
    let mut events = backend.subscribe();
    let committed = put(&*backend, "a.md", "a");
    let event = events.try_recv().expect("an event");
    let BackendEvent::HeadMoved(moved) = event else {
        panic!("expected a head move, got {event:?}");
    };
    assert_eq!(moved.from, None);
    assert_eq!(moved.to, committed.revision);
    assert_eq!(moved.changes, committed.changes);
    assert_eq!(moved.origin, Origin::Local);
}

pub fn concurrent_writes_lose_nothing(subject: &impl Subject) {
    let backend = subject.open();
    put(&*backend, "count", "0");
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let backend = Arc::clone(&backend);
            scope.spawn(move || {
                for _ in 0..5 {
                    backend
                        .commit(&author(), &mut |head| {
                            let count: u32 = head
                                .read_text("count")?
                                .expect("count")
                                .parse()
                                .expect("a number");
                            Ok(Edit::new(
                                "Count",
                                vec![Op::put("count", (count + 1).to_string())],
                            ))
                        })
                        .expect("commit");
                }
            });
        }
    });
    assert_eq!(text(&*backend, "count").as_deref(), Some("20"));
    assert_eq!(backend.log(&LogQuery::default()).expect("log").len(), 21);
}

pub fn history_survives_a_reopen(subject: &impl Subject) {
    let first = {
        let backend = subject.open();
        let first = put(&*backend, "a.md", "a");
        put(&*backend, "a.md", "b");
        first
    };
    let backend = subject.open();
    assert_eq!(text(&*backend, "a.md").as_deref(), Some("b"));
    assert_eq!(backend.log(&LogQuery::default()).expect("log").len(), 2);
    let old = backend.at(&first.revision.id).expect("at");
    assert_eq!(old.read_text("a.md").expect("read").as_deref(), Some("a"));
}

pub fn list_names_direct_children_only(subject: &impl Subject) {
    let backend = subject.open();
    backend
        .commit(&author(), &mut |_| {
            Ok(Edit::new(
                "Seed",
                vec![
                    Op::put("tasks/00001-a.md", "a"),
                    Op::put("tasks/00002-b.md", "b"),
                    Op::put("tasks/deep/x.md", "x"),
                    Op::put("tags/bug.md", "bug"),
                ],
            ))
        })
        .expect("commit");
    let head = backend.head().expect("head");
    assert_eq!(
        head.list("tasks").expect("list"),
        vec!["tasks/00001-a.md", "tasks/00002-b.md"]
    );
    assert_eq!(head.list("none").expect("list"), Vec::<String>::new());
}

pub fn paging_from_an_unknown_revision_is_refused(subject: &impl Subject) {
    let backend = subject.open();
    put(&*backend, "a.md", "a");
    let result = backend.log(&LogQuery {
        prefix: String::new(),
        before: Some(op_backend::RevisionId::new("999999")),
        limit: None,
    });
    assert!(
        matches!(result, Err(BackendError::UnknownRevision(_))),
        "{result:?}"
    );
}
