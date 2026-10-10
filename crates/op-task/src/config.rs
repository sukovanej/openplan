use semver::Version;

use crate::Abbreviation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub version: Version,
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
const VERSION_MUST_BE: &str = "'version' must be a semver version, such as \"0.0.1\"";
// A store from before the store versions has no `version` key.
pub const FIRST_VERSION: Version = Version::new(0, 0, 1);

// Every store version keeps this key as it is, so a binary reads it from a store whose other keys
// it does not know.
pub fn version(text: &str) -> Result<Version, ConfigError> {
    version_of(&table(text)?)
}

fn version_of(table: &toml::Table) -> Result<Version, ConfigError> {
    match table.get("version") {
        None => Ok(FIRST_VERSION),
        Some(toml::Value::String(text)) => {
            Version::parse(text).map_err(|_| ConfigError::new(VERSION_MUST_BE))
        }
        Some(_) => Err(ConfigError::new(VERSION_MUST_BE)),
    }
}

impl Config {
    pub fn new(version: Version, abbreviation: Abbreviation) -> Self {
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
                .map(|abbreviation| Self::new(version.clone(), abbreviation))
                .map_err(|_| ConfigError::new(MUST_BE)),
            Some(_) => Err(ConfigError::new(MUST_BE)),
        }
    }

    pub fn to_file_string(&self) -> String {
        format!(
            "version = \"{}\"\nabbreviation = \"{}\"\n",
            self.version, self.abbreviation
        )
    }
}

fn table(text: &str) -> Result<toml::Table, ConfigError> {
    toml::from_str(text).map_err(|err| ConfigError::new(err.message()))
}

// A config in an older layout keeps its other keys as they are, so only its version moves.
pub fn restamp(text: &str, version: &Version) -> Result<String, ConfigError> {
    if let Ok(config) = Config::parse(text) {
        return Ok(Config::new(version.clone(), config.abbreviation).to_file_string());
    }
    let mut table = table(text)?;
    table.insert(
        "version".to_owned(),
        toml::Value::String(version.to_string()),
    );
    toml::to_string(&table).map_err(|err| ConfigError::new(err.to_string()))
}
