use op_backend::BackendError;
use op_task::config::ConfigError;

#[derive(Debug, thiserror::Error)]
pub enum TrackerError {
    #[error("this project has no tasks yet; `openplan init` starts them")]
    NotInitialized,
    #[error("this project already uses the abbreviation {0}")]
    AlreadyInitialized(String),
    #[error("no such task: {id}")]
    NotFound { id: String },
    #[error("no such tag: {name}")]
    TagNotFound { name: String },
    #[error("tag already exists: {name}")]
    TagExists { name: String },
    #[error("tag {name} is used by {count} task(s)")]
    TagReferenced { name: String, count: usize },
    #[error("no such doc: {name}")]
    DocNotFound { name: String },
    #[error("doc already exists: {name}")]
    DocExists { name: String },
    #[error("tag {name} is not registered")]
    TagUnregistered { name: String },
    #[error("not a task reference: {reference:?}; {}", op_task::REFERENCE_EXPECTED)]
    InvalidRef { reference: String },
    #[error("{0}")]
    Invalid(String),
    #[error(
        "{path} has no `created:` field, so it cannot be written — a write must not invent when \
         the task was created. Add the field to its frontmatter, for example `created: {example}`"
    )]
    MissingCreated { path: String, example: String },
    #[error("{path}: {reason}")]
    Unreadable { path: String, reason: String },
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Task(#[from] op_task::TaskError),
    #[error(transparent)]
    InvalidColor(#[from] op_task::tag::ParseColorError),
    #[error(transparent)]
    Backend(#[from] BackendError),
}
