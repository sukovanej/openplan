use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::Backend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    pub interval: Duration,
    pub after_commit: Duration,
    pub max_backoff: Duration,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            after_commit: Duration::from_secs(2),
            max_backoff: Duration::from_secs(600),
        }
    }
}

impl Schedule {
    fn after_failures(&self, failures: u32) -> Duration {
        let factor = 2u32.saturating_pow(failures.min(16));
        self.interval.saturating_mul(factor).min(self.max_backoff)
    }
}

// Long enough to never fire, short enough that `Instant` arithmetic cannot overflow. A request
// that arrives while a sync runs brings it forward, and the finished sync never pushes it back.
const PARKED: Duration = Duration::from_secs(365 * 24 * 60 * 60);

pub struct SyncLoop {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

struct Shared {
    schedule: Schedule,
    state: Mutex<State>,
    signal: Condvar,
}

struct State {
    due: Instant,
    stop: bool,
}

impl SyncLoop {
    // `None` for a backend with no remote, which has nothing to sync.
    pub fn start(backend: Arc<dyn Backend>, schedule: Schedule) -> Option<Self> {
        backend.remote()?;
        let shared = Arc::new(Shared {
            schedule,
            state: Mutex::new(State {
                due: Instant::now(),
                stop: false,
            }),
            signal: Condvar::new(),
        });
        let worker = {
            let shared = Arc::clone(&shared);
            std::thread::spawn(move || run(&*backend, &shared))
        };
        Some(Self {
            shared,
            worker: Some(worker),
        })
    }

    pub fn sync_soon(&self) {
        self.bring_forward(self.shared.schedule.after_commit);
    }

    pub fn sync_now(&self) {
        self.bring_forward(Duration::ZERO);
    }

    fn bring_forward(&self, delay: Duration) {
        let mut state = self.shared.lock();
        state.due = state.due.min(Instant::now() + delay);
        self.shared.signal.notify_all();
    }
}

impl Drop for SyncLoop {
    fn drop(&mut self) {
        self.shared.lock().stop = true;
        self.shared.signal.notify_all();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("sync loop mutex poisoned")
    }

    fn wait_until_due(&self) -> bool {
        let mut state = self.lock();
        loop {
            if state.stop {
                return false;
            }
            let now = Instant::now();
            if state.due <= now {
                state.due = now + PARKED;
                return true;
            }
            let wait = state.due - now;
            state = self
                .signal
                .wait_timeout(state, wait)
                .expect("sync loop mutex poisoned")
                .0;
        }
    }
}

fn run(backend: &dyn Backend, shared: &Shared) {
    let Some(remote) = backend.remote() else {
        return;
    };
    let mut failures = 0;
    while shared.wait_until_due() {
        let next = match remote.sync() {
            Ok(_) => {
                failures = 0;
                shared.schedule.interval
            }
            Err(err) => {
                failures += 1;
                let next = shared.schedule.after_failures(failures);
                tracing::warn!(error = %err, retry_in = ?next, "task sync failed");
                next
            }
        };
        let mut state = shared.lock();
        state.due = state.due.min(Instant::now() + next);
    }
}
