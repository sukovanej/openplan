use std::process::Command;

fn task(status: &str, body: &str) -> String {
    format!("---\nstatus: {status}\ncreated: 2026-01-01T00:00:00Z\n---\n{body}")
}

struct Run {
    merged: String,
    code: i32,
    stderr: String,
}

fn drive(base: &str, ours: &str, theirs: &str) -> Run {
    let dir = tempfile::tempdir().unwrap();
    let at = |name: &str, text: &str| {
        let path = dir.path().join(name);
        std::fs::write(&path, text).unwrap();
        path
    };
    let (base, ours, theirs) = (at("base", base), at("ours", ours), at("theirs", theirs));
    let output = Command::new(env!("CARGO_BIN_EXE_openplan"))
        .arg("merge-driver")
        .args([&base, &ours, &theirs])
        .args(["7", ".plan/tasks/00001-t.md", "base", "ours", "theirs"])
        .output()
        .unwrap();
    Run {
        merged: std::fs::read_to_string(&ours).unwrap(),
        code: output.status.code().unwrap(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[test]
fn edits_to_different_sections_merge() {
    let base = task(
        "todo",
        "# T\n\n## Alpha\n\nbase alpha\n\n## Beta\n\nbase beta\n",
    );
    let ours = task(
        "todo",
        "# T\n\n## Alpha\n\nOURS alpha\n\n## Beta\n\nbase beta\n",
    );
    let theirs = task(
        "todo",
        "# T\n\n## Alpha\n\nbase alpha\n\n## Beta\n\nTHEIRS beta\n",
    );

    let run = drive(&base, &ours, &theirs);

    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.merged.contains("OURS alpha"), "{}", run.merged);
    assert!(run.merged.contains("THEIRS beta"), "{}", run.merged);
    assert!(!run.merged.contains("<<<<<<<"), "{}", run.merged);
}

#[test]
fn edits_to_different_frontmatter_fields_merge() {
    let body = "# T\n\n## Alpha\n\nalpha\n";
    let base = task("todo", body);
    let ours = format!("---\nstatus: in_progress\ncreated: 2026-01-01T00:00:00Z\n---\n{body}");
    let theirs = format!("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nrank: m\n---\n{body}");

    let run = drive(&base, &ours, &theirs);

    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.merged.contains("status: in_progress"), "{}", run.merged);
    assert!(run.merged.contains("rank: m"), "{}", run.merged);
}

#[test]
fn both_sides_adding_a_tag_keep_both() {
    let body = "# T\n\n## Alpha\n\nalpha\n";
    let head = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z";
    let base = format!("{head}\ntags:\n- bug\n---\n{body}");
    let ours = format!("{head}\ntags:\n- bug\n- cli\n---\n{body}");
    let theirs = format!("{head}\ntags:\n- bug\n- ui\n---\n{body}");

    let run = drive(&base, &ours, &theirs);

    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    for tag in ["bug", "cli", "ui"] {
        assert!(run.merged.contains(&format!("- {tag}")), "{}", run.merged);
    }
}

#[test]
fn one_side_removing_a_tag_removes_it() {
    let body = "# T\n\n## Alpha\n\nalpha\n";
    let head = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z";
    let base = format!("{head}\ntags:\n- bug\n- cli\n---\n{body}");
    let ours = format!("{head}\ntags:\n- bug\n---\n{body}");
    let theirs = format!("{head}\ntags:\n- bug\n- cli\n- ui\n---\n{body}");

    let run = drive(&base, &ours, &theirs);

    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(!run.merged.contains("- cli"), "{}", run.merged);
    assert!(run.merged.contains("- ui"), "{}", run.merged);
}

#[test]
fn a_new_section_on_each_side_keeps_both() {
    let base = task("todo", "# T\n\n## Alpha\n\nalpha\n");
    let ours = task("todo", "# T\n\n## Alpha\n\nalpha\n\n## Ours\n\nours\n");
    let theirs = task("todo", "# T\n\n## Alpha\n\nalpha\n\n## Theirs\n\ntheirs\n");

    let run = drive(&base, &ours, &theirs);

    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(run.merged.contains("## Ours"), "{}", run.merged);
    assert!(run.merged.contains("## Theirs"), "{}", run.merged);
}

#[test]
fn one_side_deleting_a_section_deletes_it() {
    let base = task("todo", "# T\n\n## Alpha\n\nalpha\n\n## Beta\n\nbeta\n");
    let ours = task("todo", "# T\n\n## Alpha\n\nalpha\n");
    let theirs = task(
        "todo",
        "# T\n\n## Alpha\n\nOURS-FREE alpha\n\n## Beta\n\nbeta\n",
    );

    let run = drive(&base, &ours, &theirs);

    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert!(!run.merged.contains("## Beta"), "{}", run.merged);
    assert!(run.merged.contains("OURS-FREE alpha"), "{}", run.merged);
}

#[test]
fn the_same_section_on_both_sides_conflicts() {
    let base = task("todo", "# T\n\n## Alpha\n\nbase alpha\n\n## Beta\n\nbeta\n");
    let ours = task("todo", "# T\n\n## Alpha\n\nOURS alpha\n\n## Beta\n\nbeta\n");
    let theirs = task(
        "todo",
        "# T\n\n## Alpha\n\nTHEIRS alpha\n\n## Beta\n\nbeta\n",
    );

    let run = drive(&base, &ours, &theirs);

    assert_eq!(run.code, 1);
    assert!(run.merged.contains("<<<<<<< ours"), "{}", run.merged);
    assert!(run.merged.contains(">>>>>>> theirs"), "{}", run.merged);
    assert!(run.stderr.contains("\"Alpha\""), "stderr: {}", run.stderr);
    // The section that agreed stays out of the markers.
    assert_eq!(run.merged.matches("<<<<<<<").count(), 1, "{}", run.merged);
    assert!(run.merged.contains("## Beta\n\nbeta\n"), "{}", run.merged);
}

#[test]
fn the_same_frontmatter_field_on_both_sides_conflicts() {
    let body = "# T\n\n## Alpha\n\nalpha\n";
    let run = drive(
        &task("todo", body),
        &task("in_progress", body),
        &task("done", body),
    );

    assert_eq!(run.code, 1);
    assert!(run.merged.contains("<<<<<<< ours"), "{}", run.merged);
    assert!(run.stderr.contains("frontmatter"), "stderr: {}", run.stderr);
}

#[test]
fn a_file_that_holds_conflict_markers_does_not_panic() {
    let marked = task(
        "todo",
        "# T\n\n## Alpha\n\n<<<<<<< ours\na\n=======\nb\n>>>>>>> theirs\n",
    );
    let run = drive(
        &task("todo", "# T\n\n## Alpha\n\nbase\n"),
        &marked,
        &task("todo", "# T\n\n## Alpha\n\nother\n"),
    );

    assert_eq!(run.code, 1);
    assert!(!run.stderr.contains("panicked"), "stderr: {}", run.stderr);
}

#[test]
fn an_unparsable_side_falls_back_to_the_whole_file() {
    let run = drive("not a task", "still not a task", "different rubbish");

    assert_eq!(run.code, 1);
    assert!(run.merged.contains("<<<<<<< ours"), "{}", run.merged);
    assert!(run.merged.contains("still not a task"), "{}", run.merged);
    assert!(run.merged.contains("different rubbish"), "{}", run.merged);
}

#[test]
fn an_unchanged_side_keeps_the_other_verbatim() {
    let base = task("todo", "# T\n\n## Alpha\n\nalpha\n");
    let theirs = task("done", "# T\n\n## Alpha\n\nTHEIRS\n");

    let run = drive(&base, &base, &theirs);

    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    assert_eq!(run.merged, theirs);
}

#[test]
fn an_unreadable_side_fails_rather_than_reporting_a_merge() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nope.md");
    let output = Command::new(env!("CARGO_BIN_EXE_openplan"))
        .arg("merge-driver")
        .args([&missing, &missing, &missing])
        .output()
        .unwrap();

    assert!(!output.status.success());
}
