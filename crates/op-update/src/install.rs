use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use flate2::read::GzDecoder;
use tar::Archive;

pub fn replace_executable(archive_gz: &[u8], dest: &Path) -> Result<()> {
    let name = dest
        .file_name()
        .context("the executable path has no file name")?;
    let staged = sibling(dest, "update")?;
    let mut archive = Archive::new(GzDecoder::new(Cursor::new(archive_gz)));
    let mut found = false;
    for entry in archive.entries().context("reading the CLI archive")? {
        let mut entry = entry.context("reading the CLI archive")?;
        let path = entry.path().context("reading an archive entry path")?;
        if path.file_name() != Some(name) || !entry.header().entry_type().is_file() {
            continue;
        }
        entry
            .unpack(&staged)
            .with_context(|| format!("writing {}", staged.display()))?;
        found = true;
        break;
    }
    if !found {
        bail!(
            "the CLI archive holds no file named {}",
            name.to_string_lossy()
        );
    }
    make_executable(&staged)?;
    fs::rename(&staged, dest)
        .inspect_err(|_| {
            let _ = fs::remove_file(&staged);
        })
        .with_context(|| format!("replacing {}", dest.display()))
}

pub fn replace_bundle(archive_gz: &[u8], dest: &Path) -> Result<()> {
    let staging = sibling(dest, "update")?;
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).with_context(|| format!("creating {}", staging.display()))?;
    let result = unpack_bundle(archive_gz, &staging).and_then(|staged| swap_dir(&staged, dest));
    let _ = fs::remove_dir_all(&staging);
    result
}

fn unpack_bundle(archive_gz: &[u8], staging: &Path) -> Result<PathBuf> {
    let mut archive = Archive::new(GzDecoder::new(Cursor::new(archive_gz)));
    archive
        .unpack(staging)
        .context("unpacking the app archive")?;
    let mut bundles = fs::read_dir(staging)
        .with_context(|| format!("reading {}", staging.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "app") && path.is_dir());
    match (bundles.next(), bundles.next()) {
        (Some(bundle), None) => Ok(bundle),
        (None, _) => bail!("the app archive holds no .app bundle at its top level"),
        (Some(_), Some(_)) => bail!("the app archive holds more than one .app bundle"),
    }
}

fn swap_dir(staged: &Path, dest: &Path) -> Result<()> {
    let retired = sibling(dest, "old")?;
    let had_old = dest.exists();
    if had_old {
        fs::rename(dest, &retired).with_context(|| format!("moving {} aside", dest.display()))?;
    }
    if let Err(err) = fs::rename(staged, dest) {
        if had_old {
            let _ = fs::rename(&retired, dest);
        }
        return Err(err).with_context(|| format!("replacing {}", dest.display()));
    }
    if had_old {
        let _ = fs::remove_dir_all(&retired);
    }
    Ok(())
}

// A temporary name next to the destination keeps the final rename on one file system, which is
// what makes it atomic.
fn sibling(dest: &Path, suffix: &str) -> Result<PathBuf> {
    let parent = dest
        .parent()
        .with_context(|| format!("{} has no parent directory", dest.display()))?;
    let name = dest
        .file_name()
        .with_context(|| format!("{} has no file name", dest.display()))?
        .to_string_lossy();
    Ok(parent.join(format!(".{name}.{suffix}.{}", std::process::id())))
}

fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("marking {} executable", path.display()))
}
