use std::sync::Arc;

use op_backend::{Actor, ChangeKind, Edit, Op};
use op_backend_local::{LocalBackend, Options};
use op_task::comment::NewComment;
use op_task::tag::Tag;
use op_task::{Abbreviation, Status, Task, Timestamp};
use op_tracker::{Described, FieldChange, HistoryQuery, TagChange, TaskChange, Tracker};

struct Fixture {
    _dir: tempfile::TempDir,
    tracker: Tracker,
}

fn actor() -> Actor {
    Actor::new("Ada").with_email("ada@example.com")
}

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().expect("time")
}

fn abbreviation() -> Abbreviation {
    "OPP".parse().expect("abbreviation")
}

fn started() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let backend = LocalBackend::open(dir.path(), Options::default()).expect("open");
    let tracker = Tracker::new(Arc::new(backend));
    tracker.init(&actor(), abbreviation()).expect("init");
    Fixture { _dir: dir, tracker }
}

fn create(tracker: &Tracker, title: &str) -> u64 {
    tracker
        .create_task(&actor(), &Task::new(title, Status::Todo, stamp()))
        .expect("create")
        .number
}

fn newest(tracker: &Tracker) -> (String, Described) {
    let entry = tracker
        .history(&HistoryQuery::default())
        .expect("history")
        .remove(0);
    let described = tracker.describe(&entry).expect("describe");
    (entry.revision.message, described)
}

fn foreign_write(tracker: &Tracker, message: &str, ops: Vec<Op>) {
    tracker
        .backend()
        .commit(&actor(), &mut |_| Ok(Edit::new(message, ops.clone())))
        .expect("commit")
        .expect("a revision");
}

#[test]
fn a_revision_another_tool_wrote_is_described_from_its_documents() {
    let fixture = started();
    let tracker = &fixture.tracker;
    let dependency = create(tracker, "Dependency");
    let number = create(tracker, "Old title");
    let plan = tracker.plan().expect("plan");
    let path = plan.path_of(number).expect("path").to_owned();
    let edited = plan
        .raw(number)
        .expect("raw")
        .replace(
            "status: todo",
            "status: done\ndependencies:\n- ./00001-dependency.md\ntags:\n- bug",
        )
        .replace("# Old title", "# New title\n\nMore detail.");

    foreign_write(
        tracker,
        "Set it to done",
        vec![
            Op::remove(&path),
            Op::put("tasks/00002-new-title.md", edited.as_bytes()),
        ],
    );

    let (message, described) = newest(tracker);
    assert_eq!(message, "Set it to done");
    assert_eq!(
        described.tasks,
        vec![TaskChange {
            number,
            kind: ChangeKind::Modified,
            title: Some("New title".to_owned()),
            fields: vec![
                FieldChange::Status {
                    from: Status::Todo,
                    to: Status::Done
                },
                FieldChange::Dependencies {
                    from: Vec::new(),
                    to: vec![dependency]
                },
                FieldChange::Tags {
                    from: Vec::new(),
                    to: vec!["bug".to_owned()]
                },
                FieldChange::Title {
                    from: Some("Old title".to_owned()),
                    to: Some("New title".to_owned())
                },
                FieldChange::Description,
            ],
        }]
    );
    assert_eq!(
        described.lines(Some(abbreviation())),
        vec![
            "OPP-2: status → done, dependencies → OPP-1, tags → bug, title → \"New title\", \
             description"
        ]
    );
}

#[test]
fn a_field_openplan_does_not_model_and_an_unreadable_frontmatter_are_named() {
    let fixture = started();
    let tracker = &fixture.tracker;
    let number = create(tracker, "A");
    let plan = tracker.plan().expect("plan");
    let path = plan.path_of(number).expect("path").to_owned();
    let raw = plan.raw(number).expect("raw");

    let estimated = raw.replace("status: todo", "status: todo\nestimate: 3");
    foreign_write(tracker, "x", vec![Op::put(&path, estimated.as_bytes())]);
    let (_, described) = newest(tracker);
    assert_eq!(
        described.tasks[0].fields,
        vec![FieldChange::Other("estimate".to_owned())]
    );

    let broken = raw.replace("status: todo", "status: [todo");
    foreign_write(tracker, "y", vec![Op::put(&path, broken.as_bytes())]);
    let (_, described) = newest(tracker);
    assert_eq!(described.tasks[0].fields, vec![FieldChange::Frontmatter]);
}

#[test]
fn a_comment_and_a_delete_are_described() {
    let fixture = started();
    let tracker = &fixture.tracker;
    let number = create(tracker, "Doomed");
    tracker
        .add_comment(
            &actor(),
            number,
            &NewComment {
                at: stamp(),
                author: "Ada".to_owned(),
                agent: None,
                text: "Why?".to_owned(),
            },
        )
        .expect("comment");
    let (message, described) = newest(tracker);
    assert_eq!(message, "OPP-1: comment");
    assert_eq!(
        described.tasks[0].fields,
        vec![FieldChange::Comments {
            added: 1,
            removed: 0
        }]
    );

    tracker.delete_task(&actor(), number).expect("delete");
    let (message, described) = newest(tracker);
    assert_eq!(message, "OPP-1: delete \"Doomed\"");
    assert_eq!(
        described.tasks,
        vec![TaskChange {
            number,
            kind: ChangeKind::Removed,
            title: Some("Doomed".to_owned()),
            fields: Vec::new(),
        }]
    );
}

#[test]
fn the_start_names_the_project_and_its_tags() {
    let fixture = started();
    let (message, described) = newest(&fixture.tracker);
    assert_eq!(
        message,
        "Start the OPP tasks\n\ntag bug: create\ntag draft: create\ntag feature: create"
    );
    assert_eq!(described.config, Some(ChangeKind::Added));
}

#[test]
fn a_tag_rename_is_one_tag_that_moved() {
    let fixture = started();
    let tracker = &fixture.tracker;
    tracker
        .create_tag(&actor(), &Tag::new("Backend", None).expect("tag"))
        .expect("create tag");
    let number = create(tracker, "A");
    tracker
        .update_task(&actor(), number, |task| {
            task.set_tags(vec!["backend".to_owned()]);
            Ok(())
        })
        .expect("tag");

    tracker
        .rename_tag(&actor(), "backend", "Server")
        .expect("rename");

    let (message, described) = newest(tracker);
    assert_eq!(
        message,
        "tag backend: rename to server\n\nOPP-1: tags → server"
    );
    assert_eq!(
        described.tags,
        vec![TagChange {
            name: "server".to_owned(),
            kind: ChangeKind::Modified,
            renamed_from: Some("backend".to_owned()),
        }]
    );
}

#[test]
fn a_tag_removed_and_another_added_stay_two_changes() {
    let fixture = started();
    let tracker = &fixture.tracker;
    let fresh = Tag::new("Server", None).expect("tag");
    foreign_write(
        tracker,
        "z",
        vec![
            Op::remove("tags/draft.md"),
            Op::put("tags/server.md", fresh.to_file_string().expect("text")),
        ],
    );
    let (_, described) = newest(tracker);
    let kinds: Vec<(&str, ChangeKind)> = described
        .tags
        .iter()
        .map(|tag| (tag.name.as_str(), tag.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("draft", ChangeKind::Removed),
            ("server", ChangeKind::Added)
        ]
    );
}
