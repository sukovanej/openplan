use std::collections::{BTreeMap, BTreeSet, HashMap};

use op_api::{Problem, ProblemCode};

use crate::Index;

type Found = HashMap<u64, Vec<Problem>>;

impl Index {
    pub(crate) fn find_problems(
        &self,
        tags: &BTreeSet<String>,
        shadowed: &BTreeMap<u64, Vec<String>>,
    ) -> Found {
        let mut found = Found::new();
        let mut parents = BTreeMap::new();
        let mut waits_for: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        for (&number, entry) in &self.tasks {
            for problem in entry.metadata.problems() {
                push(&mut found, number, ProblemCode::Field, problem);
            }
            if entry.titles != 1 {
                push(
                    &mut found,
                    number,
                    ProblemCode::Title,
                    "a task needs exactly one `# ` title".to_owned(),
                );
            }
            for problem in &entry.comment_problems {
                push(&mut found, number, ProblemCode::Comment, problem.clone());
            }
            for problem in &entry.diagram_problems {
                push(&mut found, number, ProblemCode::Diagram, problem.clone());
            }
            if let Some(parent) = entry.metadata.parent() {
                match self.existing(parent) {
                    Some(target) => {
                        parents.insert(number, target);
                    }
                    None => push(
                        &mut found,
                        number,
                        ProblemCode::Reference,
                        format!("the parent {parent} does not exist"),
                    ),
                }
            }
            for dependency in entry.metadata.dependencies() {
                match self.existing(dependency) {
                    Some(target) => waits_for.entry(number).or_default().push(target),
                    None => push(
                        &mut found,
                        number,
                        ProblemCode::Reference,
                        format!("the dependency {dependency} does not exist"),
                    ),
                }
            }
            let named: BTreeSet<u64> = entry.body_refs.iter().copied().collect();
            for target in named.into_iter().filter(|n| !self.tasks.contains_key(n)) {
                push(
                    &mut found,
                    number,
                    ProblemCode::Reference,
                    format!("the text names {}, which does not exist", self.key(target)),
                );
            }
            for tag in entry.metadata.tags() {
                if !tags.contains(tag) {
                    push(
                        &mut found,
                        number,
                        ProblemCode::Tag,
                        format!("the tag {tag} is not registered"),
                    );
                }
            }
        }
        for (&number, paths) in shadowed {
            push(
                &mut found,
                number,
                ProblemCode::DuplicateNumber,
                format!(
                    "{} also takes this number, so openplan reads only one of the files",
                    paths.join(", ")
                ),
            );
        }
        self.parent_cycles(&parents, &mut found);
        self.dependency_cycles(&waits_for, &mut found);
        for problems in found.values_mut() {
            problems.sort_by(|a, b| (a.code, &a.message).cmp(&(b.code, &b.message)));
        }
        found
    }

    fn existing(&self, key: &str) -> Option<u64> {
        self.number(op_task::ref_target(key))
            .filter(|number| self.tasks.contains_key(number))
    }

    fn parent_cycles(&self, parents: &BTreeMap<u64, u64>, found: &mut Found) {
        for &start in parents.keys() {
            let mut chain = vec![start];
            let mut at = start;
            while let Some(&next) = parents.get(&at) {
                if next == start {
                    chain.push(start);
                    push(
                        found,
                        start,
                        ProblemCode::ParentCycle,
                        format!("the task is its own ancestor: {}", self.path(&chain)),
                    );
                    break;
                }
                // A cycle above that leaves this task out: its own members report it.
                if chain.contains(&next) {
                    break;
                }
                chain.push(next);
                at = next;
            }
        }
    }

    // The shortest way back to the task through what it waits for, found breadth first.
    fn dependency_cycles(&self, waits_for: &BTreeMap<u64, Vec<u64>>, found: &mut Found) {
        for &start in waits_for.keys() {
            let mut came_from: HashMap<u64, u64> = HashMap::new();
            let mut queue = std::collections::VecDeque::from([start]);
            let mut closed = None;
            while let Some(at) = queue.pop_front() {
                for &next in waits_for.get(&at).into_iter().flatten() {
                    if next == start {
                        closed = Some(at);
                        break;
                    }
                    if let std::collections::hash_map::Entry::Vacant(slot) = came_from.entry(next) {
                        slot.insert(at);
                        queue.push_back(next);
                    }
                }
                if closed.is_some() {
                    break;
                }
            }
            let Some(last) = closed else {
                continue;
            };
            let mut chain = vec![last];
            let mut at = last;
            while at != start {
                at = came_from[&at];
                chain.push(at);
            }
            chain.reverse();
            chain.push(start);
            push(
                found,
                start,
                ProblemCode::DependencyCycle,
                format!("the task waits for itself: {}", self.path(&chain)),
            );
        }
    }

    fn path(&self, chain: &[u64]) -> String {
        chain
            .iter()
            .map(|number| self.key(*number))
            .collect::<Vec<_>>()
            .join(" → ")
    }
}

fn push(found: &mut Found, number: u64, code: ProblemCode, message: String) {
    found
        .entry(number)
        .or_default()
        .push(Problem { code, message });
}
