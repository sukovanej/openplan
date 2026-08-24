use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::{Duration, Instant};

use op_api::{Abbreviation, ChangeEvent, ProjectStatus, ProjectView};
use op_git::Repo;
use op_index::{Index, IndexError};
use op_presence::Registry as PresenceRegistry;
use op_store::{Config, STORE_DIR, Store, StoreError};
use op_watch::{Change, Watcher};
use tokio::sync::broadcast;

use crate::ProjectEntry;

// How long a rebuild is trusted while nothing has invalidated it. The watcher is what clears the
// gate, and [[OPP-52]] says it can miss a working-tree edit; without a ceiling a missed event would
// turn "stale until the next refresh" into "stale until the next watched change".
const TRUSTED: Duration = Duration::from_secs(3);

// Two misses in sequence, so a root that momentarily reads as absent does not demote the project.
const ROOT_MISSES: u32 = 2;
pub const ROOT_POLL: Duration = Duration::from_secs(5);

// The one place task ids are handed out. Handlers release the index mutex before their flock write,
// so two concurrent creates would each compute the same `max + 1` from the matrix; this counter is
// bumped atomically instead, and re-seeded from the floor on every call so a merge bringing in
// higher ids — or a restart — advances it. Nothing is persisted: the ids on disk are the floor.
#[derive(Debug, Default)]
pub struct IdCounter(Mutex<u64>);

impl IdCounter {
    // `u64::MAX` is never issued, only used as the exhausted marker: any id already carrying it
    // (hand-written, or merged in) would otherwise pin the counter there and hand the same number
    // to every later create. Losing one number of 2^64 keeps "issued at most once" unconditional.
    pub fn issue_above(&self, floor: u64) -> Option<u64> {
        let mut next = self.0.lock().expect("id counter mutex poisoned");
        let issued = (*next).max(floor.saturating_add(1));
        if issued == u64::MAX {
            *next = u64::MAX;
            return None;
        }
        *next = issued + 1;
        Some(issued)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("no {STORE_DIR} task store found at {0}")]
    NoStore(PathBuf),
    #[error("no git repository at {0}")]
    NoRepo(PathBuf),
    #[error(transparent)]
    Store(#[from] StoreError),
}

// A path is registered by the checkout that can serve it, not by the directory the caller happened
// to stand in: the daemon outlives whichever worktree issued the first write, and a root pointing
// into a removed one can no longer resolve a branch. Fall back to the caller only when the main
// checkout cannot serve — a bare repository, or a branch that does not carry the task store.
pub fn serve_root(repo: &Repo, root: &Path) -> PathBuf {
    repo.main_worktree()
        .filter(|main| Store::discover(main).is_ok())
        .unwrap_or_else(|| root.to_path_buf())
}

// A registered path whose store or repository cannot be opened is skipped, not fatal: one broken
// checkout must not take the task UI away from every other project on the machine. A name is the
// coordinate every route resolves through, so a hand-written duplicate is skipped for the same
// reason — the second entry would otherwise silently shadow the first.
pub fn open_projects(entries: &[ProjectEntry]) -> Vec<Project> {
    let mut names = BTreeSet::new();
    let mut repos = BTreeSet::new();
    entries
        .iter()
        // The daemon only ever writes a name `slug` produced, so this is about a name written by
        // hand. `team/alpha` and the empty name are not path segments any request can carry, so the
        // project would be served but unreachable — a silent one, where this is a loud one.
        .filter(|entry| {
            crate::registry::is_usable_name(&entry.name) || {
                tracing::error!(
                    project = %entry.name,
                    path = %entry.path.display(),
                    "skipping project: the name cannot address a project; use lowercase letters, \
                     digits, and dashes"
                );
                false
            }
        })
        .filter(|entry| {
            names.insert(entry.name.clone()) || {
                tracing::error!(project = %entry.name, "skipping project: the name is already taken");
                false
            }
        })
        .filter_map(
            |entry| match Project::open(entry.name.clone(), entry.path.clone()) {
                Ok(project) => Some(project),
                Err(err) => {
                    tracing::error!(
                        project = %entry.name,
                        path = %entry.path.display(),
                        error = format!("{err:#}"),
                        "skipping project"
                    );
                    None
                }
            },
        )
        // A task number is issued at most once per *repository*, and each project issues it from its
        // own counter above its own floor. Two worktrees of one repository registered as two
        // projects would each hand out the same number, into two different `.plan` directories, and
        // the store's id-taken retry cannot see the clash: each write lands in a file the other
        // never looks at. `AppState::register` refuses the second one, so a hand-written registry
        // has to be refused here for the same reason.
        .filter(|project| {
            repos.insert(project.git_common_dir().to_owned()) || {
                tracing::error!(
                    project = %project.name(),
                    path = %project.path.display(),
                    "skipping project: another project already serves this repository"
                );
                false
            }
        })
        .collect()
}

// What a rebuild is measured against. `generation` is what keeps an invalidation that lands *during*
// a rebuild: the rebuild clears the flag only when the generation it started at still stands, so a
// change the walk was too early to see leaves the project dirty rather than falsely clean.
#[derive(Debug)]
struct Freshness {
    dirty: bool,
    generation: u64,
    rebuilt_at: Option<Instant>,
}

impl Default for Freshness {
    fn default() -> Self {
        Self {
            dirty: true,
            generation: 0,
            rebuilt_at: None,
        }
    }
}

// The independent ways a project stops being servable. They are tracked apart so the watchdog
// promoting a returned root cannot also clear a broken `config.toml`, and the other way round.
#[derive(Debug, Default)]
struct Health {
    root_gone: bool,
    config_error: Option<String>,
    // The last rebuild's failure, if it failed. Without it a repository that cannot be read would
    // leave the merged board quietly — that board skips what cannot answer — while `/api/projects`
    // still reported the project healthy with no tasks.
    index_error: Option<String>,
}

// Everything scoped to one repository. Each project keeps its own index mutex, so a rebuild in one —
// object-DB reads and flock waits — never blocks a read in another. `IdCounter` is per project too,
// which is what keeps "a number is issued at most once per repository" true across N of them.
pub struct Project {
    name: RwLock<String>,
    pub path: PathBuf,
    // Held beside the index as well as in it. `/api/projects` renders one of these per project, and
    // it is on the hot path of every write now that the CLI resolves its project there; reading it
    // through the index would make each listing wait on every project's rebuild in turn.
    abbreviation: RwLock<Abbreviation>,
    pub index: Arc<Mutex<Index>>,
    pub presence: Arc<Mutex<PresenceRegistry>>,
    git_common_dir: String,
    store: Store,
    repo: Repo,
    ids: Arc<IdCounter>,
    freshness: Mutex<Freshness>,
    health: Mutex<Health>,
    watcher: Mutex<Option<Watcher>>,
    root_misses: AtomicU32,
}

impl Project {
    pub fn new(
        name: impl Into<String>,
        path: PathBuf,
        repo: Repo,
        store: Store,
        config: &Config,
    ) -> Self {
        Self {
            name: RwLock::new(name.into()),
            path,
            abbreviation: RwLock::new(config.abbreviation),
            index: Arc::new(Mutex::new(Index::new(config))),
            presence: Arc::new(Mutex::new(PresenceRegistry::new())),
            git_common_dir: git_common_dir(&repo),
            store,
            repo,
            ids: Arc::new(IdCounter::default()),
            freshness: Mutex::new(Freshness::default()),
            health: Mutex::new(Health::default()),
            watcher: Mutex::new(None),
            root_misses: AtomicU32::new(0),
        }
    }

    pub fn open(name: impl Into<String>, path: PathBuf) -> Result<Self, OpenError> {
        let repo = Repo::discover(&path).map_err(|_| OpenError::NoRepo(path.clone()))?;
        let store = Store::discover(&path).map_err(|err| match err {
            StoreError::StoreMissing => OpenError::NoStore(path.clone()),
            other => OpenError::Store(other),
        })?;
        let config = Config::read(store.root()).map_err(StoreError::Config)?;
        Ok(Self::new(name, path, repo, store, &config))
    }

    pub fn name(&self) -> String {
        self.name.read().expect("name lock poisoned").clone()
    }

    pub fn rename(&self, name: &str) {
        *self.name.write().expect("name lock poisoned") = name.to_owned();
    }

    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn ids(&self) -> &IdCounter {
        &self.ids
    }

    pub fn git_common_dir(&self) -> &str {
        &self.git_common_dir
    }

    pub fn view(&self) -> ProjectView {
        ProjectView {
            name: self.name(),
            root: self.path.display().to_string(),
            git_common_dir: self.git_common_dir.clone(),
            abbreviation: self.abbreviation().to_string(),
            status: self.status(),
        }
    }

    pub fn abbreviation(&self) -> Abbreviation {
        *self
            .abbreviation
            .read()
            .expect("abbreviation lock poisoned")
    }

    // Every id the matrix holds was formatted with the abbreviation it was built under, and
    // `Index::set_config` only drops the parse cache — the matrix keeps the old spellings. A
    // read that skipped the rebuild would hand one of them to `Index::number_of`, which panics on a
    // key this abbreviation cannot parse and poisons the index for the project's whole life. So the
    // index guard is held until the project is marked stale: releasing it between the two leaves a
    // window where the index carries the new abbreviation, the matrix the old spellings, and nothing
    // yet says a rebuild is due.
    pub fn set_config(&self, config: &Config) {
        let mut index = self.lock_index();
        *self
            .abbreviation
            .write()
            .expect("abbreviation lock poisoned") = config.abbreviation;
        index.set_config(config);
        self.mark_dirty();
    }

    // A read's index. The rebuild walks every local branch, and the merged board multiplies that by N
    // projects per request, so it runs only when something says the matrix can have moved.
    pub fn read_index(&self) -> Result<MutexGuard<'_, Index>, IndexError> {
        let mut index = self.lock_index();
        let Some(generation) = self.stale_at() else {
            return Ok(index);
        };
        self.record_rebuild(index.rebuild(&self.repo, &self.store))?;
        self.mark_fresh(generation);
        Ok(index)
    }

    // A failed rebuild leaves the project dirty, so the next read tries again and a repository that
    // becomes readable clears this by itself.
    fn record_rebuild(&self, result: Result<(), IndexError>) -> Result<(), IndexError> {
        let reason = result.as_ref().err().map(|err| err.to_string());
        if let Some(reason) = &reason {
            tracing::error!(project = %self.name(), %reason, "the project index could not be rebuilt");
        }
        self.lock_health().index_error = reason;
        result
    }

    // An index that has just walked the repository, in all conditions. A write needs it because the
    // gate answers "has anything changed since the last walk" where a write asks "is this branch
    // writable right now". A one-shot caller needs it because the gate is cleared by the watcher,
    // and a caller with no change stream to re-read on cannot wait out a watch that is late or
    // ([[OPP-52]]) missed the edit outright.
    pub fn rebuilt_index(&self) -> Result<MutexGuard<'_, Index>, IndexError> {
        let generation = self.generation();
        let mut index = self.lock_index();
        self.record_rebuild(index.rebuild(&self.repo, &self.store))?;
        self.mark_fresh(generation);
        Ok(index)
    }

    pub fn mark_dirty(&self) {
        let mut freshness = self.lock_freshness();
        freshness.dirty = true;
        freshness.generation = freshness.generation.wrapping_add(1);
    }

    // What stops this project from answering at all. A rebuild that failed is deliberately not among
    // them: trying again is the only thing that can find the repository readable, so every route
    // keeps trying and reports the failure it gets, rather than refusing forever from a latched flag.
    pub fn blocked(&self) -> Option<String> {
        let health = self.lock_health();
        if health.root_gone {
            return Some(format!(
                "the project root {} no longer exists, so no branch resolves",
                self.path.display()
            ));
        }
        health.config_error.clone()
    }

    pub fn status(&self) -> ProjectStatus {
        let reason = self
            .blocked()
            .or_else(|| self.lock_health().index_error.clone());
        match reason {
            Some(reason) => ProjectStatus::Error { reason },
            None => ProjectStatus::Ok,
        }
    }

    // A valid change applies live and every key re-renders. One that leaves the file missing or
    // invalid demotes this project alone: the daemon keeps serving every other one, and the next
    // valid config promotes this one again.
    pub fn reload_config(&self) {
        match Config::read(self.store.root()) {
            Ok(config) => {
                // Applied before the project is unblocked. The other order leaves a window where a
                // read passes the gate, finds the index fresh, and serves the old abbreviation's
                // keys.
                self.set_config(&config);
                self.lock_health().config_error = None;
                self.report_config(&config);
            }
            Err(err) => {
                let reason = err.to_string();
                tracing::error!(project = %self.name(), %reason, "the store config can no longer be read");
                self.lock_health().config_error = Some(reason);
            }
        }
    }

    // What the rebuild will make of the config, rather than what the file asks for. A
    // `default_branch` no local branch carries is not an error — the task view falls back and keeps
    // serving — so this log line is the only place it is ever mentioned.
    fn report_config(&self, config: &Config) {
        let resolved = self
            .repo
            .default_branch(config.default_branch.as_deref())
            .ok()
            .flatten();
        tracing::info!(
            project = %self.name(),
            abbreviation = %config.abbreviation,
            default_branch = %resolved.as_deref().unwrap_or("(none)"),
            "store config reloaded"
        );
        if let Some(configured) = config.default_branch.as_deref() {
            if resolved.as_deref() != Some(configured) {
                tracing::warn!(
                    project = %self.name(),
                    %configured,
                    "default_branch names no local branch; the task view falls back"
                );
            }
        }
    }

    // One watchdog tick. Answers whether the project's status moved, which is what the caller turns
    // into a `ProjectsChanged`.
    pub fn poll_root(&self) -> bool {
        let misses = match self.path.is_dir() {
            true => {
                self.root_misses.store(0, Ordering::Relaxed);
                0
            }
            false => self.root_misses.fetch_add(1, Ordering::Relaxed) + 1,
        };
        let gone = misses >= ROOT_MISSES;
        let mut health = self.lock_health();
        let moved = health.root_gone != gone;
        health.root_gone = gone;
        if moved {
            match gone {
                true => {
                    tracing::warn!(project = %self.name(), root = %self.path.display(), "project root is gone")
                }
                false => {
                    tracing::info!(project = %self.name(), root = %self.path.display(), "project root is back")
                }
            }
        }
        moved
    }

    pub fn is_watched(&self) -> bool {
        self.watcher
            .lock()
            .expect("watcher mutex poisoned")
            .is_some()
    }

    pub fn stop_watch(&self) {
        let watcher = self.watcher.lock().expect("watcher mutex poisoned").take();
        drop(watcher);
    }

    fn lock_index(&self) -> MutexGuard<'_, Index> {
        self.index.lock().expect("index mutex poisoned")
    }

    fn lock_freshness(&self) -> MutexGuard<'_, Freshness> {
        self.freshness.lock().expect("freshness mutex poisoned")
    }

    fn lock_health(&self) -> MutexGuard<'_, Health> {
        self.health.lock().expect("health mutex poisoned")
    }

    fn generation(&self) -> u64 {
        self.lock_freshness().generation
    }

    // The generation to clear on success, or `None` when the last rebuild still stands. A project
    // with no live watcher has nothing to invalidate it, so it is never counted as fresh.
    fn stale_at(&self) -> Option<u64> {
        let watched = self.is_watched();
        let freshness = self.lock_freshness();
        let fresh = watched
            && !freshness.dirty
            && freshness
                .rebuilt_at
                .is_some_and(|at| at.elapsed() < TRUSTED);
        (!fresh).then_some(freshness.generation)
    }

    fn mark_fresh(&self, generation: u64) {
        let mut freshness = self.lock_freshness();
        if freshness.generation != generation {
            return;
        }
        freshness.dirty = false;
        freshness.rebuilt_at = Some(Instant::now());
    }
}

impl std::fmt::Debug for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("name", &self.name())
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

// Blocking: `Watcher::start` scans every branch and hashes each worktree's task files. A watcher
// that will not start stays a logged degradation — reads keep working, they just never skip a
// rebuild, because `stale_at` counts an unwatched project as always dirty.
pub fn start_watch(project: &Arc<Project>, events: broadcast::Sender<ChangeEvent>) {
    if project.is_watched() {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let watcher = match Watcher::start(project.repo.clone(), project.store.clone(), tx) {
        Ok(watcher) => watcher,
        Err(err) => {
            tracing::warn!(project = %project.name(), "watch disabled: {err}");
            return;
        }
    };
    *project.watcher.lock().expect("watcher mutex poisoned") = Some(watcher);

    // The bridge holds a strong reference, so the project outlives its map entry until the watcher
    // is stopped; `stop_watch` is what ends the channel and with it this thread.
    let watched = Arc::clone(project);
    std::thread::spawn(move || {
        for change in rx {
            tracing::debug!(project = %watched.name(), ?change, "watcher change forwarded");
            watched.mark_dirty();
            // The watcher reports a task by number, the file layer's spelling; the key it renders as
            // is the daemon's to decide.
            let event = match change {
                Change::Task { number, branch } => ChangeEvent::TaskChanged {
                    project: watched.name(),
                    id: watched.abbreviation().format_key(number),
                    branch,
                },
                Change::Tags { branch } => ChangeEvent::TagsChanged {
                    project: watched.name(),
                    branch,
                },
                Change::Config => {
                    watched.reload_config();
                    ChangeEvent::ProjectsChanged
                }
            };
            let _ = events.send(event);
        }
    });
}

pub(crate) fn git_common_dir(repo: &Repo) -> String {
    repo.git_common_dir()
        .canonicalize()
        .unwrap_or_else(|_| repo.git_common_dir())
        .display()
        .to_string()
}
