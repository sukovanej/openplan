use op_task::{Frontmatter, Status, Task};

fn task(frontmatter: Frontmatter, body: &str) -> Task {
    Task {
        frontmatter,
        body: body.to_owned(),
    }
}

#[test]
fn frontmatter_roundtrips() {
    let original = task(
        Frontmatter {
            status: Status::InProgress,
            parent: Some("epic-42".to_owned()),
            deps: vec!["task-1".to_owned(), "task-2#Design".to_owned()],
            extra: Default::default(),
        },
        "# Title\n\nbody\n",
    );

    let text = original.to_file_string().unwrap();
    let parsed = Task::from_file_string(&text).unwrap();
    assert_eq!(parsed, original);
}

#[test]
fn minimal_frontmatter_is_just_status() {
    let text = task(
        Frontmatter {
            status: Status::Todo,
            parent: None,
            deps: vec![],
            extra: Default::default(),
        },
        "# Title\n",
    )
    .to_file_string()
    .unwrap();

    assert!(
        text.starts_with("---\nstatus: todo\n---\n"),
        "unexpected frontmatter: {text:?}"
    );
}

#[test]
fn parses_crlf_frontmatter_and_preserves_body() {
    let text = "---\r\nstatus: in_progress\r\n---\r\n# Title\r\nbody\r\n";
    let parsed = Task::from_file_string(text).unwrap();
    assert_eq!(parsed.frontmatter.status, Status::InProgress);
    assert_eq!(parsed.body, "# Title\r\nbody\r\n");
}

#[test]
fn unknown_frontmatter_keys_are_preserved() {
    let text = "---\nstatus: todo\nrank: 3.5\nassignee: milan\n---\n# Task C\n";
    let mut parsed = Task::from_file_string(text).unwrap();
    parsed.set_status(Status::Done);

    let written = parsed.to_file_string().unwrap();
    assert!(
        written.contains("rank: 3.5"),
        "rank must survive: {written}"
    );
    assert!(
        written.contains("assignee: milan"),
        "assignee must survive: {written}"
    );
    assert!(written.contains("status: done"));
}

#[test]
fn missing_closing_fence_is_rejected() {
    assert!(Task::from_file_string("---\nstatus: todo\n").is_err());
    assert!(Task::from_file_string("no frontmatter here\n").is_err());
}

#[test]
fn task_file_roundtrips_with_title() {
    let original = task(
        Frontmatter {
            status: Status::Todo,
            parent: None,
            deps: vec![],
            extra: Default::default(),
        },
        "# Ship it\n\nbody text\n",
    );

    let text = original.to_file_string().unwrap();
    let parsed = Task::from_file_string(&text).unwrap();
    assert_eq!(parsed, original);
    assert_eq!(parsed.title().as_deref(), Some("Ship it"));
}
