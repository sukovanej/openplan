use std::str::FromStr;

use op_task::tag::Color;

#[test]
fn every_color_roundtrips_through_str() {
    for color in Color::ALL {
        assert_eq!(Color::from_str(color.as_str()).ok(), Some(color));
    }
}

#[test]
fn unknown_color_is_rejected_and_lists_the_palette() {
    let error = Color::from_str("#ff0000").unwrap_err().to_string();
    assert!(error.contains("\"#ff0000\""), "{error}");
    for color in Color::ALL {
        assert!(error.contains(color.as_str()), "{error}");
    }
}

#[test]
fn palette_names_are_unique() {
    let mut names: Vec<&str> = Color::ALL.iter().map(Color::as_str).collect();
    names.sort_unstable();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count);
}

#[test]
fn the_default_color_is_stable_for_a_name() {
    assert_eq!(Color::for_name("backend"), Color::for_name("backend"));
    assert_eq!(
        [
            Color::for_name("backend"),
            Color::for_name("frontend"),
            Color::for_name("wip"),
            Color::for_name("docs"),
            Color::for_name("2026-q1"),
        ]
        .map(|c| c.as_str()),
        ["amber", "red", "cyan", "violet", "slate"]
    );
}

#[test]
fn the_default_color_spreads_across_the_palette() {
    let mut used: Vec<&str> = (0..200)
        .map(|n| Color::for_name(&format!("tag-{n}")).as_str())
        .collect();
    used.sort_unstable();
    used.dedup();
    assert_eq!(used.len(), Color::ALL.len());
}
