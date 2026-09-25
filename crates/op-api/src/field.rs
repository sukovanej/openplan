use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;

use op_task::Timestamp;

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

// A frontmatter field on the read path: its parsed value, a per-field error, or a conflict that
// sync left for a person to settle. A client renders every field that parsed and flags the rest.
// Serialized untagged: a value is its bare JSON (`"todo"`, `null`, `["a"]`); an error and a
// conflict are objects with a `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum Field<T> {
    Value(T),
    Error(FieldError),
    Conflict(Box<FieldConflict<T>>),
}

// Two sides of a sync changed the field differently. `value` is the published version, in force
// until someone sets the field; `sides` holds both versions, the published one last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct FieldConflict<T> {
    pub kind: ConflictTag,
    pub value: T,
    pub sides: Vec<ConflictSide<T>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictTag {
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConflictSide<T> {
    pub label: String,
    pub value: T,
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

// A conflict reads as its published value, so the board, the tree, and the sort treat the task as
// the team last saw it.
impl<T> Field<T> {
    pub fn value(self) -> Option<T> {
        match self {
            Field::Value(value) => Some(value),
            Field::Conflict(conflict) => Some(conflict.value),
            Field::Error(_) => None,
        }
    }

    pub fn map<U>(self, f: impl Fn(T) -> U) -> Field<U> {
        match self {
            Field::Value(value) => Field::Value(f(value)),
            Field::Error(err) => Field::Error(err),
            Field::Conflict(conflict) => Field::Conflict(Box::new(FieldConflict {
                kind: conflict.kind,
                value: f(conflict.value),
                sides: conflict
                    .sides
                    .into_iter()
                    .map(|side| ConflictSide {
                        label: side.label,
                        value: f(side.value),
                    })
                    .collect(),
            })),
        }
    }

    pub fn into_result(self) -> op_task::FieldResult<T> {
        match self {
            Field::Value(value) => Ok(value),
            Field::Conflict(conflict) => Ok(conflict.value),
            Field::Error(FieldError::Missing) => Err(op_task::FieldError::Missing),
            Field::Error(FieldError::Invalid { message }) => {
                Err(op_task::FieldError::Invalid(message))
            }
        }
    }

    pub fn as_error(&self) -> Option<&FieldError> {
        match self {
            Field::Error(err) => Some(err),
            Field::Value(_) | Field::Conflict(_) => None,
        }
    }

    pub fn as_value(&self) -> Option<&T> {
        match self {
            Field::Value(value) => Some(value),
            Field::Conflict(conflict) => Some(&conflict.value),
            Field::Error(_) => None,
        }
    }

    pub fn is_conflict(&self) -> bool {
        matches!(self, Field::Conflict(_))
    }
}

impl<T: PartialEq + Clone> Field<T> {
    // The published version and the other one `conflict` keeps. An other version that does not
    // parse leaves the published one alone; the task still counts the conflict.
    pub fn in_conflict(self, other: Field<T>, conflict: &op_task::FieldConflict) -> Field<T> {
        match (self, other) {
            (Field::Value(value), Field::Value(other)) if value != other => {
                Field::Conflict(Box::new(FieldConflict {
                    kind: ConflictTag::Conflict,
                    sides: vec![
                        ConflictSide {
                            label: conflict.other_label.clone(),
                            value: other,
                        },
                        ConflictSide {
                            label: conflict.label.clone(),
                            value: value.clone(),
                        },
                    ],
                    value,
                }))
            }
            (published, _) => published,
        }
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
