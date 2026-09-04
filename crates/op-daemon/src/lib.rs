use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use fs2::FileExt as _;

pub use op_api::DaemonInfo;

pub const DEFAULT_PORT: u16 = 7373;

// The port the daemon binds unless told otherwise. A write brings the daemon up itself, with no
// `--port` to carry, so the override has to be reachable from the environment too.
pub fn default_port() -> u16 {
    std::env::var("OPENPLAN_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

pub fn base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
