use std::path::Path;
use std::process::Command;

fn oplan() -> Command {
    Command::new(env!("CARGO_BIN_EXE_oplan"))
}

fn write(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
}

#[test]
fn list_reports_real_status_and_title() {
    let dir = tempfile::tempdir().unwrap();
    let tasks = dir.path().join(".plan/tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    write(
        &tasks.join("shipit.md"),
        "---\nstatus: done\n---\n# Ship it\n",
    );

    let out = oplan()
        .arg("--root")
        .arg(dir.path())
        .arg("list")
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("shipit"), "{stdout}");
    assert!(
        stdout.contains("Done"),
        "status must be read from the file: {stdout}"
    );
    assert!(
        stdout.contains("Ship it"),
        "title must be read from the body: {stdout}"
    );
}

#[test]
fn list_discovers_store_from_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    let nested = dir.path().join("crates/thing/src");
    std::fs::create_dir_all(&nested).unwrap();

    let out = oplan().current_dir(&nested).arg("list").output().unwrap();

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("no tasks yet"));
}

#[test]
fn merge_driver_clean_conflict_and_read_error() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.md");
    let ours = dir.path().join("ours.md");
    let theirs = dir.path().join("theirs.md");
    let content = "---\nstatus: todo\n---\n# T\n\n## Plan\nhi\n";
    write(&base, content);
    write(&ours, content);
    write(&theirs, content);

    let clean = oplan()
        .arg("merge-driver")
        .args([&base, &ours, &theirs])
        .output()
        .unwrap();
    assert!(
        clean.status.success(),
        "identical inputs should merge cleanly"
    );

    write(&theirs, "---\nstatus: todo\n---\n# T\n\n## Plan\nBYE\n");
    let conflict = oplan()
        .arg("merge-driver")
        .args([&base, &ours, &theirs])
        .output()
        .unwrap();
    assert!(
        !conflict.status.success(),
        "divergent inputs should conflict"
    );

    let missing = dir.path().join("nope.md");
    let read_error = oplan()
        .arg("merge-driver")
        .args([&missing, &missing, &missing])
        .output()
        .unwrap();
    assert!(
        !read_error.status.success(),
        "unreadable inputs must fail, not report a clean merge"
    );
}
