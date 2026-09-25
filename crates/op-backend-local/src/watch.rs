use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

use crate::disk::is_hidden;

const SETTLE: Duration = Duration::from_millis(200);

pub(crate) struct Watcher {
    stop: Sender<Signal>,
    worker: Option<JoinHandle<()>>,
    _notifier: RecommendedWatcher,
}

enum Signal {
    Changed,
    Stop,
}

impl Watcher {
    pub fn start(root: &Path, on_settled: impl Fn() + Send + 'static) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let changed = tx.clone();
        let base = root.to_path_buf();
        let mut notifier =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                if let Ok(event) = result
                    && !matches!(event.kind, notify::EventKind::Access(_))
                    && event.paths.iter().any(|path| is_document(&base, path))
                {
                    let _ = changed.send(Signal::Changed);
                }
            })?;
        notifier.watch(root, RecursiveMode::Recursive)?;
        let worker = std::thread::spawn(move || {
            let mut pending = false;
            loop {
                let next = match pending {
                    true => rx.recv_timeout(SETTLE),
                    false => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                };
                match next {
                    Ok(Signal::Changed) => pending = true,
                    Ok(Signal::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                    Err(RecvTimeoutError::Timeout) => {
                        pending = false;
                        on_settled();
                    }
                }
            }
        });
        Ok(Self {
            stop: tx,
            worker: Some(worker),
            _notifier: notifier,
        })
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.stop.send(Signal::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn is_document(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        relative
            .components()
            .all(|component| !is_hidden(&component.as_os_str().to_string_lossy()))
    })
}
