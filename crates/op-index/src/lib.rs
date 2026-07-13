use std::collections::HashMap;

use op_api::{Matrix, TaskSummary};

#[derive(Debug, Default)]
pub struct Index {
    matrix: Matrix,
    blob_cache: HashMap<String, TaskSummary>,
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
}
