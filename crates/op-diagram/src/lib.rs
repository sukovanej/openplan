mod graph;
mod sequence;

use serde::{Deserialize, Serialize};

pub use graph::{Cluster, Edge, Graph, Node, Row, Shape};
pub use sequence::{
    Block, Message, Note, NotePlacement, Operator, Participant, ParticipantKind, Section, Sequence,
    SequenceItem,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Diagram {
    Graph(Graph),
    Sequence(Sequence),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    Down,
    Up,
    Right,
    Left,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stroke {
    #[default]
    Solid,
    Dotted,
    Thick,
    Invisible,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Head {
    #[default]
    None,
    Arrow,
    OpenArrow,
    Circle,
    Cross,
    ExactlyOne,
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
}
