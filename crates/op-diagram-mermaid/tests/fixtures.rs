use op_diagram::Diagram;
use op_diagram_mermaid::parse;

#[test]
fn parses() {
    insta::glob!("fixtures/*.mmd", |path| {
        let source = std::fs::read_to_string(path).unwrap();
        let diagram = parse(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let json = serde_json::to_string(&diagram).unwrap();
        let back: Diagram = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back,
            diagram,
            "{} changes on its way through JSON",
            path.display()
        );
        insta::assert_json_snapshot!(diagram);
    });
}
