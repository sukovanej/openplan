mod common;

use common::{ends, error, graph, ids, node};
use op_diagram::{Direction, Head, Row, Shape, Stroke};

fn rows(shape: &Shape) -> Vec<Vec<&str>> {
    match shape {
        Shape::Table { rows } => rows
            .iter()
            .map(|row| row.cells.iter().map(String::as_str).collect())
            .collect(),
        other => panic!("expected a table, got {other:?}"),
    }
}

#[test]
fn an_entity_is_a_table_with_a_row_for_each_attribute() {
    let graph = graph(
        "erDiagram
  CUSTOMER {
    string name
    string id PK
    int address_id FK \"where it ships\"
    string email UK
    string code PK, FK
    string note \"free text\"
    varchar(255) title
    string[] tags
  }",
    );
    assert_eq!(graph.direction, Direction::Down);
    assert_eq!(node(&graph, "CUSTOMER").label, ["CUSTOMER"]);
    assert_eq!(
        rows(&node(&graph, "CUSTOMER").shape),
        [
            vec!["string", "name"],
            vec!["string", "id", "PK"],
            vec!["int", "address_id", "FK", "where it ships"],
            vec!["string", "email", "UK"],
            vec!["string", "code", "PK, FK"],
            vec!["string", "note", "", "free text"],
            vec!["varchar(255)", "title"],
            vec!["string[]", "tags"],
        ]
    );
}

#[test]
fn reads_each_cardinality_on_each_side() {
    for (relationship, tail, stroke, head) in [
        ("||--||", Head::ExactlyOne, Stroke::Solid, Head::ExactlyOne),
        ("|o--o|", Head::ZeroOrOne, Stroke::Solid, Head::ZeroOrOne),
        ("}o--o{", Head::ZeroOrMore, Stroke::Solid, Head::ZeroOrMore),
        ("}|--|{", Head::OneOrMore, Stroke::Solid, Head::OneOrMore),
        ("||..o{", Head::ExactlyOne, Stroke::Dotted, Head::ZeroOrMore),
    ] {
        let graph = graph(&format!("erDiagram\n  A {relationship} B : has"));
        let edge = &graph.edges[0];
        assert_eq!(
            (edge.tail, edge.stroke, edge.head, edge.min_length),
            (tail, stroke, head, 1),
            "{relationship}"
        );
        assert_eq!(edge.label, ["has"], "{relationship}");
    }
}

#[test]
fn an_entity_named_only_in_a_relationship_is_an_empty_table() {
    let graph = graph("erDiagram\n  CUSTOMER ||--o{ ORDER : places");
    assert_eq!(ids(&graph), ["CUSTOMER", "ORDER"]);
    assert_eq!(ends(&graph), [("CUSTOMER", "ORDER")]);
    assert_eq!(node(&graph, "ORDER").shape, Shape::Table { rows: vec![] });
}

#[test]
fn a_relationship_needs_no_spaces_around_its_symbols() {
    let graph = graph("erDiagram\n  CUSTOMER||--o{ORDER:places");
    assert_eq!(ends(&graph), [("CUSTOMER", "ORDER")]);
}

#[test]
fn quotes_hold_names_and_labels_with_spaces() {
    let graph = graph("erDiagram\n  \"line item\" }|..|| \"order\" : \"belongs to\"");
    assert_eq!(ends(&graph), [("line item", "order")]);
    assert_eq!(graph.edges[0].label, ["belongs to"]);
}

#[test]
fn a_second_block_adds_rows_to_the_same_entity() {
    let graph = graph(
        "erDiagram
  A ||--|| B : is
  A {
    int one
  }
  A {
    int two
  }",
    );
    assert_eq!(ids(&graph), ["A", "B"]);
    assert_eq!(
        rows(&node(&graph, "A").shape),
        [vec!["int", "one"], vec!["int", "two"]]
    );
}

#[test]
fn a_block_may_close_on_the_line_of_its_attribute() {
    let graph = graph("erDiagram\n  A { int one }\n  B");
    assert_eq!(ids(&graph), ["A", "B"]);
    assert_eq!(
        node(&graph, "A").shape,
        Shape::Table {
            rows: vec![Row {
                cells: vec!["int".into(), "one".into()]
            }]
        }
    );
}

#[test]
fn reads_a_direction() {
    assert_eq!(
        graph("erDiagram\n  direction LR\n  A ||--|| B : is").direction,
        Direction::Right
    );
}

#[test]
fn refuses_a_label_with_spaces_and_no_quotes() {
    assert_eq!(
        error("erDiagram\n  A ||--|| B : is part of").message,
        "put a label with spaces in quotes"
    );
}

#[test]
fn refuses_a_relationship_without_a_label() {
    assert_eq!(
        error("erDiagram\n  A ||--|| B").message,
        "expected `:` and the label of the relationship"
    );
}

#[test]
fn refuses_a_cardinality_it_does_not_know() {
    let refused = error("erDiagram\n  A |x--o{ B : has");
    assert_eq!((refused.line, refused.column), (2, 5));
    assert!(
        refused
            .message
            .starts_with("expected a relationship such as `||--o{`")
    );
}

#[test]
fn refuses_a_key_it_does_not_know() {
    assert_eq!(
        error("erDiagram\n  A {\n    int id XK\n  }").message,
        "`XK` is not a key; use `PK`, `FK`, or `UK`"
    );
}

#[test]
fn refuses_an_attribute_with_no_name() {
    let refused = error("erDiagram\n  A {\n    int\n  }");
    assert_eq!(refused.line, 3);
    assert_eq!(
        refused.message,
        "expected the name of the attribute after its type"
    );
}

#[test]
fn refuses_an_entity_with_no_closing_brace() {
    assert_eq!(
        error("erDiagram\n  A {\n    int id").message,
        "this entity has no closing `}`"
    );
}

#[test]
fn refuses_a_character_it_does_not_know() {
    let refused = error("erDiagram\n  A ||--|| B : is!");
    assert_eq!((refused.line, refused.column), (2, 18));
    assert_eq!(refused.message, "unexpected `!`");
}
