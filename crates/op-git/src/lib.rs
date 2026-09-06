use std::cmp::Ordering;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use jiff::Timestamp;

const OBJECT_CACHE_BYTES: usize = 8 * 1024 * 1024;
// A safety valve on the last-change walk: an id whose change lies deeper than this stays undated,
// which reports as no `updated` and, for ranking, as older than any dated branch.
const HISTORY_WALK_BUDGET: usize = 4096;

// When a task's last change was authored, or why the commit that made it cannot say. Author time,
// not commit time: a rebase, squash, or amend rewrites the commit date of everything it replays, so
// ranking or dating by that makes the most recently rebased branch look like the newest work on
// every task it carries — including the ones it never touched.
pub type ChangeTime = Result<Timestamp, String>;

#[derive(Debug, Clone)]
pub struct TaskChange {
    pub commit: String,
    pub at: ChangeTime,
}

#[derive(Debug, Clone)]
pub struct Repo {
    inner: gix::ThreadSafeRepository,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub op_in_progress: bool,
}

pub mod rolling_updates;

pub use rolling_updates::{ROLLING_UPDATES_BRANCH, Rebased};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("could not open git repository: {0}")]
    Open(String),
    #[error("{0}")]
    Command(String),
    #[error("{branch} cannot fast-forward there")]
    NotFastForward { branch: String },
    #[error("{} has uncommitted task changes", path.display())]
    WorktreeDirty { path: std::path::PathBuf },
    #[error("no such branch: {0}")]
    NoSuchBranch(String),
    #[error("object database error: {0}")]
    Object(String),
}

impl Repo {
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, GitError> {
        let inner = gix::ThreadSafeRepository::discover(path.as_ref())
            .map_err(|e| GitError::Open(e.to_string()))?;
        Ok(Self { inner })
    }

    fn repo(&self) -> gix::Repository {
        self.inner.to_thread_local()
    }

    pub fn user_name(&self) -> Option<String> {
        Some(
            self.repo()
                .config_snapshot()
                .string("user.name")?
                .to_string(),
        )
    }

    pub fn local_branches(&self) -> Result<Vec<String>, GitError> {
        let repo = self.repo();
        let platform = repo
            .references()
            .map_err(|e| GitError::Open(e.to_string()))?;
        let iter = platform
            .prefixed("refs/heads/")
            .map_err(|e| GitError::Open(e.to_string()))?;
        let mut names = Vec::new();
        for reference in iter {
            let reference = reference.map_err(|e| GitError::Open(e.to_string()))?;
            names.push(reference.name().shorten().to_string());
        }
        Ok(names)
    }

    // The branch this worktree has checked out; `None` on a detached HEAD.
    pub fn current_branch(&self) -> Option<String> {
        self.repo()
            .head_name()
            .ok()
            .flatten()
            .map(|name| name.shorten().to_string())
    }

    pub fn op_in_progress(&self) -> bool {
        op_markers_present(self.repo().git_dir())
    }

    // The shared `.git` directory (identical for the main checkout and every linked worktree), where
    // `refs/`, `packed-refs`, `HEAD`, and `worktrees/` live — the git-side change sources to watch.
    pub fn git_common_dir(&self) -> PathBuf {
        self.repo().common_dir().to_path_buf()
    }

    // The main checkout's working copy — the one worktree a repository cannot lose, unlike the
    // linked worktrees that come and go. `None` for a bare repository.
    pub fn main_worktree(&self) -> Option<PathBuf> {
        worktree_of(&self.repo().main_repo().ok()?).map(|worktree| worktree.path)
    }

    // Every worktree (main + linked) with a working copy on disk, paired with its checked-out
    // branch. A branch appears at most once — git allows it checked out in one worktree only.
    pub fn worktrees(&self) -> Result<Vec<Worktree>, GitError> {
        let repo = self.repo();
        let mut out = Vec::new();
        if let Ok(main) = repo.main_repo() {
            out.extend(worktree_of(&main));
        }
        for proxy in repo
            .worktrees()
            .map_err(|e| GitError::Open(e.to_string()))?
        {
            if let Ok(linked) = proxy.into_repo_with_possibly_inaccessible_worktree() {
                out.extend(worktree_of(&linked));
            }
        }
        Ok(out)
    }

    pub fn worktree_branch(&self, path: &Path) -> Result<Option<String>, GitError> {
        Ok(self
            .worktrees()?
            .into_iter()
            .find(|worktree| same_path(&worktree.path, path))
            .and_then(|worktree| worktree.branch))
    }

    pub fn branch_task_blobs(&self, branch: &str) -> Result<Vec<(String, String)>, GitError> {
        let repo = self.repo();
        let tree = repo
            .try_find_reference(format!("refs/heads/{branch}").as_str())
            .map_err(|e| GitError::Open(e.to_string()))?
            .ok_or_else(|| GitError::NoSuchBranch(branch.to_owned()))?
            .peel_to_tree()
            .map_err(|e| GitError::Object(e.to_string()))?;
        task_blobs(&tree)
    }

    pub fn commit_task_blobs(&self, commit_hex: &str) -> Result<Vec<(String, String)>, GitError> {
        let oid = object_id(commit_hex)?;
        let repo = self.repo();
        let tree = repo
            .find_object(oid)
            .map_err(|e| GitError::Object(e.to_string()))?
            .peel_to_tree()
            .map_err(|e| GitError::Object(e.to_string()))?;
        task_blobs(&tree)
    }

    // The newest commit on `branch` that changed each of `ids`, found by walking history newest-first
    // and stopping once every id is dated. Bounded to the requested ids — which the caller draws from
    // the branch's cells — so a walk for a short branch tip need not descend the whole default-branch
    // history.
    pub fn task_changes(
        &self,
        branch: &str,
        ids: &HashSet<String>,
    ) -> Result<HashMap<String, TaskChange>, GitError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let tip = object_id(&self.branch_commit(branch)?)?;
        let mut repo = self.repo();
        repo.object_cache_size_if_unset(OBJECT_CACHE_BYTES);

        let mut needed: HashSet<&str> = ids.iter().map(String::as_str).collect();
        let mut changes = HashMap::new();
        let walk = repo
            .rev_walk([tip])
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            ))
            .all()
            .map_err(|e| GitError::Object(e.to_string()))?;

        for (visited, info) in walk.enumerate() {
            if needed.is_empty() || visited >= HISTORY_WALK_BUDGET {
                break;
            }
            let info = info.map_err(|e| GitError::Object(e.to_string()))?;
            let this = commit_task_map(&repo, info.id)?;
            let parents: Vec<HashMap<String, String>> = info
                .parent_ids
                .iter()
                .map(|parent| commit_task_map(&repo, *parent))
                .collect::<Result<_, _>>()?;
            // Read once per commit, and only for a commit that changes something asked for: a
            // commit touching no task must not be able to fail the walk with its own header.
            let mut when = None;
            needed.retain(|id| match this.get(*id) {
                // Every parent, not just the first: a merge carrying a task version unchanged from
                // either side did not edit it, and crediting the merge would restamp every task the
                // merged branch touched. Only a version matching no parent — a conflict resolved by
                // hand — is the merge's own change.
                Some(blob) if parents.iter().all(|parent| parent.get(*id) != Some(blob)) => {
                    let at = when.get_or_insert_with(|| author_time(&repo, info.id));
                    changes.insert(
                        (*id).to_owned(),
                        TaskChange {
                            commit: info.id.to_string(),
                            at: at.clone(),
                        },
                    );
                    false
                }
                _ => true,
            });
        }
        Ok(changes)
    }

    pub fn branch_commit(&self, branch: &str) -> Result<String, GitError> {
        let repo = self.repo();
        let id = repo
            .try_find_reference(format!("refs/heads/{branch}").as_str())
            .map_err(|e| GitError::Open(e.to_string()))?
            .ok_or_else(|| GitError::NoSuchBranch(branch.to_owned()))?
            .peel_to_id()
            .map_err(|e| GitError::Object(e.to_string()))?;
        Ok(id.detach().to_string())
    }

    // All common ancestors of each branch commit with `target`, resolved in one graph walk shared
    // across every branch (with an object cache) rather than a fresh walk per call. A branch with
    // no shared history yields an empty list, so its tasks all read as newly added; a criss-cross
    // history yields several bases, and the caller treats a task as unchanged if it matches any.
    pub fn merge_bases_against(
        &self,
        target_hex: &str,
        branch_hexes: &[String],
    ) -> Result<Vec<Vec<String>>, GitError> {
        let target = object_id(target_hex)?;
        let mut repo = self.repo();
        repo.object_cache_size_if_unset(OBJECT_CACHE_BYTES);
        let cache = repo
            .commit_graph_if_enabled()
            .map_err(|e| GitError::Object(e.to_string()))?;
        let mut graph = repo.revision_graph(cache.as_ref());
        let others = [target];
        let mut out = Vec::with_capacity(branch_hexes.len());
        for hex in branch_hexes {
            let commit = object_id(hex)?;
            let bases = repo
                .merge_bases_many_with_graph(commit, &others, &mut graph)
                .map_err(|e| GitError::Object(e.to_string()))?;
            out.push(
                bases
                    .into_iter()
                    .map(|id| id.detach().to_string())
                    .collect(),
            );
        }
        Ok(out)
    }

    // Which commit of each pair contains the other: `Less` when the first is an ancestor of the
    // second, `Greater` for the reverse, `None` when neither reaches the other (nor when they are
    // the same commit, which supersedes nothing). One graph walk is shared across every pair.
    pub fn ancestry(&self, pairs: &[(&str, &str)]) -> Result<Vec<Option<Ordering>>, GitError> {
        if pairs.is_empty() {
            return Ok(Vec::new());
        }
        let mut repo = self.repo();
        repo.object_cache_size_if_unset(OBJECT_CACHE_BYTES);
        let cache = repo
            .commit_graph_if_enabled()
            .map_err(|e| GitError::Object(e.to_string()))?;
        let mut graph = repo.revision_graph(cache.as_ref());
        let mut out = Vec::with_capacity(pairs.len());
        for (first_hex, second_hex) in pairs {
            let first = object_id(first_hex)?;
            let second = object_id(second_hex)?;
            if first == second {
                out.push(None);
                continue;
            }
            let bases: Vec<gix::ObjectId> = repo
                .merge_bases_many_with_graph(first, &[second], &mut graph)
                .map_err(|e| GitError::Object(e.to_string()))?
                .into_iter()
                .map(|id| id.detach())
                .collect();
            out.push(match (bases.contains(&first), bases.contains(&second)) {
                (true, false) => Some(Ordering::Less),
                (false, true) => Some(Ordering::Greater),
                _ => None,
            });
        }
        Ok(out)
    }

    // The branch the task view treats as the merge target: `configured` when it names a real local
    // branch, else `main`, else `master`, else none.
    pub fn default_branch(&self, configured: Option<&str>) -> Result<Option<String>, GitError> {
        if let Some(name) = configured {
            // A configured name is typed by hand, and one that cannot spell a ref at all — a space,
            // a `*`, a trailing `/` — makes the lookup itself fail rather than come back empty. It
            // still names no branch, so it falls back like any other name no branch carries.
            if self.branch_exists(name).unwrap_or(false) {
                return Ok(Some(name.to_owned()));
            }
        }
        for candidate in ["main", "master"] {
            if self.branch_exists(candidate)? {
                return Ok(Some(candidate.to_owned()));
            }
        }
        Ok(None)
    }

    fn branch_exists(&self, branch: &str) -> Result<bool, GitError> {
        Ok(self
            .repo()
            .try_find_reference(format!("refs/heads/{branch}").as_str())
            .map_err(|e| GitError::Open(e.to_string()))?
            .is_some())
    }

    // The commit that first added `path` (repo-relative), with its author time — the answer
    // `lint --fix` backfills a missing `created:` from, and the one `StoreError::MissingCreated`
    // already tells a reader to look up by hand. `None` for a path no commit has ever added.
    pub fn first_commit(&self, path: &Path) -> Result<Option<TaskChange>, GitError> {
        let mut repo = self.repo();
        repo.object_cache_size_if_unset(OBJECT_CACHE_BYTES);
        let Ok(head) = repo.head_id() else {
            return Ok(None);
        };
        let walk = repo
            .rev_walk([head.detach()])
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            ))
            .all()
            .map_err(|e| GitError::Object(e.to_string()))?;
        // The whole history, not the first match: a path deleted and later restored is added by
        // several commits, and a newest-first walk meets the restore first. The creation is the
        // oldest add, so each match overwrites the last — as in `git log --diff-filter=A`, whose
        // final line is the answer.
        let mut oldest_add = None;
        for info in walk {
            let info = info.map_err(|e| GitError::Object(e.to_string()))?;
            if !tree_has_path(&repo, info.id, path)? {
                continue;
            }
            let mut added = true;
            for parent in &info.parent_ids {
                if tree_has_path(&repo, *parent, path)? {
                    added = false;
                    break;
                }
            }
            if added {
                oldest_add = Some(TaskChange {
                    commit: info.id.to_string(),
                    at: author_time(&repo, info.id),
                });
            }
        }
        Ok(oldest_add)
    }

    pub fn read_blob(&self, oid_hex: &str) -> Result<Vec<u8>, GitError> {
        let oid = object_id(oid_hex)?;
        let repo = self.repo();
        let object = repo
            .find_object(oid)
            .map_err(|e| GitError::Object(e.to_string()))?;
        Ok(object.detach().data)
    }

    pub fn hash_blob(&self, bytes: &[u8]) -> Result<String, GitError> {
        let oid = gix::objs::compute_hash(self.repo().object_hash(), gix::objs::Kind::Blob, bytes)
            .map_err(|e| GitError::Object(e.to_string()))?;
        Ok(oid.to_string())
    }
}

// Git stores an author date as a bare i64 and validates nothing, so a clock-skewed machine or an
// importer can leave one no calendar can express. Such a commit dates nothing — reported, not
// raised: it must cost the tasks it changed their `updated`, and nothing else its own.
fn author_time(repo: &gix::Repository, commit: gix::ObjectId) -> Result<Timestamp, String> {
    let object = repo
        .find_object(commit)
        .map_err(|e| format!("commit {commit} is unreadable: {e}"))?
        .into_commit();
    let author = object
        .author()
        .map_err(|e| format!("commit {commit} has an unreadable author: {e}"))?;
    Timestamp::from_second(author.seconds())
        .map_err(|e| format!("commit {commit} has an unusable author date: {e}"))
}

fn tree_has_path(
    repo: &gix::Repository,
    commit: gix::ObjectId,
    path: &Path,
) -> Result<bool, GitError> {
    let tree = repo
        .find_object(commit)
        .map_err(|e| GitError::Object(e.to_string()))?
        .peel_to_tree()
        .map_err(|e| GitError::Object(e.to_string()))?;
    Ok(tree
        .lookup_entry_by_path(path)
        .map_err(|e| GitError::Object(e.to_string()))?
        .is_some_and(|entry| entry.mode().is_blob()))
}

fn object_id(hex: &str) -> Result<gix::ObjectId, GitError> {
    gix::ObjectId::from_hex(hex.as_bytes()).map_err(|e| GitError::Object(e.to_string()))
}

fn commit_task_map(
    repo: &gix::Repository,
    commit: gix::ObjectId,
) -> Result<HashMap<String, String>, GitError> {
    let tree = repo
        .find_object(commit)
        .map_err(|e| GitError::Object(e.to_string()))?
        .peel_to_tree()
        .map_err(|e| GitError::Object(e.to_string()))?;
    Ok(task_blobs(&tree)?.into_iter().collect())
}

fn task_blobs(tree: &gix::Tree) -> Result<Vec<(String, String)>, GitError> {
    let Some(entry) = tree
        .lookup_entry_by_path(".plan/tasks")
        .map_err(|e| GitError::Object(e.to_string()))?
    else {
        return Ok(Vec::new());
    };
    if !entry.mode().is_tree() {
        return Ok(Vec::new());
    }
    let subtree = entry
        .object()
        .map_err(|e| GitError::Object(e.to_string()))?
        .into_tree();
    // Keyed by id, not by file name: a task renamed on one branch (the name carries a title slug)
    // is the same task, and the matrix must line its versions up rather than read them as two.
    // Two names of one number is a hand-made state; the lowest wins, as it does in the store, so a
    // read and a write cannot land on different files.
    let mut found: BTreeMap<u64, (String, String)> = BTreeMap::new();
    for entry in subtree.iter() {
        let entry = entry.map_err(|e| GitError::Object(e.to_string()))?;
        if !entry.mode().is_blob() {
            continue;
        }
        let name = entry.filename().to_string();
        let Some(number) = name.strip_suffix(".md").and_then(op_task::file_id) else {
            continue;
        };
        let blob = (name, entry.oid().to_string());
        match found.entry(number) {
            Entry::Vacant(slot) => {
                slot.insert(blob);
            }
            Entry::Occupied(mut slot) if blob.0 < slot.get().0 => {
                slot.insert(blob);
            }
            Entry::Occupied(_) => {}
        }
    }
    Ok(found
        .into_iter()
        .map(|(number, (_, oid))| (number.to_string(), oid))
        .collect())
}

fn worktree_of(repo: &gix::Repository) -> Option<Worktree> {
    let path = repo.workdir()?.to_path_buf();
    if !path.is_dir() {
        return None;
    }
    let branch = repo
        .head_name()
        .ok()
        .flatten()
        .map(|name| name.shorten().to_string());
    Some(Worktree {
        path,
        branch,
        op_in_progress: op_markers_present(repo.git_dir()),
    })
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn op_markers_present(git_dir: &Path) -> bool {
    ["MERGE_HEAD", "rebase-merge", "rebase-apply"]
        .iter()
        .any(|marker| git_dir.join(marker).exists())
}
