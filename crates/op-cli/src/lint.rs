use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Result, bail};
use op_backend::Actor;
use op_lint::{CreatedSource, Diagnostic, Snapshot};
use op_server::{Location, STORE_DIR};
use op_task::Timestamp;
use op_tracker::{HistoryQuery, Tracker};

// Skills live in the checkout that holds the tasks, so an install run from a subdirectory lands
// there too. A directory with no tasks yet takes the root it was given.
pub fn skills_root(root: &Path) -> PathBuf {
    Location::find(root, None).map_or_else(|_| root.to_path_buf(), |location| location.root)
}

// Lint reads the tasks where they live, without the daemon, as a pre-commit hook and a fresh clone
// need it to. A fix goes back through the tracker as one revision, which the daemon then reads like
// any write from another process.
pub fn run(root: &Path, targets: &[String], json: bool, fix: bool) -> Result<ExitCode> {
    let location = Location::find(root, None)?;
    let machine = op_server::machine_actor(&location.root);
    let tracker = Tracker::new(op_server::open_backend(&location, &machine, false)?);
    let dir = documents_dir(&location);
    let snapshot = Snapshot::from_plan(&tracker.plan()?, &dir, &location.root)?;
    let selected = target_paths(&snapshot, &location, &dir, targets)?;

    let snapshot = if fix {
        let actor = actor(&location.root, machine);
        let wanted = |path: &Path| {
            selected
                .as_ref()
                .is_none_or(|set| set.contains(&lint_path(path)))
        };
        op_lint::fix_plan(
            &tracker,
            &actor,
            &snapshot,
            &dir,
            &FirstRevision { tracker: &tracker },
            &wanted,
        )?;
        Snapshot::from_plan(&tracker.plan()?, &dir, &location.root)?
    } else {
        snapshot
    };

    let diagnostics = op_lint::lint(&snapshot);
    let shown: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|d| {
            selected
                .as_ref()
                .is_none_or(|set| set.contains(&lint_path(&d.path)))
        })
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&shown)?);
    } else {
        for diagnostic in &shown {
            println!("{diagnostic}");
        }
        let checked = selected.as_ref().map_or_else(
            || snapshot.files().len() + snapshot.tags().len() + present_skills(&snapshot),
            |set| set.len(),
        );
        println!(
            "checked {checked} file{}, found {} problem{}",
            plural(checked),
            shown.len(),
            plural(shown.len())
        );
    }
    Ok(if shown.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

// Where the documents read as living. A git project's documents are on their own branch, but their
// links into the code were written from `.plan/tasks/`, so they resolve from there in both kinds.
fn documents_dir(location: &Location) -> PathBuf {
    location.root.join(STORE_DIR)
}

fn actor(root: &Path, machine: Actor) -> Actor {
    let identity = crate::author::identity(root);
    let actor = match identity.name {
        Some(name) => Actor {
            name,
            email: identity.email,
            via: None,
        },
        None => machine,
    };
    Actor {
        via: identity.agent,
        ..actor
    }
}

fn present_skills(snapshot: &Snapshot) -> usize {
    snapshot
        .skills()
        .iter()
        .filter(|skill| skill.source.is_some())
        .count()
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

// The whole project is always scanned; targets only pick which files this run reports on and
// repairs, so an agent lints the one task it wrote. `None` means no filter.
fn target_paths(
    snapshot: &Snapshot,
    location: &Location,
    dir: &Path,
    targets: &[String],
) -> Result<Option<HashSet<PathBuf>>> {
    if targets.is_empty() {
        return Ok(None);
    }
    let mut set = HashSet::new();
    for target in targets {
        // A target that resolves to nothing would filter every diagnostic away and pass, so it stops
        // the run instead.
        let Some(path) = target_path(snapshot, location, dir, target) else {
            bail!("no task, tag, or skill file matches {target}");
        };
        set.insert(path);
    }
    Ok(Some(set))
}

fn target_path(
    snapshot: &Snapshot,
    location: &Location,
    dir: &Path,
    target: &str,
) -> Option<PathBuf> {
    if let Some(number) = snapshot.abbreviation().parse_key(target) {
        return snapshot.file(number).map(|file| lint_path(&file.path));
    }
    let spellings = [
        lint_path(Path::new(target)),
        lint_path(&location.root.join(target)),
        lint_path(&dir.join(target)),
    ];
    snapshot
        .files()
        .iter()
        .map(|file| &file.path)
        .chain(snapshot.tags().iter().map(|tag| &tag.path))
        .chain(snapshot.skills().iter().map(|skill| &skill.path))
        .map(|path| lint_path(path))
        .find(|path| spellings.contains(path))
}

// A symlinked checkout spells one file two ways; both meet at the deepest directory that exists.
fn lint_path(path: &Path) -> PathBuf {
    for ancestor in path.ancestors() {
        let Ok(resolved) = ancestor.canonicalize() else {
            continue;
        };
        let rest = path.strip_prefix(ancestor).unwrap_or(Path::new(""));
        return if rest.as_os_str().is_empty() {
            resolved
        } else {
            resolved.join(rest)
        };
    }
    path.to_path_buf()
}

// When a task first appeared: the oldest revision in its history.
struct FirstRevision<'a> {
    tracker: &'a Tracker,
}

impl CreatedSource for FirstRevision<'_> {
    fn created(&self, path: &Path) -> Option<Timestamp> {
        let stem = path.file_stem()?.to_str()?;
        let number = op_task::file_id(stem)?;
        let history = self
            .tracker
            .task_history(number, &HistoryQuery::default())
            .ok()?;
        Some(history.last()?.revision.at)
    }
}
