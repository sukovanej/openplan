mod diagnostic;
mod fix;
mod rules;
mod snapshot;

use std::path::PathBuf;

use op_store::{Store, StoreError};

pub use diagnostic::{Code, Diagnostic, Position, Severity, Span};
pub use fix::{CreatedSource, Fix, Uncommitted, apply, file_fixes, fix};
pub use rules::{STORE_RULES, Sink, StoreRule, TASK_RULES, TaskRule};
pub use snapshot::{Snapshot, TaskFile, github_slug};

pub fn lint(snapshot: &Snapshot) -> Vec<Diagnostic> {
    let mut sink = Sink::new();
    for file in snapshot.files() {
        for rule in TASK_RULES {
            rule(snapshot, file, &mut sink);
        }
    }
    for rule in STORE_RULES {
        rule(snapshot, &mut sink);
    }
    sink.into_diagnostics()
}

pub fn fix_store(store: &Store, created: &dyn CreatedSource) -> Result<Vec<PathBuf>, StoreError> {
    let snapshot = Snapshot::from_store(store)?;
    let fixed = fix::fix(&snapshot, created);
    let mut changed = Vec::new();
    for file in snapshot.files() {
        let Some(after) = fixed.get(&file.path) else {
            continue;
        };
        if after != &file.source {
            replace_file(store, file, after)?;
            changed.push(file.path.clone());
        }
    }
    Ok(changed)
}

// Two files can claim one number, and `Store::replace_raw` resolves a number back to the lowest of
// them — which would write one file's fix over the other and destroy its content. A fix belongs to
// the file it was computed from, so it goes to that path.
fn replace_file(store: &Store, file: &TaskFile, contents: &str) -> Result<(), StoreError> {
    store.with_lock(file.number, || {
        let temp = store.tasks_dir().join(format!(
            ".op-lint-{}-{}.tmp",
            std::process::id(),
            file.number
        ));
        std::fs::write(&temp, contents)?;
        std::fs::rename(&temp, &file.path)?;
        Ok(())
    })
}
