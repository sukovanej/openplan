use crate::Abbreviation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub version: u32,
    pub abbreviation: Abbreviation,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}: {reason}", crate::layout::CONFIG)]
pub struct ConfigError {
    reason: String,
}

impl ConfigError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    pub fn missing() -> Self {
        Self::new(REQUIRED)
    }
}

const REQUIRED: &str = "'abbreviation' required";
const MUST_BE: &str = "'abbreviation' must be exactly three uppercase letters";
const VERSION_MUST_BE: &str = "'version' must be a whole number from 1";
// A store from before the store versions has no `version` key.
pub const FIRST_VERSION: u32 = 1;

// Every store version keeps this key as it is, so a binary reads it from a store whose other keys
// it does not know.
pub fn version(text: &str) -> Result<u32, ConfigError> {
    version_of(&table(text)?)
}

fn version_of(table: &toml::Table) -> Result<u32, ConfigError> {
    match table.get("version") {
        None => Ok(FIRST_VERSION),
        Some(toml::Value::Integer(number)) => u32::try_from(*number)
            .ok()
            .filter(|number| *number >= FIRST_VERSION)
            .ok_or_else(|| ConfigError::new(VERSION_MUST_BE)),
        Some(_) => Err(ConfigError::new(VERSION_MUST_BE)),
    }
}

impl Config {
    pub fn new(version: u32, abbreviation: Abbreviation) -> Self {
        Self {
            version,
            abbreviation,
        }
    }

    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let table = table(text)?;
        let version = version_of(&table)?;
        match table.get("abbreviation") {
            None => Err(ConfigError::new(REQUIRED)),
            Some(toml::Value::String(text)) => text
                .parse()
                .map(|abbreviation| Self::new(version, abbreviation))
                .map_err(|_| ConfigError::new(MUST_BE)),
            Some(_) => Err(ConfigError::new(MUST_BE)),
        }
    }

    pub fn to_file_string(&self) -> String {
        let mut table = toml::Table::new();
        table.insert(
            "abbreviation".to_owned(),
            toml::Value::String(self.abbreviation.to_string()),
        );
        written(table, self.version)
    }
}

fn table(text: &str) -> Result<toml::Table, ConfigError> {
    toml::from_str(text).map_err(|err| ConfigError::new(err.message()))
}

// Only the `version` key moves, so two binaries that know different keys write the same bytes.
pub fn restamp(text: &str, version: u32) -> Result<String, ConfigError> {
    Ok(written(table(text)?, version))
}

fn written(mut table: toml::Table, version: u32) -> String {
    table.insert("version".to_owned(), toml::Value::Integer(version.into()));
    toml::to_string(&table).expect("a table of strings and integers serializes")
}
