use std::sync::Arc;

use op_backend::{Actor, Backend, Signer};
use op_backend_conformance::Subject;
use op_backend_local::{LocalBackend, Options};

struct Local(tempfile::TempDir);

impl Subject for Local {
    fn open(&self) -> Arc<dyn Backend> {
        Arc::new(
            LocalBackend::open(
                self.0.path().join(".plan"),
                Options::new(Signer::fixed(Actor::new("filesystem"))),
            )
            .expect("open"),
        )
    }
}

op_backend_conformance::suite!(|| Local(tempfile::tempdir().expect("tempdir")));
