use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const REGISTRY_FILE: &str = "registry.toml";

const NAME_FALLBACK: &str = "project";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRegistry {
    #[serde(default, rename = "project")]
    entries: Vec<ProjectEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("{path}: {reason}")]
    Invalid { path: PathBuf, reason: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl ProjectRegistry {
    pub fn read(path: &Path) -> Result<Option<Self>, RegistryError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        toml::from_str(&text)
            .map(Some)
            .map_err(|err| RegistryError::Invalid {
                path: path.to_path_buf(),
                reason: err.message().to_owned(),
            })
    }

    // Rename over a temporary in the same directory, so a reader never sees a half-written registry
    // and a crash mid-write leaves the previous one intact. The contents reach the disk before the
    // rename, and the rename before this returns: an empty file here parses as zero projects, which
    // would drop every project but `--root` on the next start without reporting anything.
    pub fn write(&self, path: &Path) -> Result<(), RegistryError> {
        let text = toml::to_string_pretty(self).map_err(|err| RegistryError::Invalid {
            path: path.to_path_buf(),
            reason: err.to_string(),
        })?;
        let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
        let file = std::fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut &file, text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)?;
        if let Some(dir) = path.parent() {
            std::fs::File::open(dir)?.sync_all()?;
        }
        Ok(())
    }

    pub fn entries(&self) -> &[ProjectEntry] {
        &self.entries
    }

    pub fn add(&mut self, path: PathBuf) -> &ProjectEntry {
        let name = unique_name(&path, |name| self.holds_name(name));
        self.insert(ProjectEntry { name, path })
    }

    pub fn insert(&mut self, entry: ProjectEntry) -> &ProjectEntry {
        self.entries.push(entry);
        self.entries.last().expect("just pushed")
    }

    pub fn entry_at(&self, path: &Path) -> Option<&ProjectEntry> {
        self.entries
            .iter()
            .find(|entry| same_path(&entry.path, path))
    }

    pub fn remove(&mut self, name: &str) -> Option<ProjectEntry> {
        let at = self.entries.iter().position(|entry| entry.name == name)?;
        Some(self.entries.remove(at))
    }

    pub fn holds_name(&self, name: &str) -> bool {
        self.entries.iter().any(|entry| entry.name == name)
    }
}

// The registry file and the daemon's live map are two namespaces the name has to be free in: the
// file holds entries this daemon skipped, and the map holds projects a test set up without a file.
pub fn unique_name(path: &Path, taken: impl Fn(&str) -> bool) -> String {
    let base = op_task::slug(
        &path.file_name().unwrap_or_default().to_string_lossy(),
        NAME_FALLBACK,
    );
    if !taken(&base) {
        return base;
    }
    (2u32..)
        .map(|suffix| format!("{base}-{suffix}"))
        .find(|candidate| !taken(candidate))
        .expect("the suffix range is unbounded")
}

// A name reaches the URL, so it has to survive the round trip unchanged rather than be quietly
// rewritten into something the caller never asked for.
pub fn is_usable_name(name: &str) -> bool {
    !name.is_empty() && name == op_task::slug(name, "")
}

pub fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

pub fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
