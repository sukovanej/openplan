use op_api::CreateTask;
use op_task::{Status, Timestamp};

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().unwrap()
}

fn create(title: &str, body: Option<&str>) -> op_task::Task {
    CreateTask {
        title: title.to_owned(),
        status: None,
        parent: None,
        deps: Vec::new(),
        body: body.map(str::to_owned),
    }
    .into_task(stamp())
}

#[test]
fn into_task_without_body_is_title_only() {
    let task = create("Ship login", None);
    assert_eq!(task.body, "# Ship login\n");
    assert_eq!(task.title().as_deref(), Some("Ship login"));
    assert_eq!(task.frontmatter.status, Status::Todo);
}

#[test]
fn into_task_stamps_created_from_the_supplied_clock() {
    assert_eq!(create("Ship login", None).frontmatter.created, stamp());
}

#[test]
fn into_task_with_body_places_content_below_title() {
    let task = create("Ship login", Some("Support OAuth and email login."));
    assert_eq!(
        task.body,
        "# Ship login\n\nSupport OAuth and email login.\n"
    );
    assert_eq!(task.title().as_deref(), Some("Ship login"));
}

#[test]
fn into_task_normalizes_a_single_trailing_newline() {
    let task = create("Ship login", Some("line one\nline two\n\n\n"));
    assert_eq!(task.body, "# Ship login\n\nline one\nline two\n");
}

#[test]
fn into_task_with_empty_body_stays_title_only() {
    let task = create("Ship login", Some(""));
    assert_eq!(task.body, "# Ship login\n");
}
