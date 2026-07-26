use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;

use op_task::Task;
pub use op_task::{Status, Timestamp};

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
    #[schema(nullable = false)]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub rank: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
    // Absent only for a file whose frontmatter does not parse — an unresolved merge, say — where
    // there is no `created` to read and no commit the working copy corresponds to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub created: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub updated: Option<Timestamp>,
    pub body: String,
}

impl TaskView {
    // `updated` is git-derived, so only a caller holding the history can supply it; a store-only
    // read passes `None`.
    pub fn from_task(id: String, task: &Task, updated: Option<Timestamp>) -> Self {
        let created = task.frontmatter.created;
        Self {
            id,
            title: task.title().unwrap_or_default(),
            status: task.frontmatter.status,
            parent: task.frontmatter.parent.clone(),
            rank: task.frontmatter.rank.clone(),
            deps: task.frontmatter.deps.clone(),
            created: Some(created),
            updated: clamp_updated(Some(created), updated),
            body: task.body.clone(),
        }
    }
}

// A hand-set `created` later than the last edit would otherwise show a task updated before it
// existed. Every surface reporting `updated` clamps the same way, so a task cannot read one age in
// the list and another on its own page.
pub fn clamp_updated(created: Option<Timestamp>, updated: Option<Timestamp>) -> Option<Timestamp> {
    match (created, updated) {
        (Some(created), Some(updated)) => Some(updated.max(created)),
        _ => updated,
    }
}

// A direct child of a task, in sibling (`rank`) order — enough to render the subtasks list without
// the whole task set in memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskChild {
    pub id: String,
    pub title: String,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub rank: Option<String>,
}

// A task referenced by `[[id]]` in the body, resolved to its current title and status so a chip can
// render without the client looking it up in a full list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskRef {
    pub id: String,
    pub title: String,
    pub status: Status,
}

// One task's full view on a single branch, paired with every branch it lives on so a cold-loaded
// detail page can render a branch switcher without the list in memory. The `view` is the requested
// branch's version, or the headline version for a branchless read; `headline` names the branch that
// version resolves to (the most recently changed one), so the switcher can mark it. `parent_title`,
// `children`, and `refs` carry the immediate hierarchy context so the page renders from this one
// read instead of fetching the whole task set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskDetail {
    #[serde(flatten)]
    pub view: TaskView,
    pub headline: String,
    pub branches: Vec<BranchState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub parent_title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TaskChild>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<TaskRef>,
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
    #[schema(nullable = false)]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub rank: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub updated: Option<Timestamp>,
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
    pub fn into_task(self, created: Timestamp) -> Task {
        let mut task = Task::new(&self.title, self.status.unwrap_or(Status::Todo), created);
        task.set_parent(self.parent);
        task.set_deps(self.deps);
        if let Some(body) = &self.body {
            task.append_body(body);
        }
        task
    }
}

// A three-state PATCH field: an absent key leaves the value untouched, JSON `null` clears it, and a
// value sets it. serde cannot natively tell "absent" from "null", so `Keep` comes from the field's
// `#[serde(default)]` while this `Deserialize` maps a present `null`/value to `Clear`/`Set`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FieldUpdate<T> {
    #[default]
    Keep,
    Clear,
    Set(T),
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for FieldUpdate<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(deserializer)? {
            None => FieldUpdate::Clear,
            Some(value) => FieldUpdate::Set(value),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ToSchema)]
pub struct TaskPatch {
    #[serde(default)]
    #[schema(nullable = false)]
    pub status: Option<Status>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub parent: FieldUpdate<String>,
    #[serde(default)]
    #[schema(nullable = false)]
    pub rank: Option<String>,
    #[serde(default)]
    #[schema(nullable = false)]
    pub deps: Option<Vec<String>>,
}

impl TaskPatch {
    pub fn apply(self, task: &mut Task) {
        if let Some(status) = self.status {
            task.set_status(status);
        }
        match self.parent {
            FieldUpdate::Keep => {}
            FieldUpdate::Clear => task.set_parent(None),
            FieldUpdate::Set(id) => task.set_parent(Some(id)),
        }
        if let Some(rank) = self.rank {
            task.set_rank(Some(rank));
        }
        if let Some(deps) = self.deps {
            task.set_deps(deps);
        }
    }
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
fn rank_cmp(ra: &Option<String>, ia: &str, rb: &Option<String>, ib: &str) -> std::cmp::Ordering {
    match (ra, rb) {
        (Some(x), Some(y)) => x.cmp(y).then_with(|| ia.cmp(ib)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => ia.cmp(ib),
    }
}

pub fn sibling_cmp(a: &TaskSummary, b: &TaskSummary) -> std::cmp::Ordering {
    rank_cmp(&a.rank, &a.id, &b.rank, &b.id)
}

pub fn list_item_cmp(a: &TaskListItem, b: &TaskListItem) -> std::cmp::Ordering {
    rank_cmp(&a.rank, &a.id, &b.rank, &b.id)
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

// The order status groups appear in on the board — active work first, terminal states last (§9).
const BOARD_ORDER: [Status; 6] = [
    Status::InProgress,
    Status::InReview,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BoardGroup {
    pub status: Status,
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
        let title_of: std::collections::HashMap<&str, &str> = tasks
            .iter()
            .map(|t| (t.id.as_str(), t.title.as_str()))
            .collect();
        let mut by_status: std::collections::HashMap<Status, Vec<&TaskListItem>> =
            std::collections::HashMap::new();
        for task in tasks {
            by_status.entry(task.status).or_default().push(task);
        }

        let mut groups = Vec::new();
        for status in BOARD_ORDER {
            let Some(members) = by_status.get(&status) else {
                continue;
            };
            let member_ids: std::collections::HashSet<&str> =
                members.iter().map(|m| m.id.as_str()).collect();
            let mut children_of: std::collections::HashMap<&str, Vec<&TaskListItem>> =
                std::collections::HashMap::new();
            let mut roots: Vec<&TaskListItem> = Vec::new();
            for member in members {
                match &member.parent {
                    Some(parent) if member_ids.contains(parent.as_str()) => {
                        children_of.entry(parent.as_str()).or_default().push(member);
                    }
                    _ => roots.push(member),
                }
            }
            roots.sort_by(|a, b| rank_cmp(&a.rank, &a.id, &b.rank, &b.id));
            for kids in children_of.values_mut() {
                kids.sort_by(|a, b| rank_cmp(&a.rank, &a.id, &b.rank, &b.id));
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
                if !emitted.contains(member.id.as_str()) {
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
    children_of: &std::collections::HashMap<&'a str, Vec<&'a TaskListItem>>,
    title_of: &std::collections::HashMap<&'a str, &'a str>,
    emitted: &mut std::collections::HashSet<&'a str>,
    rows: &mut Vec<BoardRow>,
) {
    if !emitted.insert(node.id.as_str()) {
        return;
    }
    let kids = children_of.get(node.id.as_str());
    let parent_title = if is_root {
        node.parent
            .as_deref()
            .and_then(|p| title_of.get(p).map(|t| t.to_string()))
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
