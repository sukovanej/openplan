use std::collections::BTreeMap;
use std::fs::{Metadata, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TEMP_ATTEMPTS: usize = 16;
const SETTLED: Duration = Duration::from_secs(2);

// A file's size, times, and identity. A file whose stamp did not move still holds what it held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    changed: (i64, i64),
    #[cfg(unix)]
    inode: u64,
}

impl Stamp {
    fn of(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt as _;
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            changed: (metadata.ctime(), metadata.ctime_nsec()),
            #[cfg(unix)]
            inode: metadata.ino(),
        }
    }

    // A file written in the last moments can change again within one tick of the file system's
    // clock and keep its stamp, so only an older stamp proves the file did not change since.
    pub fn is_settled(&self, now: SystemTime) -> bool {
        let latest = self.modified.into_iter();
        #[cfg(unix)]
        let latest = latest.chain(changed_at(self.changed));
        latest
            .max()
            .and_then(|at| now.duration_since(at).ok())
            .is_some_and(|age| age >= SETTLED)
    }
}

#[cfg(unix)]
fn changed_at((seconds, nanos): (i64, i64)) -> Option<SystemTime> {
    let seconds = u64::try_from(seconds).ok()?;
    let nanos = u32::try_from(nanos).ok()?;
    UNIX_EPOCH.checked_add(Duration::new(seconds, nanos))
}

pub(crate) fn list(root: &Path) -> io::Result<BTreeMap<String, Stamp>> {
    let mut files = BTreeMap::new();
    if root.is_dir() {
        collect(root, "", &mut files)?;
    }
    Ok(files)
}

fn collect(dir: &Path, prefix: &str, files: &mut BTreeMap<String, Stamp>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if is_hidden(&name) {
            continue;
        }
        let path = format!("{prefix}{name}");
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect(&entry.path(), &format!("{path}/"), files)?;
        } else if kind.is_file() {
            match entry.metadata() {
                Ok(metadata) => {
                    files.insert(path, Stamp::of(&metadata));
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
    }
    Ok(())
}

pub(crate) fn read(root: &Path, path: &str) -> io::Result<Option<Vec<u8>>> {
    match std::fs::read(root.join(path)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

pub(crate) fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

pub(crate) fn write(root: &Path, path: &str, content: Option<&[u8]>) -> io::Result<()> {
    let target = root.join(path);
    match content {
        Some(bytes) => {
            let dir = target
                .parent()
                .ok_or_else(|| io::Error::other(format!("{path} names no directory")))?;
            std::fs::create_dir_all(dir)?;
            let temp = write_temp(dir, bytes)?;
            std::fs::rename(&temp, &target).inspect_err(|_| {
                let _ = std::fs::remove_file(&temp);
            })
        }
        None => match std::fs::remove_file(&target) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        },
    }
}

fn write_temp(dir: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    for _ in 0..TEMP_ATTEMPTS {
        let path = dir.join(format!(".op-{}-{}.tmp", std::process::id(), random_hex()?));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(bytes)?;
                file.sync_all()?;
                return Ok(path);
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::other(format!(
        "could not open a temp file in {} after {TEMP_ATTEMPTS} attempts",
        dir.display()
    )))
}

fn random_hex() -> io::Result<String> {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).map_err(|err| io::Error::other(err.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
