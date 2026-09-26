use serde::{Deserialize, Serialize};

use crate::{Direction, Head, Stroke};

// The order of `nodes` is the order each rank starts from, so a producer that knows a good order
// (the flow puts the task that unblocks the most work first) hands it to the layout that way.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Graph {
    pub direction: Direction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<Page>,
    pub nodes: Vec<Node>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clusters: Vec<Cluster>,
    pub edges: Vec<Edge>,
}

// A graph that fills a page lays out each part that no edge joins to another on its own, and packs
// the parts in rows to the shape of the page. The flow does this; a Mermaid diagram does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Page {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub shape: Shape,
    pub label: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Icon>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cluster {
    pub id: String,
    pub label: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Icon>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<Direction>,
}

// An end names a node or a cluster.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub label: Vec<String>,
    pub stroke: Stroke,
    pub tail: Head,
    pub head: Head,
    pub min_length: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Shape {
    #[default]
    Rectangle,
    Rounded,
    Stadium,
    Subroutine,
    Cylinder,
    Circle,
    DoubleCircle,
    Asymmetric,
    Diamond,
    Hexagon,
    LeanRight,
    LeanLeft,
    Trapezoid,
    InvertedTrapezoid,
    Table {
        rows: Vec<Row>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Row {
    pub cells: Vec<String>,
}

// The icons are Lucide's, named as Lucide names them. A producer picks what an icon means; the
// flow marks the status of a task with one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Icon {
    CircleAlert,
    CircleCheck,
    CircleDashed,
    CircleDot,
    CircleEllipsis,
    CircleX,
    Clock,
    Eye,
}
