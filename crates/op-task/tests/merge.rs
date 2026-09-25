use op_task::conflict::Labels;
use op_task::{Status, Task, merge};

fn task(text: &str) -> Task {
    Task::from_file_string(text).expect("a task")
}

fn labels() -> Labels {
    Labels {
        ours: "Ann (1111111)".to_owned(),
        theirs: "Ben (2222222)".to_owned(),
    }
}

fn merged(base: &str, ours: &str, theirs: &str) -> Task {
    merge::task(&task(base), &task(ours), &task(theirs), &labels())
}

const BASE: &str = "---
status: todo
created: 2026-01-01T00:00:00Z
---
# Parser

## Goal

Parse.

## Plan

Later.
";

#[test]
fn edits_to_different_fields_and_lines_combine() {
    let ours = BASE
        .replace("status: todo", "status: in_progress")
        .replace("Later.", "Now.");
    let theirs = BASE.replace("Parse.", "Parse fast.");

    let task = merged(BASE, &ours, &theirs);

    assert_eq!(task.conflict_count(), 0);
    assert_eq!(
        task.to_file_string().expect("text"),
        BASE.replace("status: todo", "status: in_progress")
            .replace("Later.", "Now.")
            .replace("Parse.", "Parse fast.")
    );
}

#[test]
fn a_field_both_sides_changed_keeps_both_with_theirs_in_force() {
    let ours = BASE.replace("status: todo", "status: done");
    let theirs = BASE.replace("status: todo", "status: cancelled");

    let task = merged(BASE, &ours, &theirs);

    assert_eq!(task.frontmatter.status, Status::Cancelled);
    assert_eq!(task.conflict_count(), 1);
    let text = task.to_file_string().expect("text");
    assert!(
        text.contains(
            "<<<<<<< Ann (1111111)\nstatus: done\n=======\nstatus: cancelled\n>>>>>>> Ben (2222222)\n"
        ),
        "{text}"
    );
    assert_eq!(Task::from_file_string(&text).expect("a task"), task);
}

#[test]
fn lines_both_sides_changed_keep_both_with_theirs_in_force() {
    let ours = BASE.replace("Parse.", "Parse ours.");
    let theirs = BASE.replace("Parse.", "Parse theirs.");

    let task = merged(BASE, &ours, &theirs);

    assert!(
        task.body.contains(
            "<<<<<<< Ann (1111111)\nParse ours.\n=======\nParse theirs.\n>>>>>>> Ben (2222222)\n"
        ),
        "{}",
        task.body
    );
    assert_eq!(task.conflict_count(), 1);
    assert_eq!(task.title().as_deref(), Some("Parser"));
}

#[test]
fn a_title_both_sides_changed_reads_as_the_published_one() {
    let ours = BASE.replace("# Parser", "# Parser for ours");
    let theirs = BASE.replace("# Parser", "# Parser for theirs");

    let task = merged(BASE, &ours, &theirs);

    assert_eq!(task.title().as_deref(), Some("Parser for theirs"));
    assert_eq!(task.conflict_count(), 1);
}

#[test]
fn a_conflict_one_side_resolved_stays_resolved() {
    let conflicted = merged(
        BASE,
        &BASE.replace("status: todo", "status: done"),
        &BASE.replace("status: todo", "status: cancelled"),
    );
    let base = conflicted.to_file_string().expect("text");
    let mut resolved = conflicted.clone();
    resolved.set_status(Status::Done);
    let resolved = resolved.to_file_string().expect("text");
    let theirs = base.replace("Later.", "Later, with notes.");

    let task = merged(&base, &resolved, &theirs);

    assert_eq!(task.frontmatter.status, Status::Done);
    assert_eq!(task.conflict_count(), 0);
    assert!(task.body.contains("Later, with notes."));
}

#[test]
fn a_new_conflict_over_an_old_one_does_not_nest() {
    let conflicted = merged(
        BASE,
        &BASE.replace("Parse.", "Parse ours."),
        &BASE.replace("Parse.", "Parse theirs."),
    );
    let base = conflicted.to_file_string().expect("text");
    let blocked = op_task::conflict::in_body(&conflicted.body);
    let ours = base.replace(
        &conflicted.body[blocked[0].range.clone()],
        "Parse by Ann.\n",
    );
    let theirs = base.replace(
        &conflicted.body[blocked[0].range.clone()],
        "Parse by Ben.\n",
    );

    let task = merged(&base, &ours, &theirs);

    assert_eq!(task.body.matches("<<<<<<<").count(), 1, "{}", task.body);
    assert!(task.body.contains("Parse by Ann.") && task.body.contains("Parse by Ben."));
}

#[test]
fn comments_from_both_sides_are_kept_in_time_order() {
    let with = |entries: &str| format!("{BASE}\n## Comments\n\n{entries}");
    let first = "### 2026-01-02T00:00:00Z by Ada\n\n> first\n";
    let ours = with(&format!(
        "{first}\n### 2026-01-04T00:00:00Z by Ada\n\n> ours\n"
    ));
    let theirs = with(&format!(
        "{first}\n### 2026-01-03T00:00:00Z by Bob\n\n> theirs\n"
    ));

    let task = merged(&with(first), &ours, &theirs);

    let texts: Vec<String> = op_task::comment::parse(&task.body)
        .into_iter()
        .map(|comment| comment.text)
        .collect();
    assert_eq!(texts, vec!["first", "theirs", "ours"]);
    assert_eq!(task.conflict_count(), 0);
}

#[test]
fn tags_and_dependencies_merge_as_sets() {
    let base = BASE.replace("status: todo", "status: todo\ntags: [a]");
    let ours = BASE.replace("status: todo", "status: todo\ntags: [a, b]");
    let theirs = BASE.replace("status: todo", "status: todo\ntags: [c]");

    let task = merged(&base, &ours, &theirs);

    assert_eq!(task.frontmatter.tags, vec!["b", "c"]);
    assert_eq!(task.conflict_count(), 0);
}
