use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use op_api::{Matrix, MatrixCell, Status, TaskBranches, TaskSummary, TaskVersion, TaskView};
use op_git::{Repo, Worktree};
use op_store::{Store, StoreError};
use op_task::Task;

#[derive(Debug, Default)]
pub struct Index {
    matrix: Matrix,
    // Keyed by blob OID, which is content-addressed, so an entry never goes stale — the cache
    // grows across rebuilds and is only ever added to. Holds the id-independent parse (the id is
    // the filename, absent from the blob) so two tasks sharing a blob don't leak each other's id.
    blob_cache: HashMap<String, Version>,
    live: HashMap<String, Store>,
}

#[derive(Debug, Clone)]
struct Version {
    title: String,
    status: Status,
    parent: Option<String>,
    conflicted: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error(transparent)]
    Git(#[from] op_git::GitError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl Index {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn matrix(&self) -> &Matrix {
        &self.matrix
    }

    pub fn cached_versions(&self) -> usize {
        self.blob_cache.len()
    }

    pub fn rebuild(&mut self, repo: &Repo, store: &Store) -> Result<(), IndexError> {
        self.live = live_worktrees(&repo.worktrees()?, store);
        let mut cells = Vec::new();
        for branch in repo.local_branches()? {
            let committed: HashMap<String, String> =
                repo.branch_task_blobs(&branch)?.into_iter().collect();
            match self.live.get(&branch).cloned() {
                Some(worktree) => {
                    self.overlay_cells(repo, &branch, &worktree, &committed, &mut cells)?
                }
                None => {
                    for (id, oid) in &committed {
                        let version = self.committed_version(repo, oid)?;
                        cells.push(cell(&branch, id, oid, false, &version));
                    }
                }
            }
        }
        cells.sort_by(|a, b| (&a.task.id, &a.branch).cmp(&(&b.task.id, &b.branch)));
        self.matrix = Matrix { cells };
        Ok(())
    }

    // The branch is checked out in a settled worktree, so its effective state is the working tree:
    // hash each on-disk file to compare against the committed blob and flag divergence.
    fn overlay_cells(
        &mut self,
        repo: &Repo,
        branch: &str,
        worktree: &Store,
        committed: &HashMap<String, String>,
        cells: &mut Vec<MatrixCell>,
    ) -> Result<(), IndexError> {
        for id in worktree.task_ids()? {
            let bytes = match worktree.read_raw(&id) {
                Ok(text) => text.into_bytes(),
                Err(StoreError::NotFound { .. }) => continue,
                Err(err) => return Err(err.into()),
            };
            let working_oid = repo.hash_blob(&bytes)?;
            let dirty = committed.get(&id) != Some(&working_oid);
            let version = self.cache_bytes(&working_oid, &bytes);
            cells.push(cell(branch, &id, &working_oid, dirty, &version));
        }
        Ok(())
    }

    fn committed_version(&mut self, repo: &Repo, oid: &str) -> Result<Version, IndexError> {
        if let Some(version) = self.blob_cache.get(oid) {
            return Ok(version.clone());
        }
        let bytes = repo.read_blob(oid)?;
        Ok(self.cache_bytes(oid, &bytes))
    }

    fn cache_bytes(&mut self, oid: &str, bytes: &[u8]) -> Version {
        if let Some(version) = self.blob_cache.get(oid) {
            return version.clone();
        }
        let version = parse_version(bytes);
        self.blob_cache.insert(oid.to_owned(), version.clone());
        version
    }

    pub fn branch_summaries(&self, branch: &str) -> Vec<TaskSummary> {
        self.matrix
            .cells
            .iter()
            .filter(|cell| cell.branch == branch)
            .map(|cell| cell.task.clone())
            .collect()
    }

    pub fn task_branches(&self, id: &str) -> Option<TaskBranches> {
        let mut groups: BTreeMap<String, TaskVersion> = BTreeMap::new();
        for cell in self.matrix.cells.iter().filter(|cell| cell.task.id == id) {
            let version = groups
                .entry(cell.blob_oid.clone())
                .or_insert_with(|| TaskVersion {
                    blob_oid: cell.blob_oid.clone(),
                    summary: cell.task.clone(),
                    branches: Vec::new(),
                    dirty: false,
                    conflicted: false,
                });
            version.branches.push(cell.branch.clone());
            version.dirty |= cell.dirty;
            version.conflicted |= cell.conflicted;
        }
        if groups.is_empty() {
            return None;
        }
        let mut versions: Vec<TaskVersion> = groups.into_values().collect();
        for version in &mut versions {
            version.branches.sort();
        }
        versions.sort_by(|a, b| a.branches.cmp(&b.branches));
        Some(TaskBranches {
            id: id.to_owned(),
            versions,
        })
    }

    pub fn effective_view(
        &self,
        repo: &Repo,
        id: &str,
        branch: &str,
    ) -> Result<Option<TaskView>, IndexError> {
        Ok(self
            .effective_raw(repo, id, branch)?
            .map(|raw| view_from_raw(id, &raw)))
    }

    // The raw file text of one task as it effectively stands on `branch`: the live working copy if
    // that branch is checked out with uncommitted edits, else the committed blob at its HEAD.
    pub fn effective_raw(
        &self,
        repo: &Repo,
        id: &str,
        branch: &str,
    ) -> Result<Option<String>, IndexError> {
        let Some(cell) = self.cell(id, branch) else {
            return Ok(None);
        };
        if cell.dirty {
            match self.live.get(branch).map(|worktree| worktree.read_raw(id)) {
                Some(Ok(text)) => Ok(Some(text)),
                Some(Err(StoreError::NotFound { .. })) | None => Ok(None),
                Some(Err(err)) => Err(err.into()),
            }
        } else {
            let bytes = repo.read_blob(&cell.blob_oid)?;
            Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
        }
    }

    fn cell(&self, id: &str, branch: &str) -> Option<&MatrixCell> {
        self.matrix
            .cells
            .iter()
            .find(|cell| cell.task.id == id && cell.branch == branch)
    }
}

fn live_worktrees(worktrees: &[Worktree], store: &Store) -> HashMap<String, Store> {
    let mut live = HashMap::new();
    for worktree in worktrees {
        if worktree.op_in_progress {
            continue;
        }
        let Some(branch) = &worktree.branch else {
            continue;
        };
        let opened = if same_path(&worktree.path, store.root()) {
            Some(store.clone())
        } else {
            Store::open(&worktree.path).ok()
        };
        if let Some(opened) = opened {
            live.entry(branch.clone()).or_insert(opened);
        }
    }
    live
}

fn cell(branch: &str, id: &str, oid: &str, dirty: bool, version: &Version) -> MatrixCell {
    MatrixCell {
        branch: branch.to_owned(),
        task: version.summary(id),
        blob_oid: oid.to_owned(),
        dirty,
        conflicted: version.conflicted,
    }
}

impl Version {
    fn summary(&self, id: &str) -> TaskSummary {
        TaskSummary {
            id: id.to_owned(),
            title: self.title.clone(),
            status: self.status,
            parent: self.parent.clone(),
        }
    }
}

fn parse_version(bytes: &[u8]) -> Version {
    let text = String::from_utf8_lossy(bytes);
    let conflicted = text.contains("<<<<<<<");
    match Task::from_file_string(&text) {
        Ok(task) => Version {
            title: task.title().unwrap_or_default(),
            status: task.frontmatter.status,
            parent: task.frontmatter.parent.clone(),
            conflicted,
        },
        // Unresolved merge markers can break the frontmatter fences; keep the cell rather than
        // abort the whole rebuild, best-effort title, status left at the unstarted default.
        Err(_) => Version {
            title: best_effort_title(&text).unwrap_or_default(),
            status: Status::Backlog,
            parent: None,
            conflicted: true,
        },
    }
}

fn view_from_raw(id: &str, raw: &str) -> TaskView {
    match Task::from_file_string(raw) {
        Ok(task) => TaskView::from_task(id.to_owned(), &task),
        Err(_) => TaskView {
            id: id.to_owned(),
            title: best_effort_title(raw).unwrap_or_default(),
            status: Status::Backlog,
            parent: None,
            deps: Vec::new(),
            body: raw.to_owned(),
        },
    }
}

fn best_effort_title(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("# ").map(|title| title.trim().to_owned()))
        .filter(|title| !title.is_empty())
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
