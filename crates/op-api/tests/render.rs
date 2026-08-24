use op_api::{Abbreviation, Metadata, render_task_file};
use op_task::{Status, Task, Timestamp};

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().unwrap()
}

fn abbreviation() -> Abbreviation {
    "OPP".parse().unwrap()
}

fn rendered(task: &Task) -> String {
    let metadata = Metadata::from_frontmatter(&task.frontmatter, abbreviation());
    render_task_file(&metadata, &task.body).unwrap()
}

#[test]
fn a_rendered_file_carries_the_tags_the_task_holds() {
    let mut task = Task::new("Ship login", Status::Todo, stamp());
    task.set_tags(vec!["wip".to_owned(), "backend".to_owned()]);

    assert_eq!(rendered(&task), task.to_file_string().unwrap());
    assert!(rendered(&task).contains("tags:\n- backend\n- wip\n"));
}

#[test]
fn a_task_with_no_tags_renders_no_tags_key() {
    let task = Task::new("Ship login", Status::Todo, stamp());

    assert!(!rendered(&task).contains("tags"));
}
