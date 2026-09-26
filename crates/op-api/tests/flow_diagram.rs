use op_api::Flow;

#[test]
fn diagrams() {
    insta::glob!("flow_diagrams/*.json", |path| {
        let json = std::fs::read_to_string(path).unwrap();
        let flow: Flow = serde_json::from_str(&json).unwrap();
        insta::assert_json_snapshot!(flow.diagram(None));
    });
}
