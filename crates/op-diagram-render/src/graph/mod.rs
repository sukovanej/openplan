mod order;
mod place;
mod rank;
mod route;
mod size;

use std::collections::HashMap;

use op_diagram::{Direction, Graph, Shape, Stroke};

use crate::scene::{ClusterBox, EdgeLabel, EdgePath, NodeBox, Point, Rect, Scene};
use order::{Level, Structure};
use rank::Constraint;
use route::{Chain, Ends};

const RANK_GAP: f32 = 56.0;
const TRACK_GAP: f32 = 10.0;
const MARGIN: f32 = 8.0;
const CLUSTER_PAD: f32 = 12.0;
const HEADER_GAP: f32 = 6.0;
const LOOP_ROOM: f32 = 24.0;
const LOOP_REACH: f32 = 18.0;
const LOOP_SPREAD: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Node(usize),
    Filler,
    Port,
    Dummy,
    Label,
}

struct Vertex {
    kind: Kind,
    rank: usize,
    group: Level,
    breadth: f32,
    depth: f32,
}

impl Vertex {
    fn thin(&self) -> bool {
        matches!(self.kind, Kind::Port | Kind::Dummy)
    }

    fn on_edge(&self) -> bool {
        matches!(self.kind, Kind::Port | Kind::Dummy | Kind::Label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum End {
    Node(usize),
    Group(usize),
}

// The layout works in a frame whose ranks run down: `breadth` across a rank and `depth` along the
// ranks. The frame turns into the direction of the diagram only when the scene is built.
#[derive(Clone, Copy)]
struct Frame {
    direction: Direction,
    breadth: f32,
    depth: f32,
}

impl Frame {
    fn vertical(self) -> bool {
        matches!(self.direction, Direction::Down | Direction::Up)
    }

    fn point(self, breadth: f32, depth: f32) -> Point {
        let (x, y) = match self.direction {
            Direction::Down => (breadth, depth),
            Direction::Up => (breadth, self.depth - depth),
            Direction::Right => (depth, breadth),
            Direction::Left => (self.depth - depth, breadth),
        };
        Point {
            x: x + MARGIN,
            y: y + MARGIN,
        }
    }

    fn rect(self, breadth: (f32, f32), depth: (f32, f32)) -> Rect {
        let first = self.point(breadth.0, depth.0);
        let second = self.point(breadth.1, depth.1);
        Rect {
            x: first.x.min(second.x),
            y: first.y.min(second.y),
            width: (first.x - second.x).abs(),
            height: (first.y - second.y).abs(),
        }
    }

    fn centered(self, breadth: f32, depth: f32, width: f32, height: f32) -> Rect {
        let center = self.point(breadth, depth);
        Rect {
            x: center.x - width / 2.0,
            y: center.y - height / 2.0,
            width,
            height,
        }
    }

    // The extent of a box of `width` by `height` across a rank, and along the ranks.
    fn extent(self, width: f32, height: f32) -> (f32, f32) {
        if self.vertical() {
            (width, height)
        } else {
            (height, width)
        }
    }
}

struct Pad {
    breadth: (f32, f32),
    depth: (f32, f32),
}

// The header of a cluster sits at its top on the page, so which side of the frame takes it depends
// on the direction.
fn cluster_pad(direction: Direction, header: f32) -> Pad {
    let top = if header > 0.0 {
        CLUSTER_PAD + header + HEADER_GAP
    } else {
        CLUSTER_PAD
    };
    match direction {
        Direction::Down => Pad {
            breadth: (CLUSTER_PAD, CLUSTER_PAD),
            depth: (top, CLUSTER_PAD),
        },
        Direction::Up => Pad {
            breadth: (CLUSTER_PAD, CLUSTER_PAD),
            depth: (CLUSTER_PAD, top),
        },
        Direction::Right | Direction::Left => Pad {
            breadth: (top, CLUSTER_PAD),
            depth: (CLUSTER_PAD, CLUSTER_PAD),
        },
    }
}

fn ancestors(parents: &[Level], start: Level) -> impl Iterator<Item = usize> + '_ {
    std::iter::successors(start, move |group| parents[*group])
}

fn common_level(parents: &[Level], a: Level, b: Level) -> Level {
    let above: Vec<usize> = ancestors(parents, a).collect();
    ancestors(parents, b).find(|group| above.contains(group))
}

// A cluster's own parent chain must end, or every walk up the tree would loop.
fn group_parents(graph: &Graph, group_at: &HashMap<&str, usize>) -> Vec<Level> {
    let mut parents: Vec<Level> = graph
        .clusters
        .iter()
        .map(|cluster| {
            cluster
                .parent
                .as_deref()
                .and_then(|parent| group_at.get(parent).copied())
        })
        .collect();
    for start in 0..parents.len() {
        let mut seen = vec![start];
        let mut at = start;
        while let Some(parent) = parents[at] {
            if seen.contains(&parent) {
                parents[at] = None;
                break;
            }
            seen.push(parent);
            at = parent;
        }
    }
    parents
}

pub(crate) fn layout(graph: &Graph) -> Scene {
    let direction = graph.direction;
    let orient = Frame {
        direction,
        breadth: 0.0,
        depth: 0.0,
    };
    let node_at: HashMap<&str, usize> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(at, node)| (node.id.as_str(), at))
        .collect();
    let group_at: HashMap<&str, usize> = graph
        .clusters
        .iter()
        .enumerate()
        .map(|(at, cluster)| (cluster.id.as_str(), at))
        .collect();
    let groups = graph.clusters.len();
    let group_parent = group_parents(graph, &group_at);
    let node_group: Vec<Level> = graph
        .nodes
        .iter()
        .map(|node| {
            node.parent
                .as_deref()
                .and_then(|parent| group_at.get(parent).copied())
        })
        .collect();
    let bodies: Vec<size::Body> = graph.nodes.iter().map(size::node).collect();
    let headers: Vec<size::Header> = graph.clusters.iter().map(size::header).collect();
    let pads: Vec<Pad> = headers
        .iter()
        .map(|header| cluster_pad(direction, header.height()))
        .collect();

    // A cluster with no node and no cluster inside still needs a place, so it gets an empty vertex.
    let mut filled = vec![false; groups];
    for group in node_group.iter().flatten() {
        filled[*group] = true;
    }
    for parent in group_parent.iter().flatten() {
        filled[*parent] = true;
    }
    let mut item_group: Vec<Level> = node_group.clone();
    for (group, has) in filled.iter().enumerate() {
        if !has {
            item_group.push(Some(group));
        }
    }
    let items = item_group.len();
    let mut descendants = vec![Vec::new(); groups];
    for (item, group) in item_group.iter().enumerate() {
        for ancestor in ancestors(&group_parent, *group) {
            descendants[ancestor].push(item);
        }
    }

    let end_of = |id: &str| {
        node_at
            .get(id)
            .map(|at| End::Node(*at))
            .or_else(|| group_at.get(id).map(|at| End::Group(*at)))
    };
    let ends: Vec<Option<(End, End)>> = graph
        .edges
        .iter()
        .map(|edge| Some((end_of(&edge.from)?, end_of(&edge.to)?)))
        .collect();
    let items_of = |end: End| match end {
        End::Node(node) => vec![node],
        End::Group(group) => descendants[group].clone(),
    };
    let inside = |item: usize, end: End| match end {
        End::Node(_) => false,
        End::Group(group) => ancestors(&group_parent, item_group[item]).any(|at| at == group),
    };

    let doubled = graph.edges.iter().any(|edge| !edge.label.is_empty());
    let scale = if doubled { 2 } else { 1 };
    let mut constraints = Vec::new();
    for (edge, pair) in graph.edges.iter().zip(&ends) {
        let Some((from, to)) = *pair else {
            continue;
        };
        for a in items_of(from) {
            for b in items_of(to) {
                if a != b && !inside(a, to) && !inside(b, from) {
                    constraints.push(Constraint {
                        from: a,
                        to: b,
                        length: edge.min_length.max(1) * scale,
                    });
                }
            }
        }
    }
    let fixed: Vec<Option<usize>> = (0..items)
        .map(|item| {
            graph
                .nodes
                .get(item)
                .and_then(|node| node.rank)
                .map(|rank| rank * scale)
        })
        .collect();
    let item_rank = rank::ranks(items, &constraints, &fixed);
    let span: Vec<(usize, usize)> = descendants
        .iter()
        .map(|items| {
            let ranks = items.iter().map(|item| item_rank[*item]);
            (ranks.clone().min().unwrap_or(0), ranks.max().unwrap_or(0))
        })
        .collect();

    let mut vertices: Vec<Vertex> = (0..items)
        .map(|item| {
            let (breadth, depth) = match bodies.get(item) {
                Some(body) => orient.extent(body.width, body.height),
                None => (0.0, 0.0),
            };
            Vertex {
                kind: if item < graph.nodes.len() {
                    Kind::Node(item)
                } else {
                    Kind::Filler
                },
                rank: item_rank[item],
                group: item_group[item],
                breadth,
                depth,
            }
        })
        .collect();

    let mut chains: Vec<Chain> = Vec::new();
    let mut labels: HashMap<usize, size::Label> = HashMap::new();
    let mut loops: Vec<(usize, usize)> = Vec::new();
    let mut directs: Vec<(usize, End, End)> = Vec::new();
    for (index, (edge, pair)) in graph.edges.iter().zip(&ends).enumerate() {
        let Some((from, to)) = *pair else {
            continue;
        };
        if from == to {
            if let End::Node(node) = from {
                loops.push((index, node));
            }
            continue;
        }
        let range = |end: End| match end {
            End::Node(node) => (item_rank[node], item_rank[node]),
            End::Group(group) => span[group],
        };
        let (upper, lower, reversed) = if range(from).1 < range(to).0 {
            (from, to, false)
        } else if range(to).1 < range(from).0 {
            (to, from, true)
        } else {
            directs.push((index, from, to));
            continue;
        };
        let mut port = |group: usize, rank: usize| {
            vertices.push(Vertex {
                kind: Kind::Port,
                rank,
                group: Some(group),
                breadth: 0.0,
                depth: 0.0,
            });
            vertices.len() - 1
        };
        let (top, upper_cluster) = match upper {
            End::Node(node) => (node, None),
            End::Group(group) => (port(group, span[group].1), Some(group)),
        };
        let (bottom, lower_cluster) = match lower {
            End::Node(node) => (node, None),
            End::Group(group) => (port(group, span[group].0), Some(group)),
        };
        let (first, last) = (vertices[top].rank, vertices[bottom].rank);
        let between = common_level(&group_parent, vertices[top].group, vertices[bottom].group);
        let middle = (first + last) / 2;
        let label =
            (!edge.label.is_empty() && last - first >= 2).then(|| size::edge_label(&edge.label));
        let mut chain = vec![top];
        let mut label_vertex = None;
        for rank in first + 1..last {
            let (breadth, depth, kind) = match &label {
                Some(label) if rank == middle => {
                    let (breadth, depth) = orient.extent(label.width, label.height);
                    (breadth, depth, Kind::Label)
                }
                _ => (0.0, 0.0, Kind::Dummy),
            };
            vertices.push(Vertex {
                kind,
                rank,
                group: between,
                breadth,
                depth,
            });
            if kind == Kind::Label {
                label_vertex = Some(vertices.len() - 1);
            }
            chain.push(vertices.len() - 1);
        }
        chain.push(bottom);
        if let Some(label) = label {
            labels.insert(index, label);
        }
        chains.push(Chain {
            edge: index,
            vertices: chain,
            reversed,
            upper_cluster,
            lower_cluster,
            label_vertex,
            drawn: edge.stroke != Stroke::Invisible,
        });
    }

    let ranks = vertices
        .iter()
        .map(|vertex| vertex.rank)
        .max()
        .map_or(1, |last| last + 1);
    let links: Vec<(usize, usize, f32)> = chains
        .iter()
        .flat_map(|chain| chain.vertices.windows(2).map(|pair| (pair[0], pair[1])))
        .map(|(upper, lower)| {
            let weight = match (vertices[upper].on_edge(), vertices[lower].on_edge()) {
                (true, true) => 8.0,
                (false, false) => 1.0,
                _ => 2.0,
            };
            (upper, lower, weight)
        })
        .collect();
    let unit_links: Vec<(usize, usize)> = links
        .iter()
        .map(|(upper, lower, _)| (*upper, *lower))
        .collect();

    let mut keys: Vec<f32> = (0..vertices.len()).map(|vertex| vertex as f32).collect();
    for chain in &chains {
        let start = keys[chain.vertices[0]];
        for vertex in &chain.vertices[1..chain.vertices.len() - 1] {
            keys[*vertex] = start + 0.5;
        }
    }
    for (vertex, key) in keys.iter_mut().enumerate() {
        if let (Kind::Port, Some(group)) = (vertices[vertex].kind, vertices[vertex].group) {
            *key = descendants[group].first().map_or(*key, |item| *item as f32);
        }
    }
    let vertex_rank: Vec<usize> = vertices.iter().map(|vertex| vertex.rank).collect();
    let vertex_group: Vec<Level> = vertices.iter().map(|vertex| vertex.group).collect();
    let ordering = order::order(
        &Structure {
            ranks,
            vertex_rank: &vertex_rank,
            vertex_group: &vertex_group,
            group_parent: &group_parent,
            group_span: &span,
            links: &unit_links,
        },
        keys,
    );

    let vertex_breadth: Vec<f32> = vertices.iter().map(|vertex| vertex.breadth).collect();
    let vertex_thin: Vec<bool> = vertices.iter().map(Vertex::thin).collect();
    let mut room_after = vec![0.0; vertices.len()];
    for (_, node) in &loops {
        room_after[*node] = LOOP_ROOM;
    }
    let group_pad: Vec<(f32, f32)> = pads.iter().map(|pad| pad.breadth).collect();
    let group_min_breadth: Vec<f32> = headers
        .iter()
        .map(|header| {
            if orient.vertical() {
                header.width() + 2.0 * CLUSTER_PAD
            } else {
                0.0
            }
        })
        .collect();
    let placement = place::place(
        &ordering,
        &place::Sizes {
            vertex_breadth: &vertex_breadth,
            vertex_thin: &vertex_thin,
            vertex_room_after: &room_after,
            vertex_group: &vertex_group,
            group_parent: &group_parent,
            group_pad: &group_pad,
            group_min_breadth: &group_min_breadth,
            links: &links,
        },
    );
    let mut center = placement.center.clone();
    // An edge leaves or enters a cluster straight above or below where it goes, as far as the
    // cluster reaches.
    for chain in &chains {
        let count = chain.vertices.len();
        for (group, port, neighbor) in [
            (chain.upper_cluster, chain.vertices[0], chain.vertices[1]),
            (
                chain.lower_cluster,
                chain.vertices[count - 1],
                chain.vertices[count - 2],
            ),
        ] {
            if let Some(group) = group {
                let (left, right) = placement.group_bounds[group];
                let (before, after) = pads[group].breadth;
                center[port] =
                    center[neighbor].clamp(left + before, (right - after).max(left + before));
            }
        }
    }
    let center = &center;

    let spreads = |vertex: usize| match vertices[vertex].kind {
        Kind::Node(node) => match &graph.nodes[node].shape {
            Shape::Rectangle
            | Shape::Rounded
            | Shape::Subroutine
            | Shape::Cylinder
            | Shape::Table { .. } => true,
            Shape::Hexagon
            | Shape::Stadium
            | Shape::LeanRight
            | Shape::LeanLeft
            | Shape::Trapezoid
            | Shape::InvertedTrapezoid => orient.vertical(),
            _ => false,
        },
        _ => false,
    };
    let chain_ends: Vec<Ends> = route::ports(&chains, center, &vertex_breadth, &spreads);
    let paths: Vec<Vec<(f32, f32)>> = chains
        .iter()
        .zip(&chain_ends)
        .map(|(chain, ends)| route::links(chain, ends, center))
        .collect();
    let tracks = route::tracks(&paths, &chains, &vertex_rank, ranks);
    // Off its middle, a cylinder's top and bottom curve toward its body, so a line there ends where
    // the curve is.
    let cap = |vertex: usize, x: f32| match vertices[vertex].kind {
        Kind::Node(node) if orient.vertical() && graph.nodes[node].shape == Shape::Cylinder => {
            let reach = (x - center[vertex]) / (vertices[vertex].breadth / 2.0);
            size::CYLINDER_CAP * (1.0 - (1.0 - reach * reach).max(0.0).sqrt())
        }
        _ => 0.0,
    };

    let rows = Rows::new(
        &vertices,
        &group_parent,
        &span,
        &pads,
        ranks,
        doubled,
        &tracks.count,
    );
    let rows = rows.fit_headers(
        &headers,
        &span,
        orient.vertical(),
        &vertices,
        &group_parent,
        &pads,
    );
    let group_depth = rows.group_depths(&vertices, &group_parent, &pads, groups);
    let frame = Frame {
        direction,
        breadth: placement.breadth,
        depth: rows.total(),
    };

    let mut nodes = Vec::new();
    for (index, node) in graph.nodes.iter().enumerate() {
        let body = &bodies[index];
        let rect = frame.centered(
            center[index],
            rows.middle(vertices[index].rank),
            body.width,
            body.height,
        );
        let texts = body
            .texts
            .iter()
            .map(|text| {
                let mut text = text.clone();
                text.x += rect.x;
                text.baseline += rect.y;
                text
            })
            .collect();
        nodes.push(NodeBox {
            id: node.id.clone(),
            parent: node_group[index].map(|group| graph.clusters[group].id.clone()),
            outline: body.outline.clone(),
            rect,
            texts,
            classes: node.classes.clone(),
            link: node.link.clone(),
        });
    }
    let clusters: Vec<ClusterBox> = graph
        .clusters
        .iter()
        .enumerate()
        .map(|(group, cluster)| {
            let rect = frame.rect(placement.group_bounds[group], group_depth[group]);
            ClusterBox {
                id: cluster.id.clone(),
                parent: group_parent[group].map(|parent| graph.clusters[parent].id.clone()),
                depth: ancestors(&group_parent, Some(group)).count() - 1,
                rect,
                texts: headers[group].texts(rect.x + CLUSTER_PAD, rect.y + CLUSTER_PAD),
                classes: cluster.classes.clone(),
                link: cluster.link.clone(),
            }
        })
        .collect();

    let router = Router {
        tracks: &tracks,
        vertices: &vertices,
        rows: &rows,
        group_depth: &group_depth,
        cap: &cap,
    };
    let mut edges = Vec::new();
    for (index, chain) in chains.iter().enumerate() {
        if !chain.drawn {
            continue;
        }
        let points = router.points(index, chain, &paths[index]);
        let mut points: Vec<Point> = points
            .into_iter()
            .map(|(breadth, depth)| frame.point(breadth, depth))
            .collect();
        if chain.reversed {
            points.reverse();
        }
        let label = chain.label_vertex.map(|vertex| {
            let label = &labels[&chain.edge];
            let rect = frame.centered(
                center[vertex],
                rows.middle(vertices[vertex].rank),
                label.width,
                label.height,
            );
            EdgeLabel {
                rect,
                texts: label.texts(rect.x + rect.width / 2.0, rect.y),
            }
        });
        edges.push(edge_path(graph, chain.edge, simplify(points), label));
    }
    let rect_of = |end: End| match end {
        End::Node(node) => nodes[node].rect,
        End::Group(group) => clusters[group].rect,
    };
    for &(index, from, to) in &directs {
        if graph.edges[index].stroke == Stroke::Invisible {
            continue;
        }
        let (a, b) = (rect_of(from), rect_of(to));
        let points = vec![clip(&a, b.center()), clip(&b, a.center())];
        edges.push(edge_path(graph, index, points, None));
    }
    for &(index, node) in &loops {
        if graph.edges[index].stroke == Stroke::Invisible {
            continue;
        }
        let side = center[node] + vertices[node].breadth / 2.0;
        let middle = rows.middle(vertices[node].rank);
        let points = [
            (side, middle - LOOP_SPREAD),
            (side + LOOP_REACH, middle - LOOP_SPREAD),
            (side + LOOP_REACH, middle + LOOP_SPREAD),
            (side, middle + LOOP_SPREAD),
        ]
        .into_iter()
        .map(|(breadth, depth)| frame.point(breadth, depth))
        .collect();
        edges.push(edge_path(graph, index, points, None));
    }

    let (width, height) = if frame.vertical() {
        (frame.breadth, frame.depth)
    } else {
        (frame.depth, frame.breadth)
    };
    Scene {
        width: width + 2.0 * MARGIN,
        height: height + 2.0 * MARGIN,
        clusters,
        nodes,
        edges,
        guides: Vec::new(),
    }
}

fn edge_path(
    graph: &Graph,
    index: usize,
    points: Vec<Point>,
    label: Option<EdgeLabel>,
) -> EdgePath {
    let edge = &graph.edges[index];
    EdgePath {
        from: edge.from.clone(),
        to: edge.to.clone(),
        points,
        stroke: edge.stroke,
        tail: edge.tail,
        head: edge.head,
        label,
    }
}

struct Router<'a> {
    tracks: &'a route::Tracks,
    vertices: &'a [Vertex],
    rows: &'a Rows,
    group_depth: &'a [(f32, f32)],
    cap: &'a dyn Fn(usize, f32) -> f32,
}

impl Router<'_> {
    fn points(&self, index: usize, chain: &Chain, path: &[(f32, f32)]) -> Vec<(f32, f32)> {
        let first_vertex = chain.vertices[0];
        let last_vertex = *chain.vertices.last().expect("a chain has two vertices");
        let (first, last) = (&self.vertices[first_vertex], &self.vertices[last_vertex]);
        let exit = path[0].0;
        let entry = path[path.len() - 1].1;
        let start = match chain.upper_cluster {
            Some(group) => self.group_depth[group].1,
            None => {
                self.rows.middle(first.rank) + first.depth / 2.0 - (self.cap)(first_vertex, exit)
            }
        };
        let finish = match chain.lower_cluster {
            Some(group) => self.group_depth[group].0,
            None => self.rows.middle(last.rank) - last.depth / 2.0 + (self.cap)(last_vertex, entry),
        };
        let mut points = vec![(exit, start)];
        for (at, &(from, to)) in path.iter().enumerate() {
            let rank = self.vertices[chain.vertices[at]].rank;
            if at > 0 {
                points.push((from, self.rows.top[rank]));
                points.push((from, self.rows.top[rank] + self.rows.row[rank]));
            }
            if from != to {
                let track = self.tracks.of[&(index, at)];
                let depth = self.rows.track(rank, track, self.tracks.count[rank]);
                points.push((from, depth));
                points.push((to, depth));
            }
        }
        points.push((entry, finish));
        points
    }
}

fn simplify(points: Vec<Point>) -> Vec<Point> {
    let mut kept: Vec<Point> = Vec::with_capacity(points.len());
    for point in points {
        if kept
            .last()
            .is_some_and(|last| (last.x - point.x).abs() < 0.01 && (last.y - point.y).abs() < 0.01)
        {
            continue;
        }
        if kept.len() >= 2 {
            let (a, b) = (kept[kept.len() - 2], kept[kept.len() - 1]);
            let vertical = (a.x - b.x).abs() < 0.01 && (b.x - point.x).abs() < 0.01;
            let horizontal = (a.y - b.y).abs() < 0.01 && (b.y - point.y).abs() < 0.01;
            if vertical || horizontal {
                kept.pop();
            }
        }
        kept.push(point);
    }
    kept
}

// Where the line from the center of `rect` toward `toward` leaves the rect.
fn clip(rect: &Rect, toward: Point) -> Point {
    let center = rect.center();
    let (dx, dy) = (toward.x - center.x, toward.y - center.y);
    if dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
        return center;
    }
    let scale_x = if dx.abs() > f32::EPSILON {
        (rect.width / 2.0) / dx.abs()
    } else {
        f32::INFINITY
    };
    let scale_y = if dy.abs() > f32::EPSILON {
        (rect.height / 2.0) / dy.abs()
    } else {
        f32::INFINITY
    };
    let scale = scale_x.min(scale_y);
    Point {
        x: center.x + dx * scale,
        y: center.y + dy * scale,
    }
}

// Each rank is one row across the whole drawing. Above a row sit the headers of the clusters that
// start in it, below it the bottom padding of those that end in it, and below that the channel the
// edges turn in on the way to the next row.
struct Rows {
    row: Vec<f32>,
    before: Vec<f32>,
    after: Vec<f32>,
    channel: Vec<f32>,
    top: Vec<f32>,
}

impl Rows {
    fn new(
        vertices: &[Vertex],
        parents: &[Level],
        span: &[(usize, usize)],
        pads: &[Pad],
        ranks: usize,
        doubled: bool,
        tracks: &[usize],
    ) -> Rows {
        let mut row = vec![0.0f32; ranks];
        let mut before = vec![0.0f32; ranks];
        let mut after = vec![0.0f32; ranks];
        for vertex in vertices {
            row[vertex.rank] = row[vertex.rank].max(vertex.depth);
            let (mut above, mut below) = (0.0, 0.0);
            for group in ancestors(parents, vertex.group) {
                if span[group].0 == vertex.rank {
                    above += pads[group].depth.0;
                }
                if span[group].1 == vertex.rank {
                    below += pads[group].depth.1;
                }
            }
            before[vertex.rank] = before[vertex.rank].max(above);
            after[vertex.rank] = after[vertex.rank].max(below);
        }
        let least = if doubled { RANK_GAP / 2.0 } else { RANK_GAP };
        let channel = (0..ranks)
            .map(|rank| least.max((tracks[rank] + 1) as f32 * TRACK_GAP))
            .collect();
        let mut rows = Rows {
            row,
            before,
            after,
            channel,
            top: Vec::new(),
        };
        rows.stack();
        rows
    }

    fn stack(&mut self) {
        let mut at = 0.0;
        self.top.clear();
        for rank in 0..self.row.len() {
            at += self.before[rank];
            self.top.push(at);
            at += self.row[rank] + self.after[rank] + self.channel[rank];
        }
    }

    // In a diagram that runs sideways, a cluster header lies along the ranks, so a cluster must
    // span enough depth for it. Its last row grows until it does.
    fn fit_headers(
        mut self,
        headers: &[size::Header],
        span: &[(usize, usize)],
        vertical: bool,
        vertices: &[Vertex],
        parents: &[Level],
        pads: &[Pad],
    ) -> Rows {
        if vertical {
            return self;
        }
        for _ in 0..=headers.len() {
            let depths = self.group_depths(vertices, parents, pads, headers.len());
            let mut grew = false;
            for (group, header) in headers.iter().enumerate() {
                let need = header.width() + 2.0 * CLUSTER_PAD;
                let have = depths[group].1 - depths[group].0;
                if have + 0.5 < need {
                    self.row[span[group].1] += need - have;
                    grew = true;
                }
            }
            if !grew {
                break;
            }
            self.stack();
        }
        self
    }

    fn middle(&self, rank: usize) -> f32 {
        self.top[rank] + self.row[rank] / 2.0
    }

    fn total(&self) -> f32 {
        let last = self.row.len() - 1;
        self.top[last] + self.row[last] + self.after[last]
    }

    fn track(&self, rank: usize, track: usize, count: usize) -> f32 {
        let start = self.top[rank] + self.row[rank] + self.after[rank];
        let end = self
            .top
            .get(rank + 1)
            .map_or(start + self.channel[rank], |next| {
                next - self.before[rank + 1]
            });
        start + (end - start) * (track + 1) as f32 / (count + 1) as f32
    }

    fn group_depths(
        &self,
        vertices: &[Vertex],
        parents: &[Level],
        pads: &[Pad],
        groups: usize,
    ) -> Vec<(f32, f32)> {
        let mut bounds = vec![(f32::INFINITY, f32::NEG_INFINITY); groups];
        for vertex in vertices.iter().filter(|vertex| !vertex.on_edge()) {
            if let Some(group) = vertex.group {
                let bound = &mut bounds[group];
                bound.0 = bound.0.min(self.top[vertex.rank]);
                bound.1 = bound.1.max(self.top[vertex.rank] + self.row[vertex.rank]);
            }
        }
        let depth: Vec<usize> = (0..groups)
            .map(|group| ancestors(parents, Some(group)).count())
            .collect();
        let mut bottom_up: Vec<usize> = (0..groups).collect();
        bottom_up.sort_by_key(|group| std::cmp::Reverse(depth[*group]));
        for group in bottom_up {
            let (top, bottom) = bounds[group];
            let (top, bottom) = if top.is_finite() {
                (top, bottom)
            } else {
                (0.0, 0.0)
            };
            let padded = (top - pads[group].depth.0, bottom + pads[group].depth.1);
            bounds[group] = padded;
            if let Some(parent) = parents[group] {
                let outer = &mut bounds[parent];
                outer.0 = outer.0.min(padded.0);
                outer.1 = outer.1.max(padded.1);
            }
        }
        bounds
    }
}
