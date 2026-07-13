use std::path::{Path, PathBuf};

use fs2::FileExt as _;
use op_task::Task;

pub const STORE_DIR: &str = ".plan";

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("no {STORE_DIR}/ store found")]
    NotFound,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Task(#[from] op_task::TaskError),
}

impl Store {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        if root.join(STORE_DIR).is_dir() {
            Ok(Self { root })
        } else {
            Err(StoreError::NotFound)
        }
    }

    pub fn discover(start: impl AsRef<Path>) -> Result<Self, StoreError> {
        let start = start.as_ref();
        let base = if start.is_absolute() {
            start.to_path_buf()
        } else {
            std::env::current_dir()?.join(start)
        };
        for dir in base.ancestors() {
            if dir.join(STORE_DIR).is_dir() {
                return Ok(Self {
                    root: dir.to_path_buf(),
                });
            }
        }
        Err(StoreError::NotFound)
    }

    pub fn plan_dir(&self) -> PathBuf {
        self.root.join(STORE_DIR)
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.plan_dir().join("tasks")
    }

    pub fn task_path(&self, id: &str) -> PathBuf {
        self.tasks_dir().join(format!("{id}.md"))
    }

    pub fn task_ids(&self) -> Result<Vec<String>, StoreError> {
        let dir = self.tasks_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                ids.push(stem.to_owned());
            }
        }
        ids.sort();
        Ok(ids)
    }

    pub fn read(&self, id: &str) -> Result<Task, StoreError> {
        let text = std::fs::read_to_string(self.task_path(id))?;
        Ok(Task::from_file_string(&text)?)
    }

    pub fn with_lock<T>(
        &self,
        id: &str,
        f: impl FnOnce() -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        // Creation never conflicts (one file per task, random-slug ids), so a lock
        // only guards mutation of an existing task — open without creating, so a
        // lock never materializes a phantom empty task file.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(self.task_path(id))?;
        file.lock_exclusive()?;
        let result = f();
        let _ = fs2::FileExt::unlock(&file);
        result
    }
}
