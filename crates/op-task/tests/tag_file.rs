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

#[test]
fn a_rename_takes_the_name_and_the_display_name_from_one_string() {
    let mut tag = Tag::new("Backend", Some(Color::Teal)).unwrap();
    tag.set_description("Work behind the API.");

    tag.rename("  Infra   Team  ").unwrap();

    assert_eq!(tag.name, "infra-team");
    assert_eq!(tag.display_name().as_deref(), Some("Infra Team"));
    assert_eq!(tag.body, "# Infra Team\n\nWork behind the API.\n");
    assert_eq!(tag.color(), Color::Teal);
}

#[test]
fn a_rename_replaces_a_setext_heading_whole() {
    let mut tag =
        Tag::from_file_string("backend".to_owned(), "---\n---\nBackend\n=======\n\nWhy.\n")
            .unwrap();

    tag.rename("infra").unwrap();

    assert_eq!(tag.body, "# infra\n\nWhy.\n");
}

#[test]
fn a_rename_of_a_headingless_tag_writes_the_display_name_in() {
    let mut tag = Tag::from_file_string("backend".to_owned(), "---\n---\n").unwrap();

    tag.rename("Infra").unwrap();

    assert_eq!(tag.body, "# Infra\n");
    assert_eq!(tag.display_name().as_deref(), Some("Infra"));
}

#[test]
fn a_rename_refuses_a_name_normalization_cannot_reach() {
    let mut tag = Tag::new("Backend", None).unwrap();
    assert!(tag.rename("C++").is_err());
    assert_eq!(tag.name, "backend", "a refused rename changes nothing");
}

#[test]
fn the_description_is_what_stands_below_the_display_name() {
    let tag = Tag::from_file_string(
        "backend".to_owned(),
        "---\ncolor: teal\n---\n# Backend\n\nWork behind the API.\n\n- `op-store`\n",
    )
    .unwrap();

    assert_eq!(tag.description(), "Work behind the API.\n\n- `op-store`");
}

#[test]
fn a_tag_with_no_prose_has_no_description() {
    let tag = Tag::new("Backend", None).unwrap();
    assert_eq!(tag.description(), "");
}

#[test]
fn setting_a_description_replaces_the_old_one_and_keeps_the_heading() {
    let mut tag = Tag::new("Backend", Some(Color::Teal)).unwrap();
    tag.set_description("Work behind the API.");
    assert_eq!(tag.body, "# Backend\n\nWork behind the API.\n");

    tag.set_description("Everything the SPA calls.");
    assert_eq!(tag.body, "# Backend\n\nEverything the SPA calls.\n");
    assert_eq!(tag.display_name().as_deref(), Some("Backend"));
}

#[test]
fn an_empty_description_leaves_the_display_name_alone() {
    let mut tag = Tag::new("Backend", Some(Color::Teal)).unwrap();
    tag.set_description("Work behind the API.");

    tag.set_description("");

    assert_eq!(tag.body, "# Backend\n");
    assert_eq!(tag.description(), "");
}

#[test]
fn the_default_tags_carry_a_normalized_name_a_color_and_a_description() {
    let names: Vec<_> = op_task::tag::defaults()
        .into_iter()
        .map(|tag| {
            assert!(tag.frontmatter.color.is_some(), "{} has no color", tag.name);
            assert!(
                !tag.description().is_empty(),
                "{} has no description",
                tag.name
            );
            assert!(tag.display_name().is_some(), "{} has no heading", tag.name);
            tag.name
        })
        .collect();
    assert_eq!(names, vec!["bug", "feature", "draft"]);
}
