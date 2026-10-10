use op_task::config::{self, Config, Header};
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
        Document::of("docs/doc-store.md"),
        Document::Doc("doc-store".to_owned())
    );
    assert_eq!(
        Document::of("docs/Doc Store.md"),
        Document::Other("docs/Doc Store.md".to_owned())
    );
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
    assert_eq!(layout::doc_path("doc-store"), "docs/doc-store.md");
    assert_eq!(layout::doc_name("docs/doc-store.md"), Some("doc-store"));
    assert_eq!(layout::doc_name("docs/a/b.md"), None);
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

#[test]
fn a_config_without_a_format_is_format_one() {
    let header = Header::parse("abbreviation = \"OPP\"\n").expect("header");
    assert_eq!(
        header,
        Header {
            format: 1,
            requires: None
        }
    );
    assert!(Header::parse("format = 0").is_err());
    assert!(Header::parse("format = \"two\"").is_err());
    assert!(Header::parse("requires = 9").is_err());
}

#[test]
fn a_header_reads_from_a_config_whose_other_keys_it_does_not_know() {
    let text = "format = 7\nrequires = \"2.0.0\"\nproject_code = \"OPP\"\n";
    assert_eq!(
        Header::parse(text).expect("header"),
        Header {
            format: 7,
            requires: Some("2.0.0".to_owned())
        }
    );
    assert!(Config::parse(text).is_err());
}

#[test]
fn a_restamp_moves_only_the_header() {
    let header = Header {
        format: 2,
        requires: Some("0.0.9".to_owned()),
    };
    assert_eq!(
        config::restamp("abbreviation = \"OPP\"\n", &header).expect("restamp"),
        "format = 2\nrequires = \"0.0.9\"\nabbreviation = \"OPP\"\n"
    );
    let restamped = config::restamp("project_code = \"OPP\"\n", &header).expect("restamp");
    assert_eq!(
        Header::parse(&restamped).expect("header"),
        header,
        "{restamped}"
    );
    assert!(restamped.contains("project_code = \"OPP\""), "{restamped}");
}
