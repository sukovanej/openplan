use std::collections::BTreeMap;
use std::sync::Arc;

use op_backend::{Actor, Change, ChangeKind, LogEntry, MemorySnapshot, Revision, RevisionId};
use op_index::Index;
use op_tracker::Plan;

const CONFIG: &str = "abbreviation = \"OPP\"\n";

fn task(title: &str) -> String {
    format!("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# {title}\n")
}

fn built(files: &[(u64, String)]) -> Index {
    let mut documents = BTreeMap::from([("config.toml".to_owned(), CONFIG.as_bytes().to_vec())]);
    for (number, text) in files {
        documents.insert(
            format!("tasks/{number:05}-task-{number}.md"),
            text.as_bytes().to_vec(),
        );
    }
    let plan = Plan::read(Arc::new(MemorySnapshot::new(None, documents))).expect("plan");
    let mut index = Index::new();
    index.load(&plan).expect("load");
    index
}

fn changed(number: u64, at: &str) -> LogEntry {
    LogEntry {
        revision: Revision {
            id: RevisionId::new(at),
            parents: Vec::new(),
            author: Actor::new("Ada"),
            at: at.parse().expect("time"),
            message: String::new(),
        },
        changes: vec![Change::new(
            format!("tasks/{number:05}-task-{number}.md"),
            ChangeKind::Modified,
        )],
    }
}

fn key(number: u64) -> String {
    format!("OPP-{number}")
}

fn ids(index: &Index, query: &str) -> Vec<String> {
    index
        .search("test", query)
        .into_iter()
        .map(|hit| hit.task.id)
        .collect()
}

fn seeded() -> Index {
    built(&[
        (
            1,
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship the login page\n\nSupport OAuth.\n".to_owned(),
        ),
        (
            2,
            "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\nparent: ./00001-task-1.md\n---\n# Add validation\n".to_owned(),
        ),
    ])
}

#[test]
fn the_title_matches_without_regard_to_case() {
    let index = seeded();
    assert_eq!(ids(&index, "LOGIN"), vec![key(1)]);
    assert_eq!(ids(&index, "login page"), vec![key(1)]);
}

#[test]
fn the_body_matches() {
    assert_eq!(ids(&seeded(), "oauth"), vec![key(1)]);
}

#[test]
fn the_frontmatter_fields_match() {
    let index = seeded();
    assert_eq!(ids(&index, "done"), vec![key(2)], "the status");
    assert_eq!(
        ids(&index, "OPP-1"),
        vec![key(1), key(2)],
        "the key finds the task itself and the child that names it as a parent"
    );
}

#[test]
fn a_key_finds_its_own_task() {
    let index = seeded();
    assert_eq!(ids(&index, "opp-2"), vec![key(2)], "case does not matter");
    assert_eq!(
        ids(&index, "OPP-"),
        vec![key(1), key(2)],
        "so does a prefix"
    );
}

#[test]
fn a_query_of_nothing_but_spaces_matches_nothing() {
    let index = seeded();
    assert!(ids(&index, "  ").is_empty());
    assert!(ids(&index, "").is_empty());
    assert_eq!(ids(&index, "login page"), vec![key(1)]);
}

#[test]
fn a_query_that_matches_nothing_returns_nothing() {
    assert!(ids(&seeded(), "kubernetes").is_empty());
}

#[test]
fn hits_changed_at_the_same_time_are_ordered_by_id_as_numbers() {
    let index = built(&[
        (1, task("Shared word")),
        (2, task("Shared word")),
        (10, task("Shared word")),
    ]);
    assert_eq!(ids(&index, "shared"), vec![key(1), key(2), key(10)]);
}

#[test]
fn a_key_hit_leads_a_title_hit_and_a_title_hit_leads_a_body_hit() {
    let index = built(&[
        (1, task("Name the zeppelin")),
        (2, format!("{}\nA zeppelin needs a mast.\n", task("Fly it"))),
        (3, task("Land it")),
    ]);
    assert_eq!(ids(&index, "zeppelin"), vec![key(1), key(2)]);
}

#[test]
fn a_key_hit_leads_a_task_that_only_names_that_key() {
    let index = built(&[
        (1, task("A parent")),
        (2, task("OPP-3 in the title")),
        (3, task("A child")),
    ]);
    assert_eq!(ids(&index, "OPP-3"), vec![key(3), key(2)]);
}

#[test]
fn the_task_changed_last_leads_the_hits_that_matched_the_same_way() {
    let mut index = built(&[
        (1, task("Shared word")),
        (2, task("Shared word again")),
        (3, task("Shared word once more")),
    ]);
    index.date(&[
        changed(3, "2026-03-01T00:00:00Z"),
        changed(2, "2026-02-01T00:00:00Z"),
        changed(1, "2026-01-01T00:00:00Z"),
    ]);
    assert_eq!(ids(&index, "shared"), vec![key(3), key(2), key(1)]);
}

#[test]
fn a_key_hit_leads_a_title_hit_the_change_time_cannot_overturn() {
    let mut index = built(&[(1, task("A parent")), (2, task("OPP-1 in the title"))]);
    index.date(&[
        changed(2, "2026-03-01T00:00:00Z"),
        changed(1, "2026-01-01T00:00:00Z"),
    ]);
    assert_eq!(ids(&index, "OPP-1"), vec![key(1), key(2)]);
}
