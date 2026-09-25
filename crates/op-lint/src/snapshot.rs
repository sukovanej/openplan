use std::collections::HashSet;
use std::path::{Path, PathBuf};

use op_skills::SkillFile;
use op_task::layout::Document;
use op_task::tag::{PartialTag, parse_partial as parse_partial_tag};
use op_task::{Abbreviation, PartialTask, file_id, parse_partial};
use op_tracker::{Plan, TrackerError};

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
    skills: Vec<SkillFile>,
    // Every document the store holds, assets included. Git keeps them off the disk.
    documents: HashSet<PathBuf>,
}

impl Snapshot {
    // `dir` is where the documents read as living, so a diagnostic names a path a person can open;
    // `root` is the checkout that holds the agent skills.
    pub fn from_plan(plan: &Plan, dir: &Path, root: &Path) -> Result<Self, TrackerError> {
        let mut tasks = Vec::new();
        let mut tags = Vec::new();
        let paths = plan.snapshot().files()?;
        let documents = paths.iter().map(|path| dir.join(path)).collect();
        for path in paths {
            let Some(text) = plan.snapshot().read_text(&path)? else {
                continue;
            };
            match Document::of(&path) {
                Document::Task(_) => tasks.push((dir.join(&path), text)),
                Document::Tag(_) => tags.push((dir.join(&path), text)),
                _ => {}
            }
        }
        Ok(Self::from_files(root, plan.abbreviation()?, tasks)
            .with_documents(documents)
            .with_tags(tags)
            .with_skills(
                op_skills::installed(root).map_err(|err| TrackerError::Unreadable {
                    path: root.display().to_string(),
                    reason: err.to_string(),
                })?,
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
            tags: Vec::new(),
            skills: Vec::new(),
            documents: HashSet::new(),
        }
    }

    pub fn with_documents(mut self, documents: HashSet<PathBuf>) -> Self {
        self.documents = documents;
        self
    }

    // A link into the code (`../../crates/…`) names a file on the disk, not a document.
    pub fn holds(&self, path: &Path) -> bool {
        self.documents.contains(path) || path.exists()
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

    // Only an agent that already has a skills directory holds skills; installing them is what
    // `openplan setup-skills` does, and a repository that never asked for them owes the binary
    // nothing.
    pub fn with_skills(mut self, skills: Vec<SkillFile>) -> Self {
        self.skills = skills;
        self.skills.sort_by(|a, b| a.path.cmp(&b.path));
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

    pub fn skills(&self) -> &[SkillFile] {
        &self.skills
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
