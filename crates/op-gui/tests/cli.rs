use std::path::{Path, PathBuf};

use openplan_gui::cli::{BINARY, Missing, Search};

fn search() -> Search {
    Search {
        named: None,
        resources: None,
        path_dirs: Vec::new(),
        cargo_bin: None,
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
        cargo_bin: Some(PathBuf::from("/home/me/.cargo/bin")),
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

    assert_eq!(search.find(), Ok(wanted));
}

#[test]
fn find_takes_the_override_over_every_other_place() {
    let dir = tempfile::tempdir().unwrap();
    let wanted = executable(dir.path(), "named");
    let search = Search {
        named: Some(wanted.clone()),
        path_dirs: vec![dir.path().to_path_buf()],
        ..search()
    };
    executable(dir.path(), BINARY);

    assert_eq!(search.find(), Ok(wanted));
}

// An override that names nothing runnable must be reported, never passed over in favour of a
// binary the caller did not ask for.
#[test]
fn a_broken_override_refuses_instead_of_falling_through() {
    let dir = tempfile::tempdir().unwrap();
    let named = dir.path().join("typo");
    executable(dir.path(), BINARY);
    let search = Search {
        named: Some(named.clone()),
        path_dirs: vec![dir.path().to_path_buf()],
        ..search()
    };

    assert_eq!(search.find(), Err(Missing::Override(named)));
}

#[test]
fn find_reports_every_place_it_looked_when_none_holds_a_binary() {
    let dir = tempfile::tempdir().unwrap();
    let search = Search {
        path_dirs: vec![dir.path().to_path_buf()],
        ..search()
    };

    assert_eq!(
        search.find(),
        Err(Missing::Anywhere(vec![dir.path().join(BINARY)]))
    );
}
