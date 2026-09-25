use crate::Abbreviation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
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

impl Config {
    pub fn new(abbreviation: Abbreviation) -> Self {
        Self { abbreviation }
    }

    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let table: toml::Table =
            toml::from_str(text).map_err(|err| ConfigError::new(err.message()))?;
        match table.get("abbreviation") {
            None => Err(ConfigError::new(REQUIRED)),
            Some(toml::Value::String(text)) => text
                .parse()
                .map(Self::new)
                .map_err(|_| ConfigError::new(MUST_BE)),
            Some(_) => Err(ConfigError::new(MUST_BE)),
        }
    }

    pub fn to_file_string(&self) -> String {
        format!("abbreviation = \"{}\"\n", self.abbreviation)
    }
}
