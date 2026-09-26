use std::collections::HashSet;

use op_diagram::{Cluster, Diagram, Direction, Edge, Graph, Head, Icon, Node, Page, Shape, Stroke};
use op_task::Status;

use crate::field::Field;

use super::{Flow, FlowNode};

impl Flow {
    // A leaf is a card on the rank of its wave, a box is a cluster around its children, and an
    // edge runs from a dependency to the task that waits for it. With a page, each part of the flow
    // that no edge joins to another is laid out on its own, and the parts fill the page.
    pub fn diagram(&self, page: Option<Page>) -> Diagram {
        let boxes: HashSet<String> = self
            .nodes
            .iter()
            .filter_map(|node| match node {
                FlowNode::Box { project, id, .. } => Some(key(project, id)),
                _ => None,
            })
            .collect();
        let unresolved: HashSet<String> = self
            .nodes
            .iter()
            .filter_map(|node| match node {
                FlowNode::Unresolved { project, id } => Some(key(project, id)),
                _ => None,
            })
            .collect();
        let parent_of = |project: &str, parent: &Option<String>| {
            parent
                .as_deref()
                .map(|parent| key(project, parent))
                .filter(|parent| boxes.contains(parent))
        };
        let mut graph = Graph {
            direction: Direction::Down,
            pack: page,
            ..Graph::default()
        };
        for node in &self.nodes {
            match node {
                FlowNode::Leaf {
                    project,
                    id,
                    title,
                    status,
                    parent,
                    wave,
                    ..
                } => graph.nodes.push(Node {
                    id: key(project, id),
                    shape: Shape::Rounded,
                    label: vec![title.clone()],
                    caption: Some(id.clone()),
                    icon: Some(status_icon(status)),
                    link: Some(task_path(project, id)),
                    classes: vec![status_class(status)],
                    rank: Some(*wave),
                    parent: parent_of(project, parent),
                }),
                FlowNode::Box {
                    project,
                    id,
                    title,
                    status,
                    parent,
                } => graph.clusters.push(Cluster {
                    id: key(project, id),
                    label: vec![title.clone()],
                    caption: Some(id.clone()),
                    icon: Some(status_icon(status)),
                    link: Some(task_path(project, id)),
                    classes: vec![status_class(status)],
                    parent: parent_of(project, parent),
                    direction: None,
                }),
                FlowNode::Unresolved { project, id } => graph.nodes.push(Node {
                    id: key(project, id),
                    shape: Shape::Rounded,
                    label: vec![id.clone()],
                    icon: Some(Icon::CircleDashed),
                    classes: vec!["unresolved".to_owned()],
                    ..Node::default()
                }),
            }
        }
        graph.edges = self
            .edges
            .iter()
            .map(|edge| {
                let from = key(&edge.project, &edge.from);
                Edge {
                    stroke: if unresolved.contains(&from) {
                        Stroke::Dotted
                    } else {
                        Stroke::Solid
                    },
                    from,
                    to: key(&edge.project, &edge.to),
                    label: Vec::new(),
                    tail: Head::None,
                    head: Head::Arrow,
                    min_length: 1,
                }
            })
            .collect();
        Diagram::Graph(graph)
    }
}

// A key is unique inside one project only, and a project name holds no slash.
fn key(project: &str, id: &str) -> String {
    format!("{project}/{id}")
}

// The page of the task in the web app.
fn task_path(project: &str, id: &str) -> String {
    format!("/{project}/task/{id}")
}

fn status_class(status: &Field<Status>) -> String {
    match status.clone().value() {
        Some(status) => format!("status-{}", status.as_str().replace('_', "-")),
        None => "status-unreadable".to_owned(),
    }
}

// The marks the web app shows for each status.
fn status_icon(status: &Field<Status>) -> Icon {
    match status.clone().value() {
        Some(Status::Backlog) => Icon::CircleEllipsis,
        Some(Status::Todo) => Icon::Clock,
        Some(Status::InProgress) => Icon::CircleDot,
        Some(Status::InReview) => Icon::Eye,
        Some(Status::Done) => Icon::CircleCheck,
        Some(Status::Cancelled) => Icon::CircleX,
        None => Icon::CircleAlert,
    }
}
