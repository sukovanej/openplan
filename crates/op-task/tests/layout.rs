use op_task::config::{self, Config};
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
fn a_config_without_a_version_is_the_first_store_version() {
    assert_eq!(
        config::version("abbreviation = \"OPP\"\n").expect("version"),
        config::FIRST_VERSION
    );
    assert_eq!(config::FIRST_VERSION.to_string(), "0.0.1");
    assert!(config::version("version = \"two\"").is_err());
    assert!(config::version("version = 2").is_err());
}

#[test]
fn the_version_reads_from_a_config_whose_other_keys_it_does_not_know() {
    let text = "version = \"0.3.0\"\nproject_code = \"OPP\"\n";
    assert_eq!(config::version(text).expect("version").to_string(), "0.3.0");
    assert!(Config::parse(text).is_err());
}

#[test]
fn a_restamp_moves_only_the_version() {
    let version = semver::Version::new(0, 0, 2);
    assert_eq!(
        config::restamp("abbreviation = \"OPP\"\n", &version).expect("restamp"),
        "version = \"0.0.2\"\nabbreviation = \"OPP\"\n"
    );
    let restamped = config::restamp("project_code = \"OPP\"\n", &version).expect("restamp");
    assert_eq!(
        config::version(&restamped).expect("version"),
        version,
        "{restamped}"
    );
    assert!(restamped.contains("project_code = \"OPP\""), "{restamped}");
}
