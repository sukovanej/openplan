use std::path::PathBuf;

use op_api::BackendKind;
use op_server::{ProjectEntry, ProjectRegistry, unique_name};

#[test]
fn a_missing_file_is_absence_rather_than_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let registry = ProjectRegistry::read(&dir.path().join("registry.toml")).unwrap();
    assert!(registry.is_none(), "the first start has no registry yet");
}

#[test]
fn an_entry_survives_a_write_and_a_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.toml");

    let mut written = ProjectRegistry::default();
    add(&mut written, PathBuf::from("/Users/dev/Projects/openplan"));
    written.write(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("[[project]]"), "{text}");
    assert!(text.contains("openplan"), "{text}");

    let read = ProjectRegistry::read(&path).unwrap().unwrap();
    assert_eq!(read, written);
    assert_eq!(read.entries()[0].name, "openplan");
}

// The name is machine-local and reaches the URL, so two checkouts of the same directory name must
// still address two projects.
#[test]
fn a_repeated_directory_name_gets_a_distinct_project_name() {
    let mut registry = ProjectRegistry::default();
    for parent in ["/a", "/b", "/c"] {
        add(&mut registry, PathBuf::from(parent).join("openplan"));
    }
    let names: Vec<&str> = registry
        .entries()
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(names, vec!["openplan", "openplan-2", "openplan-3"]);
}

#[test]
fn a_directory_name_with_no_letters_falls_back_to_a_usable_name() {
    let mut registry = ProjectRegistry::default();
    assert_eq!(
        add(&mut registry, PathBuf::from("/srv/___")).name,
        "project"
    );
}

#[test]
fn a_malformed_registry_names_its_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.toml");
    std::fs::write(&path, "project = [ oops").unwrap();

    let err = ProjectRegistry::read(&path).expect_err("a broken registry cannot be guessed at");
    assert!(err.to_string().contains("registry.toml"), "{err}");
}

// An entry written before the choice existed has no `backend`, and the daemon reads what its path
// holds.
#[test]
fn an_entry_keeps_its_backend_and_an_old_entry_reads_without_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("registry.toml");
    std::fs::write(&path, "[[project]]\nname = \"old\"\npath = \"/srv/old\"\n").unwrap();
    let mut registry = ProjectRegistry::read(&path).unwrap().unwrap();
    assert_eq!(registry.entries()[0].backend, None);

    registry.insert(ProjectEntry {
        name: "new".to_owned(),
        path: PathBuf::from("/srv/new"),
        backend: Some(BackendKind::Git),
    });
    registry.write(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("backend = \"git\""), "{text}");
    assert_eq!(text.matches("backend").count(), 1, "{text}");
    let read = ProjectRegistry::read(&path).unwrap().unwrap();
    assert_eq!(read, registry);
}

#[test]
fn a_rename_keeps_the_entry_in_its_place() {
    let mut registry = ProjectRegistry::default();
    for parent in ["/a", "/b", "/c"] {
        add(&mut registry, PathBuf::from(parent).join("openplan"));
    }
    let renamed = ProjectEntry {
        name: "work".to_owned(),
        path: PathBuf::from("/b/openplan"),
        backend: Some(BackendKind::Local),
    };
    assert!(registry.replace("openplan-2", renamed.clone()));
    assert!(!registry.replace("ghost", renamed.clone()));
    assert_eq!(registry.entries()[1], renamed);
    assert!(registry.holds_name("work"));
    assert!(!registry.holds_name("openplan-2"));

    assert_eq!(registry.remove("work"), Some(renamed));
    assert_eq!(registry.entries().len(), 2);
}

fn add(registry: &mut ProjectRegistry, path: PathBuf) -> &ProjectEntry {
    let name = unique_name(&path, |name| registry.holds_name(name));
    registry.insert(ProjectEntry {
        name,
        path,
        backend: None,
    })
}
