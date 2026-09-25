use std::path::{Path, PathBuf};

use op_backend::Actor;

use crate::TASKS_REF;

// What a directory tells about the repository it sits in, before any backend opens it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkout {
    pub common_dir: PathBuf,
    // The worktree the path is in; `None` when bare.
    pub workdir: Option<PathBuf>,
    // The main worktree, which every linked worktree of the repository shares; `None` when bare.
    pub root: Option<PathBuf>,
    pub has_tasks: bool,
}

pub fn inspect(path: &Path) -> Option<Checkout> {
    let repo = gix::discover(path).ok()?;
    let common_dir = repo.common_dir().to_path_buf();
    let root = match repo.main_repo() {
        Ok(main) => main.workdir().map(Path::to_path_buf),
        Err(_) => repo.workdir().map(Path::to_path_buf),
    };
    Some(Checkout {
        has_tasks: has_tasks(&repo),
        workdir: repo.workdir().map(Path::to_path_buf),
        common_dir,
        root,
    })
}

// A fetch without the daemon leaves only the remote-tracking copy, which the backend starts from.
fn has_tasks(repo: &gix::Repository) -> bool {
    if matches!(repo.try_find_reference(TASKS_REF), Ok(Some(_))) {
        return true;
    }
    let Ok(references) = repo.references() else {
        return false;
    };
    let Ok(mut remotes) = references.prefixed("refs/openplan/remotes/") else {
        return false;
    };
    remotes.any(|reference| reference.is_ok())
}

// The name and email the repository signs commits with, or the global ones outside a repository.
pub fn identity(path: &Path) -> Option<Actor> {
    let (name, email) = match gix::discover(path) {
        Ok(repo) => {
            let config = repo.config_snapshot();
            (
                config.string("user.name").map(|name| name.to_string()),
                config.string("user.email").map(|email| email.to_string()),
            )
        }
        Err(_) => {
            let config = gix::config::File::from_globals().ok()?;
            (
                config
                    .string_by("user", None, "name")
                    .map(|name| name.to_string()),
                config
                    .string_by("user", None, "email")
                    .map(|email| email.to_string()),
            )
        }
    };
    let name = name.filter(|name| !name.trim().is_empty())?;
    let actor = Actor::new(name.trim());
    Some(match email.filter(|email| !email.trim().is_empty()) {
        Some(email) => actor.with_email(email.trim()),
        None => actor,
    })
}
