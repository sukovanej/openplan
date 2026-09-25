mod diagnostic;
mod fix;
mod rules;
mod snapshot;

use std::path::{Path, PathBuf};

use op_backend::Actor;
use op_tracker::{Tracker, TrackerError};

pub use diagnostic::{Code, Diagnostic, Position, Severity, Span};
pub use fix::{CreatedSource, Fix, Uncommitted, apply, file_fixes, fix, tag_fixes};
pub use op_skills::SkillFile;
pub use rules::{
    SKILL_RULES, STORE_RULES, Sink, SkillRule, StoreRule, TAG_RULES, TASK_RULES, TagRule, TaskRule,
};
pub use snapshot::{Snapshot, TagFile, TaskFile, github_slug};

pub fn lint(snapshot: &Snapshot) -> Vec<Diagnostic> {
    let mut sink = Sink::new();
    for file in snapshot.files() {
        for rule in TASK_RULES {
            rule(snapshot, file, &mut sink);
        }
    }
    for tag in snapshot.tags() {
        for rule in TAG_RULES {
            rule(snapshot, tag, &mut sink);
        }
    }
    for skill in snapshot.skills() {
        for rule in SKILL_RULES {
            rule(snapshot, skill, &mut sink);
        }
    }
    for rule in STORE_RULES {
        rule(snapshot, &mut sink);
    }
    sink.into_diagnostics()
}

#[derive(Debug, thiserror::Error)]
pub enum FixError {
    #[error(transparent)]
    Tracker(#[from] TrackerError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

// Every repaired document lands in one revision. `dir` is where the snapshot placed the documents,
// and `wanted` picks the files a caller asked to repair.
pub fn fix_plan(
    tracker: &Tracker,
    actor: &Actor,
    snapshot: &Snapshot,
    dir: &Path,
    created: &dyn CreatedSource,
    wanted: &dyn Fn(&Path) -> bool,
) -> Result<Vec<PathBuf>, FixError> {
    let fixed = fix::fix(snapshot, created);
    let mut changed = Vec::new();
    let mut documents = Vec::new();
    let sources = snapshot
        .files()
        .iter()
        .map(|file| (&file.path, &file.source))
        .chain(snapshot.tags().iter().map(|tag| (&tag.path, &tag.source)));
    for (path, source) in sources {
        let Some(after) = fixed.get(path) else {
            continue;
        };
        if after == source || !wanted(path) {
            continue;
        }
        let Ok(document) = path.strip_prefix(dir) else {
            continue;
        };
        documents.push((document.to_string_lossy().into_owned(), after.clone()));
        changed.push(path.clone());
    }
    if !documents.is_empty() {
        tracker.replace_documents(actor, "lint fixes", &documents)?;
    }
    for skill in snapshot.skills() {
        if !skill.matches() && wanted(&skill.path) {
            op_skills::install(skill)?;
            changed.push(skill.path.clone());
        }
    }
    Ok(changed)
}
