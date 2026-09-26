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
        tags: Vec::new(),
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
            matches!(&parent, Err(err) if err.got() == spelling),
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
fn a_refused_key_of_another_project_says_so() {
    assert_eq!(
        KeyError::new(abbreviation(), "CQR-97").to_string(),
        "CQR-97 is in another project; this project's keys start with OPP-"
    );
    assert_eq!(
        KeyError::new(abbreviation(), "CQR-97#Design").to_string(),
        "CQR-97#Design is in another project; this project's keys start with OPP-"
    );
    assert_eq!(
        KeyError::new(abbreviation(), "opp-7").to_string(),
        "not a task key: \"opp-7\"; expected OPP-42"
    );
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
fn a_body_reads_with_each_reference_to_a_task_here_as_its_key() {
    let body = "see [[./00042-ship-login.md]], [[./00007-schema.md#Design]], and [[ OPP-3 ]]\n";

    assert_eq!(
        op_api::body_to_keys(abbreviation(), "tasks", body),
        "see [[OPP-42]], [[OPP-7#Design]], and [[OPP-3]]\n"
    );
}

#[test]
fn a_body_to_keys_leaves_what_names_no_task_here_as_written() {
    for body in [
        "plain prose",
        "see [[Some Page Title]]",
        "array[[index]]",
        "from [[WEB-7]] and [[42]]",
        "the file spelling is `[[./00042-ship-login.md]]`",
        "```\nsee [[./00042-ship-login.md]]\n```\n",
    ] {
        assert_eq!(op_api::body_to_keys(abbreviation(), "tasks", body), body);
    }
}

#[test]
fn a_body_to_keys_reads_back_as_the_numbers_the_store_holds() {
    let body = "see [[./00042-ship-login.md#Design]]";

    let keyed = op_api::body_to_keys(abbreviation(), "tasks", body);

    assert_eq!(
        op_api::body_from_keys(abbreviation(), &keyed).unwrap(),
        "see [[42#Design]]"
    );
}

#[test]
fn keys_order_by_their_number() {
    let mut keys = ["OPP-10", "OPP-2", "OPP-1", "OPP-100", "OPP-9"];
    keys.sort_by(|a, b| id_cmp(a, b));
    assert_eq!(keys, ["OPP-1", "OPP-2", "OPP-9", "OPP-10", "OPP-100"]);
}

#[test]
fn a_header_carries_any_name_and_reads_back_the_same() {
    for text in ["Ada Lovelace", "Milan Šuk", "100% sure", "a\nb"] {
        let encoded = op_api::encode_header(text);
        assert!(
            encoded.bytes().all(|byte| byte.is_ascii_graphic()),
            "{encoded}"
        );
        assert_eq!(op_api::decode_header(&encoded).as_deref(), Some(text));
    }
    assert_eq!(op_api::decode_header("%zz"), None);
}

#[test]
fn a_doc_body_reads_with_a_key_for_each_task_and_a_name_for_each_doc() {
    let body = "see [[../tasks/00042-ship-login.md]] and [[./storage.md#Layout]]\n";

    assert_eq!(
        op_api::body_to_keys(abbreviation(), "docs", body),
        "see [[OPP-42]] and [[storage#Layout]]\n"
    );
}
