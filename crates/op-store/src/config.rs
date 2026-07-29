use std::io;
use std::path::Path;

use op_task::Abbreviation;

use crate::STORE_DIR;

pub const CONFIG_FILE: &str = "config.toml";

// The store's own settings, read from `.plan/config.toml`. A store with no abbreviation has no id
// space above the file layer at all, so this is a hard stop rather than a per-field degradation:
// there is nothing for a missing key to fall back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub abbreviation: Abbreviation,
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
            Some(toml::Value::String(text)) => text.parse(),
            Some(_) => return Err(ConfigError::new(MUST_BE)),
        };
        abbreviation
            .map(|abbreviation| Self { abbreviation })
            .map_err(|_| ConfigError::new(MUST_BE))
    }
}

const MUST_BE: &str = "'abbreviation' must be exactly three uppercase letters";
