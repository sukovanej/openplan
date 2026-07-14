use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use op_api::{
    BranchMark, BranchState, ChangeKind, Matrix, MatrixCell, Status, TaskBranches, TaskListItem,
    TaskSummary, TaskVersion, TaskView,
};
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
    // The branch checked out in the serve-root worktree; the headline of an aggregated task.
    current_branch: Option<String>,
    default_branch: Option<String>,
}

#[derive(Debug, Clone)]
struct Version {
    title: String,
    status: Status,
    parent: Option<String>,
}

struct DiffCtx<'a> {
    branch: &'a str,
    committed: &'a HashMap<String, String>,
    base: &'a HashMap<String, HashSet<String>>,
    default_blobs: &'a HashMap<String, String>,
    live: Option<&'a Store>,
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
        let worktrees = repo.worktrees()?;
        self.current_branch = worktrees
            .iter()
            .find(|worktree| same_path(&worktree.path, store.root()))
            .and_then(|worktree| worktree.branch.clone());
        self.live = live_worktrees(&worktrees, store);
        self.default_branch = repo.default_branch()?;

        let default_commit = self
            .default_branch
            .as_deref()
            .map(|branch| repo.branch_commit(branch))
            .transpose()?;
        let default_blobs: HashMap<String, String> = match self.default_branch.as_deref() {
            Some(branch) => repo.branch_task_blobs(branch)?.into_iter().collect(),
            None => HashMap::new(),
        };

        let branches = repo.local_branches()?;
        let base_blobs = self.base_blobs_by_branch(repo, &branches, default_commit.as_deref())?;
        let no_base = HashMap::new();

        let mut cells = Vec::new();
        for branch in &branches {
            let committed: HashMap<String, String> =
                repo.branch_task_blobs(branch)?.into_iter().collect();
            let live = self.live.get(branch).cloned();
            if self.is_baseline(branch) {
                self.baseline_cells(repo, branch, &committed, live.as_ref(), &mut cells)?;
            } else {
                let ctx = DiffCtx {
                    branch,
                    committed: &committed,
                    base: base_blobs.get(branch).unwrap_or(&no_base),
                    default_blobs: &default_blobs,
                    live: live.as_ref(),
                };
                self.diff_cells(repo, &ctx, &mut cells)?;
            }
        }
        cells.sort_by(|a, b| (&a.task.id, &a.branch).cmp(&(&b.task.id, &b.branch)));
        self.matrix = Matrix { cells };
        Ok(())
    }

    // Every non-baseline branch's task blobs at its merge-base(s) with the default branch, keyed by
    // branch then task id. The blobs for a task form a set: with a criss-cross history a task has a
    // base version per merge-base, and the branch is unchanged if it matches any of them. All the
    // merge-bases are resolved in a single shared graph walk, and each base commit's tree is read
    // once even when several branches share it.
    fn base_blobs_by_branch(
        &self,
        repo: &Repo,
        branches: &[String],
        default_commit: Option<&str>,
    ) -> Result<HashMap<String, HashMap<String, HashSet<String>>>, IndexError> {
        let Some(default_commit) = default_commit else {
            return Ok(HashMap::new());
        };
        let non_baseline: Vec<&String> = branches.iter().filter(|b| !self.is_baseline(b)).collect();
        let commits: Vec<String> = non_baseline
            .iter()
            .map(|b| repo.branch_commit(b))
            .collect::<Result<_, _>>()?;
        let bases_per_branch = repo.merge_bases_against(default_commit, &commits)?;

        let mut tree_cache: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut out = HashMap::new();
        for (branch, base_commits) in non_baseline.iter().zip(bases_per_branch) {
            let mut blobs: HashMap<String, HashSet<String>> = HashMap::new();
            for base_commit in base_commits {
                if !tree_cache.contains_key(&base_commit) {
                    let read = repo.commit_task_blobs(&base_commit)?;
                    tree_cache.insert(base_commit.clone(), read);
                }
                for (id, oid) in &tree_cache[&base_commit] {
                    blobs.entry(id.clone()).or_default().insert(oid.clone());
                }
            }
            out.insert((*branch).clone(), blobs);
        }
        Ok(out)
    }

    // Without a default branch there is nothing to diff against, so every branch stands as its own
    // baseline (a plain presence view); otherwise only the default branch is the baseline.
    fn is_baseline(&self, branch: &str) -> bool {
        match &self.default_branch {
            None => true,
            Some(default) => default == branch,
        }
    }

    fn baseline_cells(
        &mut self,
        repo: &Repo,
        branch: &str,
        committed: &HashMap<String, String>,
        live: Option<&Store>,
        cells: &mut Vec<MatrixCell>,
    ) -> Result<(), IndexError> {
        for id in present_ids(committed, live)? {
            if let Some(cell) = self.present_cell(
                repo,
                branch,
                &id,
                committed.get(&id),
                ChangeKind::Base,
                live,
            )? {
                cells.push(cell);
            }
        }
        Ok(())
    }

    fn diff_cells(
        &mut self,
        repo: &Repo,
        ctx: &DiffCtx,
        cells: &mut Vec<MatrixCell>,
    ) -> Result<(), IndexError> {
        let ids: BTreeSet<String> = present_ids(ctx.committed, ctx.live)?
            .into_iter()
            .chain(ctx.base.keys().cloned())
            .collect();
        for id in ids {
            let base_oids = ctx.base.get(&id);
            let committed_oid = ctx.committed.get(&id);
            match self.present_cell(
                repo,
                ctx.branch,
                &id,
                committed_oid,
                ChangeKind::Base,
                ctx.live,
            )? {
                Some(mut cell) => {
                    // `cell.blob_oid` is the branch's effective version — the live working copy when
                    // it is checked out, else the committed tip — so uncommitted edits count as
                    // divergence too. A task unchanged against any merge-base is skipped.
                    let kind = match base_oids {
                        None => ChangeKind::Added,
                        Some(bases) if !bases.contains(&cell.blob_oid) => ChangeKind::Modified,
                        Some(_) => continue,
                    };
                    cell.kind = kind;
                    cells.push(cell);
                }
                // Effectively absent on the branch: a deletion, surfaced only while the default
                // branch still carries the task (a removal main already made is settled). `dirty`
                // when the commit still has the task and only the working tree dropped it.
                None => {
                    if let Some(bases) = base_oids {
                        if ctx.default_blobs.contains_key(&id) {
                            let blob = deletion_blob(bases, ctx.default_blobs.get(&id));
                            let version = self.committed_version(repo, blob)?;
                            let dirty = committed_oid.is_some();
                            cells.push(cell(
                                ctx.branch,
                                &id,
                                blob,
                                ChangeKind::Deleted,
                                dirty,
                                &version,
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // The task's effective state on the branch, or None when it is absent there: the live working
    // copy when the branch is checked out, else the committed blob. `blob_oid` is that effective
    // version and `dirty` flags it diverging from the committed tip; `kind` is set by the caller.
    fn present_cell(
        &mut self,
        repo: &Repo,
        branch: &str,
        id: &str,
        committed_oid: Option<&String>,
        kind: ChangeKind,
        live: Option<&Store>,
    ) -> Result<Option<MatrixCell>, IndexError> {
        if let Some(worktree) = live {
            return match worktree.read_raw(id) {
                Ok(text) => {
                    let bytes = text.into_bytes();
                    let working_oid = repo.hash_blob(&bytes)?;
                    let dirty = committed_oid != Some(&working_oid);
                    let version = self.cache_bytes(&working_oid, &bytes);
                    Ok(Some(cell(branch, id, &working_oid, kind, dirty, &version)))
                }
                Err(StoreError::NotFound { .. }) => Ok(None),
                Err(err) => Err(err.into()),
            };
        }
        match committed_oid {
            Some(oid) => {
                let version = self.committed_version(repo, oid)?;
                Ok(Some(cell(branch, id, oid, kind, false, &version)))
            }
            None => Ok(None),
        }
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
            .filter(|cell| cell.branch == branch && cell.kind != ChangeKind::Deleted)
            .map(|cell| cell.task.clone())
            .collect()
    }

    // One row per logical task across all branches: the headline is the default branch's version
    // (the authoritative one) when present, else the current worktree's branch, else the first.
    pub fn aggregated_tasks(&self) -> Vec<TaskListItem> {
        let mut groups: BTreeMap<&str, Vec<&MatrixCell>> = BTreeMap::new();
        for cell in &self.matrix.cells {
            groups.entry(cell.task.id.as_str()).or_default().push(cell);
        }
        groups
            .into_iter()
            .map(|(id, cells)| {
                let headline = self
                    .default_branch
                    .as_deref()
                    .and_then(|default| cells.iter().find(|c| c.branch == default).copied())
                    .or_else(|| {
                        self.current_branch
                            .as_deref()
                            .and_then(|current| cells.iter().find(|c| c.branch == current).copied())
                    })
                    .unwrap_or(cells[0]);
                let mut branches: Vec<BranchState> = cells
                    .iter()
                    .map(|cell| BranchState {
                        branch: cell.branch.clone(),
                        status: cell.task.status,
                        blob_oid: cell.blob_oid.clone(),
                        dirty: cell.dirty,
                        kind: cell.kind,
                    })
                    .collect();
                branches.sort_by(|a, b| a.branch.cmp(&b.branch));
                TaskListItem {
                    id: id.to_owned(),
                    title: headline.task.title.clone(),
                    status: headline.task.status,
                    parent: headline.task.parent.clone(),
                    branches,
                }
            })
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
                });
            version.branches.push(BranchMark {
                branch: cell.branch.clone(),
                kind: cell.kind,
                dirty: cell.dirty,
            });
        }
        if groups.is_empty() {
            return None;
        }
        let mut versions: Vec<TaskVersion> = groups.into_values().collect();
        for version in &mut versions {
            version.branches.sort_by(|a, b| a.branch.cmp(&b.branch));
        }
        versions.sort_by(|a, b| {
            a.branches
                .iter()
                .map(|m| &m.branch)
                .cmp(b.branches.iter().map(|m| &m.branch))
        });
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
    // that branch is checked out with uncommitted edits, else the committed blob at its HEAD. A
    // task the branch deletes has no effective text.
    pub fn effective_raw(
        &self,
        repo: &Repo,
        id: &str,
        branch: &str,
    ) -> Result<Option<String>, IndexError> {
        let Some(cell) = self.cell(id, branch) else {
            return Ok(None);
        };
        if cell.kind == ChangeKind::Deleted {
            return Ok(None);
        }
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

// The version a deletion row displays: the default branch's current blob when a merge-base still
// holds it (so the row groups with the default branch's version), else any base blob deterministically.
fn deletion_blob<'a>(bases: &'a HashSet<String>, default_oid: Option<&'a String>) -> &'a String {
    if let Some(oid) = default_oid {
        if bases.contains(oid) {
            return oid;
        }
    }
    bases
        .iter()
        .min()
        .expect("a deletion row is only built for a non-empty base set")
}

fn present_ids(
    committed: &HashMap<String, String>,
    live: Option<&Store>,
) -> Result<BTreeSet<String>, IndexError> {
    let mut ids: BTreeSet<String> = committed.keys().cloned().collect();
    if let Some(worktree) = live {
        for id in worktree.task_ids()? {
            ids.insert(id);
        }
    }
    Ok(ids)
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

fn cell(
    branch: &str,
    id: &str,
    oid: &str,
    kind: ChangeKind,
    dirty: bool,
    version: &Version,
) -> MatrixCell {
    MatrixCell {
        branch: branch.to_owned(),
        task: version.summary(id),
        blob_oid: oid.to_owned(),
        dirty,
        kind,
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
    match Task::from_file_string(&text) {
        Ok(task) => Version {
            title: task.title().unwrap_or_default(),
            status: task.frontmatter.status,
            parent: task.frontmatter.parent.clone(),
        },
        // Unresolved merge markers can break the frontmatter fences; keep the cell rather than
        // abort the whole rebuild, best-effort title, status left at the unstarted default.
        Err(_) => Version {
            title: best_effort_title(&text).unwrap_or_default(),
            status: Status::Backlog,
            parent: None,
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
