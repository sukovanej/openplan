use crate::Abbreviation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub header: Header,
    pub abbreviation: Abbreviation,
}

// Every format keeps these keys as they are, so a binary reads them from a store whose other keys
// it does not know, and tells the reader which openplan the store needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub format: u32,
    pub requires: Option<String>,
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
const REQUIRES_MUST_BE: &str = "'requires' must be a version";
// A store from before the formats has no `format` key.
const FIRST_FORMAT: u32 = 1;

impl Header {
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        Self::of(&table(text)?)
    }

    fn of(table: &toml::Table) -> Result<Self, ConfigError> {
        let format = match table.get("format") {
            None => FIRST_FORMAT,
            Some(toml::Value::Integer(number)) => u32::try_from(*number)
                .ok()
                .filter(|number| *number >= FIRST_FORMAT)
                .ok_or_else(|| ConfigError::new(FORMAT_MUST_BE))?,
            Some(_) => return Err(ConfigError::new(FORMAT_MUST_BE)),
        };
        let requires = match table.get("requires") {
            None => None,
            Some(toml::Value::String(version)) => Some(version.clone()),
            Some(_) => return Err(ConfigError::new(REQUIRES_MUST_BE)),
        };
        Ok(Self { format, requires })
    }
}

impl Config {
    pub fn new(header: Header, abbreviation: Abbreviation) -> Self {
        Self {
            header,
            abbreviation,
        }
    }

    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let table = table(text)?;
        let header = Header::of(&table)?;
        match table.get("abbreviation") {
            None => Err(ConfigError::new(REQUIRED)),
            Some(toml::Value::String(text)) => text
                .parse()
                .map(|abbreviation| Self::new(header, abbreviation))
                .map_err(|_| ConfigError::new(MUST_BE)),
            Some(_) => Err(ConfigError::new(MUST_BE)),
        }
    }

    pub fn to_file_string(&self) -> String {
        let requires = self
            .header
            .requires
            .as_ref()
            .map(|version| format!("requires = \"{version}\"\n"))
            .unwrap_or_default();
        format!(
            "format = {}\n{requires}abbreviation = \"{}\"\n",
            self.header.format, self.abbreviation
        )
    }
}

fn table(text: &str) -> Result<toml::Table, ConfigError> {
    toml::from_str(text).map_err(|err| ConfigError::new(err.message()))
}

// A config in an older layout keeps its other keys as they are, so only its header moves.
pub fn restamp(text: &str, header: &Header) -> Result<String, ConfigError> {
    if let Ok(config) = Config::parse(text) {
        return Ok(Config::new(header.clone(), config.abbreviation).to_file_string());
    }
    let mut table = table(text)?;
    table.insert(
        "format".to_owned(),
        toml::Value::Integer(header.format.into()),
    );
    match &header.requires {
        Some(version) => table.insert("requires".to_owned(), toml::Value::String(version.clone())),
        None => table.remove("requires"),
    };
    toml::to_string(&table).map_err(|err| ConfigError::new(err.to_string()))
}
