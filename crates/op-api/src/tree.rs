use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::metadata::Metadata;
use crate::order::sibling_cmp;
use crate::task::TaskSummary;

// The subtree rooted at one task: siblings within `children` are ordered by `rank`, the tree
// built by grouping the flat task set on `parent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskTree {
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    // A schema generator that expanded this in place would never stop.
    #[schema(no_recursion)]
    pub children: Vec<TaskTree>,
}

// A subtree plus the tasks whose own subtree was left out because a parent cycle would have made
// the descent endless. A client that only rendered `tree` would show a truncated hierarchy as a
// complete one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskTreeView {
    pub tree: TaskTree,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cycles: Vec<String>,
}

impl TaskTree {
    // The subtree rooted at `root` built by grouping a flat task set on `parent`. `depth` bounds the
    // descent (`None` unbounded, `Some(0)` root only, `Some(1)` direct children); siblings are
    // ordered by `sibling_cmp`. Cycle-safe: an id already on the current path is not re-expanded and
    // is pushed onto `cycles` so callers can report it instead of looping forever.
    pub fn build(
        summaries: &[TaskSummary],
        root: &str,
        depth: Option<usize>,
        cycles: &mut Vec<String>,
    ) -> Option<TaskTree> {
        let by_id: std::collections::HashMap<&str, &TaskSummary> =
            summaries.iter().map(|s| (s.id.as_str(), s)).collect();
        let mut children_of: std::collections::HashMap<&str, Vec<&TaskSummary>> =
            std::collections::HashMap::new();
        for summary in summaries {
            if let Some(parent) = summary.metadata.parent() {
                children_of.entry(parent).or_default().push(summary);
            }
        }
        let root = by_id.get(root)?;
        let mut path = std::collections::HashSet::new();
        Some(build_node(root, depth, &children_of, &mut path, cycles))
    }
}

fn build_node(
    summary: &TaskSummary,
    depth: Option<usize>,
    children_of: &std::collections::HashMap<&str, Vec<&TaskSummary>>,
    path: &mut std::collections::HashSet<String>,
    cycles: &mut Vec<String>,
) -> TaskTree {
    path.insert(summary.id.clone());
    let mut children = Vec::new();
    if depth != Some(0) {
        let mut kids = children_of
            .get(summary.id.as_str())
            .cloned()
            .unwrap_or_default();
        kids.sort_by(|a, b| sibling_cmp(a, b));
        for kid in kids {
            if path.contains(&kid.id) {
                cycles.push(kid.id.clone());
                continue;
            }
            children.push(build_node(
                kid,
                depth.map(|d| d - 1),
                children_of,
                path,
                cycles,
            ));
        }
    }
    path.remove(&summary.id);
    TaskTree {
        id: summary.id.clone(),
        title: summary.title.clone(),
        metadata: summary.metadata.clone(),
        children,
    }
}
