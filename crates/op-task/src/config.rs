use crate::Abbreviation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub format: u32,
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
const FORMAT_MUST_BE: &str = "'format' must be a whole number from 1";
// A store from before the formats has no `format` key.
const FIRST_FORMAT: u32 = 1;

// Every format keeps this key as it is, so a binary reads it from a store whose other keys it does
// not know.
pub fn format(text: &str) -> Result<u32, ConfigError> {
    format_of(&table(text)?)
}

fn format_of(table: &toml::Table) -> Result<u32, ConfigError> {
    match table.get("format") {
        None => Ok(FIRST_FORMAT),
        Some(toml::Value::Integer(number)) => u32::try_from(*number)
            .ok()
            .filter(|number| *number >= FIRST_FORMAT)
            .ok_or_else(|| ConfigError::new(FORMAT_MUST_BE)),
        Some(_) => Err(ConfigError::new(FORMAT_MUST_BE)),
    }
}

impl Config {
    pub fn new(format: u32, abbreviation: Abbreviation) -> Self {
        Self {
            format,
            abbreviation,
        }
    }

    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let table = table(text)?;
        let format = format_of(&table)?;
        match table.get("abbreviation") {
            None => Err(ConfigError::new(REQUIRED)),
            Some(toml::Value::String(text)) => text
                .parse()
                .map(|abbreviation| Self::new(format, abbreviation))
                .map_err(|_| ConfigError::new(MUST_BE)),
            Some(_) => Err(ConfigError::new(MUST_BE)),
        }
    }

    pub fn to_file_string(&self) -> String {
        format!(
            "format = {}\nabbreviation = \"{}\"\n",
            self.format, self.abbreviation
        )
    }
}

fn table(text: &str) -> Result<toml::Table, ConfigError> {
    toml::from_str(text).map_err(|err| ConfigError::new(err.message()))
}

// A config in an older layout keeps its other keys as they are, so only its format moves.
pub fn restamp(text: &str, format: u32) -> Result<String, ConfigError> {
    if let Ok(config) = Config::parse(text) {
        return Ok(Config::new(format, config.abbreviation).to_file_string());
    }
    let mut table = table(text)?;
    table.insert("format".to_owned(), toml::Value::Integer(format.into()));
    toml::to_string(&table).map_err(|err| ConfigError::new(err.to_string()))
}
