use op_api::TaskView;
use op_task::{Status, Task, Timestamp};

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn view(created: &str, updated: Option<&str>) -> TaskView {
    let task = Task::new("T", Status::Todo, at(created));
    TaskView::from_task("t".to_owned(), &task, updated.map(at))
}

#[test]
fn a_git_derived_update_after_creation_is_reported_as_is() {
    let view = view("2026-01-01T00:00:00Z", Some("2026-02-01T00:00:00Z"));
    assert_eq!(view.metadata.created(), Some("2026-01-01T00:00:00Z"));
    assert_eq!(view.updated, Some(at("2026-02-01T00:00:00Z")));
}

#[test]
fn an_update_before_creation_clamps_up_to_created() {
    let view = view("2026-03-01T00:00:00Z", Some("2026-02-01T00:00:00Z"));
    assert_eq!(
        view.updated.map(|at| at.to_string()).as_deref(),
        view.metadata.created()
    );
}

#[test]
fn a_store_only_read_reports_created_and_no_update() {
    let view = view("2026-01-01T00:00:00Z", None);
    assert_eq!(view.metadata.created(), Some("2026-01-01T00:00:00Z"));
    assert_eq!(view.updated, None);
}
