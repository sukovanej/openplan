use op_api::{Abbreviation, Metadata, render_task_file};
use op_task::content::split;
use op_task::{Status, Task, Timestamp};

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().unwrap()
}

fn abbreviation() -> Abbreviation {
    "OPP".parse().unwrap()
}

fn rendered(task: &Task) -> String {
    let metadata = Metadata::from_frontmatter(&task.frontmatter, abbreviation());
    let title = task.title().unwrap_or_default();
    render_task_file(&metadata, &title, &split(&task.body), &[]).unwrap()
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

#[test]
fn a_rendered_file_puts_the_title_line_above_the_description() {
    let mut task = Task::new("Ship login", Status::Todo, stamp());
    task.append_body("Support OAuth.");

    assert!(rendered(&task).ends_with("---\n# Ship login\n\nSupport OAuth.\n"));
    assert_eq!(rendered(&task), task.to_file_string().unwrap());
}

#[test]
fn a_title_inside_a_conflict_block_renders_the_description_as_the_body() {
    let mut task = Task::new("Ship login", Status::Todo, stamp());
    task.body =
        "<<<<<<< Ann (1111111)\n# Log in\n=======\n# Sign in\n>>>>>>> Ben (2222222)\n\nBody.\n"
            .to_owned();

    assert_eq!(rendered(&task), task.to_file_string().unwrap());
}
