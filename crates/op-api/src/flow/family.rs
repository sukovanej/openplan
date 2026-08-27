use crate::task::{Coordinate, TaskListItem, coordinate, parent_coordinate};

use super::growth::{Kids, Members, TaskIndex};

// Which task the flow puts each task under, and which tasks each one holds. A parent whose children
// all dropped out holds none, so it reads as a plain node. A corrupt pair of files can make a task
// its own ancestor: neither task can hold the other, so both stand alone here instead of vanishing
// into a box that contains itself. The board keeps such a task visible for the same reason.
pub(crate) struct Family<'a> {
    pub(crate) parent: std::collections::HashMap<Coordinate<'a>, Coordinate<'a>>,
    pub(crate) kids: Kids<'a>,
}

impl<'a> Family<'a> {
    pub(crate) fn build(members: &[&'a TaskListItem], included: &Members<'a>) -> Family<'a> {
        let mut parent: std::collections::HashMap<Coordinate<'a>, Coordinate<'a>> = members
            .iter()
            .filter_map(|task| Some((coordinate(task), parent_coordinate(task)?)))
            .filter(|(_, above)| included.contains(above))
            .collect();
        let looping: Vec<Coordinate<'a>> = parent
            .keys()
            .copied()
            .filter(|task| climbs_back(*task, &parent))
            .collect();
        for task in looping {
            parent.remove(&task);
        }

        let mut kids: Kids<'a> = std::collections::HashMap::new();
        for task in members {
            if let Some(above) = parent.get(&coordinate(task)).copied() {
                kids.entry(above).or_default().push(task);
            }
        }
        Family { parent, kids }
    }

    pub(crate) fn holds(&self, task: &'a TaskListItem) -> bool {
        self.kids.contains_key(&coordinate(task))
    }

    pub(crate) fn under(&self, task: &'a TaskListItem) -> Option<Coordinate<'a>> {
        self.parent.get(&coordinate(task)).copied()
    }

    pub(crate) fn above(
        &self,
        task: &'a TaskListItem,
        index: &TaskIndex<'a>,
    ) -> Vec<&'a TaskListItem> {
        let mut out = Vec::new();
        let mut at = coordinate(task);
        while let Some(above) = self.parent.get(&at).copied() {
            let Some(parent) = index.get(&above).copied() else {
                break;
            };
            out.push(parent);
            at = above;
        }
        out
    }
}

fn climbs_back<'a>(
    start: Coordinate<'a>,
    parent: &std::collections::HashMap<Coordinate<'a>, Coordinate<'a>>,
) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut at = start;
    while let Some(above) = parent.get(&at).copied() {
        if above == start {
            return true;
        }
        if !seen.insert(above) {
            return false;
        }
        at = above;
    }
    false
}

// Every leaf under a task, and the task itself when it is a leaf. A dependency on a box waits for
// each of these, and the box spans the waves they land in.
pub(crate) fn leaves_under<'a>(
    task: &'a TaskListItem,
    family: &Family<'a>,
) -> Vec<&'a TaskListItem> {
    let Some(children) = family.kids.get(&coordinate(task)) else {
        return vec![task];
    };
    let mut out = Vec::new();
    let mut stack: Vec<&TaskListItem> = children.clone();
    let mut seen = std::collections::HashSet::from([coordinate(task)]);
    while let Some(node) = stack.pop() {
        if !seen.insert(coordinate(node)) {
            continue;
        }
        match family.kids.get(&coordinate(node)) {
            Some(children) => stack.extend(children.iter().copied()),
            None => out.push(node),
        }
    }
    out
}
