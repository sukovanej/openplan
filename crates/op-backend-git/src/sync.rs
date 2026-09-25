use std::path::Path;
use std::process::{Command, Stdio};

use gix::ObjectId;
use op_backend::{BackendError, BackendEvent, Origin, SyncReport, SyncStatus, Tips};

use crate::objects::{self, GitSnapshot};
use crate::{Inner, TASKS_NAME, TASKS_REF, lock, tracking_reference};

const ATTEMPTS: usize = 5;

enum Pushed {
    Accepted,
    Rejected,
}

pub(crate) fn run(inner: &Inner) -> Result<SyncReport, BackendError> {
    let _syncing = lock(&inner.syncing);
    let Some(remote) = inner.remote.as_deref() else {
        return Err(BackendError::sync(
            "this repository has no remote to sync with",
        ));
    };
    let started = op_backend::now();
    let result = exchange(inner, remote);
    let previous = lock(&inner.status).clone();
    let status = match &result {
        Ok(_) => SyncStatus {
            remote: remote.to_owned(),
            last_attempt: Some(started),
            last_success: Some(started),
            ahead: 0,
            behind: 0,
            error: None,
        },
        Err(err) => {
            let (ahead, behind) = divergence(inner, remote).unwrap_or((0, 0));
            SyncStatus {
                remote: remote.to_owned(),
                last_attempt: Some(started),
                last_success: previous.last_success,
                ahead,
                behind,
                error: Some(err.to_string()),
            }
        }
    };
    *lock(&inner.status) = status.clone();
    inner.events.send(BackendEvent::Sync(status));
    result
}

fn exchange(inner: &Inner, remote: &str) -> Result<SyncReport, BackendError> {
    let tracking = tracking_reference(remote);
    let mut report = SyncReport::default();
    for _ in 0..ATTEMPTS {
        let theirs = fetch(inner, remote, &tracking)?;
        let Some(ours) = integrate(inner, remote, theirs, &mut report)? else {
            return Ok(report);
        };
        if Some(ours) == theirs {
            return Ok(report);
        }
        match push(inner, remote, ours)? {
            Pushed::Accepted => {
                report.sent += count(&inner.local(), ours, theirs)?;
                objects::set_reference(&inner.local(), &tracking, ours)?;
                return Ok(report);
            }
            Pushed::Rejected => continue,
        }
    }
    Err(BackendError::sync(
        "the remote tasks kept moving while this sync ran; the next sync tries again",
    ))
}

// Brings the remote's commits into the local tasks: a fast-forward where one side contains the
// other, a merge commit where both moved. Returns the local tip afterwards.
fn integrate(
    inner: &Inner,
    remote: &str,
    theirs: Option<ObjectId>,
    report: &mut SyncReport,
) -> Result<Option<ObjectId>, BackendError> {
    let Some(theirs) = theirs else {
        return inner.tip();
    };
    let _writing = lock(&inner.writing);
    let repo = inner.local();
    for _ in 0..ATTEMPTS {
        let ours = inner.tip()?;
        inner.catch_up(ours)?;
        let base = match ours {
            Some(ours) => merge_base(&repo, ours, theirs)?,
            None => None,
        };
        if ours == Some(theirs) || (base == Some(theirs)) {
            return Ok(ours);
        }
        let fast_forward = ours.is_none() || base == ours;
        let (next, message) = match fast_forward {
            true => (theirs, "Fast-forward to the remote tasks".to_owned()),
            false => {
                let ours = ours.expect("diverged tasks have a tip");
                let (merged, notes) = merge(inner, &repo, base, ours, theirs)?;
                let mut message = format!("Merge {remote} {TASKS_NAME}");
                if !notes.is_empty() {
                    message = format!("{message}\n\n{}", notes.join("\n"));
                }
                let commit = objects::write_commit(
                    &repo,
                    merged,
                    &[ours, theirs],
                    &inner.machine,
                    op_backend::now(),
                    &message,
                )?;
                (commit, message)
            }
        };
        if !objects::move_reference(&repo, TASKS_REF, ours, next, &inner.machine, &message)? {
            continue;
        }
        let changes = inner.changes_between(ours, Some(next))?;
        report.received += count(&repo, theirs, ours)?;
        report.merged |= !fast_forward;
        report.changes = changes.clone();
        inner.announce(Some(next), changes, Origin::Remote)?;
        return Ok(Some(next));
    }
    Err(BackendError::Contended)
}

fn merge(
    inner: &Inner,
    repo: &gix::Repository,
    base: Option<ObjectId>,
    ours: ObjectId,
    theirs: ObjectId,
) -> Result<(ObjectId, Vec<String>), BackendError> {
    let tips = (
        objects::read_revision(repo, ours)?,
        objects::read_revision(repo, theirs)?,
    );
    let base = GitSnapshot::of(&inner.repo, base)?;
    let ours = GitSnapshot::of(&inner.repo, Some(ours))?;
    let theirs = GitSnapshot::of(&inner.repo, Some(theirs))?;
    let resolution = op_backend::merge(
        &base,
        &ours,
        &theirs,
        Tips {
            ours: &tips.0,
            theirs: &tips.1,
        },
        &*inner.policy,
    )?;
    Ok((
        objects::write_tree(repo, &theirs, &resolution.ops)?.tree,
        resolution.notes,
    ))
}

fn merge_base(
    repo: &gix::Repository,
    one: ObjectId,
    two: ObjectId,
) -> Result<Option<ObjectId>, BackendError> {
    match repo.merge_base(one, two) {
        Ok(base) => Ok(Some(base.detach())),
        Err(gix::repository::merge_base::Error::NotFound { .. }) => Ok(None),
        Err(err) => Err(objects::storage(err)),
    }
}

fn count(
    repo: &gix::Repository,
    tip: ObjectId,
    hidden: Option<ObjectId>,
) -> Result<usize, BackendError> {
    let walk = repo
        .rev_walk([tip])
        .with_hidden(hidden)
        .all()
        .map_err(objects::storage)?;
    Ok(walk.count())
}

fn divergence(inner: &Inner, remote: &str) -> Result<(usize, usize), BackendError> {
    let repo = inner.local();
    let ours = inner.tip()?;
    let theirs = objects::tip(&repo, &tracking_reference(remote))?;
    let ahead = match ours {
        Some(ours) => count(&repo, ours, theirs)?,
        None => 0,
    };
    let behind = match theirs {
        Some(theirs) => count(&repo, theirs, ours)?,
        None => 0,
    };
    Ok((ahead, behind))
}

fn fetch(inner: &Inner, remote: &str, tracking: &str) -> Result<Option<ObjectId>, BackendError> {
    match fetch_into(&inner.common_dir, remote, tracking)? {
        true => objects::tip(&inner.local(), tracking),
        false => Ok(None),
    }
}

// Git fetches no reference outside `refs/heads/` and `refs/tags/` by itself, so a clone holds no
// tasks until this fetch. `false` when the repository has no such remote, or the remote no tasks.
pub fn fetch_tasks(path: &Path, remote: &str) -> Result<bool, BackendError> {
    let repo = gix::discover(path).map_err(objects::storage)?;
    if repo.find_remote(remote).is_err() {
        return Ok(false);
    }
    fetch_into(repo.common_dir(), remote, &tracking_reference(remote))
}

fn fetch_into(common_dir: &Path, remote: &str, tracking: &str) -> Result<bool, BackendError> {
    let refspec = format!("+{TASKS_REF}:{tracking}");
    match git(
        common_dir,
        &["fetch", "--quiet", "--no-tags", remote, &refspec],
    ) {
        Ok(()) => Ok(true),
        Err(message) if message.contains("couldn't find remote ref") => Ok(false),
        Err(message) => Err(BackendError::sync(message)),
    }
}

// `--no-verify`: the repository's pre-push hooks guard its code, and a sync every half minute must
// not run them.
fn push(inner: &Inner, remote: &str, commit: ObjectId) -> Result<Pushed, BackendError> {
    let refspec = format!("{commit}:{TASKS_REF}");
    match git(
        &inner.common_dir,
        &["push", "--quiet", "--no-verify", remote, &refspec],
    ) {
        Ok(()) => Ok(Pushed::Accepted),
        Err(message) if is_rejection(&message) => Ok(Pushed::Rejected),
        Err(message) => Err(BackendError::sync(message)),
    }
}

fn is_rejection(message: &str) -> bool {
    [
        "[rejected]",
        "non-fast-forward",
        "fetch first",
        "stale info",
    ]
    .iter()
    .any(|sign| message.contains(sign))
}

// The git CLI rather than gix for the network, so the user's SSH setup and credential helpers work
// as they do for every other push.
fn git(common_dir: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(common_dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("cannot run git: {err}"))?;
    match output.status.success() {
        true => Ok(()),
        false => Err(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
    }
}
