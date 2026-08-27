use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use op_task::{Abbreviation, Status, Timestamp};

use crate::field::{Field, FieldError, Rfc3339};
use crate::keys::key_of;

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
