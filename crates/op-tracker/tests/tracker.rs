use std::sync::Arc;

use op_backend::{Actor, LogQuery};
use op_backend_local::{LocalBackend, Options};
use op_task::comment::NewComment;
use op_task::tag::Tag;
use op_task::{Status, Task, Timestamp};
use op_tracker::{HistoryQuery, Tracker, TrackerError};

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

fn fresh() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let backend = LocalBackend::open(dir.path(), Options::default()).expect("open");
    Fixture {
        _dir: dir,
        tracker: Tracker::new(Arc::new(backend)),
    }
}

fn started() -> Fixture {
    let fixture = fresh();
    fixture
        .tracker
        .init(&actor(), "OPP".parse().expect("abbreviation"))
        .expect("init");
    fixture
}

fn create(tracker: &Tracker, title: &str) -> u64 {
    tracker
        .create_task(&actor(), &Task::new(title, Status::Todo, stamp()))
        .expect("create")
        .number
}

fn messages(tracker: &Tracker) -> Vec<String> {
    tracker
        .history(&HistoryQuery::default())
        .expect("history")
        .into_iter()
        .map(|entry| entry.revision.message)
        .collect()
}

#[test]
fn init_writes_the_config_and_the_default_tags_once() {
    let fixture = fresh();
    let tracker = &fixture.tracker;
    assert!(matches!(
        tracker.plan().expect("plan").config(),
        Err(TrackerError::NotInitialized)
    ));
    assert!(
        tracker
            .init(&actor(), "OPP".parse().expect("abbr"))
            .expect("init")
            .is_some()
    );
    let plan = tracker.plan().expect("plan");
    assert_eq!(plan.abbreviation().expect("abbr").as_str(), "OPP");
    assert_eq!(
        plan.tags().expect("tags").len(),
        op_task::tag::defaults().len()
    );
    assert!(
        tracker
            .init(&actor(), "OPP".parse().expect("abbr"))
            .expect("init")
            .is_none()
    );
    assert!(matches!(
        tracker.init(&actor(), "WEB".parse().expect("abbr")),
        Err(TrackerError::AlreadyInitialized(existing)) if existing == "OPP"
    ));
}

#[test]
fn a_task_needs_a_started_project() {
    let fixture = fresh();
    let result = fixture
        .tracker
        .create_task(&actor(), &Task::new("A", Status::Todo, stamp()));
    assert!(matches!(result, Err(TrackerError::NotInitialized)));
}

#[test]
fn create_numbers_tasks_in_order_and_names_the_file() {
    let fixture = started();
    let tracker = &fixture.tracker;
    assert_eq!(create(tracker, "Write the parser"), 1);
    assert_eq!(create(tracker, "Write the parser"), 2);
    let plan = tracker.plan().expect("plan");
    assert_eq!(plan.path_of(1), Some("tasks/00001-write-the-parser.md"));
    assert_eq!(
        plan.task(2).expect("task").title().as_deref(),
        Some("Write the parser")
    );
    assert_eq!(messages(tracker)[0], "OPP-2: create \"Write the parser\"");
}

#[test]
fn a_title_must_be_one_heading() {
    let fixture = started();
    let mut task = Task::new("A", Status::Todo, stamp());
    task.body = "# A\n# B\n".to_owned();
    assert!(matches!(
        fixture.tracker.create_task(&actor(), &task),
        Err(TrackerError::Invalid(_))
    ));
}

#[test]
fn update_keeps_the_body_and_describes_the_change() {
    let fixture = started();
    let tracker = &fixture.tracker;
    let number = create(tracker, "A");
    tracker
        .update_task(&actor(), number, |task| {
            task.append_body("Some *markdown*  with  spacing.");
            Ok(())
        })
        .expect("update");
    let updated = tracker
        .update_task(&actor(), number, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .expect("update");
    assert!(
        updated
            .value
            .body
            .contains("Some *markdown*  with  spacing.")
    );
    assert_eq!(messages(tracker)[0], "OPP-1: status → done");
    assert_eq!(messages(tracker)[1], "OPP-1: description");
    let unchanged = tracker
        .update_task(&actor(), number, |_| Ok(()))
        .expect("update");
    assert!(unchanged.committed.is_none());
}

#[test]
fn references_are_checked_and_written_as_the_target_file() {
    let fixture = started();
    let tracker = &fixture.tracker;
    let parent = create(tracker, "Parent");
    let child = create(tracker, "Child");
    let refuse = |mutate: fn(&mut Task)| {
        tracker.update_task(&actor(), child, move |task| {
            mutate(task);
            Ok(())
        })
    };
    assert!(matches!(
        refuse(|task| task.set_parent(Some("9".to_owned()))),
        Err(TrackerError::Invalid(_))
    ));
    assert!(matches!(
        refuse(|task| task.set_parent(Some("2".to_owned()))),
        Err(TrackerError::Invalid(_))
    ));
    assert!(matches!(
        refuse(|task| task.set_dependencies(vec!["9".to_owned()])),
        Err(TrackerError::Invalid(_))
    ));
    assert!(matches!(
        refuse(|task| task.set_rank(Some("A!".to_owned()))),
        Err(TrackerError::Invalid(_))
    ));
    assert!(matches!(
        refuse(|task| task.set_tags(vec!["nope".to_owned()])),
        Err(TrackerError::TagUnregistered { .. })
    ));

    tracker
        .update_task(&actor(), child, |task| {
            task.set_parent(Some(parent.to_string()));
            task.append_body("See [[1]].");
            Ok(())
        })
        .expect("update");
    let raw = tracker.plan().expect("plan").raw(child).expect("raw");
    assert!(raw.contains("parent: ./00001-parent.md"), "{raw}");
    assert!(raw.contains("See [[./00001-parent.md]]."), "{raw}");
    assert_eq!(messages(tracker)[0], "OPP-2: parent → OPP-1, description");

    assert!(matches!(
        tracker.update_task(&actor(), parent, |task| {
            task.set_parent(Some(child.to_string()));
            Ok(())
        }),
        Err(TrackerError::Invalid(message)) if message.contains("descendant")
    ));
}

#[test]
fn a_dangling_reference_does_not_block_an_unrelated_edit() {
    let fixture = started();
    let tracker = &fixture.tracker;
    let gone = create(tracker, "Gone");
    let task = create(tracker, "Task");
    tracker
        .update_task(&actor(), task, |task| {
            task.set_dependencies(vec![gone.to_string()]);
            Ok(())
        })
        .expect("update");
    tracker.delete_task(&actor(), gone).expect("delete");
    tracker
        .update_task(&actor(), task, |task| {
            task.set_status(Status::InProgress);
            Ok(())
        })
        .expect("an unrelated edit");
}

#[test]
fn delete_removes_the_task_and_says_which() {
    let fixture = started();
    let tracker = &fixture.tracker;
    let number = create(tracker, "Doomed");
    tracker.delete_task(&actor(), number).expect("delete");
    assert!(!tracker.plan().expect("plan").exists(number));
    assert_eq!(messages(tracker)[0], "OPP-1: delete \"Doomed\"");
    assert!(matches!(
        tracker.delete_task(&actor(), number),
        Err(TrackerError::NotFound { id }) if id == "OPP-1"
    ));
    assert_eq!(create(tracker, "Next"), 1, "a deleted number is free again");
}

#[test]
fn a_comment_is_appended_to_the_log() {
    let fixture = started();
    let tracker = &fixture.tracker;
    let number = create(tracker, "A");
    tracker
        .add_comment(
            &actor(),
            number,
            &NewComment {
                at: stamp(),
                author: "Ada".to_owned(),
                agent: Some("claude".to_owned()),
                text: "Looks good.".to_owned(),
            },
        )
        .expect("comment");
    let task = tracker.plan().expect("plan").task(number).expect("task");
    let comments = op_task::comment::parse(&task.body);
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "Looks good.");
    assert_eq!(messages(tracker)[0], "OPP-1: comment");
}

#[test]
fn a_tag_rename_moves_every_task_in_one_revision() {
    let fixture = started();
    let tracker = &fixture.tracker;
    tracker
        .create_tag(&actor(), &Tag::new("Backend", None).expect("tag"))
        .expect("create tag");
    assert!(matches!(
        tracker.create_tag(&actor(), &Tag::new("backend", None).expect("tag")),
        Err(TrackerError::TagExists { .. })
    ));
    let number = create(tracker, "A");
    tracker
        .update_task(&actor(), number, |task| {
            task.set_tags(vec!["backend".to_owned()]);
            Ok(())
        })
        .expect("tag");
    assert!(matches!(
        tracker.delete_tag(&actor(), "backend", false),
        Err(TrackerError::TagReferenced { count: 1, .. })
    ));
    let before = tracker
        .history(&HistoryQuery::default())
        .expect("history")
        .len();
    let (renamed, retagged) = tracker
        .rename_tag(&actor(), "backend", "Server")
        .expect("rename")
        .value;
    assert_eq!(renamed.name, "server");
    assert_eq!(retagged, vec![number]);
    assert_eq!(
        tracker
            .history(&HistoryQuery::default())
            .expect("history")
            .len(),
        before + 1
    );
    let plan = tracker.plan().expect("plan");
    assert_eq!(
        plan.task(number).expect("task").frontmatter.tags,
        vec!["server"]
    );
    assert!(!plan.tag_names().contains("backend"));
    tracker
        .delete_tag(&actor(), "server", true)
        .expect("forced delete");
}

#[test]
fn task_history_and_old_versions() {
    let fixture = started();
    let tracker = &fixture.tracker;
    let number = create(tracker, "A");
    create(tracker, "B");
    tracker
        .update_task(&actor(), number, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .expect("update");
    let history = tracker
        .task_history(number, &HistoryQuery::default())
        .expect("history");
    let said: Vec<&str> = history
        .iter()
        .map(|entry| entry.revision.message.as_str())
        .collect();
    assert_eq!(said, vec!["OPP-1: status → done", "OPP-1: create \"A\""]);
    assert_eq!(history[0].revision.author, actor());
    let first = tracker
        .task_at(number, &history[1].revision.id)
        .expect("old")
        .expect("present");
    assert!(first.contains("status: todo"));
    let before = tracker
        .backend()
        .log(&LogQuery::default())
        .expect("log")
        .last()
        .expect("init")
        .revision
        .id
        .clone();
    assert_eq!(tracker.task_at(number, &before).expect("old"), None);
}

#[test]
fn a_changed_body_keeps_exactly_one_title() {
    let fixture = started();
    let number = create(&fixture.tracker, "A");
    let result = fixture.tracker.update_task(&actor(), number, |task| {
        task.body = "# A\n\n# B\n".to_owned();
        Ok(())
    });
    assert!(matches!(result, Err(TrackerError::Invalid(_))));
}

#[test]
fn create_refuses_when_no_number_is_left() {
    let fixture = started();
    let last = u64::MAX;
    fixture
        .tracker
        .backend()
        .commit(&actor(), &mut |_| {
            Ok(op_backend::Edit::new(
                "Plant",
                vec![op_backend::Op::put(
                    op_task::layout::task_path(last, "Last"),
                    Task::new("Last", Status::Todo, stamp())
                        .to_file_string()
                        .expect("text"),
                )],
            ))
        })
        .expect("commit");
    let result = fixture
        .tracker
        .create_task(&actor(), &Task::new("Next", Status::Todo, stamp()));
    assert!(
        matches!(&result, Err(TrackerError::Invalid(message)) if message.contains("no number is left")),
        "{result:?}"
    );
    assert_eq!(fixture.tracker.plan().expect("plan").numbers().count(), 1);
}
