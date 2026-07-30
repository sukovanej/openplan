use op_api::{CreateTask, KeyError, Metadata, Status, TaskPatch, TaskSummary, id_cmp};
use op_task::{Abbreviation, Task, Timestamp};

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().unwrap()
}

fn abbreviation() -> Abbreviation {
    "OPP".parse().unwrap()
}

fn create(parent: Option<&str>, dependencies: &[&str], body: Option<&str>) -> CreateTask {
    CreateTask {
        title: "T".to_owned(),
        status: None,
        parent: parent.map(str::to_owned),
        dependencies: dependencies.iter().map(|d| (*d).to_owned()).collect(),
        body: body.map(str::to_owned),
    }
}

#[test]
fn a_read_renders_references_as_keys() {
    let mut task = Task::new("T", Status::Todo, stamp());
    task.set_parent(Some("7".to_owned()));
    task.set_dependencies(vec!["8".to_owned(), "9#Design".to_owned()]);

    let metadata = Metadata::from_frontmatter(&task.frontmatter, abbreviation());
    assert_eq!(metadata.parent(), Some("OPP-7"));
    assert_eq!(metadata.dependencies(), ["OPP-8", "OPP-9#Design"]);
}

#[test]
fn a_lenient_read_renders_references_as_keys_too() {
    let raw = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: ./00007-epic.md\n\
               dependencies:\n  - ./00008-other.md#Design\n---\n# T\n";
    let summary = TaskSummary::from_partial(
        "OPP-1".to_owned(),
        op_task::parse_partial(raw),
        abbreviation(),
    );
    assert_eq!(summary.metadata.parent(), Some("OPP-7"));
    assert_eq!(summary.metadata.dependencies(), ["OPP-8#Design"]);
}

#[test]
fn a_write_takes_keys_and_hands_the_file_layer_numbers() {
    let task = create(Some("OPP-7"), &["OPP-8", "OPP-9#Design"], None)
        .into_task(stamp(), abbreviation())
        .unwrap();
    assert_eq!(task.frontmatter.parent.as_deref(), Some("7"));
    assert_eq!(task.frontmatter.dependencies, ["8", "9#Design"]);
}

#[test]
fn a_write_in_any_other_spelling_is_refused() {
    for spelling in ["7", "opp-7", "OPP-007", "WEB-7", "epic-1"] {
        let parent = create(Some(spelling), &[], None).into_task(stamp(), abbreviation());
        assert!(
            matches!(&parent, Err(KeyError { got, .. }) if got == spelling),
            "parent {spelling:?} must be refused: {parent:?}"
        );
        let dependency = create(None, &[spelling], None).into_task(stamp(), abbreviation());
        assert!(
            dependency.is_err(),
            "dependency {spelling:?} must be refused"
        );

        let patch = TaskPatch {
            dependencies: Some(vec![spelling.to_owned()]),
            ..TaskPatch::default()
        };
        let mut task = Task::new("T", Status::Todo, stamp());
        assert!(
            patch.apply(&mut task, abbreviation()).is_err(),
            "patching a dependency to {spelling:?} must be refused"
        );
    }
}

#[test]
fn a_body_reference_written_as_a_key_reaches_the_file_layer_as_a_number() {
    let task = create(None, &[], Some("see [[OPP-42]] and [[OPP-7#Design]]"))
        .into_task(stamp(), abbreviation())
        .unwrap();
    assert!(
        task.body.contains("see [[42]] and [[7#Design]]"),
        "{}",
        task.body
    );
}

#[test]
fn a_body_reference_in_another_spelling_is_refused() {
    for spelling in ["[[42]]", "[[WEB-7]]", "[[42#Design]]"] {
        let body = format!("see {spelling}");
        assert!(
            op_api::body_from_keys(abbreviation(), &body).is_err(),
            "{spelling} names no task here, so it must not be written"
        );
    }
}

#[test]
fn a_body_that_carries_no_key_is_left_exactly_as_written() {
    for body in [
        "plain prose",
        "see [[Some Page Title]]",
        "array[[index]]",
        "see [[./00042-ship-login-page.md]]",
    ] {
        assert_eq!(op_api::body_from_keys(abbreviation(), body).unwrap(), body);
    }
}

// A body documenting the spelling is not naming a task, so quoted text is neither refused nor
// rewritten — this task's own file explains `[[42]]` in exactly that way.
#[test]
fn a_quoted_reference_is_prose_and_is_left_alone() {
    for body in [
        "the old spelling was `[[42]]`",
        "a foreign `[[WEB-7]]` names nothing here",
        "```\nsee [[42]]\n```\n",
        "and `[[OPP-42]]` is the one that works",
    ] {
        assert_eq!(
            op_api::body_from_keys(abbreviation(), body).unwrap(),
            body,
            "{body:?} is quoted source, not a reference"
        );
    }
}

#[test]
fn keys_order_by_their_number() {
    let mut keys = ["OPP-10", "OPP-2", "OPP-1", "OPP-100", "OPP-9"];
    keys.sort_by(|a, b| id_cmp(a, b));
    assert_eq!(keys, ["OPP-1", "OPP-2", "OPP-9", "OPP-10", "OPP-100"]);
}
