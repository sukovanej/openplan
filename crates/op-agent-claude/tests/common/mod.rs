#![allow(dead_code)]

use op_agent::{AgentEvent, Effects, Protocol};
use op_agent_claude::Driver;
use op_claude::{StreamInput, StreamOutput};

pub struct Replay {
    pub driver: Driver,
    pub effects: Effects<StreamInput>,
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
}

pub fn replay(name: &str, fixture: &str) -> Replay {
    let mut driver = Driver::new();
    let mut effects = Effects::default();
    driver.prompt("the prompt".to_owned(), &mut effects);
    for (line, source) in op_claude::parse_jsonl::<StreamOutput>(name, fixture).zip(1..) {
        let output = line.unwrap_or_else(|error| panic!("line {source}: {error}"));
        driver.read(output, &mut effects);
    }
    Replay { driver, effects }
}
