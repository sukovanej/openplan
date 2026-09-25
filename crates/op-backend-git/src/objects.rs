use std::cmp::Ordering;
use std::collections::BTreeMap;

use gix::ObjectId;
use gix::objs::tree::{EntryKind, EntryRef};
use gix::refs::Target;
use gix::refs::transaction::{Change as RefChange, LogChange, PreviousValue, RefEdit, RefLog};
use op_backend::{
    Actor, BackendError, Change, ChangeKind, Op, Revision, RevisionId, Snapshot, Timestamp,
};

const VIA: &str = "Via: ";

pub(crate) type Entries = BTreeMap<String, ObjectId>;

pub(crate) struct GitSnapshot {
    repo: gix::ThreadSafeRepository,
    revision: Option<RevisionId>,
    pub(crate) tree: Option<ObjectId>,
    pub(crate) entries: Entries,
}

impl GitSnapshot {
    pub fn of(
        repo: &gix::ThreadSafeRepository,
        commit: Option<ObjectId>,
    ) -> Result<Self, BackendError> {
        let local = repo.to_thread_local();
        let tree = commit.map(|id| tree_of(&local, id)).transpose()?;
        let entries = match tree {
            Some(tree) => flatten(&local, tree, "")?,
            None => Entries::new(),
        };
        Ok(Self {
            repo: repo.clone(),
            revision: commit.map(revision_id),
            tree,
            entries,
        })
    }
}

impl Snapshot for GitSnapshot {
    fn revision(&self) -> Option<&RevisionId> {
        self.revision.as_ref()
    }

    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, BackendError> {
        let Some(id) = self.entries.get(path) else {
            return Ok(None);
        };
        let repo = self.repo.to_thread_local();
        let blob = repo.find_blob(*id).map_err(storage)?;
        Ok(Some(blob.data.clone()))
    }

    fn files(&self) -> Result<Vec<String>, BackendError> {
        Ok(self.entries.keys().cloned().collect())
    }
}

pub(crate) fn revision_id(id: ObjectId) -> RevisionId {
    RevisionId::new(id.to_string())
}

pub(crate) fn object_id(revision: &RevisionId) -> Result<ObjectId, BackendError> {
    ObjectId::from_hex(revision.as_str().as_bytes())
        .map_err(|_| BackendError::UnknownRevision(revision.clone()))
}

pub(crate) fn tip(
    repo: &gix::Repository,
    reference: &str,
) -> Result<Option<ObjectId>, BackendError> {
    let Some(mut found) = repo.try_find_reference(reference).map_err(storage)? else {
        return Ok(None);
    };
    let id = found.peel_to_id().map_err(storage)?;
    Ok(Some(id.detach()))
}

pub(crate) fn tree_of(repo: &gix::Repository, commit: ObjectId) -> Result<ObjectId, BackendError> {
    let commit = repo.find_commit(commit).map_err(storage)?;
    Ok(commit.tree_id().map_err(storage)?.detach())
}

// Symlinks and submodules name no document, so they are left out rather than read as one.
pub(crate) fn flatten(
    repo: &gix::Repository,
    tree: ObjectId,
    prefix: &str,
) -> Result<Entries, BackendError> {
    let mut entries = Entries::new();
    collect(repo, tree, prefix, &mut entries)?;
    Ok(entries)
}

fn collect(
    repo: &gix::Repository,
    tree: ObjectId,
    prefix: &str,
    entries: &mut Entries,
) -> Result<(), BackendError> {
    let tree = repo.find_tree(tree).map_err(storage)?;
    for entry in tree.decode().map_err(storage)?.entries.iter() {
        let path = format!("{prefix}{}", entry.filename);
        match entry.mode.kind() {
            EntryKind::Tree => collect(repo, entry.oid.to_owned(), &format!("{path}/"), entries)?,
            EntryKind::Blob | EntryKind::BlobExecutable => {
                entries.insert(path, entry.oid.to_owned());
            }
            EntryKind::Link | EntryKind::Commit => {}
        }
    }
    Ok(())
}

pub(crate) fn subtree(
    repo: &gix::Repository,
    root: ObjectId,
    dir: &str,
) -> Result<Option<ObjectId>, BackendError> {
    let tree = repo.find_tree(root).map_err(storage)?;
    let Some(entry) = tree.lookup_entry_by_path(dir).map_err(storage)? else {
        return Ok(None);
    };
    Ok(entry.mode().is_tree().then(|| entry.object_id()))
}

// Compares object ids, so a subtree both sides share is never read.
pub(crate) fn tree_changes(
    repo: &gix::Repository,
    from: Option<ObjectId>,
    to: Option<ObjectId>,
) -> Result<Vec<Change>, BackendError> {
    let mut changes = Vec::new();
    diff_trees(repo, from, to, "", &mut changes)?;
    changes.sort();
    Ok(changes)
}

fn diff_trees(
    repo: &gix::Repository,
    from: Option<ObjectId>,
    to: Option<ObjectId>,
    prefix: &str,
    changes: &mut Vec<Change>,
) -> Result<(), BackendError> {
    if from == to {
        return Ok(());
    }
    let find = |id: Option<ObjectId>| id.map(|id| repo.find_tree(id)).transpose();
    let (old, new) = (find(from).map_err(storage)?, find(to).map_err(storage)?);
    let (old, new) = (documents(old.as_ref())?, documents(new.as_ref())?);
    let (mut old, mut new) = (old.iter().peekable(), new.iter().peekable());
    loop {
        let order = match (old.peek(), new.peek()) {
            (None, None) => return Ok(()),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(a), Some(b)) => git_order(a, b),
        };
        let (a, b) = match order {
            Ordering::Less => (old.next(), None),
            Ordering::Greater => (None, new.next()),
            Ordering::Equal => (old.next(), new.next()),
        };
        compare(repo, a, b, prefix, changes)?;
    }
}

// Two entries of one name are both trees or both blobs: `git_order` sorts a tree as its name and a
// slash, so a blob that became a tree reads as one removed and one added path.
fn compare(
    repo: &gix::Repository,
    old: Option<&EntryRef<'_>>,
    new: Option<&EntryRef<'_>>,
    prefix: &str,
    changes: &mut Vec<Change>,
) -> Result<(), BackendError> {
    let Some(entry) = new.or(old) else {
        return Ok(());
    };
    let path = format!("{prefix}{}", entry.filename);
    if entry.mode.is_tree() {
        let id = |entry: Option<&EntryRef<'_>>| entry.map(|entry| entry.oid.to_owned());
        return diff_trees(repo, id(old), id(new), &format!("{path}/"), changes);
    }
    let kind = match (old, new) {
        (None, Some(_)) => ChangeKind::Added,
        (Some(_), None) => ChangeKind::Removed,
        (Some(old), Some(new)) if old.oid != new.oid => ChangeKind::Modified,
        _ => return Ok(()),
    };
    changes.push(Change::new(path, kind));
    Ok(())
}

// Symlinks and submodules name no document, as in `flatten`.
fn documents<'a>(tree: Option<&'a gix::Tree<'_>>) -> Result<Vec<EntryRef<'a>>, BackendError> {
    let Some(tree) = tree else {
        return Ok(Vec::new());
    };
    let mut entries: Vec<EntryRef<'a>> = tree
        .decode()
        .map_err(storage)?
        .entries
        .into_iter()
        .filter(|entry| {
            matches!(
                entry.mode.kind(),
                EntryKind::Tree | EntryKind::Blob | EntryKind::BlobExecutable
            )
        })
        .collect();
    entries.sort_by(git_order);
    Ok(entries)
}

fn git_order(a: &EntryRef<'_>, b: &EntryRef<'_>) -> Ordering {
    sort_key(a).cmp(sort_key(b))
}

fn sort_key<'a>(entry: &EntryRef<'a>) -> impl Iterator<Item = u8> + 'a {
    let slash = entry.mode.is_tree().then_some(b'/');
    entry.filename.iter().copied().chain(slash)
}

// The blob a revision holds at `path`, or `None` where it holds no document there.
pub(crate) fn blob_at(
    repo: &gix::Repository,
    tree: ObjectId,
    path: &str,
) -> Result<Option<ObjectId>, BackendError> {
    let tree = repo.find_tree(tree).map_err(storage)?;
    let entry = tree.lookup_entry_by_path(path).map_err(storage)?;
    Ok(entry
        .filter(|entry| {
            matches!(
                entry.mode().kind(),
                EntryKind::Blob | EntryKind::BlobExecutable
            )
        })
        .map(|entry| entry.object_id()))
}

pub(crate) fn changes(from: &Entries, to: &Entries) -> Vec<Change> {
    let mut out = Vec::new();
    for (path, id) in from {
        match to.get(path) {
            None => out.push(Change::new(path.clone(), ChangeKind::Removed)),
            Some(other) if other != id => out.push(Change::new(path.clone(), ChangeKind::Modified)),
            Some(_) => {}
        }
    }
    for path in to.keys() {
        if !from.contains_key(path) {
            out.push(Change::new(path.clone(), ChangeKind::Added));
        }
    }
    out.sort();
    out
}

pub(crate) struct Written {
    pub tree: ObjectId,
    pub changes: Vec<Change>,
}

pub(crate) fn write_tree(
    repo: &gix::Repository,
    base: &GitSnapshot,
    ops: &[Op],
) -> Result<Written, BackendError> {
    let mut entries = base.entries.clone();
    let root = base
        .tree
        .unwrap_or_else(|| ObjectId::empty_tree(repo.object_hash()));
    let mut editor = repo.edit_tree(root).map_err(storage)?;
    for op in ops {
        op_backend::check_path(op.path())?;
        match op {
            Op::Put { path, bytes } => {
                let id = repo.write_blob(bytes).map_err(storage)?.detach();
                editor
                    .upsert(path.as_str(), EntryKind::Blob, id)
                    .map_err(storage)?;
                entries.insert(path.clone(), id);
            }
            Op::Remove { path } => {
                if entries.remove(path).is_some() {
                    editor.remove(path.as_str()).map_err(storage)?;
                }
            }
        }
    }
    let tree = editor.write().map_err(storage)?.detach();
    Ok(Written {
        tree,
        changes: changes(&base.entries, &entries),
    })
}

pub(crate) fn write_commit(
    repo: &gix::Repository,
    tree: ObjectId,
    parents: &[ObjectId],
    author: &Actor,
    at: Timestamp,
    message: &str,
) -> Result<ObjectId, BackendError> {
    let signature = signature(author, at);
    let commit = gix::objs::Commit {
        tree,
        parents: parents.iter().copied().collect(),
        author: signature.clone(),
        committer: signature,
        encoding: None,
        message: full_message(message, author).into(),
        extra_headers: Vec::new(),
    };
    Ok(repo.write_object(&commit).map_err(storage)?.detach())
}

fn signature(author: &Actor, at: Timestamp) -> gix::actor::Signature {
    gix::actor::Signature {
        name: author.name.as_str().into(),
        email: author.email.as_deref().unwrap_or_default().into(),
        time: gix::date::Time {
            seconds: at.as_second(),
            offset: 0,
        },
    }
}

fn full_message(message: &str, author: &Actor) -> String {
    match &author.via {
        Some(agent) => format!("{}\n\n{VIA}{agent}\n", message.trim_end()),
        None => format!("{}\n", message.trim_end()),
    }
}

pub(crate) fn read_revision(
    repo: &gix::Repository,
    id: ObjectId,
) -> Result<Revision, BackendError> {
    let commit = repo.find_commit(id).map_err(storage)?;
    let decoded = commit.decode().map_err(storage)?;
    let author = decoded.author().map_err(storage)?;
    let seconds = author.time().map_err(storage)?.seconds;
    let text = decoded.message.to_string();
    let (message, via) = split_via(text.trim_end());
    let email = author.email.to_string();
    Ok(Revision {
        id: revision_id(id),
        parents: decoded.parents().map(revision_id).collect(),
        author: Actor {
            name: author.name.to_string(),
            email: (!email.is_empty()).then_some(email),
            via,
        },
        at: Timestamp::from_second(seconds).map_err(storage)?,
        message,
    })
}

fn split_via(text: &str) -> (String, Option<String>) {
    match text.rsplit_once("\n\n") {
        Some((message, trailer)) if trailer.starts_with(VIA) && !trailer.contains('\n') => (
            message.to_owned(),
            Some(trailer[VIA.len()..].trim().to_owned()),
        ),
        _ => (text.to_owned(), None),
    }
}

// `false` when the reference no longer holds `expected`, so the caller can read it again and retry.
pub(crate) fn move_reference(
    repo: &gix::Repository,
    reference: &str,
    expected: Option<ObjectId>,
    new: ObjectId,
    committer: &Actor,
    message: &str,
) -> Result<bool, BackendError> {
    let edit = RefEdit {
        change: RefChange::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: format!("openplan: {}", first_line(message)).into(),
            },
            expected: match expected {
                Some(id) => PreviousValue::MustExistAndMatch(Target::Object(id)),
                None => PreviousValue::MustNotExist,
            },
            new: Target::Object(new),
        },
        name: reference.try_into().map_err(storage)?,
        deref: false,
    };
    let signature = signature(committer, op_backend::now());
    let mut time = gix::date::parse::TimeBuf::default();
    match repo.edit_references_as(Some(edit), Some(signature.to_ref(&mut time))) {
        Ok(_) => Ok(true),
        Err(err) if is_lock_contention(&err) => Ok(false),
        Err(err) => match tip(repo, reference)? == expected {
            true => Err(storage(err)),
            false => Ok(false),
        },
    }
}

fn is_lock_contention(err: &gix::reference::edit::Error) -> bool {
    use std::error::Error as _;
    let mut source: Option<&dyn std::error::Error> = err.source();
    while let Some(cause) = source {
        if cause.to_string().contains("lock") {
            return true;
        }
        source = cause.source();
    }
    err.to_string().contains("lock")
}

// Other processes retry the same reference, and a random pause keeps them from colliding again.
pub(crate) fn pause_before_retry() {
    let jitter = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos() % 20);
    std::thread::sleep(std::time::Duration::from_millis(1 + u64::from(jitter)));
}

pub(crate) fn set_reference(
    repo: &gix::Repository,
    reference: &str,
    new: ObjectId,
) -> Result<(), BackendError> {
    let edit = RefEdit {
        change: RefChange::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: "openplan: sync".into(),
            },
            expected: PreviousValue::Any,
            new: Target::Object(new),
        },
        name: reference.try_into().map_err(storage)?,
        deref: false,
    };
    let signature = signature(&Actor::new("openplan"), op_backend::now());
    let mut time = gix::date::parse::TimeBuf::default();
    repo.edit_references_as(Some(edit), Some(signature.to_ref(&mut time)))
        .map_err(storage)?;
    Ok(())
}

fn first_line(message: &str) -> &str {
    message.lines().next().unwrap_or_default()
}

pub(crate) fn storage(err: impl std::fmt::Display) -> BackendError {
    BackendError::storage(err)
}
