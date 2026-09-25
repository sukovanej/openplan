use std::path::Path;
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use op_api::{BackendKind, ProjectView};
use op_backend_git::{GitBackend, TASKS_REF};
use op_client::Client;
use op_server::{Location, OpenError, STORE_DIR};
use op_tracker::TaskMergePolicy;

use crate::daemon::daemon_base_url;

pub fn init(
    root: &Path,
    daemon_url: Option<&str>,
    backend: Option<BackendKind>,
    abbreviation: Option<&str>,
) -> Result<()> {
    let path = std::fs::canonicalize(root)
        .with_context(|| format!("no such directory: {}", root.display()))?;
    let view = register(&path, daemon_url, backend, abbreviation)?;
    println!(
        "{} keeps its {} tasks {}",
        view.name,
        view.abbreviation,
        place(&view)
    );
    Ok(())
}

// A repository that kept its tasks in `.plan/` beside the code moves them out: onto the tasks branch
// with the history of `.plan/`, or into a local directory with a history of its own.
pub fn migrate(root: &Path, daemon_url: Option<&str>, backend: Option<BackendKind>) -> Result<()> {
    let checkout = match Location::find(root, None) {
        Err(OpenError::NeedsMigration(checkout)) => checkout,
        Ok(location) => bail!(
            "{} already keeps its tasks {}; there is nothing to migrate",
            location.root.display(),
            match location.kind {
                BackendKind::Git => format!("in the git ref {TASKS_REF}"),
                BackendKind::Local => format!("in {}", location.root.join(STORE_DIR).display()),
            }
        ),
        Err(err) => return Err(err.into()),
    };
    let kind = backend.unwrap_or(BackendKind::Git);
    if kind == BackendKind::Git {
        let location = Location::find(&checkout, Some(BackendKind::Git))?;
        let machine = op_server::machine_actor(&location.root);
        let mut options = op_backend_git::Options::new(Arc::new(TaskMergePolicy));
        options.machine = machine;
        // Opened at the checkout, so the history copied is that of the branch the caller stands on.
        let backend = GitBackend::open(&checkout, options)?;
        let imported = backend.import(&checkout, STORE_DIR)?;
        println!(
            "copied {} revision{} of {STORE_DIR}/ to the git ref {TASKS_REF}",
            imported.revisions,
            if imported.revisions == 1 { "" } else { "s" }
        );
        if imported.uncommitted {
            println!("copied the edits in {STORE_DIR}/ that no commit holds yet");
        }
    }
    let view = register(&checkout, daemon_url, Some(kind), None)?;
    println!(
        "{} keeps its {} tasks {}",
        view.name,
        view.abbreviation,
        place(&view)
    );
    match kind {
        BackendKind::Git => println!(
            "next: remove {STORE_DIR}/ from the code branch with `git rm -r {STORE_DIR}` and commit; \
             the daemon pushes {TASKS_REF} to the remote"
        ),
        BackendKind::Local => println!(
            "next: stop committing {STORE_DIR}/; add it to .gitignore and remove it from the index \
             with `git rm -r --cached {STORE_DIR}`"
        ),
    }
    Ok(())
}

fn register(
    path: &Path,
    daemon_url: Option<&str>,
    backend: Option<BackendKind>,
    abbreviation: Option<&str>,
) -> Result<ProjectView> {
    let client = Client::default().with_identity(crate::author::identity(path));
    let base_url = daemon_base_url(&client, daemon_url)?;
    let (view, _) = client.register_project(&base_url, path, backend, abbreviation)?;
    Ok(view)
}

fn place(view: &ProjectView) -> String {
    match view.backend {
        BackendKind::Git => format!("in the git ref {TASKS_REF}"),
        BackendKind::Local => format!("in {}", Path::new(&view.root).join(STORE_DIR).display()),
    }
}
