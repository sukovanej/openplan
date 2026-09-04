use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

pub const BINARY: &str = "openplan";
pub const OVERRIDE: &str = "OPENPLAN_BIN";

#[derive(Debug, PartialEq, Eq)]
pub enum Missing {
    Override(PathBuf),
    Anywhere(Vec<PathBuf>),
}

pub struct Search {
    pub named: Option<PathBuf>,
    pub resources: Option<PathBuf>,
    pub path_dirs: Vec<PathBuf>,
    pub cargo_bin: Option<PathBuf>,
}

impl Search {
    pub fn from_env(resources: Option<PathBuf>) -> Self {
        Self {
            named: var_path(OVERRIDE),
            resources,
            path_dirs: std::env::var_os("PATH")
                .map(|path| std::env::split_paths(&path).collect())
                .unwrap_or_default(),
            cargo_bin: var_path("CARGO_HOME")
                .or_else(|| var_path("HOME").map(|home| home.join(".cargo")))
                .map(|cargo| cargo.join("bin")),
        }
    }

    // A macOS app started from the dock inherits a PATH that holds neither the app bundle nor a
    // developer's cargo directory, so both are searched by name rather than left to PATH.
    pub fn places(&self) -> Vec<PathBuf> {
        let mut places = Vec::new();
        places.extend(self.named.clone());
        places.extend(
            self.resources
                .iter()
                .map(|dir| dir.join("bin").join(BINARY)),
        );
        places.extend(self.path_dirs.iter().map(|dir| dir.join(BINARY)));
        places.extend(self.cargo_bin.iter().map(|dir| dir.join(BINARY)));
        places
    }

    // An override that names nothing runnable is a mistake to report, never a reason to start a
    // different binary than the one the caller asked for.
    pub fn find(&self) -> Result<PathBuf, Missing> {
        if let Some(named) = &self.named {
            return runnable(named)
                .then(|| named.clone())
                .ok_or_else(|| Missing::Override(named.clone()));
        }
        let places = self.places();
        match places.iter().find(|place| runnable(place)) {
            Some(found) => Ok(found.clone()),
            None => Err(Missing::Anywhere(places)),
        }
    }
}

fn var_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn runnable(path: &Path) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}
