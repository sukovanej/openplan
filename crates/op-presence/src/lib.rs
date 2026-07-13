use std::collections::HashMap;

use op_api::Presence;

#[derive(Debug, Default)]
pub struct Registry {
    claims: HashMap<String, Presence>,
}

#[derive(Debug, thiserror::Error)]
pub enum PresenceError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn claim(&mut self, presence: Presence) {
        self.claims.insert(presence.task_id.clone(), presence);
    }

    pub fn release(&mut self, task_id: &str) -> Option<Presence> {
        self.claims.remove(task_id)
    }

    pub fn get(&self, task_id: &str) -> Option<&Presence> {
        self.claims.get(task_id)
    }
}
