use std::sync::Arc;

use crate::{Actor, BackendError};

// Signs what a backend writes on its own: merges, reference logs, and hand edits. It is asked at
// each such write, so a name set after the backend opened signs the next one.
#[derive(Clone)]
pub struct Signer(Arc<dyn Fn() -> Result<Actor, BackendError> + Send + Sync>);

impl Signer {
    pub fn new(sign: impl Fn() -> Result<Actor, BackendError> + Send + Sync + 'static) -> Self {
        Self(Arc::new(sign))
    }

    pub fn fixed(actor: Actor) -> Self {
        Self::new(move || Ok(actor.clone()))
    }

    pub fn sign(&self) -> Result<Actor, BackendError> {
        (self.0)()
    }
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Signer")
    }
}
