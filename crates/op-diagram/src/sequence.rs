use serde::{Deserialize, Serialize};

use crate::{Head, Stroke};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sequence {
    pub participants: Vec<Participant>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub autonumber: bool,
    pub items: Vec<SequenceItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participant {
    pub id: String,
    pub label: Vec<String>,
    pub kind: ParticipantKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantKind {
    Box,
    Actor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SequenceItem {
    Message(Message),
    Note(Note),
    Activate { participant: String },
    Deactivate { participant: String },
    Block(Block),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text: Vec<String>,
    pub stroke: Stroke,
    pub tail: Head,
    pub head: Head,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub placement: NotePlacement,
    pub text: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotePlacement {
    LeftOf(String),
    RightOf(String),
    Over(String),
    Spanning(String, String),
}

// The first section carries the label of the block itself; each later one is an `else`, an `and`,
// or an `option`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub operator: Operator,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Loop,
    Alt,
    Opt,
    Par,
    Critical,
    Break,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub label: Vec<String>,
    pub items: Vec<SequenceItem>,
}
