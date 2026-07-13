use op_md::{Target, headings, title};

#[test]
fn extracts_title_and_sections() {
    let body = "# My Task\n\nintro\n\n## Plan\n\n- a\n\n## Notes\n";
    assert_eq!(title(body).as_deref(), Some("My Task"));

    let sections: Vec<_> = headings(body)
        .into_iter()
        .filter(|h| h.level == 2)
        .map(|h| h.text)
        .collect();
    assert_eq!(sections, vec!["Plan", "Notes"]);
}

#[test]
fn parses_dotted_target() {
    let target = Target::parse("Plan.Design.1");
    assert_eq!(target.segments, vec!["Plan", "Design", "1"]);
}
