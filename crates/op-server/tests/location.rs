mod common;

use std::path::Path;

use common::repository;
use op_api::BackendKind;
use op_server::{Location, OpenError, STORE_DIR};

fn canonical(path: &Path) -> std::path::PathBuf {
    path.canonicalize().unwrap()
}

fn store_with_history(root: &Path) {
    std::fs::create_dir_all(root.join(STORE_DIR)).unwrap();
    std::fs::write(
        root.join(STORE_DIR).join(op_backend_local::HISTORY_FILE),
        "",
    )
    .unwrap();
}

// The daemon's home is `~/.plan`: a directory of that name without a history is not a task store.
#[test]
fn a_plan_directory_without_history_above_a_project_is_no_store() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(STORE_DIR)).unwrap();
    let project = home.path().join("projects/foo");
    std::fs::create_dir_all(&project).unwrap();

    assert!(matches!(
        Location::find(&project, None),
        Err(OpenError::NoStore(path)) if path == canonical(&project)
    ));
    let local = Location::find(&project, Some(BackendKind::Local)).unwrap();
    assert_eq!(local.root, canonical(&project));

    repository(&project);
    assert!(matches!(
        Location::find(&project, None),
        Err(OpenError::NoStore(_))
    ));
}

#[test]
fn a_local_store_is_found_from_a_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    store_with_history(dir.path());
    let nested = dir.path().join("a/b");
    std::fs::create_dir_all(&nested).unwrap();

    let location = Location::find(&nested, None).unwrap();
    assert_eq!(location.kind, BackendKind::Local);
    assert_eq!(location.root, canonical(dir.path()));
    assert_eq!(
        Location::find(&nested, Some(BackendKind::Local))
            .unwrap()
            .root,
        canonical(dir.path())
    );
}

#[test]
fn a_store_copied_by_hand_is_local() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(STORE_DIR)).unwrap();
    std::fs::write(
        dir.path().join(STORE_DIR).join("config.toml"),
        "abbreviation = \"OPP\"\n",
    )
    .unwrap();

    let location = Location::find(dir.path(), None).unwrap();
    assert_eq!(location.kind, BackendKind::Local);
    assert_eq!(location.root, canonical(dir.path()));
}

#[test]
fn tasks_beside_the_code_need_a_migration() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(STORE_DIR)).unwrap();
    let checkout = home.path().join("repo");
    std::fs::create_dir_all(checkout.join(STORE_DIR).join("tasks")).unwrap();
    std::fs::write(
        checkout.join(STORE_DIR).join("config.toml"),
        "abbreviation = \"OPP\"\n",
    )
    .unwrap();
    repository(&checkout);
    let nested = checkout.join("src");
    std::fs::create_dir_all(&nested).unwrap();

    assert!(matches!(
        Location::find(&nested, None),
        Err(OpenError::NeedsMigration(root)) if root == canonical(&checkout)
    ));
}
