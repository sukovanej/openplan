use serde::de::{Deserialize, DeserializeOwned, Deserializer};

pub mod content;
pub mod stream;
pub mod transcript;

pub use content::{ContentBlock, KnownContentBlock, Message, MessageContent, Role, Usage};
pub use stream::{KnownStreamInput, KnownStreamOutput, StreamInput, StreamOutput};
pub use transcript::{Record, TranscriptRecord};

// Claude Code ships without a published schema and changes both wire formats between patch
// releases, so every model here keeps unrecognised fields in `extra` and every tagged union falls
// back to a raw value. A parse must never fail on a newer CLI, and a re-serialise must not silently
// drop what this crate did not know about.
pub type Extra = serde_json::Map<String, serde_json::Value>;

// The same field arrives absent on one record and explicitly `null` on the next — `stop_reason` does
// both within a single session — and re-serialising the two the same way would corrupt the log.
// `None` means the key was missing; `Some(None)` means it was present and null.
pub type Nullable<T> = Option<Option<T>>;

pub fn nullable<'de, T, D>(deserializer: D) -> Result<Nullable<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

#[derive(Debug, thiserror::Error)]
#[error("{path}:{line}: {source}")]
pub struct ParseError {
    pub path: String,
    pub line: usize,
    #[source]
    pub source: serde_json::Error,
}

pub fn parse_jsonl<'a, T: DeserializeOwned>(
    path: &'a str,
    src: &'a str,
) -> impl Iterator<Item = Result<T, ParseError>> + use<'a, T> {
    src.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(move |(index, line)| {
            serde_json::from_str(line).map_err(|source| ParseError {
                path: path.to_owned(),
                line: index + 1,
                source,
            })
        })
}
