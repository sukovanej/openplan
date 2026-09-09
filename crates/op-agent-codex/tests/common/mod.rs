#![allow(dead_code)]

use op_agent::{AgentEvent, Effects, Protocol, SessionOptions};
use op_agent_codex::{
    Driver,
    protocol::{Incoming, Outgoing},
};

pub struct Replay {
    pub driver: Driver,
    pub effects: Effects<Outgoing>,
}

impl Replay {
    pub fn events(&self) -> &[AgentEvent] {
        &self.effects.events
    }

    pub fn find<T>(&self, pick: impl Fn(&AgentEvent) -> Option<T>) -> Option<T> {
        self.effects.events.iter().find_map(pick)
    }

    pub fn all<T>(&self, pick: impl Fn(&AgentEvent) -> Option<T>) -> Vec<T> {
        self.effects.events.iter().filter_map(pick).collect()
    }

    pub fn methods(&self) -> Vec<&str> {
        self.effects
            .inputs
            .iter()
            .map(|input| match input {
                Outgoing::Request { method, .. } | Outgoing::Notification { method, .. } => method,
                Outgoing::Response { .. } => "response",
                Outgoing::Failure { .. } => "failure",
            })
            .collect()
    }
}

pub fn messages(fixture: &str) -> Vec<Incoming> {
    fixture
        .lines()
        .filter(|line| !line.trim().is_empty())
        .zip(1..)
        .map(|(line, source)| {
            serde_json::from_str(line).unwrap_or_else(|error| panic!("line {source}: {error}"))
        })
        .collect()
}

pub fn options() -> SessionOptions {
    SessionOptions::new("/tmp").model("gpt")
}

pub fn replay(fixture: &str, options: SessionOptions) -> Replay {
    let mut driver = Driver::new(options);
    let mut effects = Effects::default();
    driver.open(&mut effects);
    driver.prompt("the prompt".to_owned(), &mut effects);
    for message in messages(fixture) {
        driver.read(message, &mut effects);
    }
    Replay { driver, effects }
}
