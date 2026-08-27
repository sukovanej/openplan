use crate::order::{id_cmp, rank_cmp};
use crate::task::{Coordinate, TaskListItem, coordinate};

use super::FlowCycles;
use super::family::{Family, leaves_under};
use super::growth::{Members, TaskIndex, dependency_target, remaining_dependencies};

// Where each leaf sits: the wave it belongs to, the order inside that wave, and how much work waits
// for it.
pub(crate) struct Layout<'a> {
    pub(crate) leaves: Vec<&'a TaskListItem>,
    pub(crate) waves: Vec<Vec<usize>>,
    pub(crate) blocks: Vec<usize>,
}

impl<'a> Layout<'a> {
    pub(crate) fn build(
        leaves: Vec<&'a TaskListItem>,
        included: &Members<'a>,
        family: &Family<'a>,
        index: &TaskIndex<'a>,
    ) -> Result<Layout<'a>, FlowCycles> {
        let place: std::collections::HashMap<Coordinate<'a>, usize> = leaves
            .iter()
            .enumerate()
            .map(|(at, leaf)| (coordinate(leaf), at))
            .collect();
        let successors = waits_for(&leaves, included, family, index, &place);
        let wave_of = layer(&successors).map_err(|cycles| FlowCycles {
            cycles: cycle_ids(cycles, &leaves),
        })?;
        let blocks = blocks_counts(&successors, &wave_of);

        let depth = wave_of.iter().copied().max().map_or(0, |last| last + 1);
        let mut waves = vec![Vec::new(); depth];
        for (leaf, wave) in wave_of.iter().copied().enumerate() {
            waves[wave].push(leaf);
        }
        for wave in &mut waves {
            wave.sort_by(|a, b| {
                blocks[*b].cmp(&blocks[*a]).then_with(|| {
                    rank_cmp(
                        leaves[*a].metadata.rank(),
                        leaves[*b].metadata.rank(),
                        || {
                            id_cmp(&leaves[*a].id, &leaves[*b].id)
                                .then_with(|| leaves[*a].project.cmp(&leaves[*b].project))
                        },
                    )
                })
            });
        }
        Ok(Layout {
            leaves,
            waves,
            blocks,
        })
    }
}

// Which leaves wait for which. A child inherits the dependencies of its parents, so nothing inside a
// box starts before the box may start; a dependency on a box waits for each leaf inside it. An
// unresolved dependency adds no edge here: no work completes it, so it must not push the task that
// names it into a later wave.
fn waits_for<'a>(
    leaves: &[&'a TaskListItem],
    included: &Members<'a>,
    family: &Family<'a>,
    index: &TaskIndex<'a>,
    place: &std::collections::HashMap<Coordinate<'a>, usize>,
) -> Vec<Vec<usize>> {
    let mut successors = vec![std::collections::BTreeSet::new(); leaves.len()];
    for (at, leaf) in leaves.iter().enumerate() {
        for source in std::iter::once(*leaf).chain(family.above(leaf, index)) {
            for dependency in remaining_dependencies(source) {
                let Some(target) = dependency_target(index, source, dependency) else {
                    continue;
                };
                if !included.contains(&coordinate(target)) {
                    continue;
                }
                for blocker in leaves_under(target, family) {
                    if let Some(from) = place.get(&coordinate(blocker)) {
                        successors[*from].insert(at);
                    }
                }
            }
        }
    }
    successors
        .into_iter()
        .map(|targets| targets.into_iter().collect())
        .collect()
}

// Longest-path layering: a task lands one wave behind the last thing it waits for, so a person can
// start wave `k` once wave `k-1` is complete. The members of a cycle never come ready, and the
// request fails on them rather than answering with an order that does not exist.
fn layer(successors: &[Vec<usize>]) -> Result<Vec<usize>, Vec<Vec<usize>>> {
    let mut waiting = vec![0usize; successors.len()];
    for targets in successors {
        for target in targets {
            waiting[*target] += 1;
        }
    }
    let mut wave = vec![0usize; successors.len()];
    let mut ready: std::collections::BTreeSet<usize> = waiting
        .iter()
        .enumerate()
        .filter(|(_, count)| **count == 0)
        .map(|(at, _)| at)
        .collect();
    let mut layered = 0;
    while let Some(node) = ready.pop_first() {
        layered += 1;
        for target in &successors[node] {
            wave[*target] = wave[*target].max(wave[node] + 1);
            waiting[*target] -= 1;
            if waiting[*target] == 0 {
                ready.insert(*target);
            }
        }
    }
    match layered == successors.len() {
        true => Ok(wave),
        false => Err(rings(
            waiting
                .iter()
                .enumerate()
                .filter(|(_, count)| **count > 0)
                .map(|(at, _)| at)
                .collect(),
            successors,
        )),
    }
}

// The cycles among the leaves layering could not reach. The walk follows one step at a time until it
// meets a task it stands on already, which closes a cycle. It then cuts that one step and walks
// again, so a second cycle through a task of the first one still gets its own report; a walk that
// runs out of steps drops the task it stopped on. Each round cuts a step or drops a task, so the
// search ends. Two cycles over the same tasks read as one report, because the report names the
// members and both would name the same ones.
fn rings(left: Vec<usize>, successors: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut left: std::collections::BTreeSet<usize> = left.into_iter().collect();
    let mut cut: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut named: std::collections::HashSet<std::collections::BTreeSet<usize>> =
        std::collections::HashSet::new();
    let mut cycles = Vec::new();
    while let Some(start) = left.first().copied() {
        let mut path = Vec::new();
        let mut standing: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut current = start;
        loop {
            if let Some(first) = standing.get(&current).copied() {
                let cycle = path.split_off(first);
                cut.insert((cycle[cycle.len() - 1], current));
                if named.insert(cycle.iter().copied().collect()) {
                    cycles.push(cycle);
                }
                break;
            }
            standing.insert(current, path.len());
            path.push(current);
            let step = successors[current]
                .iter()
                .copied()
                .find(|next| left.contains(next) && !cut.contains(&(current, *next)));
            match step {
                Some(next) => current = next,
                None => {
                    left.remove(&current);
                    break;
                }
            }
        }
    }
    cycles
}

// The cycle as keys, turned so the lowest key opens it. The report then reads the same whichever
// member the walk happened to start from.
fn cycle_ids(cycles: Vec<Vec<usize>>, leaves: &[&TaskListItem]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = cycles
        .into_iter()
        .map(|cycle| {
            let first = (0..cycle.len())
                .min_by(|a, b| id_cmp(&leaves[cycle[*a]].id, &leaves[cycle[*b]].id))
                .unwrap_or_default();
            cycle
                .iter()
                .cycle()
                .skip(first)
                .take(cycle.len())
                .map(|node| leaves[*node].id.clone())
                .collect()
        })
        .collect();
    out.sort_by(|a, b| id_cmp(&a[0], &b[0]));
    out
}

// How much work waits for each leaf, directly or through another leaf. It is the first sort key
// inside a wave, so the task that unblocks the most work leads it. A leaf waits only behind lower
// waves, so counting from the last wave back gives each leaf its followers already counted.
fn blocks_counts(successors: &[Vec<usize>], wave_of: &[usize]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..successors.len()).collect();
    order.sort_by_key(|node| std::cmp::Reverse(wave_of[*node]));
    let mut waiting: Vec<std::collections::HashSet<usize>> =
        vec![std::collections::HashSet::new(); successors.len()];
    for node in order {
        let mut all = std::collections::HashSet::new();
        for target in &successors[node] {
            all.insert(*target);
            all.extend(waiting[*target].iter().copied());
        }
        waiting[node] = all;
    }
    waiting.into_iter().map(|all| all.len()).collect()
}
