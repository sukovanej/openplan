use std::io;
use std::path::Path;

use op_task::Abbreviation;

use crate::STORE_DIR;

pub const CONFIG_FILE: &str = "config.toml";

// The store's own settings, read from `.plan/config.toml`. A store with no abbreviation has no id
// space above the file layer at all, so a missing `abbreviation` is a hard stop; `default_branch`
// falls back to what the repository itself says, so a missing one is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub abbreviation: Abbreviation,
    pub default_branch: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("{STORE_DIR}/{CONFIG_FILE}: {reason}")]
pub struct ConfigError {
    reason: String,
}

impl ConfigError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl Config {
    pub fn read(root: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = root.as_ref().join(STORE_DIR).join(CONFIG_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(ConfigError::new("'abbreviation' required"));
            }
            Err(e) => return Err(ConfigError::new(e.to_string())),
        };
        let table: toml::Table =
            toml::from_str(&text).map_err(|err| ConfigError::new(err.message()))?;
        let abbreviation = match table.get("abbreviation") {
            None => return Err(ConfigError::new("'abbreviation' required")),
            Some(toml::Value::String(text)) => {
                text.parse().map_err(|_| ConfigError::new(MUST_BE))?
            }
            Some(_) => return Err(ConfigError::new(MUST_BE)),
        };
        let default_branch = match table.get("default_branch") {
            None => None,
            Some(toml::Value::String(text)) => Some(text.clone()),
            Some(_) => return Err(ConfigError::new(DEFAULT_BRANCH_MUST_BE)),
        };
        Ok(Self {
            abbreviation,
            default_branch,
        })
    }
}

const MUST_BE: &str = "'abbreviation' must be exactly three uppercase letters";
const DEFAULT_BRANCH_MUST_BE: &str = "'default_branch' must be a string";
