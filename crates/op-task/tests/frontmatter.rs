use op_task::{Frontmatter, Status, Task, Timestamp};

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().unwrap()
}

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
            created: stamp(),
            parent: Some("42".to_owned()),
            rank: Some("m".to_owned()),
            deps: vec!["1".to_owned(), "2#Design".to_owned()],
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
            created: stamp(),
            parent: None,
            rank: None,
            deps: vec![],
            extra: Default::default(),
        },
        "# Title\n",
    )
    .to_file_string()
    .unwrap();

    assert!(
        text.starts_with("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n"),
        "unexpected frontmatter: {text:?}"
    );
}

#[test]
fn parses_crlf_frontmatter_and_preserves_body() {
    let text =
        "---\r\nstatus: in_progress\r\ncreated: 2026-01-01T00:00:00Z\r\n---\r\n# Title\r\nbody\r\n";
    let parsed = Task::from_file_string(text).unwrap();
    assert_eq!(parsed.frontmatter.status, Status::InProgress);
    assert_eq!(parsed.frontmatter.created, stamp());
    assert_eq!(parsed.body, "# Title\r\nbody\r\n");
}

#[test]
fn unknown_frontmatter_keys_are_preserved() {
    let text = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nestimate: 3.5\nassignee: milan\n---\n# Task C\n";
    let mut parsed = Task::from_file_string(text).unwrap();
    parsed.set_status(Status::Done);

    let written = parsed.to_file_string().unwrap();
    assert!(
        written.contains("estimate: 3.5"),
        "estimate must survive: {written}"
    );
    assert!(
        written.contains("assignee: milan"),
        "assignee must survive: {written}"
    );
    assert!(written.contains("status: done"));
}

#[test]
fn missing_closing_fence_is_rejected() {
    assert!(Task::from_file_string("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n").is_err());
    assert!(Task::from_file_string("no frontmatter here\n").is_err());
}

#[test]
fn task_file_roundtrips_with_title() {
    let original = task(
        Frontmatter {
            status: Status::Todo,
            created: stamp(),
            parent: None,
            rank: None,
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

#[test]
fn ids_are_written_quoted_and_read_back_from_either_spelling() {
    let text = task(
        Frontmatter {
            status: Status::Todo,
            created: stamp(),
            parent: Some("42".to_owned()),
            rank: None,
            deps: vec!["7".to_owned(), "8#Design".to_owned()],
            extra: Default::default(),
        },
        "# Title\n",
    )
    .to_file_string()
    .unwrap();
    assert!(text.contains("parent: '42'"), "{text}");
    assert!(text.contains("- '7'"), "{text}");

    // Hand-written frontmatter spells an id the way YAML does, as a bare integer.
    let bare = Task::from_file_string(
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: 42\ndeps:\n- 7\n- 8#Design\n---\n# Title\n",
    )
    .unwrap();
    assert_eq!(bare.frontmatter.parent.as_deref(), Some("42"));
    assert_eq!(bare.frontmatter.deps, vec!["7", "8#Design"]);
}

#[test]
fn a_reference_that_is_not_an_id_is_rejected() {
    for frontmatter in [
        "parent: ship-login-3d0c",
        "parent: 042",
        "deps:\n- ship-login-3d0c",
        "deps:\n- 042#Design",
    ] {
        let text =
            format!("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n{frontmatter}\n---\n# T\n");
        assert!(
            Task::from_file_string(&text).is_err(),
            "{frontmatter:?} must not parse as a reference"
        );
    }
}

#[test]
fn parse_id_accepts_only_the_canonical_spelling() {
    assert_eq!(op_task::parse_id("42"), Some(42));
    assert_eq!(op_task::parse_id("0"), Some(0));
    for not_an_id in [
        "042",
        "+42",
        " 42",
        "42 ",
        "",
        "4_2",
        "-1",
        "ship-login-3d0c",
        "42-ship",
    ] {
        assert_eq!(op_task::parse_id(not_an_id), None, "{not_an_id:?}");
    }
}

#[test]
fn dep_target_splits_off_the_section() {
    assert_eq!(op_task::dep_target("42"), "42");
    assert_eq!(op_task::dep_target("42#Design"), "42");
    assert_eq!(op_task::dep_target("42#A#B"), "42");
}
