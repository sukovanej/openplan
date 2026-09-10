use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use flate2::Compression;
use flate2::write::GzEncoder;
use op_update::{replace_bundle, replace_executable};

fn tar_gz(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    for (path, bytes, mode) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(*mode);
        header.set_cksum();
        builder.append_data(&mut header, path, *bytes).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

fn cli_archive(body: &[u8]) -> Vec<u8> {
    tar_gz(&[
        ("openplan-aarch64-apple-darwin/README.md", b"readme", 0o644),
        ("openplan-aarch64-apple-darwin/openplan", body, 0o755),
    ])
}

fn app_archive(marker: &[u8]) -> Vec<u8> {
    tar_gz(&[
        ("OpenPlan.app/Contents/Info.plist", b"<plist/>", 0o644),
        ("OpenPlan.app/Contents/MacOS/openplan-gui", marker, 0o755),
    ])
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn the_executable_is_replaced_in_place_and_stays_executable() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("openplan");
    std::fs::write(&exe, b"old").unwrap();

    replace_executable(&cli_archive(b"new"), &exe).unwrap();

    assert_eq!(std::fs::read(&exe).unwrap(), b"new");
    assert_eq!(mode(&exe), 0o755);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn an_archive_without_the_binary_leaves_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("openplan");
    std::fs::write(&exe, b"old").unwrap();
    let archive = tar_gz(&[("openplan-aarch64-apple-darwin/README.md", b"readme", 0o644)]);

    let err = replace_executable(&archive, &exe).unwrap_err();

    assert!(err.to_string().contains("no file named openplan"), "{err}");
    assert_eq!(std::fs::read(&exe).unwrap(), b"old");
}

#[test]
fn the_bundle_is_swapped_and_the_old_one_removed() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("OpenPlan.app");
    std::fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
    std::fs::write(bundle.join("Contents/MacOS/openplan-gui"), b"old").unwrap();
    std::fs::write(bundle.join("Contents/stale"), b"gone").unwrap();

    replace_bundle(&app_archive(b"new"), &bundle).unwrap();

    let gui = bundle.join("Contents/MacOS/openplan-gui");
    assert_eq!(std::fs::read(&gui).unwrap(), b"new");
    assert_eq!(mode(&gui), 0o755);
    assert!(!bundle.join("Contents/stale").exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn a_bundle_installs_where_none_was() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("OpenPlan.app");

    replace_bundle(&app_archive(b"new"), &bundle).unwrap();

    assert!(bundle.join("Contents/Info.plist").is_file());
}

#[test]
fn an_archive_without_a_bundle_leaves_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("OpenPlan.app");
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join("marker"), b"old").unwrap();
    let archive = tar_gz(&[("README.md", b"readme", 0o644)]);

    let err = replace_bundle(&archive, &bundle).unwrap_err();

    assert!(err.to_string().contains("no .app bundle"), "{err}");
    assert_eq!(std::fs::read(bundle.join("marker")).unwrap(), b"old");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
