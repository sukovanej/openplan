use std::collections::HashMap;

use super::order::{Entry, Level, Ordering};

const NODE_GAP: f32 = 40.0;
const THIN_GAP: f32 = 16.0;
const ROUNDS: usize = 40;

pub(super) struct Sizes<'a> {
    pub(super) vertex_breadth: &'a [f32],
    pub(super) vertex_thin: &'a [bool],
    pub(super) vertex_room_after: &'a [f32],
    pub(super) vertex_group: &'a [Level],
    pub(super) group_parent: &'a [Level],
    pub(super) group_pad: &'a [(f32, f32)],
    pub(super) group_min_breadth: &'a [f32],
    pub(super) links: &'a [Link],
    pub(super) paths: &'a [Path<'a>],
}

// One step of an edge from a vertex to one in the next rank. A port is where the step meets its
// vertex, as an offset from the center of that vertex.
pub(super) struct Link {
    pub(super) upper: usize,
    pub(super) lower: usize,
    pub(super) weight: f32,
    pub(super) upper_port: f32,
    pub(super) lower_port: f32,
}

pub(super) struct Path<'a> {
    pub(super) vertices: &'a [usize],
    pub(super) exit: f32,
    pub(super) entry: f32,
}

pub(super) struct Placement {
    pub(super) center: Vec<f32>,
    pub(super) group_bounds: Vec<(f32, f32)>,
    pub(super) breadth: f32,
}

// A link that crosses the edge of an entry, from the vertex and port inside it to those outside.
struct Touch {
    inside: (usize, f32),
    outside: (usize, f32),
    weight: f32,
}

// The entries next to one entry in one of its rows.
#[derive(Clone, Copy)]
struct Beside {
    before: Option<Entry>,
    after: Option<Entry>,
}

// The inside of an edge lines up with one end, so the edge meets that end straight and bends once,
// next to the other. It takes the end that no other edge shares that side of, since the edges that
// share a side spread along it and bend there anyway. When both ends are free, it takes the one with
// more edges, which holds its place while the other end follows.
enum Line {
    Upper,
    Lower,
    Between,
}

struct Run<'a> {
    inside: &'a [usize],
    upper: usize,
    lower: usize,
    exit: f32,
    entry: f32,
    line: Line,
}

fn runs<'a>(paths: &[Path<'a>]) -> Vec<Run<'a>> {
    let mut leaving: HashMap<usize, usize> = HashMap::new();
    let mut arriving: HashMap<usize, usize> = HashMap::new();
    for path in paths {
        let vertices = path.vertices;
        *leaving.entry(vertices[0]).or_default() += 1;
        *arriving.entry(vertices[vertices.len() - 1]).or_default() += 1;
    }
    paths
        .iter()
        .filter(|path| path.vertices.len() > 2)
        .map(|path| {
            let vertices = path.vertices;
            let (upper, lower) = (vertices[0], vertices[vertices.len() - 1]);
            let edges = |vertex| {
                leaving.get(&vertex).copied().unwrap_or(0)
                    + arriving.get(&vertex).copied().unwrap_or(0)
            };
            let line = match (leaving[&upper] == 1, arriving[&lower] == 1) {
                (true, true) if edges(lower) > edges(upper) => Line::Lower,
                (true, _) => Line::Upper,
                (false, true) => Line::Lower,
                (false, false) => Line::Between,
            };
            Run {
                inside: &vertices[1..vertices.len() - 1],
                upper,
                lower,
                exit: path.exit,
                entry: path.entry,
                line,
            }
        })
        .collect()
}

struct Levels<'a> {
    sizes: &'a Sizes<'a>,
    runs: Vec<Run<'a>>,
    left: HashMap<(Level, Entry), f32>,
    outer: Vec<f32>,
    content_offset: Vec<f32>,
}

pub(super) fn place(ordering: &Ordering, sizes: &Sizes<'_>) -> Placement {
    let groups = sizes.group_parent.len();
    let mut rows: HashMap<Level, Vec<(usize, &Vec<Entry>)>> = HashMap::new();
    for ((level, rank), entries) in &ordering.levels {
        rows.entry(*level).or_default().push((*rank, entries));
    }
    for level_rows in rows.values_mut() {
        level_rows.sort_by_key(|(rank, _)| *rank);
    }
    let mut levels = Levels {
        sizes,
        runs: runs(sizes.paths),
        left: HashMap::new(),
        outer: vec![0.0; groups],
        content_offset: vec![0.0; groups],
    };
    let depth: Vec<usize> = (0..groups)
        .map(|group| chain(sizes.group_parent, Some(group)).count())
        .collect();
    let mut bottom_up: Vec<usize> = (0..groups).collect();
    bottom_up.sort_by_key(|group| std::cmp::Reverse(depth[*group]));
    for group in bottom_up {
        let level_rows: Vec<&Vec<Entry>> = rows
            .get(&Some(group))
            .into_iter()
            .flatten()
            .map(|(_, entries)| *entries)
            .collect();
        let content = levels.place_level(Some(group), &level_rows);
        let (before, after) = sizes.group_pad[group];
        let outer = (content + before + after).max(sizes.group_min_breadth[group]);
        levels.outer[group] = outer;
        levels.content_offset[group] = before + (outer - content - before - after) / 2.0;
    }
    let root_rows: Vec<&Vec<Entry>> = rows
        .get(&None)
        .into_iter()
        .flatten()
        .map(|(_, entries)| *entries)
        .collect();
    let breadth = levels.place_level(None, &root_rows);

    let mut content_left = vec![0.0; groups];
    let mut group_bounds = vec![(0.0, 0.0); groups];
    let mut top_down: Vec<usize> = (0..groups).collect();
    top_down.sort_by_key(|group| depth[*group]);
    for group in top_down {
        let parent = sizes.group_parent[group];
        let parent_content = parent.map_or(0.0, |parent| content_left[parent]);
        let outer_left = parent_content + levels.left[&(parent, Entry::Group(group))];
        group_bounds[group] = (outer_left, outer_left + levels.outer[group]);
        content_left[group] = outer_left + levels.content_offset[group];
    }
    let center = (0..sizes.vertex_breadth.len())
        .map(|vertex| {
            let group = sizes.vertex_group[vertex];
            group.map_or(0.0, |group| content_left[group])
                + levels
                    .left
                    .get(&(group, Entry::Vertex(vertex)))
                    .copied()
                    .unwrap_or(0.0)
                + sizes.vertex_breadth[vertex] / 2.0
        })
        .collect();
    Placement {
        center,
        group_bounds,
        breadth,
    }
}

// When the weight splits evenly between two values, any point between them is as short, and the
// middle of the two keeps a parent centered over two children.
fn weighted_median(mut values: Vec<(f32, f32)>) -> Option<f32> {
    let total: f32 = values.iter().map(|(_, weight)| weight).sum();
    if total <= 0.0 {
        return None;
    }
    values.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut below = 0.0;
    for (at, &(value, weight)) in values.iter().enumerate() {
        below += weight;
        if (below - total / 2.0).abs() < 1e-3 {
            return Some(
                values
                    .get(at + 1)
                    .map_or(value, |next| (value + next.0) / 2.0),
            );
        }
        if below > total / 2.0 {
            return Some(value);
        }
    }
    values.last().map(|(value, _)| *value)
}

fn chain(parents: &[Level], start: Level) -> impl Iterator<Item = usize> + '_ {
    std::iter::successors(start, move |group| parents[*group])
}

impl Levels<'_> {
    fn width(&self, entry: Entry) -> f32 {
        match entry {
            Entry::Vertex(vertex) => self.sizes.vertex_breadth[vertex],
            Entry::Group(group) => self.outer[group],
        }
    }

    fn thin(&self, entry: Entry) -> bool {
        matches!(entry, Entry::Vertex(vertex) if self.sizes.vertex_thin[vertex])
    }

    // What a vertex draws beside itself, such as a loop and its labels.
    fn room_after(&self, entry: Entry) -> f32 {
        match entry {
            Entry::Vertex(vertex) => self.sizes.vertex_room_after[vertex],
            Entry::Group(_) => 0.0,
        }
    }

    fn gap(&self, before: Entry, after: Entry) -> f32 {
        self.room_after(before)
            + if self.thin(before) || self.thin(after) {
                THIN_GAP
            } else {
                NODE_GAP
            }
    }

    // Where the center of a vertex sits in the frame of `level`, or nothing when the vertex lies
    // outside it.
    fn position(&self, level: Level, vertex: usize) -> Option<f32> {
        let mut group = self.sizes.vertex_group[vertex];
        let mut at = self.left.get(&(group, Entry::Vertex(vertex)))?
            + self.sizes.vertex_breadth[vertex] / 2.0;
        while group != level {
            let inner = group?;
            let parent = self.sizes.group_parent[inner];
            at += self.content_offset[inner] + self.left.get(&(parent, Entry::Group(inner)))?;
            group = parent;
        }
        Some(at)
    }

    // The entry of `level` that holds a vertex: the vertex itself, or the child group it lies in.
    fn entry_of(&self, level: Level, vertex: usize) -> Option<Entry> {
        let mut entry = Entry::Vertex(vertex);
        let mut group = self.sizes.vertex_group[vertex];
        while group != level {
            let inner = group?;
            entry = Entry::Group(inner);
            group = self.sizes.group_parent[inner];
        }
        Some(entry)
    }

    fn place_level(&mut self, level: Level, rows: &[&Vec<Entry>]) -> f32 {
        let mut entries: Vec<Entry> = Vec::new();
        let mut neighbors: HashMap<Entry, Vec<Beside>> = HashMap::new();
        for row in rows {
            for (at, entry) in row.iter().enumerate() {
                if !entries.contains(entry) {
                    entries.push(*entry);
                }
                let before = at.checked_sub(1).map(|before| row[before]);
                neighbors.entry(*entry).or_default().push(Beside {
                    before,
                    after: row.get(at + 1).copied(),
                });
            }
        }
        let mut touching: HashMap<Entry, Vec<Touch>> = HashMap::new();
        for link in self.sizes.links {
            let (Some(first), Some(second)) = (
                self.entry_of(level, link.upper),
                self.entry_of(level, link.lower),
            ) else {
                continue;
            };
            if first != second {
                touching.entry(first).or_default().push(Touch {
                    inside: (link.upper, link.upper_port),
                    outside: (link.lower, link.lower_port),
                    weight: link.weight,
                });
                touching.entry(second).or_default().push(Touch {
                    inside: (link.lower, link.lower_port),
                    outside: (link.upper, link.upper_port),
                    weight: link.weight,
                });
            }
        }
        let mut run_of: HashMap<Entry, usize> = HashMap::new();
        for (at, run) in self.runs.iter().enumerate() {
            for &vertex in run.inside {
                run_of.insert(Entry::Vertex(vertex), at);
            }
        }
        for entry in &entries {
            self.left.insert((level, *entry), 0.0);
        }
        self.pack(level, rows, entries.len());
        for _ in 0..ROUNDS {
            let mut moved = 0.0f32;
            for &entry in &entries {
                let straight = match (entry, run_of.get(&entry)) {
                    (Entry::Vertex(vertex), Some(&run)) => self
                        .line_up(level, run, &neighbors)
                        .and_then(|line| Some(line - self.position(level, vertex)?)),
                    _ => None,
                };
                let Some(shift) = straight.or_else(|| self.pull(level, touching.get(&entry)))
                else {
                    continue;
                };
                let (low, high) = self.room(level, entry, &neighbors[&entry]);
                let current = self.left[&(level, entry)];
                let next = (current + shift).clamp(low, high.max(low));
                moved = moved.max((next - current).abs());
                self.left.insert((level, entry), next);
            }
            if moved < 0.5 {
                break;
            }
        }
        let lowest = entries
            .iter()
            .map(|entry| self.left[&(level, *entry)])
            .fold(f32::INFINITY, f32::min);
        let lowest = if lowest.is_finite() { lowest } else { 0.0 };
        let mut breadth: f32 = 0.0;
        for &entry in &entries {
            let left = self.left[&(level, entry)] - lowest;
            self.left.insert((level, entry), left);
            breadth = breadth.max(left + self.width(entry) + self.room_after(entry));
        }
        breadth
    }

    fn pack(&mut self, level: Level, rows: &[&Vec<Entry>], count: usize) {
        for _ in 0..=count {
            let mut changed = false;
            for row in rows {
                for pair in row.windows(2) {
                    let need = self.left[&(level, pair[0])]
                        + self.width(pair[0])
                        + self.gap(pair[0], pair[1]);
                    if self.left[&(level, pair[1])] < need {
                        self.left.insert((level, pair[1]), need);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    // The weighted median of how far the port of each neighbor outside the entry sits from the port
    // inside it that it links to: the shift that makes the links shortest in total. A mean would
    // put every parent halfway between its children, and a chain of them would zigzag. A link to a
    // long edge weighs more, so long edges run straight.
    fn pull(&self, level: Level, touches: Option<&Vec<Touch>>) -> Option<f32> {
        let offsets = touches?
            .iter()
            .filter_map(|touch| {
                let port =
                    |(vertex, port): (usize, f32)| Some(self.position(level, vertex)? + port);
                Some((port(touch.outside)? - port(touch.inside)?, touch.weight))
            })
            .collect();
        weighted_median(offsets)
    }

    // The line nearest its chosen end that the whole inside of a run fits on, or nothing while
    // the vertices beside it leave no such line.
    fn line_up(
        &self,
        level: Level,
        run: usize,
        neighbors: &HashMap<Entry, Vec<Beside>>,
    ) -> Option<f32> {
        let run = &self.runs[run];
        let target = match run.line {
            Line::Upper => self.position(level, run.upper)? + run.exit,
            Line::Lower => self.position(level, run.lower)? + run.entry,
            Line::Between => {
                (self.position(level, run.upper)?
                    + run.exit
                    + self.position(level, run.lower)?
                    + run.entry)
                    / 2.0
            }
        };
        let (mut lowest, mut highest) = (f32::NEG_INFINITY, f32::INFINITY);
        for &vertex in run.inside {
            let entry = Entry::Vertex(vertex);
            let (low, high) = self.room(level, entry, &neighbors[&entry]);
            let half = self.sizes.vertex_breadth[vertex] / 2.0;
            lowest = lowest.max(low + half);
            highest = highest.min(high + half);
        }
        (lowest <= highest).then(|| target.clamp(lowest, highest))
    }

    fn room(&self, level: Level, entry: Entry, neighbors: &[Beside]) -> (f32, f32) {
        let mut low = f32::NEG_INFINITY;
        let mut high = f32::INFINITY;
        for &Beside { before, after } in neighbors {
            if let Some(before) = before {
                low = low.max(
                    self.left[&(level, before)] + self.width(before) + self.gap(before, entry),
                );
            }
            if let Some(after) = after {
                high = high
                    .min(self.left[&(level, after)] - self.gap(entry, after) - self.width(entry));
            }
        }
        (low, high)
    }
}
