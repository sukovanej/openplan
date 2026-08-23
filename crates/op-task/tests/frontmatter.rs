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
            dependencies: vec!["1".to_owned(), "2#Design".to_owned()],
            tags: vec!["backend".to_owned(), "wip".to_owned()],
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
            dependencies: vec![],
            tags: vec![],
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
            dependencies: vec![],
            tags: vec![],
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
fn a_reference_names_the_target_file_and_reads_back_as_its_id() {
    let text = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: ./00042-ship-login.md\ndependencies:\n- ./00007-write-the-parser.md\n- ./00008-store-dtos.md#Design\n---\n# Title\n";
    let parsed = Task::from_file_string(text).unwrap();
    assert_eq!(parsed.frontmatter.parent.as_deref(), Some("42"));
    assert_eq!(parsed.frontmatter.dependencies, vec!["7", "8#Design"]);

    // The slug is a snapshot of the target's title, so a stale one still resolves.
    let stale = Task::from_file_string(
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: ./00042-the-old-title.md\n---\n# Title\n",
    )
    .unwrap();
    assert_eq!(stale.frontmatter.parent.as_deref(), Some("42"));

    // A hand-written reference may still be the bare id, in either YAML spelling.
    let bare = Task::from_file_string(
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: 42\ndependencies:\n- '7'\n- 8#Design\n---\n# Title\n",
    )
    .unwrap();
    assert_eq!(bare.frontmatter.parent.as_deref(), Some("42"));
    assert_eq!(bare.frontmatter.dependencies, vec!["7", "8#Design"]);
}

#[test]
fn a_reference_that_names_no_task_is_rejected() {
    for frontmatter in [
        "parent: ship-login-3d0c",
        "parent: 042",
        "parent: ./ship-login.md",
        "parent: ./00042-ship-login.md#Design",
        "dependencies:\n- ship-login-3d0c",
        "dependencies:\n- 042#Design",
        "dependencies:\n- ./notes.txt",
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
fn ref_id_reads_the_target_out_of_a_file_name() {
    assert_eq!(op_task::ref_id("./00042-ship-login.md"), Some(42));
    assert_eq!(op_task::ref_id("./00042-ship-login.md#Design"), Some(42));
    assert_eq!(op_task::ref_id("00042-ship-login.md"), Some(42));
    assert_eq!(op_task::ref_id("./00042.md"), Some(42));
    assert_eq!(op_task::ref_id("42"), Some(42));
    for not_a_reference in ["./ship-login.md", "./00042-ship-login.txt", "042", ""] {
        assert_eq!(
            op_task::ref_id(not_a_reference),
            None,
            "{not_a_reference:?}"
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
fn ref_target_splits_off_the_section() {
    assert_eq!(op_task::ref_target("./00042-a.md"), "./00042-a.md");
    assert_eq!(op_task::ref_target("./00042-a.md#Design"), "./00042-a.md");
    assert_eq!(op_task::ref_target("42#A#B"), "42");
}
