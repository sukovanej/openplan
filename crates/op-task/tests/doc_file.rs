use op_task::doc::{
    Doc, DocError, PartialDocMetadata, body_doc_names, joined, normalize_name, parse_partial,
    rewrite_parent,
};
use op_task::{Abbreviation, Timestamp};

fn at() -> Timestamp {
    "2026-09-14T10:00:00Z".parse().unwrap()
}

fn abbreviation() -> Abbreviation {
    "OPP".parse().unwrap()
}

#[test]
fn a_new_doc_takes_its_name_from_the_title() {
    let doc = Doc::new("System Architecture", at()).unwrap();
    assert_eq!(doc.name, "system-architecture");
    assert_eq!(doc.title().as_deref(), Some("System Architecture"));
}

#[test]
fn a_name_the_normalizer_cannot_spell_is_refused() {
    assert!(Doc::new("C++", at()).is_err());
    assert!(normalize_name("  Release  Notes ").is_ok());
    assert_eq!(normalize_name("Release Notes").unwrap(), "release-notes");
}

#[test]
fn content_round_trips_below_the_title() {
    let mut doc = Doc::new("Architecture", at()).unwrap();
    doc.set_content("The store is a directory of markdown files.\n");
    assert_eq!(
        doc.content(),
        "The store is a directory of markdown files.\n"
    );
    let text = doc.to_file_string().unwrap();
    let read = Doc::from_file_string("architecture".to_owned(), &text).unwrap();
    assert_eq!(read, doc);
}

#[test]
fn a_rename_moves_the_title_heading_with_the_name() {
    let mut doc = Doc::new("Architecture", at()).unwrap();
    doc.set_content("Body stays.");
    doc.rename("Storage Layout").unwrap();
    assert_eq!(doc.name, "storage-layout");
    assert_eq!(doc.title().as_deref(), Some("Storage Layout"));
    assert_eq!(doc.content(), "Body stays.\n");
}

#[test]
fn a_file_without_created_is_named_as_the_reason_it_cannot_be_read() {
    let err = Doc::from_file_string("x".to_owned(), "---\nother: 1\n---\n# X\n").unwrap_err();
    assert!(matches!(err, DocError::MissingCreated));
}

#[test]
fn a_partial_read_yields_the_title_of_a_file_the_model_would_reject() {
    let partial = parse_partial("# Loose Notes\n\nno frontmatter at all\n");
    assert_eq!(partial.title.as_deref(), Some("Loose Notes"));
    assert!(partial.created().is_err());
}

#[test]
fn a_body_names_docs_by_their_path_or_their_name_and_tasks_by_their_key() {
    let names = body_doc_names(
        Some(abbreviation()),
        "docs",
        "see [[./architecture.md]], [[OPP-42]], [[storage#Layout]] and [[Some Title]]",
    );
    assert_eq!(names, vec!["architecture".to_owned(), "storage".to_owned()]);
}

// A path is read against the directory of the file that holds it, so the same spelling names a doc
// from a doc and a task from a task.
#[test]
fn a_path_names_a_doc_only_when_it_resolves_into_the_docs() {
    let body = "see [[./00042-notes.md]] and [[../docs/design.md]]";
    assert_eq!(
        body_doc_names(Some(abbreviation()), "docs", body),
        vec!["00042-notes".to_owned(), "design".to_owned()]
    );
    assert_eq!(
        body_doc_names(Some(abbreviation()), "tasks", body),
        vec!["design".to_owned()]
    );
}

// A bare number is how the file layer spells a task, and a write refuses a body carrying one, so
// reading it as a doc would name a doc nobody could have written.
#[test]
fn bare_digits_name_no_doc() {
    assert!(body_doc_names(Some(abbreviation()), "docs", "see [[42]] and [[042]]").is_empty());
}

#[test]
fn a_parent_is_stored_as_the_path_of_its_file() {
    let mut doc = Doc::new("Storage", at()).unwrap();
    doc.set_parent(Some("System Architecture")).unwrap();
    let text = doc.to_file_string().unwrap();
    assert!(text.contains("parent: ./system-architecture.md"));
    let read = Doc::from_file_string("storage".to_owned(), &text).unwrap();
    assert_eq!(
        read.frontmatter.parent.as_deref(),
        Some("system-architecture")
    );
}

#[test]
fn a_doc_with_no_parent_writes_no_parent_key() {
    let doc = Doc::new("Storage", at()).unwrap();
    assert!(!doc.to_file_string().unwrap().contains("parent"));
}

#[test]
fn a_partial_read_flags_a_parent_that_is_not_the_path_of_a_doc() {
    let good =
        parse_partial("---\ncreated: 2026-01-01T00:00:00Z\nparent: ./architecture.md\n---\n# S\n");
    assert_eq!(good.parent(), Some("architecture"));
    for bad in ["C++", "architecture", "../tasks/00042-x.md"] {
        let partial = parse_partial(&format!(
            "---\ncreated: 2026-01-01T00:00:00Z\nparent: {bad}\n---\n# S\n"
        ));
        assert!(
            matches!(partial.metadata, PartialDocMetadata::Fields(ref f) if f.parent.is_err()),
            "{bad}"
        );
    }
}

#[test]
fn a_parent_by_name_is_refused_by_the_model() {
    let text = "---\ncreated: 2026-01-01T00:00:00Z\nparent: architecture\n---\n# S\n";
    assert!(matches!(
        Doc::from_file_string("s".to_owned(), text),
        Err(DocError::Parent(_))
    ));
}

// A child may be a file the model rejects, and a rename still has to carry it.
#[test]
fn a_parent_swap_reaches_a_file_the_model_would_reject() {
    let broken = "---\nparent: ./architecture.md\n---\n# Storage\n";
    let moved = rewrite_parent(broken, "architecture", Some("the-design")).unwrap();
    assert!(moved.contains("parent: ./the-design.md"));
    assert!(moved.contains("# Storage"));
    assert!(Doc::from_file_string("storage".to_owned(), broken).is_err());
}

#[test]
fn a_file_naming_no_parent_is_left_alone_by_a_swap() {
    assert!(rewrite_parent("no frontmatter at all\n", "a", Some("b")).is_none());
    assert!(
        rewrite_parent(
            "---\ncreated: 2026-01-01T00:00:00Z\n---\n# S\n",
            "a",
            Some("b")
        )
        .is_none()
    );
    assert!(rewrite_parent("---\nparent: ./other.md\n---\n# S\n", "a", Some("b")).is_none());
}

#[test]
fn a_file_with_no_frontmatter_fence_says_so_rather_than_losing_its_body() {
    let partial = parse_partial("# Loose\n\nprose\n");
    assert!(matches!(partial.metadata, PartialDocMetadata::Error(_)));
    assert_eq!(partial.title.as_deref(), Some("Loose"));
    assert!(partial.body.contains("prose"));
}

#[test]
fn a_joined_text_that_changes_nothing_is_the_current_body() {
    for current in [
        "# Storage\n\nKeep it in git.\n",
        "# Storage\nKeep it in git.\n",
        "# Storage\n",
        "Intro.\n\n# Details\n",
    ] {
        let title = op_task::doc::title_of(current).unwrap_or_default();
        let content = op_task::doc::content(current);
        assert_eq!(joined(current, &title, &content), current, "{current:?}");
    }
}

#[test]
fn a_joined_text_puts_a_new_title_and_content_in_the_layout_of_the_current_body() {
    assert_eq!(
        joined(
            "# Storage\nKeep it in git.\n",
            "Store",
            "Keep it in SQLite.\n"
        ),
        "# Store\nKeep it in SQLite.\n"
    );
    assert_eq!(
        joined("# Storage\n", "Storage", "New text.\n"),
        "# Storage\n\nNew text.\n"
    );
    assert_eq!(joined("", "Storage", ""), "# Storage\n");
}

// Only a `# ` heading that opens the body is the title. One further down is a section, and the
// text above it stays in the body.
#[test]
fn a_doc_that_opens_with_prose_has_no_title_and_keeps_all_of_its_text() {
    let text = "---\ncreated: 2026-01-01T00:00:00Z\n---\nAn intro.\n\n# Details\n\nMore.\n";
    let doc = Doc::from_file_string("notes".to_owned(), text).unwrap();

    assert_eq!(doc.title(), None);
    assert_eq!(doc.content(), "An intro.\n\n# Details\n\nMore.\n");
    assert_eq!(parse_partial(text).title, None);
}

#[test]
fn a_parent_that_moves_to_none_leaves_the_child_at_the_top_level() {
    let child = "---\ncreated: 2026-01-01T00:00:00Z\nparent: ./architecture.md\n---\n# Storage\n";

    let moved = rewrite_parent(child, "architecture", None).unwrap();

    let doc = Doc::from_file_string("storage".to_owned(), &moved).unwrap();
    assert_eq!(doc.frontmatter.parent, None);
    assert_eq!(doc.content(), "");
}
