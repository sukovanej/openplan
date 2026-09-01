mod family;
mod growth;
mod layout;
mod wiring;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use op_task::Status;

use crate::field::Field;
use crate::task::{TaskListItem, coordinate, parent_coordinate};

use family::Family;
use growth::{Kids, TaskIndex, grow, is_remaining};
use layout::Layout;
use wiring::{nodes, wire};

// Which tasks the flow grows from. Values of one field are alternatives, and the fields narrow each
// other; an empty field puts no condition of its own. A named task carries its project, because two
// stores can commit the same abbreviation and a bare key would then name two tasks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowQuery {
    pub projects: Vec<String>,
    pub statuses: Vec<Status>,
    pub tasks: Vec<(String, String)>,
    pub tags: Vec<String>,
}

impl FlowQuery {
    fn selects(&self, task: &TaskListItem) -> bool {
        let by_project =
            self.projects.is_empty() || self.projects.iter().any(|name| name == &task.project);
        // With no status named, every task that somebody can still implement is a seed. A finished
        // task seeds nothing, because the flow orders the work that is left; a file whose status
        // does not parse is work, and it stays visible.
        let by_status = match self.statuses.is_empty() {
            true => is_remaining(task),
            false => task
                .metadata
                .status()
                .is_some_and(|status| self.statuses.contains(&status)),
        };
        let by_key = self.tasks.is_empty()
            || self
                .tasks
                .iter()
                .any(|(project, id)| project == &task.project && id == &task.id);
        let by_tag = self.tags.is_empty()
            || task
                .metadata
                .tags()
                .iter()
                .any(|tag| self.tags.contains(tag));
        by_project && by_status && by_key && by_tag
    }
}

// The implementation order of one task set: a flat node list and the edges between the nodes. An
// edge runs from the dependency to the task that waits for it, which is the direction the time runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Flow {
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
}

// A `leaf` is a task somebody implements, and it is the only kind that holds a place in the order. A
// `box` is a parent: the flow draws it around its children and reads its span from them, so it takes
// no place of its own. An `unresolved` node is a dependency that names no task of its project — the
// raw text is all it has, and no work can complete it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlowNode {
    Leaf {
        project: String,
        id: String,
        title: String,
        status: Field<Status>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(nullable = false)]
        parent: Option<String>,
        wave: usize,
        position: usize,
        blocks_count: usize,
    },
    Box {
        project: String,
        id: String,
        title: String,
        status: Field<Status>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schema(nullable = false)]
        parent: Option<String>,
    },
    Unresolved {
        project: String,
        id: String,
    },
}

// No edge crosses a project, because a key resolves inside one store only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct FlowEdge {
    pub project: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("dependencies form a cycle: {}", format_cycles(.cycles))]
pub struct FlowCycles {
    pub cycles: Vec<Vec<String>>,
}

fn format_cycles(cycles: &[Vec<String>]) -> String {
    cycles
        .iter()
        .map(|cycle| {
            let mut round: Vec<&str> = cycle.iter().map(String::as_str).collect();
            round.extend(cycle.first().map(String::as_str));
            round.join(" -> ")
        })
        .collect::<Vec<_>>()
        .join("; ")
}

impl Flow {
    pub fn build(tasks: &[TaskListItem], query: &FlowQuery) -> Result<Flow, FlowCycles> {
        let index: TaskIndex<'_> = tasks.iter().map(|task| (coordinate(task), task)).collect();
        let mut children_of: Kids<'_> = std::collections::HashMap::new();
        for task in tasks {
            if let Some(parent) = parent_coordinate(task) {
                children_of.entry(parent).or_default().push(task);
            }
        }

        let included = grow(tasks, query, &index, &children_of);
        let members: Vec<&TaskListItem> = tasks
            .iter()
            .filter(|task| included.contains(&coordinate(task)))
            .collect();
        let family = Family::build(&members, &included);
        let leaves: Vec<&TaskListItem> = members
            .iter()
            .copied()
            .filter(|task| !family.holds(task))
            .collect();

        let layout = Layout::build(leaves, &included, &family, &index)?;
        let (edges, unresolved) = wire(&members, &index);
        Ok(Flow {
            nodes: nodes(&layout, &family, &index, &unresolved),
            edges,
        })
    }
}
