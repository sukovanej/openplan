use std::collections::HashMap;

pub(super) type Level = Option<usize>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Entry {
    Vertex(usize),
    Group(usize),
}

pub(super) struct Structure<'a> {
    pub(super) ranks: usize,
    pub(super) vertex_rank: &'a [usize],
    pub(super) vertex_group: &'a [Level],
    pub(super) group_parent: &'a [Level],
    pub(super) group_span: &'a [(usize, usize)],
    pub(super) links: &'a [(usize, usize)],
}

pub(super) struct Ordering {
    pub(super) ranks: Vec<Vec<usize>>,
    pub(super) levels: HashMap<(Level, usize), Vec<Entry>>,
}

const SWEEPS: usize = 12;

pub(super) fn order(structure: &Structure<'_>, initial_keys: Vec<f32>) -> Ordering {
    let tree = Tree::new(structure);
    let mut keys = initial_keys;
    let mut arranged = tree.arrange_all(&mut keys);
    let mut best = (
        crossings(structure, &arranged.ranks),
        arranged.ranks.clone(),
        keys.clone(),
    );
    let mut above = vec![Vec::new(); keys.len()];
    let mut below = vec![Vec::new(); keys.len()];
    for &(upper, lower) in structure.links {
        below[upper].push(lower);
        above[lower].push(upper);
    }
    for sweep in 0..SWEEPS {
        let downward = sweep % 2 == 0;
        let ranks: Vec<usize> = if downward {
            (1..structure.ranks).collect()
        } else {
            (0..structure.ranks.saturating_sub(1)).rev().collect()
        };
        for rank in ranks {
            let neighbors = if downward { &above } else { &below };
            let position = positions(keys.len(), &arranged.ranks);
            for &vertex in &arranged.ranks[rank] {
                let adjacent = &neighbors[vertex];
                if !adjacent.is_empty() {
                    keys[vertex] = adjacent.iter().map(|near| position[*near]).sum::<f32>()
                        / adjacent.len() as f32;
                }
            }
            arranged = tree.arrange_all(&mut keys);
        }
        let count = crossings(structure, &arranged.ranks);
        if count < best.0 {
            best = (count, arranged.ranks.clone(), keys.clone());
        }
    }
    let mut keys = best.2;
    tree.arrange_all(&mut keys)
}

fn positions(count: usize, ranks: &[Vec<usize>]) -> Vec<f32> {
    let mut position = vec![0.0; count];
    for rank in ranks {
        for (at, &vertex) in rank.iter().enumerate() {
            position[vertex] = at as f32;
        }
    }
    position
}

fn crossings(structure: &Structure<'_>, ranks: &[Vec<usize>]) -> usize {
    let position = positions(structure.vertex_rank.len(), ranks);
    let mut by_rank: Vec<Vec<(f32, f32)>> = vec![Vec::new(); structure.ranks];
    for &(upper, lower) in structure.links {
        by_rank[structure.vertex_rank[upper]].push((position[upper], position[lower]));
    }
    by_rank
        .iter()
        .map(|links| {
            let mut count = 0;
            for (at, first) in links.iter().enumerate() {
                for second in &links[at + 1..] {
                    if (first.0 - second.0) * (first.1 - second.1) < 0.0 {
                        count += 1;
                    }
                }
            }
            count
        })
        .sum()
}

struct Tree<'a> {
    structure: &'a Structure<'a>,
    children: HashMap<Level, Vec<usize>>,
    members: HashMap<(Level, usize), Vec<usize>>,
    descendants: Vec<Vec<usize>>,
}

impl<'a> Tree<'a> {
    fn new(structure: &'a Structure<'a>) -> Tree<'a> {
        let mut children: HashMap<Level, Vec<usize>> = HashMap::new();
        for (group, parent) in structure.group_parent.iter().enumerate() {
            children.entry(*parent).or_default().push(group);
        }
        let mut members: HashMap<(Level, usize), Vec<usize>> = HashMap::new();
        let mut descendants = vec![Vec::new(); structure.group_parent.len()];
        for (vertex, group) in structure.vertex_group.iter().enumerate() {
            members
                .entry((*group, structure.vertex_rank[vertex]))
                .or_default()
                .push(vertex);
            let mut at = *group;
            while let Some(group) = at {
                descendants[group].push(vertex);
                at = structure.group_parent[group];
            }
        }
        Tree {
            structure,
            children,
            members,
            descendants,
        }
    }

    // Each group has one key for all of its ranks, the mean of the keys of its vertices, so two
    // sibling groups keep one side of each other on every rank they share.
    fn arrange_all(&self, keys: &mut [f32]) -> Ordering {
        let group_keys: Vec<f32> = self
            .descendants
            .iter()
            .map(|vertices| match vertices.len() {
                0 => 0.0,
                count => vertices.iter().map(|vertex| keys[*vertex]).sum::<f32>() / count as f32,
            })
            .collect();
        let mut levels = HashMap::new();
        let ranks: Vec<Vec<usize>> = (0..self.structure.ranks)
            .map(|rank| {
                let mut flat = Vec::new();
                self.arrange(None, rank, keys, &group_keys, &mut levels, &mut flat);
                flat
            })
            .collect();
        for rank in &ranks {
            for (at, &vertex) in rank.iter().enumerate() {
                keys[vertex] = at as f32;
            }
        }
        Ordering { ranks, levels }
    }

    fn arrange(
        &self,
        level: Level,
        rank: usize,
        keys: &[f32],
        group_keys: &[f32],
        levels: &mut HashMap<(Level, usize), Vec<Entry>>,
        flat: &mut Vec<usize>,
    ) {
        let mut entries: Vec<(f32, Entry)> = self
            .members
            .get(&(level, rank))
            .into_iter()
            .flatten()
            .map(|vertex| (keys[*vertex], Entry::Vertex(*vertex)))
            .collect();
        for &group in self.children.get(&level).into_iter().flatten() {
            let (first, last) = self.structure.group_span[group];
            if first <= rank && rank <= last {
                entries.push((group_keys[group], Entry::Group(group)));
            }
        }
        entries.sort_by(|a, b| a.0.total_cmp(&b.0));
        let entries: Vec<Entry> = entries.into_iter().map(|(_, entry)| entry).collect();
        for entry in &entries {
            match *entry {
                Entry::Vertex(vertex) => flat.push(vertex),
                Entry::Group(group) => {
                    self.arrange(Some(group), rank, keys, group_keys, levels, flat)
                }
            }
        }
        levels.insert((level, rank), entries);
    }
}
