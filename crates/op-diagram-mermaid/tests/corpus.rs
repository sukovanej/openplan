mod common;

use std::path::Path;

use common::{ends, graph, ids, node, parent, sequence};
use op_diagram::{Diagram, Head, ParticipantKind, SequenceItem, Shape, Stroke};
use op_diagram_mermaid::parse;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.mmd"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn every_fixture_parses_and_survives_json() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    names.sort();
    assert_eq!(names.len(), 9);
    for path in names {
        let source = std::fs::read_to_string(&path).unwrap();
        let diagram = parse(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let json = serde_json::to_string(&diagram).unwrap();
        let back: Diagram = serde_json::from_str(&json).unwrap();
        assert_eq!(back, diagram, "{}", path.display());
    }
}

#[test]
fn opp_115_is_a_chain_of_questions() {
    let graph = graph(&fixture("opp-115"));
    assert_eq!(graph.nodes.len(), 9);
    assert_eq!(graph.edges.len(), 8);
    assert_eq!(node(&graph, "root").shape, Shape::Diamond);
    assert_eq!(node(&graph, "root").label, ["--root given?"]);
    assert_eq!(
        node(&graph, "use_ref").label,
        ["this repository; sync with openplan.remote, else origin"]
    );
    assert_eq!(graph.edges[1].label, ["no"]);
}

#[test]
fn cqr_71_links_two_tables() {
    let graph = graph(&fixture("cqr-71"));
    assert_eq!(ids(&graph), ["sessions", "users"]);
    let Shape::Table { rows } = &node(&graph, "sessions").shape else {
        panic!("expected a table");
    };
    assert_eq!(rows[0].cells, ["BLOB", "token_hash", "PK", "32 bytes"]);
    assert_eq!(
        (graph.edges[0].tail, graph.edges[0].head),
        (Head::ZeroOrMore, Head::ExactlyOne)
    );
}

#[test]
fn cqr_73_report_starts_with_the_player() {
    let sequence = sequence(&fixture("cqr-73-report"));
    assert_eq!(sequence.participants.len(), 4);
    assert_eq!(sequence.participants[0].kind, ParticipantKind::Actor);
    assert_eq!(sequence.participants[2].label, ["Game server"]);
    let SequenceItem::Message(post) = &sequence.items[3] else {
        panic!("expected a message");
    };
    assert_eq!(
        post.text,
        ["POST /question-reports {match, question, problem_kind}"]
    );
}

#[test]
fn cqr_73_tables_draw_a_relationship_that_is_not_identifying() {
    let graph = graph(&fixture("cqr-73-tables"));
    assert_eq!(graph.edges[0].stroke, Stroke::Dotted);
    assert_eq!(graph.edges[0].label, ["no FK, separate store"]);
}

#[test]
fn cqr_74_decodes_the_title_of_its_subgraph() {
    let graph = graph(&fixture("cqr-74"));
    assert_eq!(graph.clusters[0].label, ["bank/<tag>/choice.json"]);
    assert_eq!(parent(&graph, "q"), Some("fixture"));
    assert_eq!(node(&graph, "q").label, ["{ id: 5432, text: [...], ... }"]);
    assert_eq!(node(&graph, "db").shape, Shape::Cylinder);
    assert!(ends(&graph).contains(&("fixture", "seed")));
}

#[test]
fn cqr_77_keeps_the_czech_text() {
    let sequence = sequence(&fixture("cqr-77"));
    assert_eq!(sequence.participants[0].label, ["Dívka"]);
    let SequenceItem::Message(first) = &sequence.items[0] else {
        panic!("expected a message");
    };
    assert_eq!((first.from.as_str(), first.to.as_str()), ("d", "d"));
    assert_eq!(sequence.items.len(), 9);
}

#[test]
fn cqr_89_groups_the_crates() {
    let graph = graph(&fixture("cqr-89"));
    assert_eq!(graph.clusters.len(), 3);
    assert_eq!(parent(&graph, "loop"), Some("standalone"));
    assert_eq!(parent(&graph, "serve"), Some("cli"));
    assert_eq!(
        ends(&graph),
        [("core", "standalone"), ("standalone", "cli")]
    );
}

#[test]
fn cqr_90_links_to_subgraphs_before_they_exist() {
    let graph = graph(&fixture("cqr-90"));
    let clusters: Vec<&str> = graph.clusters.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(clusters, ["worker", "match"]);
    assert!(!ids(&graph).contains(&"worker"));
    assert!(!ids(&graph).contains(&"match"));
    assert_eq!(graph.edges[0].label, ["HTTPS, WebSocket upgrade"]);
    assert_eq!(parent(&graph, "alarm"), Some("match"));
    assert_eq!(node(&graph, "d1").shape, Shape::Cylinder);
}

#[test]
fn opp_117_draws_its_own_pipeline() {
    let graph = graph(&fixture("opp-117"));
    assert_eq!(graph.nodes.len(), 8);
    assert_eq!(node(&graph, "ir").shape, Shape::Hexagon);
    assert_eq!(
        node(&graph, "render").label,
        ["op-diagram-render: measure, layout, SVG"]
    );
    let last = graph.edges.last().unwrap();
    assert_eq!(
        (last.from.as_str(), last.to.as_str()),
        ("render", "browser")
    );
    assert_eq!(last.label, ["SVG"]);
}
