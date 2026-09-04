use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

pub const BINARY: &str = "openplan";
pub const OVERRIDE: &str = "OPENPLAN_BIN";

pub struct Search {
    pub named: Option<PathBuf>,
    pub resources: Option<PathBuf>,
    pub path_dirs: Vec<PathBuf>,
    pub cargo_home: Option<PathBuf>,
}

impl Search {
    pub fn from_env(resources: Option<PathBuf>) -> Self {
        Self {
            named: var_path(OVERRIDE),
            resources,
            path_dirs: std::env::var_os("PATH")
                .map(|path| std::env::split_paths(&path).collect())
                .unwrap_or_default(),
            cargo_home: var_path("HOME").map(|home| home.join(".cargo").join("bin")),
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
        places.extend(self.cargo_home.iter().map(|dir| dir.join(BINARY)));
        places
    }

    pub fn find(&self) -> Option<PathBuf> {
        self.places().into_iter().find(|place| runnable(place))
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
