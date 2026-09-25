mod backend;
mod error;
mod events;
mod merge;
mod path;
mod revision;
mod snapshot;
mod sync_loop;

pub use backend::{
    Backend, BackendEvent, BackendExt, Committed, Edit, HeadMoved, LogEntry, LogQuery, Origin,
    Remote, SyncReport, SyncStatus, Write,
};
pub use error::BackendError;
pub use events::Events;
pub use merge::{MergeInput, MergePolicy, PreferTheirs, Resolution, Tips, merge};
pub use path::check_path;
pub use revision::{Actor, Change, ChangeKind, Op, Revision, RevisionId};
pub use snapshot::{MemorySnapshot, Overlay, Snapshot, diff};
pub use sync_loop::{Schedule, SyncLoop};

pub use jiff::Timestamp;

// Git keeps whole seconds, so every backend does: one revision reads the same in all of them.
pub fn now() -> Timestamp {
    Timestamp::from_second(Timestamp::now().as_second())
        .expect("a whole second of the current time is in range")
}
