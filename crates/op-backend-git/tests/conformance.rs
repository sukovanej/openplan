use std::process::Command;
use std::sync::Arc;

use op_backend::{Backend, PreferTheirs};
use op_backend_conformance::Subject;
use op_backend_git::{GitBackend, Options};

struct Git(tempfile::TempDir);

impl Git {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let status = Command::new("git")
            .args(["init", "--quiet", "-b", "main"])
            .current_dir(dir.path())
            .status()
            .expect("git init");
        assert!(status.success());
        Self(dir)
    }
}

impl Subject for Git {
    fn open(&self) -> Arc<dyn Backend> {
        Arc::new(
            GitBackend::open(self.0.path(), Options::new(Arc::new(PreferTheirs))).expect("open"),
        )
    }
}

op_backend_conformance::suite!(Git::new);
