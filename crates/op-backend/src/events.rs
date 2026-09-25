use tokio::sync::broadcast;

use crate::BackendEvent;

const CAPACITY: usize = 256;

#[derive(Debug, Clone)]
pub struct Events(broadcast::Sender<BackendEvent>);

impl Events {
    pub fn subscribe(&self) -> broadcast::Receiver<BackendEvent> {
        self.0.subscribe()
    }

    pub fn send(&self, event: BackendEvent) {
        // A send fails only while nobody listens, and an event nobody listens for is not an error.
        let _ = self.0.send(event);
    }
}

impl Default for Events {
    fn default() -> Self {
        Self(broadcast::channel(CAPACITY).0)
    }
}
