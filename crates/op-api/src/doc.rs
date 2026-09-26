use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use op_task::content::Text;
use op_task::{Abbreviation, Timestamp, doc::Doc};

use crate::field::{Field, FieldUpdate, Rfc3339};
use crate::keys::{KeyError, body_from_keys};
use crate::metadata::MetadataErrorTag;
use crate::task::{Author, Problem, TaskRef};

// A doc referenced by `[[name]]`, resolved to its current title so a chip can render without the
// client looking it up in a full list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocRef {
    pub name: String,
    pub title: String,
}

// A doc's frontmatter, read the way a task's is: every field carries its own value or its own
// error, so a file with one bad field still renders the rest and flags only what failed. `parent`
// is the name of the doc this one nests under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocFields {
    pub created: Field<Rfc3339>,
    pub parent: Field<Option<String>>,
}

// `Fields` when the YAML is a mapping, `Error` when the fence or the YAML itself is unreadable and
// no field survives. Serialized untagged, as a task's metadata is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum DocMetadata {
    Error {
        kind: MetadataErrorTag,
        message: String,
    },
    Fields(DocFields),
}

impl DocMetadata {
    // A field `conflicts` names reads as a conflict between its published and its other version, as
    // a task's field does.
    pub fn from_partial(
        partial: &op_task::doc::PartialDocMetadata,
        conflicts: &[op_task::FieldConflict],
    ) -> Self {
        match partial {
            op_task::doc::PartialDocMetadata::Error(message) => DocMetadata::Error {
                kind: MetadataErrorTag::Error,
                message: message.clone(),
            },
            op_task::doc::PartialDocMetadata::Fields(fields) => {
                let mut fields = fields_of(fields.clone());
                for conflict in conflicts {
                    let other = fields_of(op_task::doc::other_fields(conflict));
                    match conflict.field.as_str() {
                        "created" => {
                            fields.created = fields.created.in_conflict(other.created, conflict)
                        }
                        "parent" => {
                            fields.parent = fields.parent.in_conflict(other.parent, conflict)
                        }
                        _ => {}
                    }
                }
                DocMetadata::Fields(fields)
            }
        }
    }

    pub fn conflicted_fields(&self) -> Vec<&'static str> {
        let Some(fields) = self.fields() else {
            return Vec::new();
        };
        [
            ("created", fields.created.is_conflict()),
            ("parent", fields.parent.is_conflict()),
        ]
        .into_iter()
        .filter_map(|(name, conflicted)| conflicted.then_some(name))
        .collect()
    }

    pub fn problems(&self) -> Vec<String> {
        let fields = match self {
            DocMetadata::Error { message, .. } => return vec![format!("frontmatter: {message}")],
            DocMetadata::Fields(fields) => fields,
        };
        [
            ("created", fields.created.as_error()),
            ("parent", fields.parent.as_error()),
        ]
        .into_iter()
        .filter_map(|(name, err)| {
            Some(match err? {
                crate::field::FieldError::Missing => format!("{name}: missing"),
                crate::field::FieldError::Invalid { message } => format!("{name}: {message}"),
            })
        })
        .collect()
    }

    pub fn fields(&self) -> Option<&DocFields> {
        match self {
            DocMetadata::Fields(fields) => Some(fields),
            DocMetadata::Error { .. } => None,
        }
    }

    pub fn created(&self) -> Option<Rfc3339> {
        self.fields()?.created.as_value().copied()
    }

    pub fn parent(&self) -> Option<&str> {
        self.fields()?.parent.as_value()?.as_deref()
    }
}

fn fields_of(fields: op_task::doc::PartialDocFields) -> DocFields {
    DocFields {
        created: Field::from(fields.created).map(Rfc3339),
        parent: Field::from(fields.parent),
    }
}

// A doc nested directly under another one, in name order — enough to render the nested list
// without the whole doc set in memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocChild {
    pub name: String,
    pub title: String,
}

// `name` is the file stem, which is the whole id: a doc is named by a human rather than allocated,
// so nothing translates between an in-memory spelling and a stored one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocListItem {
    pub project: String,
    pub name: String,
    pub title: String,
    // The frontmatter parsed field by field. A row carries `parent` through it, so a list builds the
    // whole tree from one read — and says which rows have a parent it could not read.
    pub metadata: DocMetadata,
    // Derived from the history rather than read from the file, so it sits outside `metadata`.
    pub updated: Field<Rfc3339>,
    // Counted like `DocDetail::conflicts`, so a row can call for attention to a conflict in the body.
    pub conflicts: usize,
    pub problems: Vec<Problem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub author: Option<Author>,
}

// One doc read for its own page. `body` is the markdown below the title heading; `refs` and
// `doc_refs` resolve the `[[…]]` the body carries, so a chip renders from this one read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocDetail {
    pub project: String,
    pub name: String,
    pub title: String,
    pub metadata: DocMetadata,
    pub updated: Field<Rfc3339>,
    pub body: String,
    // The open conflicts sync left in the doc: fields in `metadata`, and blocks of both versions in
    // `body`.
    pub conflicts: usize,
    pub problems: Vec<Problem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub author: Option<Author>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<TaskRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doc_refs: Vec<DocRef>,
    // The title of the doc `metadata.parent` names, when it resolves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub parent_title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<DocChild>,
}

// A doc as one revision left it, read the way the live doc is: `body` names each task by its key and
// each doc by its name, and `raw` is the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocSnapshot {
    pub title: String,
    pub metadata: DocMetadata,
    pub body: String,
    pub raw: String,
}

// `doc` is absent where the doc did not exist at the revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocAtRevision {
    pub name: String,
    pub revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub doc: Option<DocSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateDoc {
    // The name a human typed. It normalizes to the file stem, and it is the H1 the doc opens with.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    // The name of the doc this one nests under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

impl CreateDoc {
    pub fn into_doc(
        self,
        created: Timestamp,
        abbreviation: Abbreviation,
    ) -> Result<Doc, DocWriteError> {
        let mut doc = Doc::new(&self.name, created)?;
        doc.set_parent(self.parent.as_deref())?;
        if let Some(body) = &self.body {
            doc.set_content(&body_from_keys(abbreviation, body)?);
        }
        Ok(doc)
    }
}

// `name` renames the doc, which moves its file and rewrites its H1; `body` replaces the markdown
// below that heading; `parent` nests the doc, and a null lifts it to the top level. Omitting a key
// leaves it untouched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "FieldUpdate::is_keep")]
    #[schema(value_type = Option<String>)]
    pub parent: FieldUpdate<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocText {
    pub title: String,
    pub body: String,
}

// The web editor's save, as `WriteTaskText` is for a task. A new title renames the doc, because a
// doc's name is its title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WriteDocText {
    pub base: DocText,
    pub text: DocText,
}

impl WriteDocText {
    pub fn into_texts(self, abbreviation: Abbreviation) -> Result<(Text, Text), KeyError> {
        crate::write::texts(
            abbreviation,
            (self.base.title, &self.base.body),
            (self.text.title, &self.text.body),
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DocWriteError {
    #[error(transparent)]
    Name(#[from] op_task::doc::ParseNameError),
    #[error(transparent)]
    Key(#[from] KeyError),
}
