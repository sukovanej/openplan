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
    // and a crash mid-write leaves the previous one intact.
    pub fn write(&self, path: &Path) -> Result<(), RegistryError> {
        let text = toml::to_string_pretty(self).map_err(|err| RegistryError::Invalid {
            path: path.to_path_buf(),
            reason: err.to_string(),
        })?;
        let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn entries(&self) -> &[ProjectEntry] {
        &self.entries
    }

    pub fn add(&mut self, path: PathBuf) -> &ProjectEntry {
        let name = self.unique_name(&path);
        self.entries.push(ProjectEntry { name, path });
        self.entries.last().expect("just pushed")
    }

    fn unique_name(&self, path: &Path) -> String {
        let base = op_task::slug(
            &path.file_name().unwrap_or_default().to_string_lossy(),
            NAME_FALLBACK,
        );
        if !self.holds_name(&base) {
            return base;
        }
        (2u32..)
            .map(|suffix| format!("{base}-{suffix}"))
            .find(|candidate| !self.holds_name(candidate))
            .expect("the suffix range is unbounded")
    }

    fn holds_name(&self, name: &str) -> bool {
        self.entries.iter().any(|entry| entry.name == name)
    }
}
