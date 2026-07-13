use std::path::Path;

use op_api::ChangeEvent;

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("watch error: {0}")]
    Notify(#[from] notify::Error),
}

pub struct Watcher {
    inner: notify::RecommendedWatcher,
}

impl Watcher {
    pub fn start(
        root: impl AsRef<Path>,
        sink: std::sync::mpsc::Sender<ChangeEvent>,
    ) -> Result<Self, WatchError> {
        use notify::Watcher as _;

        let mut inner = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            // TODO(skeleton): map fs events to real ChangeEvents; debounce; settle git ops.
            if let Ok(event) = res
                && event.kind.is_modify()
            {
                let _ = sink.send(ChangeEvent::RefMoved {
                    branch: String::new(),
                });
            }
        })?;
        inner.watch(root.as_ref(), notify::RecursiveMode::Recursive)?;
        Ok(Self { inner })
    }

    pub fn stop(self) {
        drop(self.inner);
    }
}
