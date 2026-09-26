use op_task::content::{TitleError, join, split, title};

fn round_trip(body: &str) -> String {
    join(body, &title(body).unwrap_or_default(), &split(body)).unwrap()
}

const IN_CONFLICT: &str =
    "<<<<<<< Ann (1111111)\n# Login\n=======\n# Sign in\n>>>>>>> Ben (2222222)\n\nBody.\n";

#[test]
fn the_description_is_the_body_below_the_title() {
    for (body, description) in [
        ("# T\n\nBody\n", "Body\n"),
        ("\n# T\nBody", "Body"),
        ("# T", ""),
        ("# T\n", ""),
        ("T\n===\n\nBody\n", "Body\n"),
        ("  # T  \n\n\nBody\n\n## Part\n", "Body\n\n## Part\n"),
        ("Intro.\n\n# T\n\nBody\n", "Intro.\n\n\nBody\n"),
        (IN_CONFLICT, IN_CONFLICT),
        ("No title.\n", "No title.\n"),
    ] {
        assert_eq!(split(body), description, "{body:?}");
    }
}

#[test]
fn a_body_joins_back_to_the_same_bytes() {
    for body in [
        "# T\n\nBody\n",
        "\n# T\nBody",
        "# T",
        "# T\n",
        "# T\n\n",
        "T\n===\n\nBody\n",
        "# T  \r\n\r\nBody\r\n",
        "  # T  \n\n\nBody\n\n## Part\n",
        "Intro.\n\n# T\n\nBody\n",
        IN_CONFLICT,
        "No title.\n",
    ] {
        assert_eq!(round_trip(body), body, "{body:?}");
    }
}

#[test]
fn a_heading_that_touches_a_conflict_block_is_no_title_line() {
    let body = "# T\n\nfoo\n<<<<<<< Ann\nbar\n=======\nbaz\n>>>>>>> Ben\n";

    assert_eq!(
        split(body),
        "foo\n<<<<<<< Ann\nbar\n=======\nbaz\n>>>>>>> Ben\n"
    );
    assert_eq!(round_trip(body), body);
}

#[test]
fn a_new_title_replaces_the_title_line_and_keeps_the_rest() {
    assert_eq!(
        join("# T\n\nBody\n", "U", "Body\n").unwrap(),
        "# U\n\nBody\n"
    );
    assert_eq!(
        join("T\n===\n\nBody\n", "U", "Body\n").unwrap(),
        "# U\n\nBody\n"
    );
    assert_eq!(join("# T", "U", "").unwrap(), "# U");
    assert_eq!(
        join("# T\r\nBody\r\n", "U", "Body\r\n").unwrap(),
        "# U\r\nBody\r\n"
    );
}

#[test]
fn a_new_description_keeps_the_title_line_and_the_gap() {
    assert_eq!(
        join("  # T  \n\n\nBody\n", "T", "Other\n").unwrap(),
        "  # T  \n\n\nOther\n"
    );
    assert_eq!(join("# T\nBody\n", "T", "Other\n").unwrap(), "# T\nOther\n");
    assert_eq!(
        join("\n# T\n\nBody\n", "U", "Other\n").unwrap(),
        "\n# U\n\nOther\n"
    );
    assert_eq!(join("# T\n\nBody\n", "T", "").unwrap(), "# T\n");
}

#[test]
fn a_first_description_follows_one_blank_line() {
    for template in ["# T", "# T\n", "# T\n\n\n"] {
        assert_eq!(
            join(template, "T", "Body\n").unwrap(),
            "# T\n\nBody\n",
            "{template:?}"
        );
    }
    assert_eq!(join("T\n===", "T", "Body").unwrap(), "T\n===\n\nBody");
}

#[test]
fn text_before_the_title_moves_below_it_when_the_description_changes() {
    assert_eq!(
        join("Intro.\n\n# T\n\nBody\n", "T", "Intro.\n\nOther\n").unwrap(),
        "# T\n\nIntro.\n\nOther\n"
    );
}

#[test]
fn a_title_inside_a_conflict_block_keeps_the_body_and_refuses_a_new_title() {
    let edited = IN_CONFLICT.replace("Body.", "More.");

    assert_eq!(join(IN_CONFLICT, "Sign in", &edited).unwrap(), edited);
    assert_eq!(
        join(IN_CONFLICT, "Log in", &edited),
        Err(TitleError::InConflict)
    );
}

#[test]
fn a_body_with_no_title_takes_the_new_one_first() {
    assert_eq!(
        join("No title.\n", "T", "No title.\n").unwrap(),
        "# T\n\nNo title.\n"
    );
}

#[test]
fn a_title_must_be_one_non_empty_line() {
    assert_eq!(join("# T\n", "", ""), Err(TitleError::Empty));
    assert_eq!(join("# T\n", "  ", ""), Err(TitleError::Empty));
    assert_eq!(join("# T\n", "A\nB", ""), Err(TitleError::LineBreak));
    assert_eq!(join("# T\n", "A\rB", ""), Err(TitleError::LineBreak));
}
