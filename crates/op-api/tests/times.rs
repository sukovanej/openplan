use op_api::{Field, FieldError, TaskView};
use op_task::{Status, Task, Timestamp};

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn view(created: &str, updated: Result<&str, op_task::FieldError>) -> TaskView {
    let task = Task::new("T", Status::Todo, at(created));
    TaskView::from_task("t".to_owned(), &task, updated.map(at))
}

#[test]
fn a_git_derived_update_after_creation_is_reported_as_is() {
    let view = view("2026-01-01T00:00:00Z", Ok("2026-02-01T00:00:00Z"));
    assert_eq!(view.metadata.created(), Some("2026-01-01T00:00:00Z"));
    assert_eq!(
        view.updated,
        Field::Value("2026-02-01T00:00:00Z".to_owned())
    );
}

#[test]
fn an_update_before_creation_clamps_up_to_created() {
    let view = view("2026-03-01T00:00:00Z", Ok("2026-02-01T00:00:00Z"));
    assert_eq!(
        view.updated,
        Field::Value("2026-03-01T00:00:00Z".to_owned())
    );
}

#[test]
fn a_store_only_read_reports_created_and_no_update() {
    let view = view("2026-01-01T00:00:00Z", Err(op_task::FieldError::Missing));
    assert_eq!(view.metadata.created(), Some("2026-01-01T00:00:00Z"));
    assert_eq!(view.updated, Field::Error(FieldError::Missing));
}

#[test]
fn a_commit_that_could_not_be_dated_reports_why_instead_of_a_time() {
    let view = view(
        "2026-01-01T00:00:00Z",
        Err(op_task::FieldError::Invalid(
            "commit c0ffee has an unusable author date".to_owned(),
        )),
    );
    // Not clamped up to `created`: a reason is not a time, and inventing one here would hide the
    // very thing the field is reporting.
    assert_eq!(
        view.updated,
        Field::Error(FieldError::Invalid {
            message: "commit c0ffee has an unusable author date".to_owned()
        })
    );
}
