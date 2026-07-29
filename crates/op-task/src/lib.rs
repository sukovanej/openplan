use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub use jiff::Timestamp;

pub mod rank;

// Task files are hand-written and diffed, so a stored timestamp carries whole seconds — the clock's
// sub-second tail is noise no reader of a task file wants.
pub fn now() -> Timestamp {
    Timestamp::from_second(Timestamp::now().as_second())
        .expect("a whole second of the current time is in range")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Backlog,
    Todo,
    InProgress,
    InReview,
    Done,
    Cancelled,
}

impl Status {
    pub const ALL: [Status; 6] = [
        Status::Backlog,
        Status::Todo,
        Status::InProgress,
        Status::InReview,
        Status::Done,
        Status::Cancelled,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Backlog => "backlog",
            Status::Todo => "todo",
            Status::InProgress => "in_progress",
            Status::InReview => "in_review",
            Status::Done => "done",
            Status::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid status {got:?}; expected one of {}", Status::ALL.map(|s| s.as_str()).join(", "))]
pub struct ParseStatusError {
    got: String,
}

impl std::str::FromStr for Status {
    type Err = ParseStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Status::ALL
            .into_iter()
            .find(|status| status.as_str() == s)
            .ok_or_else(|| ParseStatusError { got: s.to_owned() })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    pub status: Status,
    pub created: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
    // Fields the model does not name are preserved verbatim across a read-modify-write
    // so a `set` never silently drops them.
    #[serde(flatten)]
    pub extra: serde_yaml::Mapping,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub frontmatter: Frontmatter,
    pub body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("invalid frontmatter: {0}")]
    Frontmatter(#[from] serde_yaml::Error),
    #[error("missing frontmatter fence")]
    MissingFrontmatter,
    // Its own variant, not a plain YAML error, because it is the one parse failure a caller can
    // explain in terms of what the reader must do — see `StoreError::MissingCreated`.
    #[error("no `created:` field")]
    MissingCreated,
}

impl Task {
    pub fn new(title: &str, status: Status, created: Timestamp) -> Self {
        Self {
            frontmatter: Frontmatter {
                status,
                created,
                parent: None,
                rank: None,
                deps: Vec::new(),
                extra: serde_yaml::Mapping::new(),
            },
            body: format!("# {title}\n"),
        }
    }

    pub fn append_body(&mut self, content: &str) {
        let content = content.trim_end_matches('\n');
        if content.is_empty() {
            return;
        }
        let head = self.body.trim_end_matches('\n');
        self.body = format!("{head}\n\n{content}\n");
    }

    pub fn set_status(&mut self, status: Status) {
        self.frontmatter.status = status;
    }

    pub fn set_parent(&mut self, parent: Option<String>) {
        self.frontmatter.parent = parent;
    }

    pub fn set_rank(&mut self, rank: Option<String>) {
        self.frontmatter.rank = rank;
    }

    pub fn set_deps(&mut self, deps: Vec<String>) {
        self.frontmatter.deps = deps;
    }

    pub fn title(&self) -> Option<String> {
        op_md::title(&self.body)
    }

    pub fn to_file_string(&self) -> Result<String, TaskError> {
        let fm = serde_yaml::to_string(&self.frontmatter)?;
        Ok(format!("---\n{fm}---\n{}", self.body))
    }

    pub fn from_file_string(input: &str) -> Result<Self, TaskError> {
        let (fm_src, body) = split_frontmatter(input).ok_or(TaskError::MissingFrontmatter)?;
        let fm_src = fm_src.replace('\r', "");
        match serde_yaml::from_str::<Frontmatter>(&fm_src) {
            Ok(frontmatter) => Ok(Self {
                frontmatter,
                body: body.to_owned(),
            }),
            Err(err) => Err(match serde_yaml::from_str::<serde_yaml::Mapping>(&fm_src) {
                Ok(map) if !map.contains_key("created") => TaskError::MissingCreated,
                _ => TaskError::Frontmatter(err),
            }),
        }
    }
}

// A per-field parse outcome for the read/display path, where one bad field must not sink the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldError {
    Missing,
    Invalid(String),
}

pub type FieldResult<T> = Result<T, FieldError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialFrontmatter {
    pub status: FieldResult<Status>,
    pub created: FieldResult<Timestamp>,
    pub parent: FieldResult<Option<String>>,
    pub rank: FieldResult<Option<String>>,
    pub deps: FieldResult<Vec<String>>,
}

// The frontmatter parsed as far as it can be: `Fields` when the YAML is a mapping (each field then
// succeeds or fails on its own), `Error` when the fence or the YAML itself is unparseable and no
// field can be recovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartialMetadata {
    Error(String),
    Fields(PartialFrontmatter),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialTask {
    pub metadata: PartialMetadata,
    pub title: Option<String>,
    pub body: String,
}

// The lenient counterpart to `from_file_string`: never fails, so a task with one bad field still
// yields its status, title, body, and every other readable field for the UI to render.
pub fn parse_partial(input: &str) -> PartialTask {
    match split_frontmatter(input) {
        None => PartialTask {
            metadata: PartialMetadata::Error("missing frontmatter fence".to_owned()),
            title: op_md::title(input),
            body: input.to_owned(),
        },
        Some((fm_src, body)) => {
            let metadata =
                match serde_yaml::from_str::<serde_yaml::Mapping>(&fm_src.replace('\r', "")) {
                    Ok(map) => PartialMetadata::Fields(extract_fields(&map)),
                    Err(err) => PartialMetadata::Error(err.to_string()),
                };
            PartialTask {
                metadata,
                title: op_md::title(body),
                body: body.to_owned(),
            }
        }
    }
}

fn extract_fields(map: &serde_yaml::Mapping) -> PartialFrontmatter {
    PartialFrontmatter {
        status: required(map, "status"),
        created: required(map, "created"),
        parent: optional_id(map, "parent"),
        rank: optional_id(map, "rank"),
        deps: match map.get("deps") {
            None => Ok(Vec::new()),
            Some(serde_yaml::Value::Sequence(items)) => items
                .iter()
                .map(|item| match item {
                    serde_yaml::Value::String(id) => Ok(id.clone()),
                    _ => Err(FieldError::Invalid(
                        "expected a list of task ids".to_owned(),
                    )),
                })
                .collect(),
            Some(_) => Err(FieldError::Invalid(
                "expected a list of task ids".to_owned(),
            )),
        },
    }
}

fn required<T: serde::de::DeserializeOwned>(
    map: &serde_yaml::Mapping,
    field: &str,
) -> FieldResult<T> {
    match map.get(field) {
        None => Err(FieldError::Missing),
        Some(value) => serde_yaml::from_value::<T>(value.clone())
            .map_err(|err| FieldError::Invalid(err.to_string())),
    }
}

fn optional_id(map: &serde_yaml::Mapping, field: &str) -> FieldResult<Option<String>> {
    match map.get(field) {
        None | Some(serde_yaml::Value::Null) => Ok(None),
        Some(serde_yaml::Value::String(id)) => Ok(Some(id.clone())),
        Some(_) => Err(FieldError::Invalid("expected a string".to_owned())),
    }
}

fn is_fence(line: &str) -> bool {
    line.trim_end_matches('\n').trim_end_matches('\r') == "---"
}

fn split_frontmatter(input: &str) -> Option<(&str, &str)> {
    let rest = input
        .strip_prefix("---\n")
        .or_else(|| input.strip_prefix("---\r\n"))?;
    let fm_start = input.len() - rest.len();
    let mut pos = fm_start;
    for line in rest.split_inclusive('\n') {
        if is_fence(line) {
            return Some((&input[fm_start..pos], &input[pos + line.len()..]));
        }
        pos += line.len();
    }
    None
}
