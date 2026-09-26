use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fs2::FileExt as _;

use op_api::DaemonInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    pub pid: u32,
    pub version: String,
    pub bundle: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateRecord {
    pub auto: bool,
    pub last_check: Option<LastCheck>,
    // The version the daemon installed before it started again. The new process confirms it.
    pub installing: Option<String>,
}

impl Default for UpdateRecord {
    fn default() -> Self {
        Self {
            auto: true,
            last_check: None,
            installing: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastCheck {
    pub at: u64,
    pub result: String,
}

pub struct Home {
    dir: PathBuf,
}

impl Home {
    pub fn resolve() -> Result<Self> {
        let dir = match std::env::var_os("OPENPLAN_HOME").filter(|v| !v.is_empty()) {
            Some(v) => PathBuf::from(v),
            None => home_dir()
                .context("could not determine home directory; set OPENPLAN_HOME")?
                .join(".plan"),
        };
        Ok(Self { dir })
    }

    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn info_path(&self) -> PathBuf {
        self.dir.join("daemon.json")
    }

    pub fn app_info_path(&self) -> PathBuf {
        self.dir.join("app.json")
    }

    pub fn update_path(&self) -> PathBuf {
        self.dir.join("update.json")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.dir.join("daemon.lock")
    }

    pub fn start_lock_path(&self) -> PathBuf {
        self.dir.join("daemon.start.lock")
    }

    pub fn log_path(&self) -> PathBuf {
        self.dir.join("daemon.log")
    }

    pub fn ensure_dir(&self) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)
    }

    pub fn read_info(&self) -> Option<DaemonInfo> {
        let text = std::fs::read_to_string(self.info_path()).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn write_info(&self, info: &DaemonInfo) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(info)?;
        let tmp = self.dir.join(format!("daemon.json.{}.tmp", info.pid));
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, self.info_path())?;
        Ok(())
    }

    pub fn clear_info(&self) {
        let _ = std::fs::remove_file(self.info_path());
    }

    pub fn read_app_info(&self) -> Option<AppInfo> {
        let text = std::fs::read_to_string(self.app_info_path()).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn write_app_info(&self, info: &AppInfo) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(info)?;
        let tmp = self.dir.join(format!("app.json.{}.tmp", info.pid));
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, self.app_info_path())?;
        Ok(())
    }

    pub fn read_update(&self) -> UpdateRecord {
        std::fs::read_to_string(self.update_path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn edit_update(&self, change: impl FnOnce(&mut UpdateRecord)) -> Result<()> {
        let mut record = self.read_update();
        change(&mut record);
        self.ensure_dir()?;
        let bytes = serde_json::to_vec_pretty(&record)?;
        let tmp = self
            .dir
            .join(format!("update.json.{}.tmp", std::process::id()));
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, self.update_path())?;
        Ok(())
    }

    pub fn open_lock(&self) -> io::Result<File> {
        Self::open_lock_file(&self.lock_path())
    }

    pub fn open_start_lock(&self) -> io::Result<File> {
        Self::open_lock_file(&self.start_lock_path())
    }

    fn open_lock_file(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
    }

    // A free lock proves no live daemon holds it: fs2's flock is released when the
    // holding process's file handle closes, i.e. on exit — a truer liveness signal
    // than the recorded pid, which the OS may have recycled.
    pub fn lock_is_free(&self) -> Result<bool> {
        let lock = self.open_lock()?;
        match lock.try_lock_exclusive() {
            Ok(()) => {
                fs2::FileExt::unlock(&lock).ok();
                Ok(true)
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}
