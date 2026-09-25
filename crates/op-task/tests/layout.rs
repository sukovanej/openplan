use op_task::config::Config;
use op_task::layout::{self, Document};

#[test]
fn paths_name_their_documents() {
    assert_eq!(Document::of("config.toml"), Document::Config);
    assert_eq!(
        Document::of("tasks/00042-write-the-parser.md"),
        Document::Task(42)
    );
    assert_eq!(Document::of("tasks/123456-big.md"), Document::Task(123_456));
    assert_eq!(Document::of("tags/bug.md"), Document::Tag("bug".to_owned()));
    assert_eq!(
        Document::of("assets/a/b.png"),
        Document::Asset("a/b.png".to_owned())
    );
    assert_eq!(
        Document::of("tasks/notes/x.md"),
        Document::Other("tasks/notes/x.md".to_owned())
    );
    assert_eq!(
        Document::of("tasks/readme.md"),
        Document::Other("tasks/readme.md".to_owned())
    );
}

#[test]
fn a_task_path_carries_its_number_and_title() {
    assert_eq!(
        layout::task_path(42, "Write the parser"),
        "tasks/00042-write-the-parser.md"
    );
    assert_eq!(layout::task_prefix(42), "tasks/00042-");
    assert!(layout::task_path(42, "Other title").starts_with(&layout::task_prefix(42)));
    assert!(!layout::task_path(420, "x").starts_with(&layout::task_prefix(42)));
    assert_eq!(layout::tag_path("bug"), "tags/bug.md");
    assert_eq!(layout::tag_name("tags/bug.md"), Some("bug"));
}

#[test]
fn the_config_round_trips() {
    let config = Config::parse("abbreviation = \"OPP\"\n").expect("config");
    assert_eq!(config.abbreviation.as_str(), "OPP");
    assert_eq!(
        Config::parse(&config.to_file_string()).expect("config"),
        config
    );
    assert!(Config::parse("").is_err());
    assert!(Config::parse("abbreviation = \"op\"").is_err());
}
