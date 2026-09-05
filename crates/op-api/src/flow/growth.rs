use op_task::Status;

use crate::task::{Coordinate, TaskListItem, coordinate, parent_coordinate};

use super::FlowQuery;

pub(crate) fn is_remaining(task: &TaskListItem) -> bool {
    !matches!(
        task.metadata.status(),
        Some(Status::Done) | Some(Status::Cancelled)
    )
}

pub(crate) type TaskIndex<'a> = std::collections::HashMap<Coordinate<'a>, &'a TaskListItem>;
pub(crate) type Members<'a> = std::collections::HashSet<Coordinate<'a>>;
pub(crate) type Kids<'a> = std::collections::HashMap<Coordinate<'a>, Vec<&'a TaskListItem>>;

// A dependency resolves inside its own project, and a `#section` suffix aims at a part of a task
// rather than at another task.
pub(crate) fn dependency_target<'a>(
    index: &TaskIndex<'a>,
    task: &'a TaskListItem,
    dependency: &'a str,
) -> Option<&'a TaskListItem> {
    index
        .get(&(task.project.as_str(), op_task::ref_target(dependency)))
        .copied()
}

fn parent_task<'a>(index: &TaskIndex<'a>, task: &'a TaskListItem) -> Option<&'a TaskListItem> {
    index.get(&parent_coordinate(task)?).copied()
}

// The seeds and what the flow needs around them: what each included task waits for, the parent chain
// above it, and the children a parent has left. The three rules feed each other until nothing new
// arrives, so the whole reachable neighbourhood lands in the set and no depth bounds it. A task
// nobody can complete any more — `done` or `cancelled` — enters as a parent only: a finished
// dependency is not work, and a finished child is not part of the box.
pub(crate) fn grow<'a>(
    tasks: &'a [TaskListItem],
    query: &FlowQuery,
    index: &TaskIndex<'a>,
    children_of: &Kids<'a>,
) -> Members<'a> {
    let mut included = std::collections::HashSet::new();
    let mut queue: Vec<&TaskListItem> = tasks.iter().filter(|task| query.selects(task)).collect();
    while let Some(task) = queue.pop() {
        if !included.insert(coordinate(task)) {
            continue;
        }
        for dependency in remaining_dependencies(task) {
            if let Some(target) = dependency_target(index, task, dependency)
                && is_remaining(target)
            {
                queue.push(target);
            }
        }
        if let Some(parent) = parent_task(index, task) {
            queue.push(parent);
        }
        for child in children_of.get(&coordinate(task)).into_iter().flatten() {
            if is_remaining(child) {
                queue.push(child);
            }
        }
    }
    included
}

// A task nobody can complete any more declares nothing that can still block work, so its
// dependencies leave the flow with it.
pub(crate) fn remaining_dependencies(task: &TaskListItem) -> &[String] {
    match is_remaining(task) {
        true => task.metadata.dependencies(),
        false => &[],
    }
}
