use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Notify;

// The daemon updates itself on its own schedule. A project whose tasks need a newer openplan asks
// it to look now, and its fault says how that went.
#[derive(Debug, Default)]
pub struct SelfUpdate {
    wanted: Notify,
    automatic: AtomicBool,
    outcome: Mutex<Option<String>>,
}

impl SelfUpdate {
    pub fn want(&self) {
        *self.lock_outcome() = None;
        self.wanted.notify_one();
    }

    pub async fn wanted(&self) {
        self.wanted.notified().await;
    }

    // Whether the daemon installs a release by itself; the daemon sets it before each check.
    pub fn set_automatic(&self, automatic: bool) {
        self.automatic.store(automatic, Ordering::Relaxed);
    }

    pub fn automatic(&self) -> bool {
        self.automatic.load(Ordering::Relaxed)
    }

    // What a person can do after a check that a project asked for installed nothing. Only the
    // daemon knows its update channel, so it says it in full.
    pub fn record(&self, outcome: String) {
        *self.lock_outcome() = Some(outcome);
    }

    pub fn outcome(&self) -> Option<String> {
        self.lock_outcome().clone()
    }

    fn lock_outcome(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.outcome.lock().expect("update outcome poisoned")
    }
}
