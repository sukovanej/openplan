use op_task::{FieldError, PartialMetadata, Status, Timestamp, parse_partial};

fn fields(input: &str) -> op_task::PartialFrontmatter {
    match parse_partial(input).metadata {
        PartialMetadata::Fields(fields) => fields,
        PartialMetadata::Error(message) => panic!("expected recoverable fields, got {message}"),
    }
}

#[test]
fn a_well_formed_task_parses_every_field() {
    let parsed = parse_partial(
        "---\nstatus: in_progress\ncreated: 2026-01-01T00:00:00Z\nparent: epic\nrank: m\ndeps:\n  - a\n---\n# Title\n\nbody\n",
    );
    assert_eq!(parsed.title.as_deref(), Some("Title"));
    assert_eq!(parsed.body, "# Title\n\nbody\n");
    let fields = match parsed.metadata {
        PartialMetadata::Fields(fields) => fields,
        PartialMetadata::Error(message) => panic!("{message}"),
    };
    assert_eq!(fields.status, Ok(Status::InProgress));
    assert_eq!(
        fields.created,
        Ok("2026-01-01T00:00:00Z".parse::<Timestamp>().unwrap())
    );
    assert_eq!(fields.parent, Ok(Some("epic".to_owned())));
    assert_eq!(fields.rank, Ok(Some("m".to_owned())));
    assert_eq!(fields.deps, Ok(vec!["a".to_owned()]));
}

// The case the strict parser turns into a total failure: one absent field must not cost the reader
// the status, parent, or title that are sitting right there.
#[test]
fn a_missing_field_costs_only_itself() {
    let fields = fields("---\nstatus: in_progress\nparent: epic\n---\n# Legacy\n");
    assert_eq!(fields.created, Err(FieldError::Missing));
    assert_eq!(fields.status, Ok(Status::InProgress));
    assert_eq!(fields.parent, Ok(Some("epic".to_owned())));
    assert_eq!(fields.deps, Ok(Vec::new()));
}

#[test]
fn an_invalid_field_reports_why_and_spares_the_others() {
    let fields = fields("---\nstatus: nonsense\ncreated: not-a-date\nrank: m\n---\n# T\n");
    assert!(matches!(fields.status, Err(FieldError::Invalid(_))));
    assert!(matches!(fields.created, Err(FieldError::Invalid(_))));
    assert_eq!(fields.rank, Ok(Some("m".to_owned())));
}

#[test]
fn a_wrongly_typed_field_is_invalid_rather_than_ignored() {
    let fields = fields(
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: 7\ndeps: nope\n---\n# T\n",
    );
    assert!(matches!(fields.parent, Err(FieldError::Invalid(_))));
    assert!(matches!(fields.deps, Err(FieldError::Invalid(_))));
}

#[test]
fn unresolved_merge_markers_leave_no_field_recoverable() {
    let parsed = parse_partial(
        "---\n<<<<<<< HEAD\nstatus: todo\n=======\nstatus: done\n>>>>>>> other\n---\n# Head\n",
    );
    assert!(matches!(parsed.metadata, PartialMetadata::Error(_)));
    // The title still comes back, so the row can name itself even when its metadata cannot.
    assert_eq!(parsed.title.as_deref(), Some("Head"));
}

#[test]
fn a_file_without_a_fence_reports_that_and_keeps_its_text() {
    let parsed = parse_partial("no frontmatter here\n\n# Still titled\n");
    match parsed.metadata {
        PartialMetadata::Error(message) => assert!(message.contains("fence"), "{message}"),
        PartialMetadata::Fields(_) => panic!("a file with no fence has no fields"),
    }
    assert_eq!(parsed.title.as_deref(), Some("Still titled"));
    assert_eq!(parsed.body, "no frontmatter here\n\n# Still titled\n");
}
