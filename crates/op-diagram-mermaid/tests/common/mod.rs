#![allow(dead_code)]

use op_diagram::{Diagram, Graph, Node, Sequence};
use op_diagram_mermaid::{ParseError, parse};

pub fn graph(source: &str) -> Graph {
    match parse(source) {
        Ok(Diagram::Graph(graph)) => graph,
        other => panic!("expected a graph from {source:?}, got {other:?}"),
    }
}

pub fn sequence(source: &str) -> Sequence {
    match parse(source) {
        Ok(Diagram::Sequence(sequence)) => sequence,
        other => panic!("expected a sequence from {source:?}, got {other:?}"),
    }
}

pub fn error(source: &str) -> ParseError {
    match parse(source) {
        Err(error) => error,
        Ok(diagram) => panic!("expected {source:?} to fail, got {diagram:?}"),
    }
}

pub fn ends(graph: &Graph) -> Vec<(&str, &str)> {
    graph
        .edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect()
}

pub fn ids(graph: &Graph) -> Vec<&str> {
    graph.nodes.iter().map(|node| node.id.as_str()).collect()
}

pub fn node<'a>(graph: &'a Graph, id: &str) -> &'a Node {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .unwrap_or_else(|| panic!("no node `{id}` in {graph:?}"))
}

pub fn parent<'a>(graph: &'a Graph, id: &str) -> Option<&'a str> {
    node(graph, id).parent.as_deref()
}
