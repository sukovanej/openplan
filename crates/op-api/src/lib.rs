use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;

use op_task::Task;
pub use op_task::tag::Color;
use op_task::tag::{ParseNameError, Tag};
pub use op_task::{Abbreviation, Status, Timestamp};

pub const ADMIN_HEADER: &str = "x-openplan-admin";

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
    // The members of each dependency cycle a request could not order. A client links the keys, which
    // it cannot do with a sentence. Every other refusal sends the message alone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cycles: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
// The daemon serves every registered repository, so it names none of them here. A client asks
// `/api/projects` which repository it is talking to.
pub struct DaemonInfo {
    pub pid: u32,
    pub port: u16,
    pub version: String,
    pub started_at: u64,
}

// One repository the daemon serves. `git_common_dir` is what a client matches its own checkout
// against: it is the same string for every worktree of a repository, where `root` names only the one
// checkout this daemon indexes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProjectView {
    pub name: String,
    pub root: String,
    pub git_common_dir: String,
    pub abbreviation: String,
    pub status: ProjectStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProjectStatus {
    Ok,
    Error { reason: String },
}

impl ProjectStatus {
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Ok => None,
            Self::Error { reason } => Some(reason),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RegisterProject {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RenameProject {
    pub name: String,
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
    // Tag names, not keys: a tag is identified by the name a task file spells, so nothing here is
    // translated the way a reference is.
    pub tags: Field<Vec<String>>,
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
                tags: fields.tags.into(),
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
            tags: Field::Value(fm.tags.clone()),
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
        push("tags", fields.tags.as_error());
        out
    }

    pub fn dependencies(&self) -> &[String] {
        match self.fields().map(|fields| &fields.dependencies) {
            Some(Field::Value(dependencies)) => dependencies,
            _ => &[],
        }
    }

    pub fn tags(&self) -> &[String] {
        match self.fields().map(|fields| &fields.tags) {
            Some(Field::Value(tags)) => tags,
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

// A reference as a task file spells it. `Metadata` renders one as this store's key, which is the id
// every surface above the store speaks — but the store reads a task file back, and there a
// reference is the number the file layer allocates. A rendering the store cannot read would lose
// the parent of any task written back from it. A reference that is not key-shaped came through
// `key_of` unchanged and is already in the file spelling.
// A plain reference is a number, and YAML writes a number unquoted; one aimed at a section carries
// text the number cannot hold, so it stays a string.
fn file_reference(reference: &str) -> serde_yaml::Value {
    let target = op_task::ref_target(reference);
    match (key_number(target), target == reference) {
        (Some(number), true) => number.into(),
        (Some(number), false) => reference.replacen(target, &number.to_string(), 1).into(),
        (None, _) => reference.into(),
    }
}

// A task the daemon could not parse well enough to write back as a file. `status` and `created` are
// what makes a task file one — the store refuses a write without them — so a rendering missing
// either would look like a task file and destroy the task if it were written over one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "this task cannot be rendered as a task file: its frontmatter is missing `status`, `created`, \
     or both. Read the file itself to repair it."
)]
pub struct RenderError;

// A task file rebuilt from the state the daemon holds, for a caller that asked for markdown rather
// than JSON. The daemon parses; nothing above it keeps the bytes, so this is a canonical rendering
// and not the file: key order, spacing, and keys no field names are normalized away, and a field
// that did not parse is left out rather than guessed at (`Metadata::problems` names those).
pub fn render_task_file(
    metadata: &Metadata,
    body: &str,
    comments: &[Comment],
) -> Result<String, RenderError> {
    let mut frontmatter = serde_yaml::Mapping::new();
    let mut put = |key: &str, value: serde_yaml::Value| {
        frontmatter.insert(serde_yaml::Value::String(key.to_owned()), value);
    };
    let fields = metadata.fields().ok_or(RenderError)?;
    {
        let (Some(status), Some(created)) = (fields.status.as_value(), fields.created.as_value())
        else {
            return Err(RenderError);
        };
        put("status", status.as_str().into());
        put("created", created.to_string().into());
        if let Some(Some(parent)) = fields.parent.as_value() {
            put("parent", file_reference(parent));
        }
        if let Some(Some(rank)) = fields.rank.as_value() {
            put("rank", rank.as_str().into());
        }
        match fields.dependencies.as_value() {
            Some(dependencies) if !dependencies.is_empty() => put(
                "dependencies",
                dependencies
                    .iter()
                    .map(|d| file_reference(d))
                    .collect::<Vec<_>>()
                    .into(),
            ),
            _ => {}
        }
        match fields.tags.as_value() {
            Some(tags) if !tags.is_empty() => put(
                "tags",
                tags.iter().map(String::as_str).collect::<Vec<_>>().into(),
            ),
            _ => {}
        }
    }
    let yaml = serde_yaml::to_string(&frontmatter).map_err(|_| RenderError)?;
    let parsed: Vec<op_task::comment::Comment> = comments.iter().map(Into::into).collect();
    let body = op_task::comment::with_comments(body, &parsed);
    Ok(format!("---\n{yaml}---\n{body}"))
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

// One entry of a task's comment log. `at` and `author` are fields on the read path for the same
// reason the frontmatter's are: a hand-damaged heading must still deliver the text it introduces.
// `agent` is an `Option` rather than a field, because a comment a person typed has no agent and
// that is not a failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Comment {
    pub at: Field<Rfc3339>,
    pub author: Field<String>,
    // Always on the wire, `null` for a comment a person typed: a reader tells "no agent" from "the
    // daemon did not say" without knowing which keys this shape may drop.
    #[serde(default)]
    pub agent: Option<String>,
    pub text: String,
}

impl From<&op_task::comment::Comment> for Comment {
    fn from(comment: &op_task::comment::Comment) -> Self {
        Self {
            at: Field::from(comment.at.clone()).map(Rfc3339),
            author: Field::from(comment.author.clone()),
            agent: comment.agent.clone(),
            text: comment.text.clone(),
        }
    }
}

impl From<&op_task::comment::NewComment> for Comment {
    fn from(comment: &op_task::comment::NewComment) -> Self {
        Self {
            at: Field::Value(Rfc3339(comment.at)),
            author: Field::Value(comment.author.clone()),
            agent: comment.agent.clone(),
            text: comment.text.clone(),
        }
    }
}

impl From<&Comment> for op_task::comment::Comment {
    fn from(comment: &Comment) -> Self {
        Self {
            at: comment.at.clone().into_result().map(|at| at.0),
            author: comment.author.clone().into_result(),
            agent: comment.agent.clone(),
            text: comment.text.clone(),
        }
    }
}

// One branch's whole log, for the read that spans every branch. Grouped rather than flat so no
// entry repeats the branch it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BranchComments {
    pub branch: String,
    pub comments: Vec<Comment>,
}

// A comment to append. The daemon stamps the time, because it is the single in-band writer and one
// clock must order every write; the caller carries the identity, because only the CLI process sees
// the environment that names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateComment {
    pub text: String,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub agent: Option<String>,
}

// One task read for the detail page: `metadata` parsed field by field, so a file with one bad field
// still renders everything else and flags only what failed, and `body` always the raw markdown.
// `updated` is derived from git rather than read from the file, so it sits outside `metadata`.
// Paired with every branch it lives on, so a cold-loaded page renders a branch switcher without the
// list in memory; `headline` names the branch the shown version resolves to. `parent_title`,
// `children`, and `refs` carry the immediate hierarchy so the page renders from this one read.
// `write_target` names where an edit of the shown version lands, and whether it can land there.
// `depends_on` is what this task waits for, in the order the file lists it; `blocks` is every task
// that waits for this one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskDetail {
    pub project: String,
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    pub body: String,
    pub updated: Field<Rfc3339>,
    pub headline: String,
    pub branches: Vec<BranchState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub write_target: Option<WriteTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub parent_title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TaskChild>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<TaskRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<TaskRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<TaskRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
}

// One logical task aggregated across every branch it lives on: `metadata` and `title` come from the
// `headline` branch (the most recently changed one), plus one `branches` entry per branch so a
// client can render badges, mark the headline, and spot divergence. `project` is the coordinate the
// key alone cannot carry: two stores can commit the same abbreviation, so `id` names a task only
// within its project. `write_target` reads as it does on `TaskDetail`: a row the aggregate shows
// from another branch says here where acting on it would land.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskListItem {
    pub project: String,
    pub id: String,
    pub title: String,
    pub metadata: Metadata,
    // How many entries the log holds, not the log itself: a row shows a count, and shipping every
    // comment of every task would make the list read carry the whole store's prose.
    pub comment_count: usize,
    pub updated: Field<Rfc3339>,
    pub headline: String,
    pub branches: Vec<BranchState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub write_target: Option<WriteTarget>,
}

// Where a write to one task lands, and whether it can land there now. A read scoped to a branch
// writes to that branch; a read of the aggregate writes wherever the task lives. `writable` is false
// while no live worktree holds `branch` — none has it checked out, or the one that does is mid-merge
// — and the daemon then refuses every write to the task. A client reads this to offer only the
// actions that can succeed, and to name the branch that stops the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WriteTarget {
    pub branch: String,
    pub writable: bool,
}

// One task a search matched, with the branch whose version of it matched. The task itself is the
// aggregated row every other list read answers with, so a hit renders exactly like a list row;
// `branch` is what the hit adds — where the matching text lives, which is the headline branch
// whenever that branch matches too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SearchHit {
    pub task: TaskListItem,
    pub branch: String,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
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
        task.set_tags(self.tags);
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

// One registered tag. `name` is the identity a task's `tags:` holds; `display` is the heading a
// human reads, which differs from the name only in case and separators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TagView {
    pub name: String,
    pub display: String,
    pub color: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub description: Option<String>,
}

impl From<&Tag> for TagView {
    fn from(tag: &Tag) -> Self {
        let description = tag.description();
        Self {
            name: tag.name.clone(),
            display: tag.display_name().unwrap_or_else(|| tag.name.clone()),
            color: tag.color(),
            description: (!description.is_empty()).then_some(description),
        }
    }
}

// `name` is the name a human typed, so `Front End` registers `front-end` and reads back as
// `# Front End`. An omitted color is derived from the name rather than left unset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateTag {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub description: Option<String>,
}

impl CreateTag {
    pub fn into_tag(self) -> Result<Tag, ParseNameError> {
        let mut tag = Tag::new(&self.name, self.color)?;
        if let Some(description) = &self.description {
            tag.set_description(description);
        }
        Ok(tag)
    }
}

// `name` renames: it carries the new display name, and its normalization is the tag's new identity,
// so a change of case alone moves the heading and leaves the file where it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TagPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "FieldUpdate::is_keep")]
    #[schema(value_type = Option<String>)]
    pub description: FieldUpdate<String>,
}

impl TagPatch {
    pub fn changes_content(&self) -> bool {
        self.color.is_some() || !self.description.is_keep()
    }

    // The rename is the store's to carry out — it moves the file and rewrites every task that
    // references the tag — so this covers only what lives inside the tag file.
    pub fn apply(self, tag: &mut Tag) {
        if let Some(color) = self.color {
            tag.set_color(color);
        }
        match self.description {
            FieldUpdate::Keep => {}
            FieldUpdate::Clear => tag.set_description(""),
            FieldUpdate::Set(description) => tag.set_description(&description),
        }
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
    // The whole set replaces the old one, which is what the store validates: a name the branch does
    // not register fails the write even when the task already carried it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub tags: Option<Vec<String>>,
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
        if let Some(tags) = self.tags {
            task.set_tags(tags);
        }
        Ok(())
    }
}

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
// without scrolling for it. A merged board holds tasks from several projects, so the project name
// breaks a tie before the id does: two stores can issue the same key, and the order must not depend
// on which project was read first. It only breaks a tie. Each store ranks its own tasks, so ranked
// tasks from two projects interleave, and every ranked task still leads every unranked one — a
// merged board is ordered, not grouped by project.
pub fn board_cmp(a: &TaskListItem, b: &TaskListItem) -> std::cmp::Ordering {
    rank_cmp(a.metadata.rank(), b.metadata.rank(), || {
        newest_first(a.updated.as_value(), b.updated.as_value())
            .then_with(|| a.project.cmp(&b.project))
            .then_with(|| id_cmp(&a.id, &b.id))
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

// Where a task sits. Two stores can commit the same abbreviation, so every map in the merged board
// keys on this rather than on the id alone; a parent reference then resolves in the task's own
// project, which is the only project it can name.
type Coordinate<'a> = (&'a str, &'a str);

fn coordinate(task: &TaskListItem) -> Coordinate<'_> {
    (task.project.as_str(), task.id.as_str())
}

fn parent_coordinate(task: &TaskListItem) -> Option<Coordinate<'_>> {
    Some((task.project.as_str(), task.metadata.parent()?))
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

fn is_remaining(task: &TaskListItem) -> bool {
    !matches!(
        task.metadata.status(),
        Some(Status::Done) | Some(Status::Cancelled)
    )
}

type TaskIndex<'a> = std::collections::HashMap<Coordinate<'a>, &'a TaskListItem>;
type Members<'a> = std::collections::HashSet<Coordinate<'a>>;
type Kids<'a> = std::collections::HashMap<Coordinate<'a>, Vec<&'a TaskListItem>>;

// A dependency resolves inside its own project, and a `#section` suffix aims at a part of a task
// rather than at another task.
fn dependency_target<'a>(
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

// The seeds and what the flow needs around them: what each included task waits for, the parent chain
// above it, and the children a parent has left. The three rules feed each other until nothing new
// arrives, so the whole reachable neighbourhood lands in the set and no depth bounds it. A task
// nobody can complete any more — `done` or `cancelled` — enters as a parent only: a finished
// dependency is not work, and a finished child is not part of the box.
fn grow<'a>(
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

// Which task the flow puts each task under, and which tasks each one holds. A parent whose children
// all dropped out holds none, so it reads as a plain node. A corrupt pair of files can make a task
// its own ancestor: neither task can hold the other, so both stand alone here instead of vanishing
// into a box that contains itself. The board keeps such a task visible for the same reason.
struct Family<'a> {
    parent: std::collections::HashMap<Coordinate<'a>, Coordinate<'a>>,
    kids: Kids<'a>,
}

impl<'a> Family<'a> {
    fn build(members: &[&'a TaskListItem], included: &Members<'a>) -> Family<'a> {
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

    fn holds(&self, task: &'a TaskListItem) -> bool {
        self.kids.contains_key(&coordinate(task))
    }

    fn under(&self, task: &'a TaskListItem) -> Option<Coordinate<'a>> {
        self.parent.get(&coordinate(task)).copied()
    }

    fn above(&self, task: &'a TaskListItem, index: &TaskIndex<'a>) -> Vec<&'a TaskListItem> {
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

// A task nobody can complete any more declares nothing that can still block work, so its
// dependencies leave the flow with it.
fn remaining_dependencies(task: &TaskListItem) -> &[String] {
    match is_remaining(task) {
        true => task.metadata.dependencies(),
        false => &[],
    }
}

// Every leaf under a task, and the task itself when it is a leaf. A dependency on a box waits for
// each of these, and the box spans the waves they land in.
fn leaves_under<'a>(task: &'a TaskListItem, family: &Family<'a>) -> Vec<&'a TaskListItem> {
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

// Where each leaf sits: the wave it belongs to, the order inside that wave, and how much work waits
// for it.
struct Layout<'a> {
    leaves: Vec<&'a TaskListItem>,
    waves: Vec<Vec<usize>>,
    blocks: Vec<usize>,
}

impl<'a> Layout<'a> {
    fn build(
        leaves: Vec<&'a TaskListItem>,
        included: &Members<'a>,
        family: &Family<'a>,
        index: &TaskIndex<'a>,
    ) -> Result<Layout<'a>, FlowCycles> {
        let place: std::collections::HashMap<Coordinate<'a>, usize> = leaves
            .iter()
            .enumerate()
            .map(|(at, leaf)| (coordinate(leaf), at))
            .collect();
        let successors = waits_for(&leaves, included, family, index, &place);
        let wave_of = layer(&successors).map_err(|cycles| FlowCycles {
            cycles: cycle_ids(cycles, &leaves),
        })?;
        let blocks = blocks_counts(&successors, &wave_of);

        let depth = wave_of.iter().copied().max().map_or(0, |last| last + 1);
        let mut waves = vec![Vec::new(); depth];
        for (leaf, wave) in wave_of.iter().copied().enumerate() {
            waves[wave].push(leaf);
        }
        for wave in &mut waves {
            wave.sort_by(|a, b| {
                blocks[*b].cmp(&blocks[*a]).then_with(|| {
                    rank_cmp(
                        leaves[*a].metadata.rank(),
                        leaves[*b].metadata.rank(),
                        || {
                            id_cmp(&leaves[*a].id, &leaves[*b].id)
                                .then_with(|| leaves[*a].project.cmp(&leaves[*b].project))
                        },
                    )
                })
            });
        }
        Ok(Layout {
            leaves,
            waves,
            blocks,
        })
    }
}

// Which leaves wait for which. A child inherits the dependencies of its parents, so nothing inside a
// box starts before the box may start; a dependency on a box waits for each leaf inside it. An
// unresolved dependency adds no edge here: no work completes it, so it must not push the task that
// names it into a later wave.
fn waits_for<'a>(
    leaves: &[&'a TaskListItem],
    included: &Members<'a>,
    family: &Family<'a>,
    index: &TaskIndex<'a>,
    place: &std::collections::HashMap<Coordinate<'a>, usize>,
) -> Vec<Vec<usize>> {
    let mut successors = vec![std::collections::BTreeSet::new(); leaves.len()];
    for (at, leaf) in leaves.iter().enumerate() {
        for source in std::iter::once(*leaf).chain(family.above(leaf, index)) {
            for dependency in remaining_dependencies(source) {
                let Some(target) = dependency_target(index, source, dependency) else {
                    continue;
                };
                if !included.contains(&coordinate(target)) {
                    continue;
                }
                for blocker in leaves_under(target, family) {
                    if let Some(from) = place.get(&coordinate(blocker)) {
                        successors[*from].insert(at);
                    }
                }
            }
        }
    }
    successors
        .into_iter()
        .map(|targets| targets.into_iter().collect())
        .collect()
}

// Longest-path layering: a task lands one wave behind the last thing it waits for, so a person can
// start wave `k` once wave `k-1` is complete. The members of a cycle never come ready, and the
// request fails on them rather than answering with an order that does not exist.
fn layer(successors: &[Vec<usize>]) -> Result<Vec<usize>, Vec<Vec<usize>>> {
    let mut waiting = vec![0usize; successors.len()];
    for targets in successors {
        for target in targets {
            waiting[*target] += 1;
        }
    }
    let mut wave = vec![0usize; successors.len()];
    let mut ready: std::collections::BTreeSet<usize> = waiting
        .iter()
        .enumerate()
        .filter(|(_, count)| **count == 0)
        .map(|(at, _)| at)
        .collect();
    let mut layered = 0;
    while let Some(node) = ready.pop_first() {
        layered += 1;
        for target in &successors[node] {
            wave[*target] = wave[*target].max(wave[node] + 1);
            waiting[*target] -= 1;
            if waiting[*target] == 0 {
                ready.insert(*target);
            }
        }
    }
    match layered == successors.len() {
        true => Ok(wave),
        false => Err(rings(
            waiting
                .iter()
                .enumerate()
                .filter(|(_, count)| **count > 0)
                .map(|(at, _)| at)
                .collect(),
            successors,
        )),
    }
}

// The cycles among the leaves layering could not reach. The walk follows one step at a time until it
// meets a task it stands on already, which closes a cycle. It then cuts that one step and walks
// again, so a second cycle through a task of the first one still gets its own report; a walk that
// runs out of steps drops the task it stopped on. Each round cuts a step or drops a task, so the
// search ends. Two cycles over the same tasks read as one report, because the report names the
// members and both would name the same ones.
fn rings(left: Vec<usize>, successors: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut left: std::collections::BTreeSet<usize> = left.into_iter().collect();
    let mut cut: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut named: std::collections::HashSet<std::collections::BTreeSet<usize>> =
        std::collections::HashSet::new();
    let mut cycles = Vec::new();
    while let Some(start) = left.first().copied() {
        let mut path = Vec::new();
        let mut standing: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut current = start;
        loop {
            if let Some(first) = standing.get(&current).copied() {
                let cycle = path.split_off(first);
                cut.insert((cycle[cycle.len() - 1], current));
                if named.insert(cycle.iter().copied().collect()) {
                    cycles.push(cycle);
                }
                break;
            }
            standing.insert(current, path.len());
            path.push(current);
            let step = successors[current]
                .iter()
                .copied()
                .find(|next| left.contains(next) && !cut.contains(&(current, *next)));
            match step {
                Some(next) => current = next,
                None => {
                    left.remove(&current);
                    break;
                }
            }
        }
    }
    cycles
}

// The cycle as keys, turned so the lowest key opens it. The report then reads the same whichever
// member the walk happened to start from.
fn cycle_ids(cycles: Vec<Vec<usize>>, leaves: &[&TaskListItem]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = cycles
        .into_iter()
        .map(|cycle| {
            let first = (0..cycle.len())
                .min_by(|a, b| id_cmp(&leaves[cycle[*a]].id, &leaves[cycle[*b]].id))
                .unwrap_or_default();
            cycle
                .iter()
                .cycle()
                .skip(first)
                .take(cycle.len())
                .map(|node| leaves[*node].id.clone())
                .collect()
        })
        .collect();
    out.sort_by(|a, b| id_cmp(&a[0], &b[0]));
    out
}

// How much work waits for each leaf, directly or through another leaf. It is the first sort key
// inside a wave, so the task that unblocks the most work leads it. A leaf waits only behind lower
// waves, so counting from the last wave back gives each leaf its followers already counted.
fn blocks_counts(successors: &[Vec<usize>], wave_of: &[usize]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..successors.len()).collect();
    order.sort_by_key(|node| std::cmp::Reverse(wave_of[*node]));
    let mut waiting: Vec<std::collections::HashSet<usize>> =
        vec![std::collections::HashSet::new(); successors.len()];
    for node in order {
        let mut all = std::collections::HashSet::new();
        for target in &successors[node] {
            all.insert(*target);
            all.extend(waiting[*target].iter().copied());
        }
        waiting[node] = all;
    }
    waiting.into_iter().map(|all| all.len()).collect()
}

// The edges the flow draws: the dependencies each included task declares, as the file declares them.
// An inherited dependency draws none of its own — the arrow lands on the box, and the box holds the
// child. A dependency that is complete draws nothing at all.
fn wire<'a>(
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
fn nodes<'a>(
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MatrixCell {
    pub branch: String,
    pub task: TaskSummary,
    pub blob_oid: String,
    pub dirty: bool,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Matrix {
    pub cells: Vec<MatrixCell>,
}

// One task viewed across every branch it lives on, its cells grouped by blob OID so identical
// versions collapse into a single row and divergent ones stand out. A `Deleted` mark carries
// the pre-deletion blob, so a branch that removes the task groups under the version it removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskBranches {
    pub id: String,
    pub versions: Vec<TaskVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TaskVersion {
    pub blob_oid: String,
    pub summary: TaskSummary,
    pub branches: Vec<BranchMark>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
    TaskChanged {
        project: String,
        id: String,
        branch: String,
    },
    RefMoved {
        project: String,
        branch: String,
    },
    PresenceChanged {
        project: String,
        task_id: String,
    },
    // A tag was registered, recolored, re-described, renamed, or deleted. A client answers all five
    // the same way — read the registry again — so they are one event rather than five.
    TagsChanged {
        project: String,
        branch: String,
    },
    // Membership, a rename, a status change, or a new abbreviation. A client answers all four the
    // same way — read `/api/projects` again — so they are one event rather than four.
    ProjectsChanged,
    // The stream dropped events and cannot say which, so the client re-reads everything on screen.
    Resync,
    DaemonStopping,
}
