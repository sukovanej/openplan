use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

const TEMP_ATTEMPTS: usize = 16;

pub(crate) fn scan(root: &Path) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    if root.is_dir() {
        collect(root, "", &mut files)?;
    }
    Ok(files)
}

fn collect(dir: &Path, prefix: &str, files: &mut BTreeMap<String, Vec<u8>>) -> io::Result<()> {
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
            match std::fs::read(entry.path()) {
                Ok(bytes) => {
                    files.insert(path, bytes);
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
    }
    Ok(())
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
