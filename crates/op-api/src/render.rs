use crate::comment::Comment;
use crate::keys::key_number;
use crate::metadata::Metadata;

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
