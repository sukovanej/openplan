#[derive(Debug, Clone, Copy)]
pub(super) struct Constraint {
    pub(super) from: usize,
    pub(super) to: usize,
    pub(super) length: usize,
}

pub(super) fn ranks(
    count: usize,
    constraints: &[Constraint],
    fixed: &[Option<usize>],
) -> Vec<usize> {
    let acyclic = break_cycles(count, constraints);
    let ranks: Vec<i64> = if fixed.iter().any(Option::is_some) {
        with_fixed(count, &acyclic, fixed)
    } else {
        longest_path(count, &acyclic)
    };
    let lowest = ranks.iter().copied().min().unwrap_or(0);
    ranks
        .into_iter()
        .map(|rank| usize::try_from(rank - lowest).unwrap_or(0))
        .collect()
}

// Reversing each edge that a depth-first walk finds going back to a vertex on its own path leaves
// a graph with no cycle, and keeps the two ends of that edge near each other.
fn break_cycles(count: usize, constraints: &[Constraint]) -> Vec<Constraint> {
    let mut kept: Vec<Constraint> = constraints
        .iter()
        .copied()
        .filter(|constraint| constraint.from != constraint.to)
        .collect();
    let mut outgoing = vec![Vec::new(); count];
    for (at, constraint) in kept.iter().enumerate() {
        outgoing[constraint.from].push(at);
    }
    #[derive(Clone, Copy, PartialEq)]
    enum Visit {
        New,
        OnPath,
        Done,
    }
    let mut visit = vec![Visit::New; count];
    let mut back = Vec::new();
    for start in 0..count {
        if visit[start] != Visit::New {
            continue;
        }
        visit[start] = Visit::OnPath;
        let mut path = vec![(start, 0usize)];
        while let Some(top) = path.last_mut() {
            let (vertex, next) = *top;
            match outgoing[vertex].get(next) {
                Some(&at) => {
                    top.1 += 1;
                    let target = kept[at].to;
                    match visit[target] {
                        Visit::New => {
                            visit[target] = Visit::OnPath;
                            path.push((target, 0));
                        }
                        Visit::OnPath => back.push(at),
                        Visit::Done => {}
                    }
                }
                None => {
                    visit[vertex] = Visit::Done;
                    path.pop();
                }
            }
        }
    }
    for at in back {
        let constraint = &mut kept[at];
        std::mem::swap(&mut constraint.from, &mut constraint.to);
    }
    kept
}

fn topological(count: usize, constraints: &[Constraint]) -> Vec<usize> {
    let mut incoming = vec![0usize; count];
    let mut outgoing = vec![Vec::new(); count];
    for constraint in constraints {
        incoming[constraint.to] += 1;
        outgoing[constraint.from].push(constraint.to);
    }
    let mut ready: std::collections::VecDeque<usize> =
        (0..count).filter(|vertex| incoming[*vertex] == 0).collect();
    let mut order = Vec::with_capacity(count);
    while let Some(vertex) = ready.pop_front() {
        order.push(vertex);
        for &target in &outgoing[vertex] {
            incoming[target] -= 1;
            if incoming[target] == 0 {
                ready.push_back(target);
            }
        }
    }
    order
}

fn length(constraint: &Constraint) -> i64 {
    i64::try_from(constraint.length).unwrap_or(i64::MAX)
}

// A vertex lands one rank below the lowest thing it follows. That leaves every source on the first
// rank, however far below it its successors sit, so a source then moves down to just above them.
fn longest_path(count: usize, constraints: &[Constraint]) -> Vec<i64> {
    let order = topological(count, constraints);
    let mut ranks = vec![0i64; count];
    let mut has_predecessor = vec![false; count];
    for constraint in constraints {
        has_predecessor[constraint.to] = true;
    }
    for &vertex in &order {
        for constraint in constraints
            .iter()
            .filter(|constraint| constraint.from == vertex)
        {
            ranks[constraint.to] = ranks[constraint.to].max(ranks[vertex] + length(constraint));
        }
    }
    for vertex in (0..count).filter(|vertex| !has_predecessor[*vertex]) {
        let lowest_below = constraints
            .iter()
            .filter(|constraint| constraint.from == vertex)
            .map(|constraint| ranks[constraint.to] - length(constraint))
            .min();
        if let Some(rank) = lowest_below {
            ranks[vertex] = rank;
        }
    }
    ranks
}

// The producer fixes some ranks, as the flow does with its waves. A free vertex goes one rank below
// what it follows, or one rank above what follows it.
fn with_fixed(count: usize, constraints: &[Constraint], fixed: &[Option<usize>]) -> Vec<i64> {
    let order = topological(count, constraints);
    let mut ranks: Vec<Option<i64>> = fixed
        .iter()
        .map(|rank| rank.and_then(|rank| i64::try_from(rank).ok()))
        .collect();
    for &vertex in &order {
        if ranks[vertex].is_none() {
            ranks[vertex] = constraints
                .iter()
                .filter(|constraint| constraint.to == vertex)
                .filter_map(|constraint| {
                    ranks[constraint.from].map(|rank| rank + length(constraint))
                })
                .max();
        }
    }
    for &vertex in order.iter().rev() {
        if ranks[vertex].is_none() {
            ranks[vertex] = constraints
                .iter()
                .filter(|constraint| constraint.from == vertex)
                .filter_map(|constraint| ranks[constraint.to].map(|rank| rank - length(constraint)))
                .min();
        }
    }
    ranks.into_iter().map(|rank| rank.unwrap_or(0)).collect()
}
