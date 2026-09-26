use std::collections::HashMap;

use op_diagram::{Diagram, Direction, Graph};
use op_diagram_render::{ClusterBox, Point, Rect, Scene, layout, svg, text_width};

#[test]
fn graphs() {
    insta::glob!(
        "../../op-diagram-mermaid/tests/diagrams",
        "**/*.mmd",
        |path| {
            let source = std::fs::read_to_string(path).unwrap();
            let Diagram::Graph(graph) = op_diagram_mermaid::parse(&source).unwrap() else {
                return;
            };
            insta::assert_binary_snapshot!(".svg", reviewable(&checked(&graph, path)));
        }
    );
}

#[test]
fn ir() {
    insta::glob!("ir/*.json", |path| {
        let json = std::fs::read_to_string(path).unwrap();
        let Diagram::Graph(graph) = serde_json::from_str(&json).unwrap() else {
            panic!("{} holds no graph", path.display());
        };
        insta::assert_binary_snapshot!(".svg", reviewable(&checked(&graph, path)));
    });
}

// GitHub shows an SVG file of a diff as an image. The SVG leaves its colors to the page, so the
// snapshot takes a stylesheet in the page's place.
fn reviewable(drawn: &str) -> Vec<u8> {
    let opened = drawn.find('>').expect("the SVG opens with a tag") + 1;
    format!(
        "{}<style>{}</style>{}",
        &drawn[..opened],
        include_str!("review.css"),
        &drawn[opened..]
    )
    .into_bytes()
}

fn checked(graph: &Graph, path: &std::path::Path) -> String {
    let scene = layout(graph);
    let drawn = svg(&scene);
    let mut problems = problems(&scene);
    problems.extend(waves(graph, &scene));
    problems.extend(unsafe_markup(&drawn));
    assert!(
        problems.is_empty(),
        "{}:\n{}",
        path.display(),
        problems.join("\n")
    );
    drawn
}

// Each rank that the producer fixes is one row: the flow draws a wave across the whole page.
fn waves(graph: &Graph, scene: &Scene) -> Vec<String> {
    let vertical = matches!(graph.direction, Direction::Down | Direction::Up);
    let mut rows: HashMap<usize, Vec<(&str, f32)>> = HashMap::new();
    for node in &graph.nodes {
        let (Some(rank), Some(drawn)) = (
            node.rank,
            scene.nodes.iter().find(|drawn| drawn.id == node.id),
        ) else {
            continue;
        };
        let center = drawn.rect.center();
        rows.entry(rank)
            .or_default()
            .push((&node.id, if vertical { center.y } else { center.x }));
    }
    rows.into_iter()
        .filter_map(|(rank, nodes)| {
            let first = nodes[0].1;
            nodes
                .iter()
                .find(|(_, at)| (at - first).abs() > 0.5)
                .map(|(id, _)| format!("the node {id} leaves the row of rank {rank}"))
        })
        .collect()
}

fn unsafe_markup(drawn: &str) -> Vec<String> {
    [
        "<script",
        "<img",
        "<b>",
        "<g onload",
        "onmouseover=\"",
        "javascript:",
        "evil.example",
    ]
    .into_iter()
    .filter(|needle| drawn.contains(needle))
    .map(|needle| format!("the SVG holds `{needle}`"))
    .collect()
}

fn problems(scene: &Scene) -> Vec<String> {
    let mut problems = Vec::new();
    let cluster_at: HashMap<&str, &ClusterBox> = scene
        .clusters
        .iter()
        .map(|cluster| (cluster.id.as_str(), cluster))
        .collect();
    let within = |parent: Option<&str>, group: &str| within(&cluster_at, parent, group);
    for (at, first) in scene.nodes.iter().enumerate() {
        for second in &scene.nodes[at + 1..] {
            if first.rect.inset(0.5).overlaps(&second.rect) {
                problems.push(format!("the nodes {} and {} overlap", first.id, second.id));
            }
        }
        if let Some(parent) = &first.parent {
            if !cluster_at[parent.as_str()]
                .rect
                .inset(-0.5)
                .contains(&first.rect)
            {
                problems.push(format!(
                    "the node {} lies outside its cluster {parent}",
                    first.id
                ));
            }
        }
        for cluster in &scene.clusters {
            if !within(first.parent.as_deref(), &cluster.id)
                && cluster.rect.inset(0.5).overlaps(&first.rect)
            {
                problems.push(format!(
                    "the node {} overlaps the cluster {}",
                    first.id, cluster.id
                ));
            }
        }
    }
    for (at, first) in scene.clusters.iter().enumerate() {
        for text in &first.texts {
            if text.x + text_width(&text.content, text.size, text.weight) > first.rect.right() + 0.5
            {
                problems.push(format!(
                    "the header of the cluster {} runs out of it",
                    first.id
                ));
            }
        }
        if let Some(parent) = &first.parent {
            if !cluster_at[parent.as_str()]
                .rect
                .inset(-0.5)
                .contains(&first.rect)
            {
                problems.push(format!(
                    "the cluster {} lies outside its cluster {parent}",
                    first.id
                ));
            }
        }
        for second in &scene.clusters[at + 1..] {
            let nested = within(first.parent.as_deref(), &second.id)
                || within(second.parent.as_deref(), &first.id);
            if !nested && first.rect.inset(0.5).overlaps(&second.rect) {
                problems.push(format!(
                    "the clusters {} and {} overlap",
                    first.id, second.id
                ));
            }
        }
    }
    for edge in &scene.edges {
        for node in &scene.nodes {
            if node.id == edge.from || node.id == edge.to {
                continue;
            }
            let inner = node.rect.inset(1.0);
            if edge
                .points
                .windows(2)
                .any(|pair| crosses(pair[0], pair[1], &inner))
            {
                problems.push(format!(
                    "the edge {} -> {} crosses the node {}",
                    edge.from, edge.to, node.id
                ));
            }
            if let Some(label) = &edge.label {
                if label.rect.inset(0.5).overlaps(&node.rect) {
                    problems.push(format!(
                        "the label of {} -> {} overlaps the node {}",
                        edge.from, edge.to, node.id
                    ));
                }
            }
        }
    }
    problems
}

fn within<'a>(
    clusters: &HashMap<&'a str, &'a ClusterBox>,
    mut parent: Option<&'a str>,
    group: &str,
) -> bool {
    while let Some(at) = parent {
        if at == group {
            return true;
        }
        parent = clusters
            .get(at)
            .and_then(|cluster| cluster.parent.as_deref());
    }
    false
}

// Liang–Barsky: whether any part of the segment from `a` to `b` lies inside `rect`.
fn crosses(a: Point, b: Point, rect: &Rect) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let mut low: f32 = 0.0;
    let mut high: f32 = 1.0;
    for (p, q) in [
        (-dx, a.x - rect.x),
        (dx, rect.right() - a.x),
        (-dy, a.y - rect.y),
        (dy, rect.bottom() - a.y),
    ] {
        if p.abs() < f32::EPSILON {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                low = low.max(t);
            } else {
                high = high.min(t);
            }
        }
    }
    low < high
}
