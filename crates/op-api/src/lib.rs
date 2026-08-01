use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;

use op_task::Task;
pub use op_task::{Abbreviation, Status, Timestamp};

pub const ADMIN_HEADER: &str = "x-oplan-admin";

// A spelling of an id the store has no id for. One key spelling is accepted and nothing else is, so
// a refusal names the form that would have worked rather than guessing at what was meant.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a task key: {got:?}; expected {expected}")]
pub struct KeyError {
    pub got: String,
    pub expected: String,
}

impl KeyError {
    pub fn new(abbreviation: Abbreviation, got: &str) -> Self {
        Self {
            got: got.to_owned(),
            expected: abbreviation.format_key(42),
        }
    }
}

fn reference_of(abbreviation: Abbreviation, key: &str) -> Result<String, KeyError> {
    abbreviation
        .parse_ref(key)
        .ok_or_else(|| KeyError::new(abbreviation, key))
}

// In-memory references are the numbers the file layer allocates — `op_task` normalizes every stored
// spelling to one — and above the store each is rendered as this store's key.
fn key_of(abbreviation: Abbreviation, reference: &str) -> String {
    abbreviation
        .format_ref(reference)
        .unwrap_or_else(|| reference.to_owned())
}

// A body as the store carries it in memory: a `[[…]]` in the key spelling becomes the number the file
// layer names. Any other spelling of a reference is refused rather than written as prose — a bare
// number and another store's key both name no task here, and a file that already holds one shows it
// as the plain text it is.
pub fn body_from_keys(abbreviation: Abbreviation, body: &str) -> Result<String, KeyError> {
    let mut out = String::new();
    let mut last = 0;
    for (span, inner) in op_task::body_ref_spans(body) {
        let target = op_task::ref_target(inner);
        if let Some(reference) = abbreviation.parse_ref(inner) {
            out.push_str(&body[last..span.start]);
            out.push_str(&format!("[[{reference}]]"));
            last = span.end;
        } else if op_task::parse_id(target).is_some() || op_task::is_key_shaped(target) {
            return Err(KeyError::new(abbreviation, target));
        }
    }
    if last == 0 {
        return Ok(body.to_owned());
    }
    out.push_str(&body[last..]);
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ApiErrorBody {
    pub message: String,
}

// The store's settings a client needs to speak its id space: the abbreviation every key carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct StoreConfig {
    #[schema(pattern = "^[A-Z]{3}$")]
    pub abbreviation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DaemonInfo {
    pub pid: u32,
    pub port: u16,
    pub version: String,
    pub started_at: u64,
    // The git common directory of the repository it indexes — the same for every worktree of that
    // repository, so a client can tell whether this daemon's answers are about its own store, and a
    // write routed here cannot land in a same-named branch of another repository. Optional because
    // this file outlives the binary that wrote it: a newer CLI must still be able to read, and stop,
    // a daemon started before the field existed.
    #[serde(default)]
    pub repo: Option<String>,
}

// Every read surface carries the same `metadata`: the frontmatter parsed field by field, so a file
// with one bad field still renders the rest and flags only what failed. `title` and `body` come from
// the markdown, which has no schema to violate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
}

impl TaskSummary {
    pub fn from_partial(
        id: String,
        partial: op_task::PartialTask,
        abbreviation: Abbreviation,
    ) -> Self {
        Self {
            id,
            title: partial.title.unwrap_or_default(),
            metadata: Metadata::from_partial(partial.metadata, abbreviation),
        }
    }

    pub fn from_task(id: String, task: &Task, abbreviation: Abbreviation) -> Self {
        Self {
            id,
            title: task.title().unwrap_or_default(),
            metadata: Metadata::from_frontmatter(&task.frontmatter, abbreviation),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskView {
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    pub body: String,
    // Derived from git rather than read from the file, so it sits outside `metadata`.
    pub updated: Field<Rfc3339>,
}

impl TaskView {
    pub fn from_partial(
        id: String,
        partial: op_task::PartialTask,
        updated: op_task::FieldResult<Timestamp>,
        abbreviation: Abbreviation,
    ) -> Self {
        let metadata = Metadata::from_partial(partial.metadata, abbreviation);
        let created = metadata.created();
        Self {
            id,
            title: partial.title.unwrap_or_default(),
            updated: updated_field(created, updated),
            metadata,
            body: partial.body,
        }
    }

    // `updated` is git-derived, so only a caller holding the history can supply it; a store-only
    // read passes `None`.
    pub fn from_task(
        id: String,
        task: &Task,
        updated: op_task::FieldResult<Timestamp>,
        abbreviation: Abbreviation,
    ) -> Self {
        Self {
            id,
            title: task.title().unwrap_or_default(),
            updated: updated_field(Some(task.frontmatter.created), updated),
            metadata: Metadata::from_frontmatter(&task.frontmatter, abbreviation),
            body: task.body.clone(),
        }
    }
}

// An instant on the wire. JSON has no time type, so it travels as RFC3339 text; wrapping it keeps
// the Rust side a real `Timestamp` instead of re-parsing at each use, and gives utoipa a schema for
// a type it cannot see inside `Field<T>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = DateTime)]
pub struct Rfc3339(pub Timestamp);

impl From<Timestamp> for Rfc3339 {
    fn from(at: Timestamp) -> Self {
        Self(at)
    }
}

impl std::fmt::Display for Rfc3339 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// A frontmatter field on the read path: its parsed value, or a per-field error, so a client can
// render every field that parsed and flag only the ones that did not. Serialized untagged — a value
// is its bare JSON (`"todo"`, `null`, `["a"]`), an error is a `{ "kind": … }` object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum Field<T> {
    Value(T),
    Error(FieldError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldError {
    Missing,
    Invalid { message: String },
}

impl<T> From<op_task::FieldResult<T>> for Field<T> {
    fn from(result: op_task::FieldResult<T>) -> Self {
        match result {
            Ok(value) => Field::Value(value),
            Err(op_task::FieldError::Missing) => Field::Error(FieldError::Missing),
            Err(op_task::FieldError::Invalid(message)) => {
                Field::Error(FieldError::Invalid { message })
            }
        }
    }
}

impl<T> Field<T> {
    pub fn value(self) -> Option<T> {
        match self {
            Field::Value(value) => Some(value),
            Field::Error(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct FrontmatterFields {
    pub status: Field<Status>,
    // RFC3339 UTC, already validated by the parser — a value that did not parse arrives as an error
    // rather than as text.
    pub created: Field<Rfc3339>,
    pub parent: Field<Option<String>>,
    pub rank: Field<Option<String>>,
    pub dependencies: Field<Vec<String>>,
}

// The task's metadata as parsed: `Fields` when the YAML is a mapping (each field carries its own
// value or error), `Error` when the fence or the YAML itself is unrecoverable. Serialized untagged:
// the error is `{ "kind": "error", "message": … }`, the fields case a plain object of the fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum Metadata {
    Error {
        kind: MetadataErrorTag,
        message: String,
    },
    Fields(FrontmatterFields),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetadataErrorTag {
    Error,
}

impl Metadata {
    pub fn from_partial(partial: op_task::PartialMetadata, abbreviation: Abbreviation) -> Self {
        match partial {
            op_task::PartialMetadata::Error(message) => Metadata::Error {
                kind: MetadataErrorTag::Error,
                message,
            },
            op_task::PartialMetadata::Fields(fields) => Metadata::Fields(FrontmatterFields {
                status: fields.status.into(),
                created: Field::from(fields.created).map(Rfc3339),
                parent: Field::from(fields.parent)
                    .map(|parent| parent.map(|p| key_of(abbreviation, &p))),
                rank: fields.rank.into(),
                dependencies: Field::from(fields.dependencies).map(|dependencies| {
                    dependencies
                        .iter()
                        .map(|d| key_of(abbreviation, d))
                        .collect()
                }),
            }),
        }
    }
}

impl<T> Field<T> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Field<U> {
        match self {
            Field::Value(value) => Field::Value(f(value)),
            Field::Error(err) => Field::Error(err),
        }
    }

    pub fn into_result(self) -> op_task::FieldResult<T> {
        match self {
            Field::Value(value) => Ok(value),
            Field::Error(FieldError::Missing) => Err(op_task::FieldError::Missing),
            Field::Error(FieldError::Invalid { message }) => {
                Err(op_task::FieldError::Invalid(message))
            }
        }
    }

    pub fn as_error(&self) -> Option<&FieldError> {
        match self {
            Field::Value(_) => None,
            Field::Error(err) => Some(err),
        }
    }

    pub fn as_value(&self) -> Option<&T> {
        match self {
            Field::Value(value) => Some(value),
            Field::Error(_) => None,
        }
    }
}

impl Metadata {
    pub fn from_frontmatter(fm: &op_task::Frontmatter, abbreviation: Abbreviation) -> Self {
        Metadata::Fields(FrontmatterFields {
            status: Field::Value(fm.status),
            created: Field::Value(Rfc3339(fm.created)),
            parent: Field::Value(fm.parent.as_deref().map(|p| key_of(abbreviation, p))),
            rank: Field::Value(fm.rank.clone()),
            dependencies: Field::Value(
                fm.dependencies
                    .iter()
                    .map(|d| key_of(abbreviation, d))
                    .collect(),
            ),
        })
    }

    pub fn fields(&self) -> Option<&FrontmatterFields> {
        match self {
            Metadata::Fields(fields) => Some(fields),
            Metadata::Error { .. } => None,
        }
    }

    // The display value: a field that failed has none, so a client shows the failure instead of a
    // fabricated one.
    pub fn status(&self) -> Option<Status> {
        self.fields()?.status.as_value().copied()
    }

    // The status as a field, so a surface that renders a badge can show the failure instead of a
    // status the file never claimed. A file whose metadata did not parse at all fails every field.
    pub fn status_field(&self) -> Field<Status> {
        match self {
            Metadata::Fields(fields) => fields.status.clone(),
            Metadata::Error { message, .. } => Field::Error(FieldError::Invalid {
                message: message.clone(),
            }),
        }
    }

    // The structural values. A field that failed reads as absent, which is how the tree, the board
    // grouping, and the sort already handle a task that genuinely has no parent or rank — the
    // failure itself stays visible in `metadata`.
    pub fn parent(&self) -> Option<&str> {
        self.fields()?.parent.as_value()?.as_deref()
    }

    pub fn rank(&self) -> Option<&str> {
        self.fields()?.rank.as_value()?.as_deref()
    }

    pub fn created(&self) -> Option<Timestamp> {
        self.fields()?.created.as_value().map(|at| at.0)
    }

    // Every field that failed, named, for a surface that reports what is wrong rather than only
    // that something is.
    pub fn problems(&self) -> Vec<String> {
        let fields = match self {
            Metadata::Error { message, .. } => return vec![format!("frontmatter: {message}")],
            Metadata::Fields(fields) => fields,
        };
        let mut out = Vec::new();
        let mut push = |name: &str, err: Option<&FieldError>| {
            if let Some(err) = err {
                out.push(match err {
                    FieldError::Missing => format!("{name}: missing"),
                    FieldError::Invalid { message } => format!("{name}: {message}"),
                });
            }
        };
        push("status", fields.status.as_error());
        push("created", fields.created.as_error());
        push("parent", fields.parent.as_error());
        push("rank", fields.rank.as_error());
        push("dependencies", fields.dependencies.as_error());
        out
    }

    pub fn dependencies(&self) -> &[String] {
        match self.fields().map(|fields| &fields.dependencies) {
            Some(Field::Value(dependencies)) => dependencies,
            _ => &[],
        }
    }
}

// A hand-set `created` later than the last edit would otherwise show a task updated before it
// existed. Every surface reporting `updated` clamps the same way, so a task cannot read one age in
// the list and another on its own page.
pub fn updated_field(
    created: Option<Timestamp>,
    updated: op_task::FieldResult<Timestamp>,
) -> Field<Rfc3339> {
    updated
        .map(|at| match created {
            Some(created) => Rfc3339(at.max(created)),
            None => Rfc3339(at),
        })
        .into()
}

// A direct child of a task, in sibling (`rank`) order — enough to render the subtasks list without
// the whole task set in memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskChild {
    pub id: String,
    pub title: String,
    pub status: Field<Status>,
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
    pub status: Field<Status>,
}

// One task read for the detail page: `metadata` parsed field by field, so a file with one bad field
// still renders everything else and flags only what failed, and `body` always the raw markdown.
// `updated` is derived from git rather than read from the file, so it sits outside `metadata`.
// Paired with every branch it lives on, so a cold-loaded page renders a branch switcher without the
// list in memory; `headline` names the branch the shown version resolves to. `parent_title`,
// `children`, and `refs` carry the immediate hierarchy so the page renders from this one read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskDetail {
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    pub body: String,
    pub updated: Field<Rfc3339>,
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

// One logical task aggregated across every branch it lives on: `metadata` and `title` come from the
// `headline` branch (the most recently changed one), plus one `branches` entry per branch so a
// client can render badges, mark the headline, and spot divergence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskListItem {
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    pub updated: Field<Rfc3339>,
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
    pub status: Field<Status>,
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
    pub dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl CreateTask {
    pub fn into_task(
        self,
        created: Timestamp,
        abbreviation: Abbreviation,
    ) -> Result<Task, KeyError> {
        let mut task = Task::new(&self.title, self.status.unwrap_or(Status::Backlog), created);
        task.set_parent(
            self.parent
                .as_deref()
                .map(|parent| reference_of(abbreviation, parent))
                .transpose()?,
        );
        task.set_dependencies(
            self.dependencies
                .iter()
                .map(|dependency| reference_of(abbreviation, dependency))
                .collect::<Result<_, _>>()?,
        );
        if let Some(body) = &self.body {
            task.append_body(&body_from_keys(abbreviation, body)?);
        }
        Ok(task)
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

// `Keep` means "omit the key", which only the holding field's `skip_serializing_if` can express; a
// `Keep` that reaches here anyway serializes as `null`, the nearest wire value.
impl<T: Serialize> Serialize for FieldUpdate<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            FieldUpdate::Keep | FieldUpdate::Clear => serializer.serialize_none(),
            FieldUpdate::Set(value) => serializer.serialize_some(value),
        }
    }
}

impl<T> FieldUpdate<T> {
    pub fn is_keep(&self) -> bool {
        matches!(self, FieldUpdate::Keep)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub status: Option<Status>,
    #[serde(default, skip_serializing_if = "FieldUpdate::is_keep")]
    #[schema(value_type = Option<String>)]
    pub parent: FieldUpdate<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub rank: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub dependencies: Option<Vec<String>>,
}

impl TaskPatch {
    pub fn apply(self, task: &mut Task, abbreviation: Abbreviation) -> Result<(), KeyError> {
        if let Some(status) = self.status {
            task.set_status(status);
        }
        match self.parent {
            FieldUpdate::Keep => {}
            FieldUpdate::Clear => task.set_parent(None),
            FieldUpdate::Set(key) => task.set_parent(Some(reference_of(abbreviation, &key)?)),
        }
        if let Some(rank) = self.rank {
            task.set_rank(Some(rank));
        }
        if let Some(dependencies) = self.dependencies {
            task.set_dependencies(
                dependencies
                    .iter()
                    .map(|dependency| reference_of(abbreviation, dependency))
                    .collect::<Result<_, _>>()?,
            );
        }
        Ok(())
    }
}

// The subtree rooted at one task: siblings within `children` are ordered by `rank`, the tree
// built by grouping the flat task set on `parent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskTree {
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    pub children: Vec<TaskTree>,
}

// A `rank` is an order somebody set on purpose, so ranked tasks hold the top of the list
// and `unranked` decides among the rest.
fn rank_cmp(
    ra: Option<&str>,
    rb: Option<&str>,
    unranked: impl Fn() -> std::cmp::Ordering,
) -> std::cmp::Ordering {
    match (ra, rb) {
        (Some(x), Some(y)) => x.cmp(y).then_with(unranked),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => unranked(),
    }
}

// A key is a number behind a prefix, so it orders as one — text order would file `OPP-10`
// between `OPP-1` and `OPP-2`. The prefix is constant within a store, so dropping it changes nothing.
pub fn id_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    match (key_number(a), key_number(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        _ => a.cmp(b),
    }
}

fn key_number(key: &str) -> Option<u64> {
    op_task::parse_id(key.rsplit_once('-')?.1)
}

// Rank ties break by id so the order is stable across rebuilds.
pub fn sibling_cmp(a: &TaskSummary, b: &TaskSummary) -> std::cmp::Ordering {
    rank_cmp(a.metadata.rank(), b.metadata.rank(), || {
        id_cmp(&a.id, &b.id)
    })
}

pub fn list_item_cmp(a: &TaskListItem, b: &TaskListItem) -> std::cmp::Ordering {
    rank_cmp(a.metadata.rank(), b.metadata.rank(), || {
        id_cmp(&a.id, &b.id)
    })
}

// The board leads with the work touched most recently, so what someone just changed is at hand
// without scrolling for it.
pub fn board_cmp(a: &TaskListItem, b: &TaskListItem) -> std::cmp::Ordering {
    rank_cmp(a.metadata.rank(), b.metadata.rank(), || {
        newest_first(a.updated.as_value(), b.updated.as_value()).then_with(|| id_cmp(&a.id, &b.id))
    })
}

// An `updated` that could not be derived places the task nowhere on the scale, so it sorts after
// every dated one rather than reading as the oldest.
fn newest_first(a: Option<&Rfc3339>, b: Option<&Rfc3339>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(x), Some(y)) => y.0.cmp(&x.0),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
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
        let title_of: std::collections::HashMap<&str, &str> = tasks
            .iter()
            .map(|t| (t.id.as_str(), t.title.as_str()))
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
            let member_ids: std::collections::HashSet<&str> =
                members.iter().map(|m| m.id.as_str()).collect();
            let mut children_of: std::collections::HashMap<&str, Vec<&TaskListItem>> =
                std::collections::HashMap::new();
            let mut roots: Vec<&TaskListItem> = Vec::new();
            for member in members {
                match member.metadata.parent() {
                    Some(parent) if member_ids.contains(parent) => {
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
        node.metadata
            .parent()
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
// versions collapse into a single row and divergent ones stand out. A `Deleted` mark carries
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
    // The store's `config.toml` changed, so every key on screen may render differently; a client
    // re-reads the config along with whatever it is showing.
    ConfigChanged,
    DaemonStopping,
}
