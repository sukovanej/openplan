use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub use op_task::Status;
use op_task::Task;

pub const ADMIN_HEADER: &str = "x-oplan-admin";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ApiErrorBody {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DaemonInfo {
    pub pid: u32,
    pub port: u16,
    pub version: String,
    pub started_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<String>,
}

impl TaskSummary {
    pub fn from_task(id: String, task: &Task) -> Self {
        Self {
            id,
            title: task.title().unwrap_or_default(),
            status: task.frontmatter.status,
            parent: task.frontmatter.parent.clone(),
            rank: task.frontmatter.rank.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskView {
    pub id: String,
    pub title: String,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
    pub body: String,
}

impl TaskView {
    pub fn from_task(id: String, task: &Task) -> Self {
        Self {
            id,
            title: task.title().unwrap_or_default(),
            status: task.frontmatter.status,
            parent: task.frontmatter.parent.clone(),
            rank: task.frontmatter.rank.clone(),
            deps: task.frontmatter.deps.clone(),
            body: task.body.clone(),
        }
    }
}

// One task's full view on a single branch, paired with every branch it lives on so a cold-loaded
// detail page can render a branch switcher without the list in memory. The `view` is the requested
// branch's version, or the headline version for a branchless read; `headline` names the branch that
// version resolves to (the most recently changed one), so the switcher can mark it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskDetail {
    #[serde(flatten)]
    pub view: TaskView,
    pub headline: String,
    pub branches: Vec<BranchState>,
}

// One logical task aggregated across every branch it lives on: the `status`/`title` come from the
// `headline` branch (the most recently changed one), plus one `branches` entry per branch so a
// client can render badges, mark the headline, and spot divergence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskListItem {
    pub id: String,
    pub title: String,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<String>,
    pub headline: String,
    pub branches: Vec<BranchState>,
}

// How a branch's committed version of a task stands against the default branch's merge-base:
// `Base` is the default branch itself, the other three are the ways a branch diverges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Base,
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BranchState {
    pub branch: String,
    pub status: Status,
    pub blob_oid: String,
    pub dirty: bool,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateTask {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl CreateTask {
    pub fn into_task(self) -> Task {
        let mut task = Task::new(&self.title, self.status.unwrap_or(Status::Todo));
        task.set_parent(self.parent);
        task.set_deps(self.deps);
        if let Some(body) = &self.body {
            task.append_body(body);
        }
        task
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    // Three-state so a client can tell "leave the parent alone" from "clear it": an absent key is
    // `None`, JSON `null` is `Some(None)` (unparent to top level), and an id is `Some(Some(id))`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option"
    )]
    pub parent: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deps: Option<Vec<String>>,
}

impl TaskPatch {
    pub fn apply(self, task: &mut Task) {
        if let Some(status) = self.status {
            task.set_status(status);
        }
        if let Some(parent) = self.parent {
            task.set_parent(parent);
        }
        if let Some(rank) = self.rank {
            task.set_rank(Some(rank));
        }
        if let Some(deps) = self.deps {
            task.set_deps(deps);
        }
    }
}

fn double_option<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

// The subtree rooted at one task: siblings within `children` are ordered by `rank` (§3.2), the tree
// built by grouping the flat task set on `parent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskTree {
    pub id: String,
    pub title: String,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<String>,
    pub children: Vec<TaskTree>,
}

// Siblings sort by `rank` ascending; a task without a rank sorts last, ties broken by id so the
// order is stable across rebuilds (§4).
pub fn sibling_cmp(a: &TaskSummary, b: &TaskSummary) -> std::cmp::Ordering {
    match (&a.rank, &b.rank) {
        (Some(x), Some(y)) => x.cmp(y).then_with(|| a.id.cmp(&b.id)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.id.cmp(&b.id),
    }
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
            if let Some(parent) = &summary.parent {
                children_of
                    .entry(parent.as_str())
                    .or_default()
                    .push(summary);
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
        status: summary.status,
        parent: summary.parent.clone(),
        rank: summary.rank.clone(),
        children,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatrixCell {
    pub branch: String,
    pub task: TaskSummary,
    pub blob_oid: String,
    pub dirty: bool,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Matrix {
    pub cells: Vec<MatrixCell>,
}

// One task viewed across every branch it lives on, its cells grouped by blob OID so identical
// versions collapse into a single row and divergent ones stand out (§7.4). A `Deleted` mark carries
// the pre-deletion blob, so a branch that removes the task groups under the version it removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskBranches {
    pub id: String,
    pub versions: Vec<TaskVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskVersion {
    pub blob_oid: String,
    pub summary: TaskSummary,
    pub branches: Vec<BranchMark>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchMark {
    pub branch: String,
    pub kind: ChangeKind,
    pub dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presence {
    pub task_id: String,
    pub actor: String,
    pub worktree: String,
    pub branch: String,
    pub last_heartbeat_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChangeEvent {
    TaskChanged { id: String, branch: String },
    RefMoved { branch: String },
    PresenceChanged { task_id: String },
    DaemonStopping,
}
