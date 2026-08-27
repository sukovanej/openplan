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
