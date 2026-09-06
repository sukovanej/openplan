mod driver;
mod launch;
mod translate;

use std::path::PathBuf;

use op_agent::{Agent, AgentError, AgentKind, Session, SessionOptions, process, session};

pub use launch::{Settings, Skills, Tools};
pub use translate::Translator;

pub struct ClaudeCode {
    binary: PathBuf,
    tools: Tools,
    skills: Skills,
    settings: Settings,
}

impl Default for ClaudeCode {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("claude"),
            tools: Tools::All,
            skills: Skills::Enabled,
            settings: Settings::Inherit,
        }
    }
}

impl ClaudeCode {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn binary(mut self, binary: impl Into<PathBuf>) -> Self {
        self.binary = binary.into();
        self
    }

    pub fn tools(mut self, tools: Tools) -> Self {
        self.tools = tools;
        self
    }

    pub fn skills(mut self, skills: Skills) -> Self {
        self.skills = skills;
        self
    }

    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }
}

impl Agent for ClaudeCode {
    fn kind(&self) -> AgentKind {
        AgentKind::ClaudeCode
    }

    fn start(&self, options: SessionOptions) -> Result<Session, AgentError> {
        let command = launch::command(
            &self.binary,
            &self.tools,
            &self.skills,
            &self.settings,
            &options,
        );
        let process = process::spawn(command)?;
        let (session, channel) = session::open();
        tokio::spawn(driver::run(process, channel, options));
        Ok(session)
    }
}
