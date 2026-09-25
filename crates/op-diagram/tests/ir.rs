use op_diagram::Diagram;

#[test]
fn reads_and_writes() {
    insta::glob!("ir/*.json", |path| {
        let json = std::fs::read_to_string(path).unwrap();
        let diagram: Diagram = serde_json::from_str(&json)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        insta::assert_json_snapshot!(diagram);
    });
}
