use op_task::tag::{Color, Tag, TagError};

#[test]
fn a_new_tag_normalizes_its_name_and_keeps_the_display_case() {
    let tag = Tag::new("Front End", Some(Color::Teal)).unwrap();
    assert_eq!(tag.name, "front-end");
    assert_eq!(tag.display_name().as_deref(), Some("Front End"));
    assert_eq!(tag.color(), Color::Teal);
    assert_eq!(
        tag.to_file_string().unwrap(),
        "---\ncolor: teal\n---\n# Front End\n"
    );
}

// The display name becomes an H1, and an H1 is one line.
#[test]
fn a_new_tag_folds_ragged_whitespace_out_of_the_display_name() {
    let tag = Tag::new("  Front \n  End  ", None).unwrap();
    assert_eq!(tag.name, "front-end");
    assert_eq!(tag.body, "# Front End\n");
    assert_eq!(tag.display_name().as_deref(), Some("Front End"));
}

#[test]
fn a_new_tag_without_a_color_materializes_the_derived_one() {
    let tag = Tag::new("Backend", None).unwrap();
    assert_eq!(tag.frontmatter.color, Some(Color::for_name("backend")));
    assert!(tag.to_file_string().unwrap().contains("color: "));
}

#[test]
fn a_new_tag_refuses_a_name_normalization_cannot_reach() {
    assert!(Tag::new("C++", None).is_err());
}

#[test]
fn the_body_survives_a_recolor_byte_for_byte() {
    let body = "# Back End\n\nWork that lives behind the API.\n\n- `op-store`\n- `op-server`\n";
    let text = format!("---\ncolor: red\nowner: milan\n---\n{body}");
    let mut tag = Tag::from_file_string("back-end".to_owned(), &text).unwrap();
    tag.set_color(Color::Blue);

    let written = tag.to_file_string().unwrap();
    assert_eq!(
        written,
        format!("---\ncolor: blue\nowner: milan\n---\n{body}")
    );
    assert_eq!(
        Tag::from_file_string("back-end".to_owned(), &written).unwrap(),
        tag
    );
}

#[test]
fn a_missing_color_falls_back_to_the_derived_one_and_stays_missing() {
    let text = "---\ncreated: 2026-01-01T00:00:00Z\n---\n# Backend\n";
    let tag = Tag::from_file_string("backend".to_owned(), text).unwrap();

    assert_eq!(tag.frontmatter.color, None);
    assert_eq!(tag.color(), Color::for_name("backend"));
    assert_eq!(tag.to_file_string().unwrap(), text);
}

#[test]
fn a_description_appends_below_the_display_name() {
    let mut tag = Tag::new("Backend", Some(Color::Green)).unwrap();
    tag.append_body("Work behind the API.");
    assert_eq!(tag.body, "# Backend\n\nWork behind the API.\n");
}

#[test]
fn a_file_without_a_fence_is_refused() {
    assert!(matches!(
        Tag::from_file_string("backend".to_owned(), "# Backend\n"),
        Err(TagError::MissingFrontmatter)
    ));
}

#[test]
fn an_unknown_color_in_a_file_is_refused() {
    assert!(matches!(
        Tag::from_file_string(
            "backend".to_owned(),
            "---\ncolor: fuchsia\n---\n# Backend\n"
        ),
        Err(TagError::Frontmatter(_))
    ));
}
