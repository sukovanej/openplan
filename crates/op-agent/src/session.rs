use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{
    approval::{ApprovalDecision, ApprovalId},
    error::AgentError,
    event::AgentEvent,
    options::SessionOptions,
};

const CAPACITY: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    ClaudeCode,
    Codex,
}

// Starting is synchronous because it only spawns the process and wires the channels. Everything the
// agent reports about itself, its id included, arrives on the event stream.
pub trait Agent: Send + Sync {
    fn kind(&self) -> AgentKind;
    fn start(&self, options: SessionOptions) -> Result<Session, AgentError>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Prompt(String),
    Interrupt,
    Approve {
        id: ApprovalId,
        decision: ApprovalDecision,
    },
    Shutdown,
}

pub struct Session {
    handle: SessionHandle,
    events: mpsc::Receiver<AgentEvent>,
}

#[derive(Debug, Clone)]
pub struct SessionHandle {
    commands: mpsc::Sender<Command>,
}

pub struct Channel {
    pub commands: mpsc::Receiver<Command>,
    pub events: mpsc::Sender<AgentEvent>,
}

pub fn open() -> (Session, Channel) {
    let (command_tx, commands) = mpsc::channel(CAPACITY);
    let (events, event_rx) = mpsc::channel(CAPACITY);
    let session = Session {
        handle: SessionHandle {
            commands: command_tx,
        },
        events: event_rx,
    };
    (session, Channel { commands, events })
}

impl Session {
    pub fn handle(&self) -> &SessionHandle {
        &self.handle
    }

    pub async fn next_event(&mut self) -> Option<AgentEvent> {
        self.events.recv().await
    }

    pub fn split(self) -> (SessionHandle, mpsc::Receiver<AgentEvent>) {
        (self.handle, self.events)
    }
}

impl SessionHandle {
    pub async fn prompt(&self, text: impl Into<String>) -> Result<(), AgentError> {
        self.send(Command::Prompt(text.into())).await
    }

    pub async fn interrupt(&self) -> Result<(), AgentError> {
        self.send(Command::Interrupt).await
    }

    pub async fn approve(
        &self,
        id: ApprovalId,
        decision: ApprovalDecision,
    ) -> Result<(), AgentError> {
        self.send(Command::Approve { id, decision }).await
    }

    pub async fn shutdown(&self) -> Result<(), AgentError> {
        self.send(Command::Shutdown).await
    }

    async fn send(&self, command: Command) -> Result<(), AgentError> {
        self.commands
            .send(command)
            .await
            .map_err(|_| AgentError::Stopped)
    }
}
