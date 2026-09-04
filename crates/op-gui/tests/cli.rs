use std::path::{Path, PathBuf};

use openplan_gui::cli::{BINARY, Search};

fn search() -> Search {
    Search {
        named: None,
        resources: None,
        path_dirs: Vec::new(),
        cargo_home: None,
    }
}

fn executable(dir: &Path, name: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;

    let path = dir.join(name);
    std::fs::write(&path, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[test]
fn places_run_from_the_override_to_the_cargo_directory() {
    let search = Search {
        named: Some(PathBuf::from("/named/openplan")),
        resources: Some(PathBuf::from("/bundle/Resources")),
        path_dirs: vec![PathBuf::from("/usr/bin"), PathBuf::from("/usr/local/bin")],
        cargo_home: Some(PathBuf::from("/home/me/.cargo/bin")),
    };

    assert_eq!(
        search.places(),
        vec![
            PathBuf::from("/named/openplan"),
            PathBuf::from("/bundle/Resources/bin/openplan"),
            PathBuf::from("/usr/bin/openplan"),
            PathBuf::from("/usr/local/bin/openplan"),
            PathBuf::from("/home/me/.cargo/bin/openplan"),
        ]
    );
}

#[test]
fn an_unset_place_drops_out_of_the_list() {
    assert!(search().places().is_empty());
}

#[test]
fn find_takes_the_first_runnable_place() {
    let dir = tempfile::tempdir().unwrap();
    let early = dir.path().join("early");
    let late = dir.path().join("late");
    std::fs::create_dir_all(&early).unwrap();
    std::fs::create_dir_all(&late).unwrap();
    std::fs::write(early.join(BINARY), "not executable").unwrap();
    let wanted = executable(&late, BINARY);

    let search = Search {
        path_dirs: vec![dir.path().join("missing"), early, late],
        ..search()
    };

    assert_eq!(search.find(), Some(wanted));
}

#[test]
fn find_answers_none_when_no_place_holds_a_binary() {
    let dir = tempfile::tempdir().unwrap();
    let search = Search {
        named: Some(dir.path().join(BINARY)),
        ..search()
    };

    assert_eq!(search.find(), None);
}
