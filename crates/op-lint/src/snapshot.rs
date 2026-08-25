use std::path::{Path, PathBuf};

use op_store::{Store, StoreError};
use op_task::tag::{PartialTag, parse_partial as parse_partial_tag};
use op_task::{Abbreviation, PartialTask, file_id, parse_partial};

#[derive(Debug, Clone)]
pub struct TaskFile {
    pub number: u64,
    pub path: PathBuf,
    pub source: String,
    pub task: PartialTask,
}

// The name is the file stem exactly as it is written, not the normalized one: a stem the
// normalizer would have named differently registers no tag, and that is what a rule reports.
#[derive(Debug, Clone)]
pub struct TagFile {
    pub name: String,
    pub path: PathBuf,
    pub source: String,
    pub tag: PartialTag,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    root: PathBuf,
    abbreviation: Abbreviation,
    files: Vec<TaskFile>,
    tags: Vec<TagFile>,
}

impl Snapshot {
    pub fn from_store(store: &Store) -> Result<Self, StoreError> {
        let sources: Vec<(PathBuf, String)> = read_markdown(&store.tasks_dir())?
            .into_iter()
            .filter(|(path, _)| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .and_then(file_id)
                    .is_some()
            })
            .collect();
        Ok(
            Self::from_files(store.root(), store.abbreviation(), sources)
                .with_tags(read_markdown(&store.tags_dir())?),
        )
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
            tags: Vec::new(),
        }
    }

    pub fn with_tags(mut self, files: impl IntoIterator<Item = (PathBuf, String)>) -> Self {
        self.tags = files
            .into_iter()
            .map(|(path, source)| TagFile {
                name: path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                tag: parse_partial_tag(&source),
                path,
                source,
            })
            .collect();
        self.tags.sort_by(|a, b| a.path.cmp(&b.path));
        self
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

    pub fn tags(&self) -> &[TagFile] {
        &self.tags
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

fn read_markdown(dir: &Path) -> Result<Vec<(PathBuf, String)>, StoreError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") || !path.is_file() {
            continue;
        }
        let source = std::fs::read_to_string(&path)?;
        sources.push((path, source));
    }
    Ok(sources)
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
