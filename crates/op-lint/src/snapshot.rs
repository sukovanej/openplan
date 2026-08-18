use std::path::{Path, PathBuf};

use op_store::{Store, StoreError};
use op_task::{Abbreviation, PartialTask, file_id, parse_partial};

#[derive(Debug, Clone)]
pub struct TaskFile {
    pub number: u64,
    pub path: PathBuf,
    pub source: String,
    pub task: PartialTask,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    root: PathBuf,
    abbreviation: Abbreviation,
    files: Vec<TaskFile>,
}

impl Snapshot {
    pub fn from_store(store: &Store) -> Result<Self, StoreError> {
        let dir = store.tasks_dir();
        let mut sources = Vec::new();
        if dir.exists() {
            for entry in std::fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                let is_task = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .and_then(file_id)
                    .is_some();
                if !is_task {
                    continue;
                }
                let source = std::fs::read_to_string(&path)?;
                sources.push((path, source));
            }
        }
        Ok(Self::from_files(
            store.root(),
            store.abbreviation(),
            sources,
        ))
    }

    pub fn from_files(
        root: impl Into<PathBuf>,
        abbreviation: Abbreviation,
        files: impl IntoIterator<Item = (PathBuf, String)>,
    ) -> Self {
        let mut files: Vec<TaskFile> = files
            .into_iter()
            .map(|(path, source)| {
                let number = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .and_then(file_id)
                    .unwrap_or(0);
                let task = parse_partial(&source);
                TaskFile {
                    number,
                    path,
                    source,
                    task,
                }
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Self {
            root: root.into(),
            abbreviation,
            files,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn abbreviation(&self) -> Abbreviation {
        self.abbreviation
    }

    pub fn files(&self) -> &[TaskFile] {
        &self.files
    }

    // The file a number resolves to, lowest path first as the store does — two files claiming one
    // number is a hand-made state a reference must resolve the same way a read would.
    pub fn file(&self, number: u64) -> Option<&TaskFile> {
        self.files
            .iter()
            .filter(|file| file.number == number)
            .min_by(|a, b| a.path.cmp(&b.path))
    }
}

// The anchor scheme GitHub, GitLab, and VS Code all resolve: lowercase, spaces to `-`, punctuation
// dropped, duplicates suffixed `-1`, `-2`. Kept here so our own `#Section` links stay clickable
// outside openplan.
pub fn github_slug(heading: &str) -> String {
    let mut slug = String::new();
    for ch in heading.chars() {
        if ch == ' ' {
            slug.push('-');
        } else if ch == '-' || ch == '_' || ch.is_alphanumeric() {
            slug.extend(ch.to_lowercase());
        }
    }
    slug
}
