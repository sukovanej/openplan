use op_diagram::{
    Block, Cluster, Diagram, Direction, Edge, Graph, Head, Message, Node, Note, NotePlacement,
    Operator, Participant, ParticipantKind, Row, Section, Sequence, SequenceItem, Shape, Stroke,
};
use serde_json::json;

fn round_trip(diagram: &Diagram) -> serde_json::Value {
    let value = serde_json::to_value(diagram).unwrap();
    let back: Diagram = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(&back, diagram);
    value
}

#[test]
fn a_graph_leaves_out_what_the_producer_did_not_set() {
    let graph = Diagram::Graph(Graph {
        direction: Direction::Right,
        nodes: vec![
            Node {
                id: "a".into(),
                label: vec!["A".into()],
                ..Node::default()
            },
            Node {
                id: "b".into(),
                shape: Shape::Cylinder,
                label: vec!["B".into()],
                parent: Some("box".into()),
                ..Node::default()
            },
        ],
        clusters: vec![Cluster {
            id: "box".into(),
            label: vec!["Box".into()],
            ..Cluster::default()
        }],
        edges: vec![Edge {
            from: "a".into(),
            to: "b".into(),
            label: vec![],
            stroke: Stroke::Dotted,
            tail: Head::None,
            head: Head::Arrow,
            min_length: 1,
        }],
    });
    assert_eq!(
        round_trip(&graph),
        json!({
            "kind": "graph",
            "direction": "right",
            "nodes": [
                {"id": "a", "shape": {"kind": "rectangle"}, "label": ["A"]},
                {"id": "b", "shape": {"kind": "cylinder"}, "label": ["B"], "parent": "box"},
            ],
            "clusters": [{"id": "box", "label": ["Box"]}],
            "edges": [
                {"from": "a", "to": "b", "stroke": "dotted", "tail": "none", "head": "arrow", "min_length": 1},
            ],
        })
    );
}

#[test]
fn a_flow_card_carries_its_caption_icon_link_classes_and_rank() {
    let graph = Diagram::Graph(Graph {
        nodes: vec![Node {
            id: "OPP-1".into(),
            shape: Shape::Rounded,
            label: vec!["Ship it".into()],
            caption: Some("OPP-1".into()),
            icon: Some("status-todo".into()),
            link: Some("/openplan/OPP-1".into()),
            classes: vec!["status-todo".into()],
            rank: Some(2),
            parent: None,
        }],
        ..Graph::default()
    });
    assert_eq!(
        round_trip(&graph)["nodes"][0],
        json!({
            "id": "OPP-1",
            "shape": {"kind": "rounded"},
            "label": ["Ship it"],
            "caption": "OPP-1",
            "icon": "status-todo",
            "link": "/openplan/OPP-1",
            "classes": ["status-todo"],
            "rank": 2,
        })
    );
}

#[test]
fn a_table_holds_its_rows() {
    let shape = Shape::Table {
        rows: vec![Row {
            cells: vec!["INTEGER".into(), "id".into(), "PK".into()],
        }],
    };
    assert_eq!(
        serde_json::to_value(&shape).unwrap(),
        json!({"kind": "table", "rows": [{"cells": ["INTEGER", "id", "PK"]}]})
    );
}

#[test]
fn a_sequence_names_each_item_by_its_kind() {
    let sequence = Diagram::Sequence(Sequence {
        participants: vec![Participant {
            id: "a".into(),
            label: vec!["Alice".into()],
            kind: ParticipantKind::Actor,
        }],
        autonumber: false,
        items: vec![
            SequenceItem::Message(Message {
                from: "a".into(),
                to: "a".into(),
                text: vec!["think".into()],
                stroke: Stroke::Solid,
                tail: Head::None,
                head: Head::OpenArrow,
            }),
            SequenceItem::Activate {
                participant: "a".into(),
            },
            SequenceItem::Block(Block {
                operator: Operator::Loop,
                sections: vec![Section {
                    label: vec!["daily".into()],
                    items: vec![SequenceItem::Note(Note {
                        placement: NotePlacement::Spanning("a".into(), "b".into()),
                        text: vec!["wait".into()],
                    })],
                }],
            }),
        ],
    });
    assert_eq!(
        round_trip(&sequence),
        json!({
            "kind": "sequence",
            "participants": [{"id": "a", "label": ["Alice"], "kind": "actor"}],
            "items": [
                {"kind": "message", "from": "a", "to": "a", "text": ["think"], "stroke": "solid", "tail": "none", "head": "open_arrow"},
                {"kind": "activate", "participant": "a"},
                {"kind": "block", "operator": "loop", "sections": [
                    {"label": ["daily"], "items": [
                        {"kind": "note", "placement": {"spanning": ["a", "b"]}, "text": ["wait"]},
                    ]},
                ]},
            ],
        })
    );
}
