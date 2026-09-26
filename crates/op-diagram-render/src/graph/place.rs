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
    pub(super) links: &'a [(usize, usize, f32)],
}

pub(super) struct Placement {
    pub(super) center: Vec<f32>,
    pub(super) group_bounds: Vec<(f32, f32)>,
    pub(super) breadth: f32,
}

// The entries next to one entry in one of its rows.
#[derive(Clone, Copy)]
struct Beside {
    before: Option<Entry>,
    after: Option<Entry>,
}

struct Levels<'a> {
    sizes: &'a Sizes<'a>,
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
        let mut touching: HashMap<Entry, Vec<(usize, usize, f32)>> = HashMap::new();
        for &(upper, lower, strength) in self.sizes.links {
            let (Some(first), Some(second)) =
                (self.entry_of(level, upper), self.entry_of(level, lower))
            else {
                continue;
            };
            if first != second {
                touching
                    .entry(first)
                    .or_default()
                    .push((upper, lower, strength));
                touching
                    .entry(second)
                    .or_default()
                    .push((lower, upper, strength));
            }
        }
        for entry in &entries {
            self.left.insert((level, *entry), 0.0);
        }
        self.pack(level, rows, entries.len());
        for _ in 0..ROUNDS {
            let mut moved = 0.0f32;
            for &entry in &entries {
                let Some(shift) = self.pull(level, touching.get(&entry)) else {
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

    // The weighted median of how far each neighbor outside the entry sits from the vertex inside it
    // that it links to: the shift that makes the links shortest in total. A mean would put every
    // parent halfway between its children, and a chain of them would zigzag. A link to a long edge
    // weighs more, so long edges run straight.
    fn pull(&self, level: Level, links: Option<&Vec<(usize, usize, f32)>>) -> Option<f32> {
        let offsets = links?
            .iter()
            .filter_map(|&(inside, outside, strength)| {
                Some((
                    self.position(level, outside)? - self.position(level, inside)?,
                    strength,
                ))
            })
            .collect();
        weighted_median(offsets)
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
