use op_md::{append_under, headings};

#[test]
fn ignores_a_heading_inside_a_blockquote() {
    let body = "# Task\n\n## Comments\n\n### 2026-01-01T00:00:00Z by A\n\n> # Not a section\n>\n> ## Nor this\n";
    let found: Vec<(u8, String)> = headings(body)
        .into_iter()
        .map(|h| (h.level, h.text))
        .collect();
    assert_eq!(
        found,
        vec![
            (1, "Task".to_owned()),
            (2, "Comments".to_owned()),
            (3, "2026-01-01T00:00:00Z by A".to_owned()),
        ]
    );
}

#[test]
fn creates_the_section_when_it_is_absent() {
    let body = "# Task\n\nintro\n";
    assert_eq!(
        append_under(body, 2, "Comments", "block"),
        "# Task\n\nintro\n\n## Comments\n\nblock\n"
    );
}

#[test]
fn appends_at_the_end_of_an_existing_section() {
    let body = "# Task\n\n## Comments\n\nfirst\n";
    assert_eq!(
        append_under(body, 2, "Comments", "second"),
        "# Task\n\n## Comments\n\nfirst\n\nsecond\n"
    );
}

#[test]
fn keeps_the_sections_that_follow() {
    let body = "# Task\n\n## Plan\n\na\n\n## Notes\n\nb\n";
    assert_eq!(
        append_under(body, 2, "Plan", "c"),
        "# Task\n\n## Plan\n\na\n\nc\n\n## Notes\n\nb\n"
    );
}

#[test]
fn a_quoted_heading_does_not_end_a_section() {
    let body = "# Task\n\n## Comments\n\n> ## quoted\n";
    assert_eq!(
        append_under(body, 2, "Comments", "next"),
        "# Task\n\n## Comments\n\n> ## quoted\n\nnext\n"
    );
}
