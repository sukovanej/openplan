use std::collections::HashMap;

use super::order::Level;

const PORT_GAP: f32 = 12.0;
const TRACK_ROOM: f32 = 8.0;
const STRAIGHT: f32 = 0.5;
const SNAP: f32 = 4.0;
const FREE_PORT: f32 = 16.0;
const CORNER_ROOM: f32 = 12.0;

// An edge runs down through the ranks from its upper vertex to its lower one, whatever way its
// arrow points. `reversed` records that the arrow points up.
pub(super) struct Chain {
    pub(super) edge: usize,
    pub(super) vertices: Vec<usize>,
    pub(super) reversed: bool,
    pub(super) upper_cluster: Level,
    pub(super) lower_cluster: Level,
    pub(super) label_vertex: Option<usize>,
    pub(super) drawn: bool,
}

// Where an edge leaves and enters, as offsets from the centers of its ends. A port that is the only
// one on its side can move by `free` toward the line, so the line needs no jog to reach it.
pub(super) struct Ends {
    pub(super) exit: f32,
    pub(super) entry: f32,
    pub(super) exit_free: f32,
    pub(super) entry_free: f32,
}

// Several edges that leave one side of a node spread along that side in the order of the vertices
// they go to, so they do not share one line. A pointed side takes them all at its point.
pub(super) fn ports(
    chains: &[Chain],
    order: &[usize],
    breadth: &[f32],
    spreads: &dyn Fn(usize) -> bool,
) -> Vec<Ends> {
    let mut ends: Vec<Ends> = chains
        .iter()
        .map(|_| Ends {
            exit: 0.0,
            entry: 0.0,
            exit_free: 0.0,
            entry_free: 0.0,
        })
        .collect();
    let free = |vertex: usize| (breadth[vertex] / 2.0 - CORNER_ROOM).clamp(0.0, FREE_PORT);
    let mut leaving: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut arriving: HashMap<usize, Vec<usize>> = HashMap::new();
    for (at, chain) in chains.iter().enumerate() {
        let count = chain.vertices.len();
        leaving.entry(chain.vertices[0]).or_default().push(at);
        arriving
            .entry(chain.vertices[count - 1])
            .or_default()
            .push(at);
    }
    for (vertex, mut group) in leaving {
        if !spreads(vertex) {
            continue;
        }
        if let [only] = group[..] {
            ends[only].exit_free = free(vertex);
            continue;
        }
        group.sort_by_key(|chain| order[chains[*chain].vertices[1]]);
        for (slot, offset) in spread(breadth[vertex], group.len()).into_iter().enumerate() {
            ends[group[slot]].exit = offset;
        }
    }
    for (vertex, mut group) in arriving {
        if !spreads(vertex) {
            continue;
        }
        if let [only] = group[..] {
            ends[only].entry_free = free(vertex);
            continue;
        }
        group.sort_by_key(|chain| {
            let vertices = &chains[*chain].vertices;
            order[vertices[vertices.len() - 2]]
        });
        for (slot, offset) in spread(breadth[vertex], group.len()).into_iter().enumerate() {
            ends[group[slot]].entry = offset;
        }
    }
    ends
}

fn spread(breadth: f32, count: usize) -> Vec<f32> {
    let gap = PORT_GAP.min(breadth * 0.6 / count as f32);
    (0..count)
        .map(|slot| (slot as f32 - (count as f32 - 1.0) / 2.0) * gap)
        .collect()
}

pub(super) struct Tracks {
    pub(super) count: Vec<usize>,
    pub(super) of: HashMap<(usize, usize), usize>,
}

// Where each link of a chain leaves and arrives. A line keeps its course while the next point sits
// within `SNAP` of it, so a few pixels of rounding never show as a jog.
pub(super) fn links(chain: &Chain, ends: &Ends, center: &[f32]) -> Vec<(f32, f32)> {
    let last = chain.vertices.len() - 2;
    let exit = center[chain.vertices[0]] + ends.exit;
    let entry = center[chain.vertices[last + 1]] + ends.entry;
    let target = |at: usize| {
        if at == last {
            entry
        } else {
            center[chain.vertices[at + 1]]
        }
    };
    let mut x = if (exit - target(0)).abs() <= ends.exit_free {
        target(0)
    } else {
        exit
    };
    (0..=last)
        .map(|at| {
            let target = target(at);
            let snap = if at == last {
                SNAP.max(ends.entry_free)
            } else {
                SNAP
            };
            if (x - target).abs() <= snap {
                (x, x)
            } else {
                let link = (x, target);
                x = target;
                link
            }
        })
        .collect()
}

struct Segment {
    low: f32,
    high: f32,
    from: f32,
    to: f32,
    key: (usize, usize),
}

impl Segment {
    fn holds(&self, x: f32) -> bool {
        x > self.low + STRAIGHT && x < self.high - STRAIGHT
    }

    fn meets(&self, other: &Segment) -> bool {
        self.low < other.high + TRACK_ROOM && other.low < self.high + TRACK_ROOM
    }
}

// The horizontal part of each link gets a track of its own in the channel below its upper rank.
// A segment whose line comes down inside another segment must run above it, and one whose line
// goes on down inside another must run below it; with those orders kept, two links cross only
// where no order of tracks could avoid it.
pub(super) fn tracks(
    paths: &[Vec<(f32, f32)>],
    chains: &[Chain],
    rank: &[usize],
    ranks: usize,
) -> Tracks {
    let mut channels: Vec<Vec<Segment>> = (0..ranks).map(|_| Vec::new()).collect();
    for (index, (chain, path)) in chains.iter().zip(paths).enumerate() {
        for (at, &(from, to)) in path.iter().enumerate() {
            if from != to {
                channels[rank[chain.vertices[at]]].push(Segment {
                    low: from.min(to),
                    high: from.max(to),
                    from,
                    to,
                    key: (index, at),
                });
            }
        }
    }
    let mut count = vec![0; ranks];
    let mut of = HashMap::new();
    for (channel, segments) in channels.into_iter().enumerate() {
        let size = segments.len();
        let mut above: Vec<Vec<usize>> = vec![Vec::new(); size];
        for (first, a) in segments.iter().enumerate() {
            for (second, b) in segments.iter().enumerate() {
                if first == second || !a.meets(b) {
                    continue;
                }
                if b.holds(a.from) {
                    above[second].push(first);
                }
                if b.holds(a.to) {
                    above[first].push(second);
                }
            }
        }
        let mut placed: Vec<Option<usize>> = vec![None; size];
        for _ in 0..size {
            let next = (0..size)
                .filter(|at| placed[*at].is_none())
                .min_by(|a, b| {
                    let waiting = |at: usize| {
                        above[at]
                            .iter()
                            .filter(|over| placed[**over].is_none())
                            .count()
                    };
                    waiting(*a)
                        .cmp(&waiting(*b))
                        .then(segments[*a].low.total_cmp(&segments[*b].low))
                })
                .expect("a segment is left to place");
            let lowest = above[next]
                .iter()
                .filter_map(|over| placed[*over])
                .map(|track| track + 1)
                .max()
                .unwrap_or(0);
            let track = (lowest..)
                .find(|track| {
                    (0..size).all(|other| {
                        placed[other] != Some(*track) || !segments[other].meets(&segments[next])
                    })
                })
                .expect("some track is free");
            placed[next] = Some(track);
        }
        count[channel] = placed
            .iter()
            .flatten()
            .map(|track| track + 1)
            .max()
            .unwrap_or(0);
        for (segment, track) in segments.iter().zip(placed) {
            of.insert(segment.key, track.expect("every segment is placed"));
        }
    }
    Tracks { count, of }
}
