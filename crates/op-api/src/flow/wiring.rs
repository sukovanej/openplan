use crate::order::id_cmp;
use crate::task::{TaskListItem, coordinate};

use super::family::Family;
use super::growth::{TaskIndex, dependency_target, is_remaining, remaining_dependencies};
use super::layout::Layout;
use super::{FlowEdge, FlowNode};

// The edges the flow draws: the dependencies each included task declares, as the file declares them.
// An inherited dependency draws none of its own — the arrow lands on the box, and the box holds the
// child. A dependency that is complete draws nothing at all.
pub(crate) fn wire<'a>(
    members: &[&'a TaskListItem],
    index: &TaskIndex<'a>,
) -> (Vec<FlowEdge>, Vec<(&'a str, &'a str)>) {
    let mut edges = Vec::new();
    let mut unresolved = std::collections::BTreeSet::new();
    for task in members {
        for dependency in remaining_dependencies(task) {
            let from = match dependency_target(index, task, dependency) {
                Some(target) if is_remaining(target) => target.id.clone(),
                Some(_) => continue,
                None => {
                    unresolved.insert((task.project.as_str(), dependency.as_str()));
                    dependency.clone()
                }
            };
            edges.push(FlowEdge {
                project: task.project.clone(),
                from,
                to: task.id.clone(),
            });
        }
    }
    edges.sort_by(|a, b| {
        a.project
            .cmp(&b.project)
            .then_with(|| id_cmp(&a.to, &b.to))
            .then_with(|| id_cmp(&a.from, &b.from))
    });
    edges.dedup();
    (edges, unresolved.into_iter().collect())
}

// The nodes in reading order: wave by wave, and each box before the first leaf it holds. An
// unresolved node sits at the end, because it belongs to no wave.
pub(crate) fn nodes<'a>(
    layout: &Layout<'a>,
    family: &Family<'a>,
    index: &TaskIndex<'a>,
    unresolved: &[(&str, &str)],
) -> Vec<FlowNode> {
    let mut out = Vec::new();
    let mut emitted = std::collections::HashSet::new();
    for (wave, members) in layout.waves.iter().enumerate() {
        for (position, leaf) in members.iter().copied().enumerate() {
            let task = layout.leaves[leaf];
            for parent in family.above(task, index).into_iter().rev() {
                if emitted.insert(coordinate(parent)) {
                    out.push(FlowNode::Box {
                        project: parent.project.clone(),
                        id: parent.id.clone(),
                        title: parent.title.clone(),
                        status: parent.metadata.status_field(),
                        parent: parent_in_flow(parent, family),
                    });
                }
            }
            emitted.insert(coordinate(task));
            out.push(FlowNode::Leaf {
                project: task.project.clone(),
                id: task.id.clone(),
                title: task.title.clone(),
                status: task.metadata.status_field(),
                parent: parent_in_flow(task, family),
                wave,
                position,
                blocks_count: layout.blocks[leaf],
            });
        }
    }
    for (project, id) in unresolved {
        out.push(FlowNode::Unresolved {
            project: (*project).to_owned(),
            id: (*id).to_owned(),
        });
    }
    out
}

// A parent the flow does not hold is a parent the client cannot draw a box for. The store can name
// one that no file defines, and a node pointing at it would read as a box that never arrives.
fn parent_in_flow<'a>(task: &'a TaskListItem, family: &Family<'a>) -> Option<String> {
    family.under(task).map(|(_, id)| id.to_owned())
}
