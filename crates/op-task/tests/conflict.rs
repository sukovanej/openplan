use op_task::conflict::{self, Labels, Side};
use op_task::{Status, Task, parse_partial};

fn labels() -> Labels {
    Labels {
        ours: "Ann (1111111)".to_owned(),
        theirs: "Ben (2222222)".to_owned(),
    }
}

#[test]
fn a_block_reads_back_with_both_versions_and_labels() {
    let text =
        "before\n<<<<<<< Ann (1111111)\nmine\n=======\nyours\n>>>>>>> Ben (2222222)\nafter\n";

    let blocks = conflict::blocks(text);

    assert_eq!(blocks.len(), 1);
    assert_eq!(
        blocks[0].ours,
        Side {
            label: "Ann (1111111)".to_owned(),
            text: "mine\n".to_owned()
        }
    );
    assert_eq!(blocks[0].theirs.text, "yours\n");
    assert_eq!(blocks[0].theirs.label, "Ben (2222222)");
    assert_eq!(&text[blocks[0].range.clone()], &text[7..text.len() - 6]);
    assert_eq!(conflict::published(text), "before\nyours\nafter\n");
}

#[test]
fn a_base_section_is_skipped_as_git_diff3_writes_it() {
    let text = "<<<<<<< ours\na\n||||||| base\nb\n=======\nc\n>>>>>>> theirs\n";

    let blocks = conflict::blocks(text);

    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].ours.text, "a\n");
    assert_eq!(blocks[0].theirs.text, "c\n");
}

#[test]
fn markers_in_a_code_fence_or_an_unclosed_block_are_text() {
    let fenced = "```\n<<<<<<< a\nx\n=======\ny\n>>>>>>> b\n```\n";
    let unclosed = "<<<<<<< a\nx\n=======\ny\n";

    assert!(conflict::blocks(fenced).is_empty());
    assert!(conflict::blocks(unclosed).is_empty());
}

#[test]
fn markers_quoted_in_a_comment_are_no_conflict() {
    let mut task = Task::new("Title", Status::Todo, op_task::now());
    task.append_comment(&op_task::comment::NewComment {
        at: op_task::now(),
        author: "Ada".to_owned(),
        agent: None,
        text: "It read:\n<<<<<<< a\nx\n=======\ny\n>>>>>>> b".to_owned(),
    });

    assert_eq!(task.conflict_count(), 0, "{}", task.body);
}

#[test]
fn a_merge_keeps_both_sides_of_one_change_and_takes_the_rest() {
    let base = "one\ntwo\nthree\n";
    let ours = "one\ntwo, ours\nthree\nfour\n";
    let theirs = "zero\none\ntwo, theirs\nthree\n";

    let merged = conflict::merge(base, ours, theirs, &labels());

    assert_eq!(
        merged,
        "zero\none\n<<<<<<< Ann (1111111)\ntwo, ours\n=======\ntwo, theirs\n>>>>>>> Ben (2222222)\nthree\nfour\n"
    );
}

#[test]
fn a_conflict_in_a_code_fence_covers_the_whole_fence() {
    let base = "text\n```\na\nb\n```\n";
    let ours = "text\n```\na, ours\nb\n```\n";
    let theirs = "text\n```\na\nb, theirs\n```\n";

    let merged = conflict::merge(base, ours, theirs, &labels());
    let blocks = conflict::blocks(&merged);

    assert_eq!(blocks.len(), 1, "{merged}");
    assert_eq!(blocks[0].ours.text, "```\na, ours\nb\n```\n");
    assert_eq!(blocks[0].theirs.text, "```\na\nb, theirs\n```\n");
}

const CONFLICTED: &str = "---
<<<<<<< Ann (1111111)
status: done
=======
status: cancelled
>>>>>>> Ben (2222222)
created: 2026-01-01T00:00:00Z
<<<<<<< Ann (1111111)
parent: ./00001-root.md
=======
>>>>>>> Ben (2222222)
---
# Title
";

#[test]
fn a_frontmatter_block_reads_as_the_published_value_and_a_field_conflict() {
    let task = Task::from_file_string(CONFLICTED).expect("a task");

    assert_eq!(task.frontmatter.status, Status::Cancelled);
    assert_eq!(task.frontmatter.parent, None);
    let fields: Vec<&str> = task
        .conflicts
        .iter()
        .map(|conflict| conflict.field.as_str())
        .collect();
    assert_eq!(fields, vec!["status", "parent"]);
    assert_eq!(
        task.conflicts[0].other,
        Some(serde_yaml::Value::String("done".to_owned()))
    );
    assert_eq!(task.conflicts[0].other_label, "Ann (1111111)");
    assert_eq!(task.conflicts[0].label, "Ben (2222222)");
    assert_eq!(
        task.conflicts[0].other_fields().status.expect("a status"),
        Status::Done
    );

    let partial = parse_partial(CONFLICTED);
    assert_eq!(partial.conflicts, task.conflicts);
    assert_eq!(partial.conflict_count(), 2);
}

#[test]
fn setting_a_field_resolves_its_conflict_and_keeps_the_rest() {
    let mut task = Task::from_file_string(CONFLICTED).expect("a task");

    task.set_status(Status::Done);

    let text = task.to_file_string().expect("text");
    assert!(!text.contains("status: cancelled"), "{text}");
    assert!(
        text.contains(
            "<<<<<<< Ann (1111111)\nparent: ./00001-root.md\n=======\n>>>>>>> Ben (2222222)\n"
        ),
        "{text}"
    );
    assert_eq!(Task::from_file_string(&text).expect("a task"), task);
}
