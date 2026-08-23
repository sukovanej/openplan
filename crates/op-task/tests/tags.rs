use op_task::{FieldError, PartialMetadata, Status, Task, parse_partial};

fn task(frontmatter: &str) -> Task {
    Task::from_file_string(&format!(
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n{frontmatter}---\n# Title\n\nbody\n"
    ))
    .unwrap()
}

fn fields(input: &str) -> op_task::PartialFrontmatter {
    match parse_partial(input).metadata {
        PartialMetadata::Fields(fields) => fields,
        PartialMetadata::Error(message) => panic!("expected recoverable fields, got {message}"),
    }
}

#[test]
fn tags_are_sorted_and_deduped_on_write() {
    let mut parsed = task("");
    parsed.set_tags(vec![
        "wip".to_owned(),
        "backend".to_owned(),
        "wip".to_owned(),
    ]);

    let written = parsed.to_file_string().unwrap();
    assert!(written.contains("tags:\n- backend\n- wip\n"), "{written}");
    assert_eq!(
        Task::from_file_string(&written).unwrap().frontmatter.tags,
        vec!["backend".to_owned(), "wip".to_owned()]
    );
}

#[test]
fn an_empty_set_omits_the_field() {
    let mut parsed = task("tags:\n- backend\n");
    parsed.set_tags(Vec::new());
    assert!(!parsed.to_file_string().unwrap().contains("tags"));
}

#[test]
fn writing_tags_leaves_the_body_byte_for_byte() {
    let body = "# Title\n\n  ragged   spacing\n\n```yaml\ntags: [not-frontmatter]\n```\n";
    let text = format!("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n{body}");
    let mut parsed = Task::from_file_string(&text).unwrap();
    parsed.set_tags(vec!["backend".to_owned()]);

    let written = parsed.to_file_string().unwrap();
    assert!(written.ends_with(body), "{written}");
}

// `tags:` used to ride along in `extra`. A file written before the field existed must still read,
// keep its tags, and keep every neighbouring key.
#[test]
fn a_file_written_before_the_field_existed_roundtrips() {
    let text = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- wip\n- backend\n- wip\nestimate: 3.5\n---\n# Title\n";
    let mut parsed = Task::from_file_string(text).unwrap();
    assert_eq!(
        parsed.frontmatter.tags,
        vec!["backend".to_owned(), "wip".to_owned()]
    );

    parsed.set_status(Status::Done);
    let written = parsed.to_file_string().unwrap();
    assert!(written.contains("tags:\n- backend\n- wip\n"), "{written}");
    assert!(written.contains("estimate: 3.5"), "{written}");
    assert_eq!(
        Task::from_file_string(&written).unwrap().frontmatter,
        parsed.frontmatter
    );
}

#[test]
fn a_tag_name_is_carried_as_written() {
    let parsed = task("tags:\n- gone-from-this-branch\n");
    assert_eq!(
        parsed.frontmatter.tags,
        vec!["gone-from-this-branch".to_owned()]
    );
}

// A name a human wrote unquoted still names a tag; only an entry with no name at all is a defect.
#[test]
fn an_unquoted_scalar_names_a_tag_and_a_nested_entry_does_not() {
    assert_eq!(
        task("tags:\n- 7\n- true\n").frontmatter.tags,
        vec!["7".to_owned(), "true".to_owned()]
    );
    assert!(
        Task::from_file_string(
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- [backend]\n---\n# Title\n"
        )
        .is_err()
    );
    assert!(matches!(
        fields("---\nstatus: todo\ntags:\n- [backend]\n---\n# T\n").tags,
        Err(FieldError::Invalid(_))
    ));
}

#[test]
fn the_lenient_path_sorts_tags_and_reports_a_bad_list_alone() {
    let sorted = fields("---\nstatus: todo\ntags:\n- wip\n- backend\n---\n# Title\n");
    assert_eq!(
        sorted.tags,
        Ok(vec!["backend".to_owned(), "wip".to_owned()])
    );

    let not_a_list = fields("---\nstatus: todo\ntags: backend\n---\n# Title\n");
    assert!(matches!(not_a_list.tags, Err(FieldError::Invalid(_))));
    assert_eq!(not_a_list.status, Ok(Status::Todo));

    let absent = fields("---\nstatus: todo\n---\n# Title\n");
    assert_eq!(absent.tags, Ok(Vec::new()));
}
