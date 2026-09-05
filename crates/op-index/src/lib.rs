use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use op_api::{
    BranchComments, BranchMark, BranchState, ChangeKind, Comment, Matrix, MatrixCell, Metadata,
    SearchHit, SearchMatch, TaskBranches, TaskChild, TaskDetail, TaskListItem, TaskRef,
    TaskSummary, TaskVersion, TaskView, WriteTarget, hit_cmp, id_cmp, list_item_cmp, updated_field,
};
use op_git::{ChangeTime, Repo, TaskChange, Worktree};
use op_store::{Config, RawTask, Store, StoreError};
use op_task::{Abbreviation, FieldError, FieldResult, Timestamp};

// Every id the index holds and hands out is a key; the numbers it reads from file names and
// git trees stop at the two conversions below, `keyed` and `number_of`.
#[derive(Debug)]
pub struct Index {
    abbreviation: Abbreviation,
    matrix: Matrix,
    // branch -> every task that branch holds, keyed by id. The matrix records only divergence, so a
    // task identical to its merge-base has no cell there; a read scoped to a branch still has to
    // find it, and has to tell "the branch does not carry this task" from "the branch agrees with
    // main about it". Present for every local branch, so an unknown branch is refused rather than
    // answered with nothing.
    branch_versions: HashMap<String, BTreeMap<String, BranchVersion>>,
    // Keyed by blob OID, which is content-addressed, so an entry never goes stale — the cache
    // grows across rebuilds and is only ever added to. Holds the id-independent parse (the id is
    // the filename, absent from the blob) so two tasks sharing a blob don't leak each other's id.
    blob_cache: HashMap<String, Version>,
    live: HashMap<String, Store>,
    // branch -> when its worktree last wrote each task file. A working-tree edit belongs to no
    // commit, so the file itself is the only thing that can date it.
    live_times: HashMap<String, HashMap<String, Timestamp>>,
    // The branch checked out in the serve-root worktree; the headline of an aggregated task.
    current_branch: Option<String>,
    // What `.plan/config.toml` asks for, and what the repository could give: a configured branch
    // that no longer exists leaves the resolved one on the autodetected fallback.
    configured_default_branch: Option<String>,
    default_branch: Option<String>,
    // (branch, number) -> the branch's last commit to touch the task, and when it was authored.
    // Absent for a task whose change lies deeper than the walk budget, and for one no commit holds
    // yet. Keyed by the number, the spelling git's file names carry.
    changes: HashMap<(String, String), TaskChange>,
    // branch -> a walk result reusable across the per-request rebuilds, instead of re-walking
    // history every time.
    change_cache: HashMap<String, BranchChanges>,
    // id -> the branch whose version supersedes the rest. Resolved during the rebuild because it
    // asks git which change commit contains which, and the matrix alone cannot answer that.
    headlines: HashMap<String, String>,
    // change commit -> the commits it has been compared against, and whether it descends from
    // each. History is immutable, so an answer holds forever: the map grows across rebuilds and is
    // only ever added to, sparing the graph a walk per contested pair on every request.
    ancestry: HashMap<String, HashMap<String, bool>>,
    max_id: Option<u64>,
    // What the dirty gate above is measured by: a read that skips the walk leaves this untouched.
    rebuilds: u64,
}

#[derive(Debug, Clone)]
struct BranchChanges {
    tip: String,
    // The ids the walk asked for: an id it found no commit for is absent from `changes`, so only
    // the request tells a cache hit from a task still to be dated.
    requested: HashSet<String>,
    changes: HashMap<String, TaskChange>,
}

#[derive(Debug, Clone)]
struct Version {
    title: String,
    metadata: Metadata,
    comment_count: usize,
    haystack: Haystack,
}

// Everything a search reads, lowercased once here so a query re-cases no field per keystroke. Split
// where the ranking cuts: a title hit outranks one that only the body or the frontmatter carries.
#[derive(Debug, Clone)]
struct Haystack {
    title: String,
    rest: String,
}

#[derive(Debug)]
struct Matched<'a> {
    matched: SearchMatch,
    branches: BTreeSet<&'a str>,
}

// How one task stands on one branch: the blob its effective text hashes to — the live working copy
// when the branch is checked out, else the committed tip — and whether those two differ.
#[derive(Debug, Clone)]
struct BranchVersion {
    blob_oid: String,
    dirty: bool,
}

// One branch as a rebuild sees it: the ids it holds, the blob each is committed as, and the
// worktree holding it when the branch is checked out.
struct BranchScan<'a> {
    branch: &'a str,
    present: &'a BTreeSet<String>,
    committed: &'a HashMap<String, String>,
    live: Option<&'a BTreeMap<String, RawTask>>,
}

struct DiffCtx<'a> {
    scan: BranchScan<'a>,
    base: &'a HashMap<String, HashSet<String>>,
    default_blobs: &'a HashMap<String, String>,
}

pub struct HierarchyContext {
    pub parent_title: Option<String>,
    pub children: Vec<TaskChild>,
    pub refs: Vec<TaskRef>,
    pub depends_on: Vec<TaskRef>,
    pub blocks: Vec<TaskRef>,
}

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error(transparent)]
    Git(#[from] op_git::GitError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl Index {
    pub fn new(config: &Config) -> Self {
        Self {
            abbreviation: config.abbreviation,
            matrix: Matrix::default(),
            branch_versions: HashMap::new(),
            blob_cache: HashMap::new(),
            live: HashMap::new(),
            live_times: HashMap::new(),
            current_branch: None,
            configured_default_branch: config.default_branch.clone(),
            default_branch: None,
            changes: HashMap::new(),
            change_cache: HashMap::new(),
            headlines: HashMap::new(),
            ancestry: HashMap::new(),
            max_id: None,
            rebuilds: 0,
        }
    }

    pub fn rebuilds(&self) -> u64 {
        self.rebuilds
    }

    // A cached version holds its parent and dependencies as keys, so a new abbreviation invalidates
    // every one of them; the next rebuild re-parses what it needs. A new default branch only moves
    // the baseline, which the rebuild recomputes anyway.
    pub fn set_config(&mut self, config: &Config) {
        if self.abbreviation != config.abbreviation {
            self.abbreviation = config.abbreviation;
            self.blob_cache.clear();
        }
        self.configured_default_branch = config.default_branch.clone();
    }

    pub fn abbreviation(&self) -> Abbreviation {
        self.abbreviation
    }

    pub fn matrix(&self) -> &Matrix {
        &self.matrix
    }

    fn keyed(&self, blobs: Vec<(String, String)>) -> HashMap<String, String> {
        blobs
            .into_iter()
            .filter_map(|(id, oid)| {
                // A tree lists whatever `.plan/tasks/*.md` a commit holds; only a file a number names
                // is a task, so the matrix and the store agree on what exists.
                Some((self.abbreviation.format_key(op_task::parse_id(&id)?), oid))
            })
            .collect()
    }

    fn number_of(&self, key: &str) -> u64 {
        self.abbreviation
            .parse_key(key)
            .expect("every matrix id is a key this index formatted")
    }

    fn keyed_raw(&self, raw: BTreeMap<u64, RawTask>) -> BTreeMap<String, RawTask> {
        raw.into_iter()
            .map(|(number, task)| (self.abbreviation.format_key(number), task))
            .collect()
    }

    pub fn cached_versions(&self) -> usize {
        self.blob_cache.len()
    }

    pub fn rebuild(&mut self, repo: &Repo, store: &Store) -> Result<(), IndexError> {
        self.rebuilds += 1;
        // The index's abbreviation is the store's, whatever handle the caller passed: a live config
        // change lands here, and every store this hands back out must speak the new keys.
        let store = &store.with_abbreviation(self.abbreviation);
        let worktrees = repo.worktrees()?;
        self.current_branch = worktrees
            .iter()
            .find(|worktree| same_path(&worktree.path, store.root()))
            .and_then(|worktree| worktree.branch.clone());
        self.live = live_worktrees(&worktrees, store);
        self.live_times.clear();
        self.default_branch = repo.default_branch(self.configured_default_branch.as_deref())?;

        let default_commit = self
            .default_branch
            .as_deref()
            .map(|branch| repo.branch_commit(branch))
            .transpose()?;
        let default_blobs: HashMap<String, String> = match self.default_branch.as_deref() {
            Some(branch) => self.keyed(repo.branch_task_blobs(branch)?),
            None => HashMap::new(),
        };

        let branches = repo.local_branches()?;
        // A cache entry is keyed by branch name and only ever checked against that branch's tip, so
        // a deleted branch's walk would sit there for the daemon's whole life — and a new branch
        // reusing the name would meet a stale entry.
        self.change_cache
            .retain(|branch, _| branches.contains(branch));
        let base_blobs = self.base_blobs_by_branch(repo, &branches, default_commit.as_deref())?;
        let no_base = HashMap::new();

        let mut cells = Vec::new();
        self.branch_versions.clear();
        // Every worktree's files, not just those of the branches walked below: a task can hold a
        // number while no branch commits it — an unborn HEAD has no branch at all, and a worktree
        // mid-merge is excluded from `live`. The number is taken the moment the file exists.
        let mut max_id = on_disk_id_floor(&worktrees, store);
        for branch in &branches {
            let committed = self.keyed(repo.branch_task_blobs(branch)?);
            let live = self
                .live
                .get(branch)
                .map(Store::read_all_raw)
                .transpose()?
                .map(|raw| self.keyed_raw(raw));
            if let Some(live) = live.as_ref() {
                self.live_times.insert(branch.clone(), live_times(live));
            }
            let present = present_ids(&committed, live.as_ref());
            // Every id the branch holds, not just the cells it contributes: a task identical to its
            // merge base is skipped below, and its number would otherwise read as free.
            max_id = max_id.max(present.iter().map(|key| self.number_of(key)).max());
            let scan = BranchScan {
                branch,
                present: &present,
                committed: &committed,
                live: live.as_ref(),
            };
            let mut versions = BTreeMap::new();
            if self.is_baseline(branch) {
                self.baseline_cells(repo, &scan, &mut versions, &mut cells)?;
            } else {
                let ctx = DiffCtx {
                    scan,
                    base: base_blobs.get(branch).unwrap_or(&no_base),
                    default_blobs: &default_blobs,
                };
                self.diff_cells(repo, &ctx, &mut versions, &mut cells)?;
            }
            self.branch_versions.insert(branch.clone(), versions);
        }
        self.max_id = max_id;
        cells.sort_by(|a, b| id_cmp(&a.task.id, &b.task.id).then_with(|| a.branch.cmp(&b.branch)));
        self.changes = self.compute_changes(repo, &cells)?;
        self.headlines = self.compute_headlines(repo, &cells)?;
        self.matrix = Matrix { cells };
        Ok(())
    }

    // Every cell's last change: the commit the headline choice compares by containment, and the
    // author time behind the task's `updated`. A dirty cell is left out: its working-tree edit
    // belongs to no commit, so both readings substitute their own answer.
    fn compute_changes(
        &mut self,
        repo: &Repo,
        cells: &[MatrixCell],
    ) -> Result<HashMap<(String, String), TaskChange>, IndexError> {
        let mut request: HashMap<String, HashSet<String>> = HashMap::new();
        for cell in cells {
            if cell.kind != ChangeKind::Deleted && !cell.dirty {
                request
                    .entry(cell.branch.clone())
                    .or_default()
                    .insert(self.number_of(&cell.task.id).to_string());
            }
        }

        let mut out = HashMap::new();
        for (branch, ids) in request {
            let tip = repo.branch_commit(&branch)?;
            let changes = match self.change_cache.get(&branch) {
                Some(cached) if cached.tip == tip && ids.is_subset(&cached.requested) => {
                    cached.changes.clone()
                }
                _ => {
                    let changes = repo.task_changes(&branch, &ids)?;
                    self.change_cache.insert(
                        branch.clone(),
                        BranchChanges {
                            tip,
                            requested: ids.clone(),
                            changes: changes.clone(),
                        },
                    );
                    changes
                }
            };
            for id in ids {
                if let Some(change) = changes.get(&id) {
                    out.insert((branch.clone(), id), change.clone());
                }
            }
        }
        Ok(out)
    }

    // The branch each task headlines with: the version no other version supersedes, and the newest
    // of those when several stand. Superseded versions are dropped before the dates are consulted
    // at all — a single pass that mixed the two would let an unrelated third branch knock out the
    // version that beats the one it then loses to.
    fn compute_headlines(
        &mut self,
        repo: &Repo,
        cells: &[MatrixCell],
    ) -> Result<HashMap<String, String>, IndexError> {
        let contenders: Vec<Vec<&MatrixCell>> = cells
            .chunk_by(|a, b| a.task.id == b.task.id)
            .map(|group| {
                group
                    .iter()
                    .filter(|cell| cell.kind != ChangeKind::Deleted)
                    .collect()
            })
            .collect();
        self.cache_ancestry(repo, &contenders)?;

        let mut headlines = HashMap::new();
        for group in contenders {
            // Containment is acyclic, so a non-empty group always leaves something standing.
            let standing = group
                .iter()
                .copied()
                .filter(|cell| !group.iter().any(|other| self.supersedes(other, cell)))
                .max_by_key(|cell| self.tie_break(cell));
            if let Some(cell) = standing {
                headlines.insert(cell.task.id.clone(), cell.branch.clone());
            }
        }
        Ok(headlines)
    }

    // Git is asked for containment rather than for a date: a version whose change commit descends
    // from another's is strictly the later one, and a rebase leaves the author date of what it
    // replays untouched while rewriting its commit date, so neither clock can answer this alone.
    fn cache_ancestry(
        &mut self,
        repo: &Repo,
        contenders: &[Vec<&MatrixCell>],
    ) -> Result<(), IndexError> {
        // Only versions that differ, and that a commit accounts for, can be contested: identical
        // blobs say the same thing whichever branch wins, and a working-tree edit is in no commit
        // for the graph to place.
        let mut unanswered: BTreeSet<(String, String)> = BTreeSet::new();
        for group in contenders {
            for (index, one) in group.iter().enumerate() {
                for other in &group[index + 1..] {
                    if one.blob_oid == other.blob_oid {
                        continue;
                    }
                    let (Some(one), Some(other)) =
                        (self.change_commit(one), self.change_commit(other))
                    else {
                        continue;
                    };
                    if self.descends_from(one, other).is_none() {
                        unanswered.insert((one.to_owned(), other.to_owned()));
                    }
                }
            }
        }

        let pairs: Vec<(String, String)> = unanswered.into_iter().collect();
        let asked: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(one, other)| (one.as_str(), other.as_str()))
            .collect();
        for ((one, other), found) in pairs.iter().zip(repo.ancestry(&asked)?) {
            self.remember(one, other, found == Some(Ordering::Greater));
            self.remember(other, one, found == Some(Ordering::Less));
        }
        Ok(())
    }

    fn remember(&mut self, commit: &str, against: &str, descends: bool) {
        self.ancestry
            .entry(commit.to_owned())
            .or_default()
            .insert(against.to_owned(), descends);
    }

    fn descends_from(&self, commit: &str, against: &str) -> Option<bool> {
        self.ancestry.get(commit)?.get(against).copied()
    }

    // Identical content supersedes nothing: both branches say the same thing, so which of them
    // headlines is a matter of recency and not of which commit came later.
    fn supersedes(&self, cell: &MatrixCell, over: &MatrixCell) -> bool {
        if cell.blob_oid == over.blob_oid {
            return false;
        }
        let (Some(commit), Some(against)) = (self.change_commit(cell), self.change_commit(over))
        else {
            return false;
        };
        self.descends_from(commit, against) == Some(true)
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

        let mut tree_cache: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut out = HashMap::new();
        for (branch, base_commits) in non_baseline.iter().zip(bases_per_branch) {
            let mut blobs: HashMap<String, HashSet<String>> = HashMap::new();
            for base_commit in base_commits {
                if !tree_cache.contains_key(&base_commit) {
                    let read = self.keyed(repo.commit_task_blobs(&base_commit)?);
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
        scan: &BranchScan,
        versions: &mut BTreeMap<String, BranchVersion>,
        cells: &mut Vec<MatrixCell>,
    ) -> Result<(), IndexError> {
        for id in scan.present {
            if let Some(cell) = self.present_cell(repo, scan, id, ChangeKind::Base)? {
                versions.insert(id.clone(), branch_version(&cell));
                cells.push(cell);
            }
        }
        Ok(())
    }

    fn diff_cells(
        &mut self,
        repo: &Repo,
        ctx: &DiffCtx,
        versions: &mut BTreeMap<String, BranchVersion>,
        cells: &mut Vec<MatrixCell>,
    ) -> Result<(), IndexError> {
        let ids: BTreeSet<&String> = ctx.scan.present.iter().chain(ctx.base.keys()).collect();
        for id in ids {
            let base_oids = ctx.base.get(id);
            let committed_oid = ctx.scan.committed.get(id);
            match self.present_cell(repo, &ctx.scan, id, ChangeKind::Base)? {
                Some(mut cell) => {
                    // Recorded before the divergence test below: the branch carries the task either
                    // way, and a version identical to the merge-base contributes no cell.
                    versions.insert(id.clone(), branch_version(&cell));
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
                        if ctx.default_blobs.contains_key(id) {
                            let blob = deletion_blob(bases, ctx.default_blobs.get(id));
                            let version = self.committed_version(repo, blob)?;
                            let dirty = committed_oid.is_some();
                            cells.push(cell(
                                ctx.scan.branch,
                                id,
                                blob,
                                ChangeKind::Deleted,
                                dirty,
                                version,
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
        scan: &BranchScan,
        id: &str,
        kind: ChangeKind,
    ) -> Result<Option<MatrixCell>, IndexError> {
        let branch = scan.branch;
        let committed_oid = scan.committed.get(id);
        if let Some(worktree) = scan.live {
            return match worktree.get(id) {
                Some(task) => {
                    let bytes = task.text.as_bytes();
                    let working_oid = repo.hash_blob(bytes)?;
                    let dirty = committed_oid != Some(&working_oid);
                    let version = self.cache_bytes(&working_oid, bytes);
                    Ok(Some(cell(branch, id, &working_oid, kind, dirty, version)))
                }
                None => Ok(None),
            };
        }
        match committed_oid {
            Some(oid) => {
                let version = self.committed_version(repo, oid)?;
                Ok(Some(cell(branch, id, oid, kind, false, version)))
            }
            None => Ok(None),
        }
    }

    // Borrowed rather than cloned: a cached parse carries the file's whole searchable text, and a
    // rebuild reads one per task per branch.
    fn committed_version(&mut self, repo: &Repo, oid: &str) -> Result<&Version, IndexError> {
        if !self.blob_cache.contains_key(oid) {
            let bytes = repo.read_blob(oid)?;
            self.cache_bytes(oid, &bytes);
        }
        Ok(&self.blob_cache[oid])
    }

    fn cache_bytes(&mut self, oid: &str, bytes: &[u8]) -> &Version {
        self.blob_cache
            .entry(oid.to_owned())
            .or_insert_with(|| parse_version(bytes, self.abbreviation))
    }

    pub fn has_branch(&self, branch: &str) -> bool {
        self.branch_versions.contains_key(branch)
    }

    pub fn has_branches(&self) -> bool {
        !self.branch_versions.is_empty()
    }

    pub fn branch_summaries(&self, branch: &str) -> Vec<TaskSummary> {
        let mut summaries: Vec<TaskSummary> = self
            .branch_of(branch)
            .filter_map(|(id, version)| Some(self.parsed(version)?.summary(id)))
            .collect();
        summaries.sort_by(|a, b| id_cmp(&a.id, &b.id));
        summaries
    }

    // The branch's own task set as list rows, which is what a caller asking about one branch means
    // by "the tasks": every task the branch carries, headlined by that branch because no other
    // branch was asked about.
    pub fn branch_tasks(&self, project: &str, branch: &str) -> Vec<TaskListItem> {
        let mut items: Vec<TaskListItem> = self
            .branch_of(branch)
            .filter_map(|(id, version)| {
                let parsed = self.parsed(version)?;
                Some(TaskListItem {
                    project: project.to_owned(),
                    id: id.clone(),
                    title: parsed.title.clone(),
                    comment_count: parsed.comment_count,
                    updated: updated_field(
                        parsed.metadata.created(),
                        self.task_updated_or_headline(id, Some(branch)),
                    ),
                    headline: branch.to_owned(),
                    branches: self.branch_state(id, branch).into_iter().collect(),
                    write_target: self.write_target(id, Some(branch)),
                    metadata: parsed.metadata.clone(),
                })
            })
            .collect();
        items.sort_by(|a, b| id_cmp(&a.id, &b.id));
        items
    }

    // The branch set of a read that answered for one branch. A branch agreeing with its merge-base
    // contributes no cell, so the matrix alone can leave out the very branch the version came from
    // — a response describing that branch's task while never naming it.
    fn states_naming(&self, cells: &[&MatrixCell], id: &str, branch: &str) -> Vec<BranchState> {
        let mut states = branch_states(cells);
        if !states.iter().any(|state| state.branch == branch)
            && let Some(state) = self.branch_state(id, branch)
        {
            states.push(state);
            states.sort_by(|a, b| a.branch.cmp(&b.branch));
        }
        states
    }

    // How one task stands on one branch — the same answer whether a caller asks about the task
    // alone or reads it in the branch's list.
    fn branch_state(&self, id: &str, branch: &str) -> Option<BranchState> {
        let version = self.branch_versions.get(branch)?.get(id)?;
        Some(BranchState {
            branch: branch.to_owned(),
            status: self.parsed(version)?.metadata.status_field(),
            blob_oid: version.blob_oid.clone(),
            dirty: version.dirty,
            // A task the branch agrees with its merge-base about has no cell to read a kind from,
            // and agreement is what `Base` says.
            kind: self
                .cell(id, branch)
                .map_or(ChangeKind::Base, |cell| cell.kind),
        })
    }

    // The matrix carries a task's summary, not its whole parse, so a row's comment count comes from
    // the blob the cell names.
    fn comment_count(&self, blob_oid: &str) -> usize {
        self.blob_cache
            .get(blob_oid)
            .map_or(0, |version| version.comment_count)
    }

    fn branch_of(&self, branch: &str) -> impl Iterator<Item = (&String, &BranchVersion)> {
        self.branch_versions.get(branch).into_iter().flatten()
    }

    // Every effective version a rebuild recorded was parsed and cached under its blob OID, so this
    // resolves for anything `branch_versions` holds.
    fn parsed(&self, version: &BranchVersion) -> Option<&Version> {
        self.blob_cache.get(&version.blob_oid)
    }

    // One row per logical task across all branches, headlined by the branch whose version
    // supersedes the rest (see `compute_headlines`). The index serves one store and holds no name
    // for it, so the project each row is stamped with is the caller's to name.
    pub fn aggregated_tasks(&self, project: &str) -> Vec<TaskListItem> {
        // The cells are ordered by id already, so grouping the runs keeps that order — collecting
        // into a map keyed by the id would re-sort it as text, filing 10 between 1 and 2.
        self.matrix
            .cells
            .chunk_by(|a, b| a.task.id == b.task.id)
            .map(|group| {
                let cells: Vec<&MatrixCell> = group.iter().collect();
                let headline = self.headline_cell(&cells);
                TaskListItem {
                    project: project.to_owned(),
                    id: headline.task.id.clone(),
                    title: headline.task.title.clone(),
                    metadata: headline.task.metadata.clone(),
                    comment_count: self.comment_count(&headline.blob_oid),
                    updated: updated_field(self.created_of(headline), self.cell_updated(headline)),
                    headline: headline.branch.clone(),
                    branches: branch_states(&cells),
                    write_target: self.write_target(&headline.task.id, None),
                }
            })
            .collect()
    }

    // Every task the query matches, on any branch, as the same aggregated rows the list reads answer
    // with — so a hit renders and opens exactly like a list row, and a task the aggregation cannot
    // reach is as absent here as it is on the board. Matching is a case-insensitive substring over
    // the key and the whole file — title, body, and frontmatter — so a query means one thing
    // everywhere it is typed. A query of nothing but spaces matches nothing rather than everything:
    // a palette that opens on the whole store is a list, not a search.
    pub fn search(&self, project: &str, query: &str) -> Vec<SearchHit> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        let needle = query.to_lowercase();
        let mut matched: HashMap<&str, Matched> = HashMap::new();
        for (branch, versions) in &self.branch_versions {
            for (id, version) in versions {
                let Some(found) = self.matched(id, version, &needle) else {
                    continue;
                };
                let entry = matched.entry(id).or_insert(Matched {
                    matched: found,
                    branches: BTreeSet::new(),
                });
                // The strongest reason the task matches anywhere is the one that ranks the row,
                // because the row stands for the task rather than for one branch's version of it.
                entry.matched = entry.matched.min(found);
                entry.branches.insert(branch);
            }
        }
        let mut hits: Vec<SearchHit> = self
            .aggregated_tasks(project)
            .into_iter()
            .filter_map(|task| {
                let matched = matched.get(task.id.as_str())?;
                // The headline branch is the version every other read answers with, so a hit names
                // it whenever its text matches too; only a match that lives nowhere else names
                // another branch.
                let branch = match matched.branches.contains(task.headline.as_str()) {
                    true => task.headline.clone(),
                    false => (*matched.branches.first()?).to_owned(),
                };
                Some(SearchHit {
                    task,
                    branch,
                    matched: matched.matched,
                })
            })
            .collect();
        hits.sort_by(hit_cmp);
        hits
    }

    // The id names the file, not the blob, so it is absent from the cached parse and is tested
    // here instead. A palette where a key does not find its own task is the one thing a reader
    // will type first.
    fn matched(&self, id: &str, version: &BranchVersion, needle: &str) -> Option<SearchMatch> {
        if id.to_lowercase().contains(needle) {
            return Some(SearchMatch::Key);
        }
        let haystack = &self.parsed(version)?.haystack;
        if haystack.title.contains(needle) {
            return Some(SearchMatch::Title);
        }
        haystack.rest.contains(needle).then_some(SearchMatch::Text)
    }

    pub fn current_branch(&self) -> Option<&str> {
        self.current_branch.as_deref()
    }

    // The highest id number any local branch or worktree holds — the floor an allocator must clear
    // for a number to be unissued repo-wide. Read from the branches and the files themselves rather
    // than from the matrix, whose cells only cover divergence.
    pub fn max_id(&self) -> Option<u64> {
        self.max_id
    }

    // The already-opened store of the worktree that has `branch` checked out and is writable — not
    // `op_in_progress`. `None` when the branch is not checked out live, so a caller can refuse a
    // write rather than fabricate a commit onto a branch no worktree holds.
    pub fn live_store(&self, branch: &str) -> Option<Store> {
        self.live.get(branch).cloned()
    }

    // Where a write to one task goes when the caller names no branch: the serve-root worktree's own
    // branch while it carries the task, else a branch that does. The aggregated reads list every
    // branch's tasks, so acting on one of those rows must not depend on which branch the serve root
    // happens to have checked out. The serve root's own claim comes first even when no worktree can
    // take the write right now, so a stalled checkout refuses rather than sending the write to
    // another branch's version. A task no branch holds resolves to the serve root, so the refusal is
    // about the task rather than about the branch.
    pub fn write_branch(&self, id: &str) -> Option<&str> {
        let current = self.current_branch.as_deref();
        if current.is_some_and(|branch| self.holds(branch, id)) {
            return current;
        }
        // Every headline is a branch that holds the task: `compute_headlines` drops deletions. A task
        // every branch agrees about contributes no cell at all, so it can be headlined by none while
        // a branch still holds it — hence the search below rather than a fall straight to the root.
        self.headlines
            .get(id)
            .map(String::as_str)
            .or_else(|| self.holding_branch(id))
            .or(current)
    }

    // Where a read's own writes go, and whether they can land: a read scoped to a branch writes to
    // that branch, and one that named none writes wherever the task lives. A client must be told
    // both, so it offers only the actions that can succeed and can name the branch that stops the
    // rest.
    pub fn write_target(&self, id: &str, branch: Option<&str>) -> Option<WriteTarget> {
        let branch = match branch {
            Some(branch) => branch,
            None => self.write_branch(id)?,
        };
        Some(WriteTarget {
            branch: branch.to_owned(),
            writable: self.live.contains_key(branch),
        })
    }

    // A branch that holds the task, preferring one a write could land on; branches are ranked by
    // name after that, so the answer never rides on hash order.
    fn holding_branch(&self, id: &str) -> Option<&str> {
        self.branch_versions
            .iter()
            .filter(|(_, tasks)| tasks.contains_key(id))
            .map(|(branch, _)| branch.as_str())
            .min_by_key(|branch| (!self.live.contains_key(*branch), *branch))
    }

    fn holds(&self, branch: &str, id: &str) -> bool {
        self.branch_versions
            .get(branch)
            .is_some_and(|tasks| tasks.contains_key(id))
    }

    pub fn task_branch_states(&self, id: &str) -> Vec<BranchState> {
        branch_states(&self.cells_of(id))
    }

    fn cells_of(&self, id: &str) -> Vec<&MatrixCell> {
        self.matrix
            .cells
            .iter()
            .filter(|cell| cell.task.id == id)
            .collect()
    }

    // The headline view (branch `None`) or a named branch's view, paired with every branch the task
    // lives on. A named branch resolves honestly — `None` when the task is absent there. The
    // branchless headline instead always yields content while any cell exists (a deletion falls back
    // to its last-known blob) so it never 404s a task the list still shows.
    pub fn task_detail(
        &self,
        repo: &Repo,
        project: &str,
        id: &str,
        branch: Option<&str>,
    ) -> Result<Option<TaskDetail>, IndexError> {
        let cells = self.cells_of(id);
        let Some(raw) = self.resolved_raw(repo, id, branch, &cells)? else {
            return Ok(None);
        };
        let updated = match branch {
            Some(branch) => self.task_updated_or_headline(id, Some(branch)),
            None => self.cell_updated(self.headline_cell(&cells)),
        };
        let view = view_from_raw(id, &raw, updated, self.abbreviation);
        let hierarchy = self.hierarchy_context(project, &view);
        Ok(Some(TaskDetail {
            project: project.to_owned(),
            id: view.id,
            title: view.title,
            metadata: view.metadata,
            comments: comments_of(&view.body),
            body: op_task::comment::strip(&view.body),
            updated: view.updated,
            // A task every branch agrees about contributes no cell at all, so a branch-scoped read
            // can resolve one the matrix cannot headline; the branch asked for is then the only
            // branch that has been named.
            headline: self
                .headline_branch(id)
                .or_else(|| branch.map(str::to_owned))
                .unwrap_or_default(),
            branches: match branch {
                Some(branch) => self.states_naming(&cells, id, branch),
                None => branch_states(&cells),
            },
            write_target: self.write_target(id, branch),
            parent_title: hierarchy.parent_title,
            children: hierarchy.children,
            refs: hierarchy.refs,
            depends_on: hierarchy.depends_on,
            blocks: hierarchy.blocks,
        }))
    }

    // The text a read resolves to: a named branch's effective version, or the version the task
    // headlines with. The branchless form always yields content while any cell exists — a deletion
    // falls back to its last-known blob — so it never 404s a task the list still shows.
    fn resolved_raw(
        &self,
        repo: &Repo,
        id: &str,
        branch: Option<&str>,
        cells: &[&MatrixCell],
    ) -> Result<Option<String>, IndexError> {
        match branch {
            Some(branch) => self.effective_raw(repo, id, branch),
            None => match cells {
                [] => Ok(None),
                cells => Ok(Some(self.cell_raw(repo, self.headline_cell(cells))?)),
            },
        }
    }

    pub fn task_comments(
        &self,
        repo: &Repo,
        id: &str,
        branch: Option<&str>,
    ) -> Result<Option<Vec<Comment>>, IndexError> {
        let cells = self.cells_of(id);
        Ok(self
            .resolved_raw(repo, id, branch, &cells)?
            .map(|raw| comments_of(&op_task::parse_partial(&raw).body)))
    }

    // Every branch that carries the task, each with its own log. Grouped rather than merged: the
    // file order inside one branch is the true order, and only the reader can decide how to
    // interleave two branches that never agreed on a clock.
    pub fn task_comments_by_branch(
        &self,
        repo: &Repo,
        id: &str,
    ) -> Result<Vec<BranchComments>, IndexError> {
        let mut branches: Vec<&String> = self
            .branch_versions
            .iter()
            .filter(|(_, held)| held.contains_key(id))
            .map(|(branch, _)| branch)
            .collect();
        branches.sort();
        branches
            .into_iter()
            .map(|branch| {
                Ok(BranchComments {
                    branch: branch.clone(),
                    comments: self
                        .task_comments(repo, id, Some(branch))?
                        .unwrap_or_default(),
                })
            })
            .collect()
    }

    // The immediate neighbourhood of a task, from the aggregated set: the parent's title (when it
    // resolves), the direct children in sibling order, every `[[id]]` in the body resolved to a
    // title/status, and both directions of its dependencies. Lets the detail read stand alone
    // without shipping the whole task list.
    pub fn hierarchy_context(&self, project: &str, view: &TaskView) -> HierarchyContext {
        let aggregated = self.aggregated_tasks(project);
        let by_id: HashMap<&str, &TaskListItem> =
            aggregated.iter().map(|t| (t.id.as_str(), t)).collect();
        let id = view.id.as_str();
        let parent_title = view
            .metadata
            .parent()
            .and_then(|p| by_id.get(p))
            .map(|t| t.title.clone());
        let mut kids: Vec<&TaskListItem> = aggregated
            .iter()
            .filter(|t| t.metadata.parent() == Some(id))
            .collect();
        kids.sort_by(|a, b| list_item_cmp(a, b));
        let children = kids
            .into_iter()
            .map(|t| TaskChild {
                id: t.id.clone(),
                title: t.title.clone(),
                status: t.metadata.status_field(),
                rank: t.metadata.rank().map(str::to_owned),
            })
            .collect();
        // Two entries may name two sections of one task, which is still one task this waits for.
        let mut waited_for = HashSet::new();
        let depends_on = view
            .metadata
            .dependencies()
            .iter()
            .filter_map(|entry| by_id.get(op_task::ref_target(entry)))
            .filter(|t| waited_for.insert(t.id.as_str()))
            .map(|t| task_ref(t))
            .collect();
        let mut blocked: Vec<&TaskListItem> =
            aggregated.iter().filter(|t| depends_on_id(t, id)).collect();
        blocked.sort_by(|a, b| list_item_cmp(a, b));
        HierarchyContext {
            parent_title,
            children,
            refs: body_refs(self.abbreviation, &view.body, &by_id),
            depends_on,
            blocks: blocked.into_iter().map(task_ref).collect(),
        }
    }

    // The branch a branchless read headlines with, for the write path which builds its own
    // `TaskDetail` from the just-written task rather than through `task_detail`.
    pub fn headline_branch(&self, id: &str) -> Option<String> {
        let cells = self.cells_of(id);
        (!cells.is_empty()).then(|| self.headline_cell(&cells).branch.clone())
    }

    // The effective text of one cell, including a deletion's last-known blob so a branchless read of
    // a task every live branch has dropped still resolves instead of 404-ing.
    fn cell_raw(&self, repo: &Repo, cell: &MatrixCell) -> Result<String, IndexError> {
        if cell.dirty && cell.kind != ChangeKind::Deleted {
            if let Some(store) = self.live.get(&cell.branch) {
                return Ok(store.read_raw(self.number_of(&cell.task.id))?);
            }
        }
        let bytes = repo.read_blob(&cell.blob_oid)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    // The version a task headlines with, decided in `compute_headlines`. Deletions never headline
    // while any version survives; when every version is a deletion it falls back to the first cell.
    fn headline_cell<'a>(&self, cells: &[&'a MatrixCell]) -> &'a MatrixCell {
        let chosen = self.headlines.get(&cells[0].task.id);
        cells
            .iter()
            .find(|cell| Some(&cell.branch) == chosen)
            .copied()
            .unwrap_or(cells[0])
    }

    // Between versions with no ancestry either way, "newest" is a judgement call: a live
    // uncommitted edit is dated by its file — and outranks any commit only when the filesystem
    // gives up no time to date it by — a cell no walk could date reads as oldest, and author time
    // settles the rest because it describes when the work was done rather than when it was last
    // replayed.
    fn tie_break(&self, cell: &MatrixCell) -> (i64, u8) {
        let authored = if cell.dirty {
            self.live_modified(cell)
                .map_or(i64::MAX, |at| at.as_second())
        } else {
            self.change_time(cell)
                .and_then(|at| at.as_ref().ok())
                .map_or(i64::MIN, |at| at.as_second())
        };
        (authored, self.headline_pref(cell))
    }

    // `created` is a property of the blob, so it comes from the same parse the cell's title and
    // status do rather than a second read of the file.
    fn created_of(&self, cell: &MatrixCell) -> Option<Timestamp> {
        self.blob_cache.get(&cell.blob_oid)?.metadata.created()
    }

    fn live_modified(&self, cell: &MatrixCell) -> Option<Timestamp> {
        self.live_times
            .get(&cell.branch)?
            .get(&cell.task.id)
            .copied()
    }

    fn change_time(&self, cell: &MatrixCell) -> Option<&ChangeTime> {
        Some(&self.change_of(cell)?.at)
    }

    fn change_commit(&self, cell: &MatrixCell) -> Option<&str> {
        Some(self.change_of(cell)?.commit.as_str())
    }

    fn change_of(&self, cell: &MatrixCell) -> Option<&TaskChange> {
        self.changes.get(&(
            cell.branch.clone(),
            self.number_of(&cell.task.id).to_string(),
        ))
    }

    // A working-tree edit has no commit to date it by, so the file dates it — otherwise every read
    // would report it as happening now, and a change made days ago would never stop being fresh.
    // Only a filesystem that gives up no time at all falls back to now. A cell no edit is live for
    // reports what its commit could say: a time, or why that commit could not give one.
    fn cell_updated(&self, cell: &MatrixCell) -> FieldResult<Timestamp> {
        if cell.kind == ChangeKind::Deleted {
            // The file is gone, and no other date on disk belongs to this task: the directory that
            // held it is restamped by every later write to any of its siblings, so reading it would
            // hand a deletion whatever moment some unrelated task was last edited.
            return Err(FieldError::Missing);
        }
        if cell.dirty {
            return Ok(self.live_modified(cell).unwrap_or_else(Timestamp::now));
        }
        match self.change_time(cell) {
            None => Err(FieldError::Missing),
            Some(Ok(at)) => Ok(*at),
            Some(Err(why)) => Err(FieldError::Invalid(why.clone())),
        }
    }

    // The task's last-change time on one branch, or on its headline branch when `branch` is `None`.
    pub fn task_updated(&self, id: &str, branch: Option<&str>) -> FieldResult<Timestamp> {
        match branch {
            Some(branch) => match self.cell(id, branch) {
                Some(cell) => self.cell_updated(cell),
                None => Err(FieldError::Missing),
            },
            None => {
                let cells = self.cells_of(id);
                if cells.is_empty() {
                    return Err(FieldError::Missing);
                }
                self.cell_updated(self.headline_cell(&cells))
            }
        }
    }

    // A branch matching its merge-base has no cell of its own, so it borrows the date of the branch
    // that did last change the task. A cell that exists but could not be dated keeps its own reason
    // rather than borrowing a time from elsewhere.
    pub fn task_updated_or_headline(
        &self,
        id: &str,
        branch: Option<&str>,
    ) -> FieldResult<Timestamp> {
        match self.task_updated(id, branch) {
            Err(FieldError::Missing) => self.task_updated(id, None),
            found => found,
        }
    }

    // A same-second tie prefers the current worktree, then the default branch, then stable order, so
    // the headline never flips between rebuilds.
    fn headline_pref(&self, cell: &MatrixCell) -> u8 {
        if self.current_branch.as_deref() == Some(cell.branch.as_str()) {
            2
        } else if self.default_branch.as_deref() == Some(cell.branch.as_str()) {
            1
        } else {
            0
        }
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
        Ok(self.effective_raw(repo, id, branch)?.map(|raw| {
            view_from_raw(
                id,
                &raw,
                self.task_updated_or_headline(id, Some(branch)),
                self.abbreviation,
            )
        }))
    }

    // The raw file text of one task as it effectively stands on `branch`: the live working copy if
    // that branch is checked out with uncommitted edits, else the committed blob at its HEAD. A
    // task the branch does not carry has no effective text.
    pub fn effective_raw(
        &self,
        repo: &Repo,
        id: &str,
        branch: &str,
    ) -> Result<Option<String>, IndexError> {
        let Some(version) = self
            .branch_versions
            .get(branch)
            .and_then(|held| held.get(id))
        else {
            return Ok(None);
        };
        if version.dirty {
            let number = self.number_of(id);
            match self
                .live
                .get(branch)
                .map(|worktree| worktree.read_raw(number))
            {
                Some(Ok(text)) => Ok(Some(text)),
                Some(Err(StoreError::NotFound { .. })) | None => Ok(None),
                Some(Err(err)) => Err(err.into()),
            }
        } else {
            let bytes = repo.read_blob(&version.blob_oid)?;
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

fn branch_version(cell: &MatrixCell) -> BranchVersion {
    BranchVersion {
        blob_oid: cell.blob_oid.clone(),
        dirty: cell.dirty,
    }
}

fn branch_states(cells: &[&MatrixCell]) -> Vec<BranchState> {
    let mut branches: Vec<BranchState> = cells
        .iter()
        .map(|cell| BranchState {
            branch: cell.branch.clone(),
            status: cell.task.metadata.status_field(),
            blob_oid: cell.blob_oid.clone(),
            dirty: cell.dirty,
            kind: cell.kind,
        })
        .collect();
    branches.sort_by(|a, b| a.branch.cmp(&b.branch));
    branches
}

fn live_times(live: &BTreeMap<String, RawTask>) -> HashMap<String, Timestamp> {
    live.iter()
        .filter_map(|(id, task)| Some((id.clone(), task.modified?)))
        .collect()
}

fn present_ids(
    committed: &HashMap<String, String>,
    live: Option<&BTreeMap<String, RawTask>>,
) -> BTreeSet<String> {
    let mut ids: BTreeSet<String> = committed.keys().cloned().collect();
    if let Some(worktree) = live {
        ids.extend(worktree.keys().cloned());
    }
    ids
}

fn on_disk_id_floor(worktrees: &[Worktree], store: &Store) -> Option<u64> {
    worktrees
        .iter()
        .filter_map(|worktree| {
            if same_path(&worktree.path, store.root()) {
                Some(store.clone())
            } else {
                Store::open(&worktree.path, store.abbreviation()).ok()
            }
        })
        .filter_map(|worktree| worktree.task_ids().ok())
        .flatten()
        .max()
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
            Store::open(&worktree.path, store.abbreviation()).ok()
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
            metadata: self.metadata.clone(),
        }
    }
}

fn parse_version(bytes: &[u8], abbreviation: Abbreviation) -> Version {
    let text = String::from_utf8_lossy(bytes);
    let partial = op_task::parse_partial(&text);
    let title = partial.title.unwrap_or_default();
    let metadata = Metadata::from_partial(partial.metadata, abbreviation);
    Version {
        haystack: haystack(&title, &partial.body, &metadata),
        comment_count: op_task::comment::parse(&partial.body).len(),
        title,
        metadata,
    }
}

fn haystack(title: &str, body: &str, metadata: &Metadata) -> Haystack {
    let mut rest = format!("{body}\n");
    if let Some(status) = metadata.status() {
        rest.push_str(status.as_str());
        rest.push('\n');
    }
    if let Some(parent) = metadata.parent() {
        rest.push_str(parent);
        rest.push('\n');
    }
    for dependency in metadata.dependencies() {
        rest.push_str(dependency);
        rest.push('\n');
    }
    Haystack {
        title: title.to_lowercase(),
        rest: rest.to_lowercase(),
    }
}

fn task_ref(item: &TaskListItem) -> TaskRef {
    TaskRef {
        id: item.id.clone(),
        title: item.title.clone(),
        status: item.metadata.status_field(),
    }
}

// A dependency may aim at a section (`OPP-42#Design`), which names the same task as the bare key.
fn depends_on_id(item: &TaskListItem, id: &str) -> bool {
    item.metadata
        .dependencies()
        .iter()
        .any(|entry| op_task::ref_target(entry) == id)
}

// Every `[[…]]` in `body` that resolves to a known task, deduplicated in first-seen order.
// Unresolvable references are skipped — the client renders those as a dangling chip anyway, so they
// need no metadata.
fn body_refs(
    abbreviation: Abbreviation,
    body: &str,
    by_id: &HashMap<&str, &TaskListItem>,
) -> Vec<TaskRef> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();
    for (_, inner) in op_task::body_ref_spans(body) {
        let Some(number) = op_task::body_ref_id(abbreviation, inner) else {
            continue;
        };
        let key = abbreviation.format_key(number);
        if let Some(item) = by_id.get(key.as_str()) {
            if seen.insert(key) {
                refs.push(task_ref(item));
            }
        }
    }
    refs
}

// The comment log as the wire carries it. Every read splits it out of the body it lives in, so one
// parser answers for it and no client renders the thread twice.
pub fn comments_of(body: &str) -> Vec<Comment> {
    op_task::comment::parse(body)
        .iter()
        .map(Comment::from)
        .collect()
}

fn view_from_raw(
    id: &str,
    raw: &str,
    updated: FieldResult<Timestamp>,
    abbreviation: Abbreviation,
) -> TaskView {
    let partial = op_task::parse_partial(raw);
    let metadata = Metadata::from_partial(partial.metadata, abbreviation);
    let created = metadata.created();
    TaskView {
        id: id.to_owned(),
        title: partial.title.unwrap_or_default(),
        updated: updated_field(created, updated),
        metadata,
        body: partial.body,
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
