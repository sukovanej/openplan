use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use op_backend::{
    Actor, Backend, BackendError, BackendEvent, Committed, Events, HeadMoved, LogEntry, LogQuery,
    MemorySnapshot, Remote, RevisionId, Schedule, Snapshot, SyncLoop, SyncReport, SyncStatus,
    Write,
};
use tokio::sync::broadcast;

#[derive(Default)]
struct Counting {
    syncs: AtomicUsize,
    fail: Mutex<bool>,
    events: Events,
}

impl Backend for Counting {
    fn head(&self) -> Result<Arc<dyn Snapshot>, BackendError> {
        Ok(Arc::new(MemorySnapshot::default()))
    }

    fn at(&self, revision: &RevisionId) -> Result<Arc<dyn Snapshot>, BackendError> {
        Err(BackendError::UnknownRevision(revision.clone()))
    }

    fn commit(&self, _: &Actor, _: Write<'_>) -> Result<Option<Committed>, BackendError> {
        Ok(None)
    }

    fn log(&self, _: &LogQuery) -> Result<Vec<LogEntry>, BackendError> {
        Ok(Vec::new())
    }

    fn refresh(&self) -> Result<Option<HeadMoved>, BackendError> {
        Ok(None)
    }

    fn remote(&self) -> Option<&dyn Remote> {
        Some(self)
    }

    fn subscribe(&self) -> broadcast::Receiver<BackendEvent> {
        self.events.subscribe()
    }
}

impl Remote for Counting {
    fn sync(&self) -> Result<SyncReport, BackendError> {
        self.syncs.fetch_add(1, Ordering::SeqCst);
        match *self.fail.lock().expect("lock") {
            true => Err(BackendError::sync("offline")),
            false => Ok(SyncReport::default()),
        }
    }

    fn status(&self) -> SyncStatus {
        SyncStatus::default()
    }
}

fn wait_for(what: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !what() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn slow() -> Schedule {
    Schedule {
        interval: Duration::from_secs(3600),
        after_commit: Duration::from_millis(20),
        max_backoff: Duration::from_secs(3600),
    }
}

#[test]
fn a_backend_without_a_remote_gets_no_loop() {
    struct Offline(Counting);
    impl Backend for Offline {
        fn head(&self) -> Result<Arc<dyn Snapshot>, BackendError> {
            self.0.head()
        }
        fn at(&self, revision: &RevisionId) -> Result<Arc<dyn Snapshot>, BackendError> {
            self.0.at(revision)
        }
        fn commit(&self, a: &Actor, w: Write<'_>) -> Result<Option<Committed>, BackendError> {
            self.0.commit(a, w)
        }
        fn log(&self, q: &LogQuery) -> Result<Vec<LogEntry>, BackendError> {
            self.0.log(q)
        }
        fn refresh(&self) -> Result<Option<HeadMoved>, BackendError> {
            self.0.refresh()
        }
        fn remote(&self) -> Option<&dyn Remote> {
            None
        }
        fn subscribe(&self) -> broadcast::Receiver<BackendEvent> {
            self.0.subscribe()
        }
    }
    assert!(SyncLoop::start(Arc::new(Offline(Counting::default())), slow()).is_none());
}

#[test]
fn the_loop_syncs_at_once_and_again_when_asked() {
    let backend = Arc::new(Counting::default());
    let sync_loop = SyncLoop::start(backend.clone(), slow()).expect("a loop");
    wait_for(|| backend.syncs.load(Ordering::SeqCst) == 1);
    sync_loop.sync_soon();
    wait_for(|| backend.syncs.load(Ordering::SeqCst) == 2);
    sync_loop.sync_now();
    wait_for(|| backend.syncs.load(Ordering::SeqCst) == 3);
}

#[test]
fn a_failure_waits_longer_before_the_next_try() {
    let backend = Arc::new(Counting::default());
    *backend.fail.lock().expect("lock") = true;
    let schedule = Schedule {
        interval: Duration::from_millis(40),
        after_commit: Duration::from_millis(1),
        max_backoff: Duration::from_millis(80),
    };
    let started = Instant::now();
    let _sync_loop = SyncLoop::start(backend.clone(), schedule).expect("a loop");
    wait_for(|| backend.syncs.load(Ordering::SeqCst) >= 3);
    assert!(started.elapsed() >= Duration::from_millis(80 + 80));
}

#[test]
fn dropping_the_loop_stops_it() {
    let backend = Arc::new(Counting::default());
    let sync_loop = SyncLoop::start(backend.clone(), slow()).expect("a loop");
    wait_for(|| backend.syncs.load(Ordering::SeqCst) == 1);
    drop(sync_loop);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(backend.syncs.load(Ordering::SeqCst), 1);
}
