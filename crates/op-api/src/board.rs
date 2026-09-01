use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use op_task::Status;

use crate::order::board_cmp;
use crate::task::{Coordinate, TaskListItem, coordinate, parent_coordinate};

// The order status groups appear in on the board — active work first, terminal states last.
const BOARD_ORDER: [Status; 6] = [
    Status::InReview,
    Status::InProgress,
    Status::Todo,
    Status::Backlog,
    Status::Done,
    Status::Cancelled,
];

// The whole task set arranged for the list view: one group per non-empty status in `BOARD_ORDER`,
// each already flattened into render-ordered rows. The client renders it verbatim — no grouping,
// sorting, or tree-walking. A task always lands in its own status group; within a group it nests
// under its parent only when the parent shares that status, otherwise it is a group-local root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Board {
    pub groups: Vec<BoardGroup>,
}

// `status` is absent for the group of tasks whose own status could not be read. They are not
// filed under a status they never claimed; they get their own group, first, so a broken file is the
// first thing seen rather than a plausible-looking row buried among real ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BoardGroup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub status: Option<Status>,
    pub rows: Vec<BoardRow>,
}

// One rendered line: the task, its indentation `depth` within the group (a group-local root is 0),
// and `has_children` for a nested row beneath it. `parent_title` is set only when the row is a
// group-local root whose real parent sits in another status group (the cross-status edge was cut),
// so the client can show an "under <parent>" hint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BoardRow {
    pub task: TaskListItem,
    pub depth: usize,
    pub has_children: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub parent_title: Option<String>,
}

impl Board {
    pub fn build(tasks: &[TaskListItem]) -> Board {
        let title_of: std::collections::HashMap<Coordinate<'_>, &str> = tasks
            .iter()
            .map(|t| (coordinate(t), t.title.as_str()))
            .collect();
        let mut by_status: std::collections::HashMap<Option<Status>, Vec<&TaskListItem>> =
            std::collections::HashMap::new();
        for task in tasks {
            by_status
                .entry(task.metadata.status())
                .or_default()
                .push(task);
        }

        let mut groups = Vec::new();
        for status in std::iter::once(None).chain(BOARD_ORDER.map(Some)) {
            let Some(members) = by_status.get(&status) else {
                continue;
            };
            let member_ids: std::collections::HashSet<Coordinate<'_>> =
                members.iter().map(|m| coordinate(m)).collect();
            let mut children_of: std::collections::HashMap<Coordinate<'_>, Vec<&TaskListItem>> =
                std::collections::HashMap::new();
            let mut roots: Vec<&TaskListItem> = Vec::new();
            for member in members {
                match parent_coordinate(member) {
                    Some(parent) if member_ids.contains(&parent) => {
                        children_of.entry(parent).or_default().push(member);
                    }
                    _ => roots.push(member),
                }
            }
            roots.sort_by(|a, b| board_cmp(a, b));
            for kids in children_of.values_mut() {
                kids.sort_by(|a, b| board_cmp(a, b));
            }

            let mut rows = Vec::new();
            let mut emitted = std::collections::HashSet::new();
            for root in &roots {
                emit_row(
                    root,
                    0,
                    true,
                    &children_of,
                    &title_of,
                    &mut emitted,
                    &mut rows,
                );
            }
            // A member trapped in a corrupt parent cycle (unreachable from any root) still appears
            // once, as a group-local root, rather than vanishing.
            for member in members {
                if !emitted.contains(&coordinate(member)) {
                    emit_row(
                        member,
                        0,
                        true,
                        &children_of,
                        &title_of,
                        &mut emitted,
                        &mut rows,
                    );
                }
            }
            groups.push(BoardGroup { status, rows });
        }
        Board { groups }
    }
}

fn emit_row<'a>(
    node: &'a TaskListItem,
    depth: usize,
    is_root: bool,
    children_of: &std::collections::HashMap<Coordinate<'a>, Vec<&'a TaskListItem>>,
    title_of: &std::collections::HashMap<Coordinate<'a>, &'a str>,
    emitted: &mut std::collections::HashSet<Coordinate<'a>>,
    rows: &mut Vec<BoardRow>,
) {
    if !emitted.insert(coordinate(node)) {
        return;
    }
    let kids = children_of.get(&coordinate(node));
    let parent_title = if is_root {
        parent_coordinate(node).and_then(|p| title_of.get(&p).map(|t| t.to_string()))
    } else {
        None
    };
    rows.push(BoardRow {
        task: node.clone(),
        depth,
        has_children: kids.is_some_and(|k| !k.is_empty()),
        parent_title,
    });
    if let Some(kids) = kids {
        for kid in kids {
            emit_row(kid, depth + 1, false, children_of, title_of, emitted, rows);
        }
    }
}
