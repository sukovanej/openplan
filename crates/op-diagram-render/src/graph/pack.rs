use std::collections::HashMap;

use op_diagram::{Graph, Page};

use crate::scene::{Point, Scene};

// Each part carries a margin of its own, so the parts stand this much more apart.
const PART_GAP: f32 = 32.0;
const TRIES: usize = 32;

// The parts of the graph that no edge joins to each other, in the order of their first node. A
// cluster and all that it holds are one part.
pub(super) fn parts(graph: &Graph) -> Vec<Graph> {
    let nodes = graph.nodes.len();
    let mut at: HashMap<&str, usize> = HashMap::new();
    for (index, cluster) in graph.clusters.iter().enumerate() {
        at.insert(&cluster.id, nodes + index);
    }
    for (index, node) in graph.nodes.iter().enumerate() {
        at.insert(&node.id, index);
    }
    let mut joined = Joined::new(nodes + graph.clusters.len());
    let parents = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (index, node.parent.as_deref()))
        .chain(
            graph
                .clusters
                .iter()
                .enumerate()
                .map(|(index, cluster)| (nodes + index, cluster.parent.as_deref())),
        );
    for (item, parent) in parents {
        if let Some(&parent) = parent.and_then(|parent| at.get(parent)) {
            joined.join(item, parent);
        }
    }
    for edge in &graph.edges {
        if let (Some(&from), Some(&to)) = (at.get(edge.from.as_str()), at.get(edge.to.as_str())) {
            joined.join(from, to);
        }
    }

    let mut part_of: HashMap<usize, usize> = HashMap::new();
    for item in 0..nodes + graph.clusters.len() {
        let root = joined.root(item);
        let next = part_of.len();
        part_of.entry(root).or_insert(next);
    }
    let mut parts: Vec<Graph> = (0..part_of.len())
        .map(|_| Graph {
            direction: graph.direction,
            ..Graph::default()
        })
        .collect();
    let mut part = |item: usize| part_of[&joined.root(item)];
    for (index, node) in graph.nodes.iter().enumerate() {
        parts[part(index)].nodes.push(node.clone());
    }
    for (index, cluster) in graph.clusters.iter().enumerate() {
        parts[part(nodes + index)].clusters.push(cluster.clone());
    }
    for edge in &graph.edges {
        let end = at
            .get(edge.from.as_str())
            .or_else(|| at.get(edge.to.as_str()));
        if let Some(&end) = end {
            parts[part(end)].edges.push(edge.clone());
        }
    }
    parts
}

struct Joined {
    parent: Vec<usize>,
}

impl Joined {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count).collect(),
        }
    }

    fn root(&mut self, item: usize) -> usize {
        let mut root = item;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut at = item;
        while self.parent[at] != root {
            at = std::mem::replace(&mut self.parent[at], root);
        }
        root
    }

    fn join(&mut self, first: usize, second: usize) {
        let (first, second) = (self.root(first), self.root(second));
        self.parent[first.max(second)] = first.min(second);
    }
}

struct Shelved {
    places: Vec<Point>,
    width: f32,
    height: f32,
}

// The parts go left to right in rows no wider than the limit, each at the top of its row. Of the
// limits from the widest part to all parts in one row, the one whose shape is nearest to the
// page wins.
pub(super) fn pack(scenes: Vec<Scene>, page: Page) -> Scene {
    let shape = page.width.max(1) as f32 / page.height.max(1) as f32;
    let sizes: Vec<(f32, f32)> = scenes
        .iter()
        .map(|scene| (scene.width, scene.height))
        .collect();
    let widest = sizes.iter().map(|size| size.0).fold(0.0, f32::max);
    let spread = sizes.iter().map(|size| size.0 + PART_GAP).sum::<f32>() - PART_GAP;
    let mut best: Option<(f32, Shelved)> = None;
    for step in 0..=TRIES {
        let limit = widest + (spread - widest) * step as f32 / TRIES as f32;
        let shelved = shelve(&sizes, limit);
        let off = (shelved.width / shelved.height.max(1.0) / shape).ln().abs();
        if best.as_ref().is_none_or(|(missed, _)| off < *missed) {
            best = Some((off, shelved));
        }
    }
    let Some((_, shelved)) = best else {
        return Scene::default();
    };
    let mut packed = Scene {
        width: shelved.width,
        height: shelved.height,
        ..Scene::default()
    };
    for (mut scene, place) in scenes.into_iter().zip(shelved.places) {
        scene.translate(place.x, place.y);
        packed.clusters.append(&mut scene.clusters);
        packed.nodes.append(&mut scene.nodes);
        packed.edges.append(&mut scene.edges);
        packed.guides.append(&mut scene.guides);
    }
    packed
}

fn shelve(sizes: &[(f32, f32)], limit: f32) -> Shelved {
    let mut places = Vec::with_capacity(sizes.len());
    let (mut x, mut y, mut row, mut width) = (0.0, 0.0, 0.0, 0.0_f32);
    for &(part_width, part_height) in sizes {
        if x > 0.0 && x + part_width > limit + 0.5 {
            y += row + PART_GAP;
            x = 0.0;
            row = 0.0;
        }
        places.push(Point { x, y });
        width = width.max(x + part_width);
        x += part_width + PART_GAP;
        row = f32::max(row, part_height);
    }
    Shelved {
        places,
        width,
        height: y + row,
    }
}
