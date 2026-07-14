use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Backlog,
    Todo,
    InProgress,
    Done,
    Cancelled,
}

impl Status {
    pub const ALL: [Status; 5] = [
        Status::Backlog,
        Status::Todo,
        Status::InProgress,
        Status::Done,
        Status::Cancelled,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Backlog => "backlog",
            Status::Todo => "todo",
            Status::InProgress => "in_progress",
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
    // Fields the model does not name (e.g. rank) are preserved verbatim across a
    // read-modify-write so a `set` never silently drops them.
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
}

impl Task {
    pub fn new(title: &str, status: Status) -> Self {
        Self {
            frontmatter: Frontmatter {
                status,
                parent: None,
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
        let frontmatter: Frontmatter = serde_yaml::from_str(&fm_src.replace('\r', ""))?;
        Ok(Self {
            frontmatter,
            body: body.to_owned(),
        })
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
