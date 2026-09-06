mod driver;
mod launch;
pub mod protocol;
pub mod translate;

use std::path::PathBuf;

use op_agent::{Agent, AgentError, AgentKind, Session, SessionOptions};

pub use crate::driver::Driver;

pub struct Codex {
    binary: PathBuf,
    config: Vec<String>,
}

impl Default for Codex {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("codex"),
            config: Vec::new(),
        }
    }
}

impl Codex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn binary(mut self, binary: impl Into<PathBuf>) -> Self {
        self.binary = binary.into();
        self
    }

    // One `-c key=value` override, in the spelling `codex --config` takes.
    pub fn config(mut self, key: &str, value: &str) -> Self {
        self.config.push(format!("{key}={value}"));
        self
    }
}

impl Agent for Codex {
    fn kind(&self) -> AgentKind {
        AgentKind::Codex
    }

    fn start(&self, options: SessionOptions) -> Result<Session, AgentError> {
        let command = launch::command(&self.binary, &self.config, &options);
        let budget = options.budget;
        op_agent::driver::start(Driver::new(options), command, budget)
    }
}
