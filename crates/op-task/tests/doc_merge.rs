use op_task::conflict::Labels;
use op_task::doc::{Doc, parse_partial, rewrite_parent};
use op_task::merge;

const CONFLICTED: &str = "---
created: 2026-01-01T00:00:00Z
<<<<<<< Ann (1111111)
parent: ./guides.md
=======
parent: ./reference.md
>>>>>>> Ben (2222222)
---
# Storage

<<<<<<< Ann (1111111)
Keep the index in memory.
=======
Keep the index in SQLite.
>>>>>>> Ben (2222222)
";

fn doc(text: &str) -> Doc {
    Doc::from_file_string("storage".to_owned(), text).expect("a doc")
}

fn file(frontmatter: &str, body: &str) -> String {
    format!("---\ncreated: 2026-01-01T00:00:00Z\n{frontmatter}---\n# Storage\n\n{body}")
}

fn labels() -> Labels {
    Labels {
        ours: "Ann (1111111)".to_owned(),
        theirs: "Ben (2222222)".to_owned(),
    }
}

#[test]
fn a_conflicted_doc_reads_its_published_version_and_keeps_the_other() {
    let read = doc(CONFLICTED);

    assert_eq!(read.frontmatter.parent.as_deref(), Some("reference"));
    assert_eq!(read.conflicts.len(), 1);
    assert_eq!(read.conflicts[0].field, "parent");
    assert_eq!(read.conflicts[0].other, Some("guides".into()));
    assert_eq!(read.conflict_count(), 2);
    assert_eq!(read.title().as_deref(), Some("Storage"));
    assert_eq!(read.to_file_string().unwrap(), CONFLICTED);
}

#[test]
fn setting_the_parent_settles_its_conflict() {
    let mut read = doc(CONFLICTED);

    read.set_parent(Some("guides")).unwrap();

    assert!(read.conflicts.is_empty());
    assert_eq!(read.conflict_count(), 1);
}

#[test]
fn the_lenient_reader_counts_the_same_conflicts() {
    let partial = parse_partial(CONFLICTED);

    assert_eq!(partial.parent(), Some("reference"));
    assert_eq!(partial.conflict_count(), 2);
}

// Both sides created the doc, so each version of the body opens with its own title.
#[test]
fn a_title_inside_a_conflict_block_leaves_the_body_whole() {
    let body = "<<<<<<< Ann (1111111)\n# Storage\n\nIn memory.\n=======\n# Storage\n\nIn SQLite.\n>>>>>>> Ben (2222222)\n";
    let read = doc(&format!("---\ncreated: 2026-01-01T00:00:00Z\n---\n{body}"));

    assert_eq!(read.content(), body);
    assert_eq!(read.title().as_deref(), Some("Storage"));
}

#[test]
fn a_block_above_the_title_leaves_the_body_whole() {
    let body = "<<<<<<< Ann (1111111)\nDraft.\n=======\nFinal.\n>>>>>>> Ben (2222222)\n# Storage\n\nIn SQLite.\n";
    let read = doc(&format!("---\ncreated: 2026-01-01T00:00:00Z\n---\n{body}"));

    assert_eq!(read.content(), body);
}

#[test]
fn a_rename_carries_both_versions_of_a_parent_conflict() {
    let rewritten = rewrite_parent(CONFLICTED, "guides", Some("handbook")).expect("a child");
    let read = doc(&rewritten);

    assert_eq!(read.frontmatter.parent.as_deref(), Some("reference"));
    assert_eq!(read.conflicts[0].other, Some("handbook".into()));
    assert!(rewritten.contains("Keep the index in memory."));
}

#[test]
fn two_parents_for_one_doc_keep_both_with_the_published_one_in_force() {
    let base = doc(&file("", "Keep an index.\n"));
    let ours = doc(&file("parent: ./guides.md\n", "Keep an index.\n"));
    let theirs = doc(&file("parent: ./reference.md\n", "Keep an index.\n"));

    let merged = merge::doc(Some(&base), &ours, &theirs, &labels());

    assert_eq!(merged.frontmatter.parent.as_deref(), Some("reference"));
    assert_eq!(merged.conflicts.len(), 1);
    assert_eq!(merged.conflicts[0].other, Some("guides".into()));
    let text = merged.to_file_string().unwrap();
    assert_eq!(doc(&text), merged);
}

#[test]
fn edits_to_different_parts_of_a_doc_both_stay_without_a_conflict() {
    let base = doc(&file("", "One.\n\nTwo.\n"));
    let ours = doc(&file("parent: ./guides.md\n", "One from Ann.\n\nTwo.\n"));
    let theirs = doc(&file("", "One.\n\nTwo from Ben.\n"));

    let merged = merge::doc(Some(&base), &ours, &theirs, &labels());

    assert_eq!(merged.frontmatter.parent.as_deref(), Some("guides"));
    assert_eq!(merged.conflict_count(), 0);
    assert_eq!(merged.content(), "One from Ann.\n\nTwo from Ben.\n");
}

// No command sets `created`, so a doc both sides created keeps the earlier time rather than a
// conflict nobody could settle.
#[test]
fn a_doc_both_sides_created_keeps_the_earlier_time_and_both_bodies() {
    let ours = doc("---\ncreated: 2026-02-01T00:00:00Z\n---\n# Storage\n\nIn memory.\n");
    let theirs = doc("---\ncreated: 2026-03-01T00:00:00Z\n---\n# Storage\n\nIn SQLite.\n");

    let merged = merge::doc(None, &ours, &theirs, &labels());

    assert_eq!(
        merged.frontmatter.created,
        "2026-02-01T00:00:00Z".parse().unwrap()
    );
    assert!(merged.conflicts.is_empty());
    assert_eq!(merged.conflict_count(), 1);
    assert_eq!(merged.title().as_deref(), Some("Storage"));
    let text = merged.to_file_string().unwrap();
    assert_eq!(doc(&text), merged);
}

// Ann's version names the deleted doc, and it moves up to the parent Ben's version names already.
#[test]
fn a_conflict_whose_versions_agree_after_a_move_is_settled() {
    let child = file(
        "<<<<<<< Ann (1111111)\nparent: ./storage.md\n=======\nparent: ./reference.md\n>>>>>>> Ben (2222222)\n",
        "Text.\n",
    );

    let moved = rewrite_parent(&child, "storage", Some("reference")).expect("a child");

    let read = doc(&moved);
    assert_eq!(read.frontmatter.parent.as_deref(), Some("reference"));
    assert!(read.conflicts.is_empty());
}
