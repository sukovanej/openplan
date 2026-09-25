use std::fmt;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RevisionId(String);

impl RevisionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub name: String,
    pub email: Option<String>,
    pub via: Option<String>,
}

impl Actor {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: None,
            via: None,
        }
    }

    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    pub fn via(mut self, agent: impl Into<String>) -> Self {
        self.via = Some(agent.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    pub id: RevisionId,
    pub parents: Vec<RevisionId>,
    pub author: Actor,
    pub at: Timestamp,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Modified,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
}

impl Change {
    pub fn new(path: impl Into<String>, kind: ChangeKind) -> Self {
        Self {
            path: path.into(),
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Put { path: String, bytes: Vec<u8> },
    Remove { path: String },
}

impl Op {
    pub fn put(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self::Put {
            path: path.into(),
            bytes: bytes.into(),
        }
    }

    pub fn remove(path: impl Into<String>) -> Self {
        Self::Remove { path: path.into() }
    }

    pub fn path(&self) -> &str {
        match self {
            Self::Put { path, .. } | Self::Remove { path } => path,
        }
    }

    pub fn content(&self) -> Option<&[u8]> {
        match self {
            Self::Put { bytes, .. } => Some(bytes),
            Self::Remove { .. } => None,
        }
    }
}
