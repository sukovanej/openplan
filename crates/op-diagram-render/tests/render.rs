use std::collections::HashMap;
use std::path::Path;

use op_diagram::{Diagram, Direction, Graph};
use op_diagram_render::{ClusterBox, Point, Rect, Scene, layout, svg, text_width};

#[test]
fn graphs() {
    insta::glob!(
        "../../op-diagram-mermaid/tests/diagrams",
        "**/*.mmd",
        |path| {
            let diagram = parse(path);
            if matches!(diagram, Diagram::Graph(_)) {
                insta::assert_binary_snapshot!(".svg", reviewable(&checked(&diagram, path)));
            }
        }
    );
}

#[test]
fn sequences() {
    insta::glob!(
        "../../op-diagram-mermaid/tests/diagrams",
        "**/*.mmd",
        |path| {
            let diagram = parse(path);
            if matches!(diagram, Diagram::Sequence(_)) {
                insta::assert_binary_snapshot!(".svg", reviewable(&checked(&diagram, path)));
            }
        }
    );
}

#[test]
fn ir() {
    insta::glob!("ir/*.json", |path| {
        let json = std::fs::read_to_string(path).unwrap();
        let diagram: Diagram = serde_json::from_str(&json).unwrap();
        insta::assert_binary_snapshot!(".svg", reviewable(&checked(&diagram, path)));
    });
}

fn parse(path: &Path) -> Diagram {
    let source = std::fs::read_to_string(path).unwrap();
    op_diagram_mermaid::parse(&source).unwrap()
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

fn checked(diagram: &Diagram, path: &Path) -> String {
    let scene = layout(diagram);
    let drawn = svg(&scene);
    let mut problems = match diagram {
        Diagram::Graph(graph) => {
            let mut problems = problems(&scene);
            problems.extend(waves(graph, &scene));
            problems
        }
        Diagram::Sequence(_) => sequence_problems(&scene),
    };
    problems.extend(outside(&scene));
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

fn outside(scene: &Scene) -> Vec<String> {
    let page = Rect {
        x: -0.5,
        y: -0.5,
        width: scene.width + 1.0,
        height: scene.height + 1.0,
    };
    let rects = scene
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.rect))
        .chain(
            scene
                .clusters
                .iter()
                .map(|cluster| (cluster.id.as_str(), cluster.rect)),
        )
        .chain(scene.edges.iter().filter_map(|edge| {
            edge.label
                .as_ref()
                .map(|label| (edge.to.as_str(), label.rect))
        }));
    rects
        .filter(|(_, rect)| !page.contains(rect))
        .map(|(id, _)| format!("{id} lies outside the drawing"))
        .collect()
}

// The timeline runs down: each message sits below the one before, and no two things of the
// timeline, a message label or a note, share any space.
fn sequence_problems(scene: &Scene) -> Vec<String> {
    let mut problems = Vec::new();
    let heads: Vec<&op_diagram_render::NodeBox> = scene
        .nodes
        .iter()
        .filter(|node| node.classes.iter().any(|class| class == "participant"))
        .collect();
    for (at, first) in heads.iter().enumerate() {
        for second in &heads[at + 1..] {
            if first.rect.inset(0.5).overlaps(&second.rect) {
                problems.push(format!(
                    "the heads of {} and {} overlap",
                    first.id, second.id
                ));
            }
        }
    }
    let mut last = f32::NEG_INFINITY;
    for edge in &scene.edges {
        let top = edge
            .points
            .iter()
            .map(|point| point.y)
            .fold(f32::INFINITY, f32::min);
        if top <= last {
            problems.push(format!(
                "the message {} -> {} does not run below the one before",
                edge.from, edge.to
            ));
        }
        last = edge.points.iter().map(|point| point.y).fold(last, f32::max);
    }
    let rows: Vec<(String, Rect)> = scene
        .edges
        .iter()
        .filter_map(|edge| {
            edge.label.as_ref().map(|label| {
                (
                    format!("the label of {} -> {}", edge.from, edge.to),
                    label.rect,
                )
            })
        })
        .chain(
            scene
                .nodes
                .iter()
                .filter(|node| node.classes.iter().any(|class| class == "note"))
                .map(|node| (node.id.clone(), node.rect)),
        )
        .collect();
    for (at, (first, a)) in rows.iter().enumerate() {
        for (second, b) in &rows[at + 1..] {
            if a.inset(0.5).overlaps(b) {
                problems.push(format!("{first} and {second} overlap"));
            }
        }
        for head in &heads {
            if a.inset(0.5).overlaps(&head.rect) {
                problems.push(format!("{first} overlaps the head of {}", head.id));
            }
        }
    }
    let cluster_at: HashMap<&str, &ClusterBox> = scene
        .clusters
        .iter()
        .map(|cluster| (cluster.id.as_str(), cluster))
        .collect();
    for cluster in &scene.clusters {
        if let Some(parent) = &cluster.parent {
            if !cluster_at[parent.as_str()]
                .rect
                .inset(-0.5)
                .contains(&cluster.rect)
            {
                problems.push(format!("{} lies outside {parent}", cluster.id));
            }
        }
    }
    problems
}
