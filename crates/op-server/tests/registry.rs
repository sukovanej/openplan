use std::path::PathBuf;

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

fn add(registry: &mut ProjectRegistry, path: PathBuf) -> &ProjectEntry {
    let name = unique_name(&path, |name| registry.holds_name(name));
    registry.insert(ProjectEntry { name, path })
}
