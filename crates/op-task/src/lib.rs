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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frontmatter {
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
