use std::collections::BTreeMap;
use std::sync::Arc;

use op_backend::{Actor, Change, ChangeKind, LogEntry, MemorySnapshot, Revision, RevisionId};
use op_index::Index;
use op_tracker::Plan;

fn plan(files: &[(&str, &str)]) -> Plan {
    let documents: BTreeMap<String, Vec<u8>> = files
        .iter()
        .map(|(path, text)| ((*path).to_owned(), text.as_bytes().to_vec()))
        .collect();
    Plan::read(Arc::new(MemorySnapshot::new(None, documents))).expect("plan")
}

const CONFIG: (&str, &str) = ("config.toml", "abbreviation = \"OPP\"\n");
const PARENT: (&str, &str) = (
    "tasks/00001-parent.md",
    "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Parent\n\nSee [[./00002-child.md]].\n\n## Comments\n\n### 2026-01-02T00:00:00Z by Ada\n\n> Hello.\n",
);
const CHILD: (&str, &str) = (
    "tasks/00002-child.md",
    "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\nparent: ./00001-parent.md\ndependencies: [./00001-parent.md]\n---\n# Child\n",
);

#[test]
fn a_detail_carries_its_neighbourhood_and_its_comments_apart() {
    let mut index = Index::new();
    index.load(&plan(&[CONFIG, PARENT, CHILD])).expect("load");
    let parent = index.detail("p", 1).expect("detail");
    assert_eq!(parent.id, "OPP-1");
    assert_eq!(parent.children.len(), 1);
    assert_eq!(parent.children[0].id, "OPP-2");
    assert_eq!(parent.blocks.len(), 1);
    assert_eq!(parent.refs.len(), 1);
    assert_eq!(parent.comments.len(), 1);
    assert_eq!(parent.title, "Parent");
    assert_eq!(parent.description, "See [[OPP-2]].\n");
    let child = index.detail("p", 2).expect("detail");
    assert_eq!(child.parent_title.as_deref(), Some("Parent"));
    assert_eq!(child.depends_on.len(), 1);
    assert_eq!(index.list("p").len(), 2);
    assert_eq!(index.item("p", 1).expect("row").comment_count, 1);
}

#[test]
fn a_load_follows_the_plan_it_is_given() {
    let mut index = Index::new();
    index.load(&plan(&[CONFIG, PARENT, CHILD])).expect("load");
    index.load(&plan(&[CONFIG, CHILD])).expect("load");
    assert!(!index.contains(1));
    assert_eq!(index.max_number(), Some(2));
    index
        .load(&plan(&[("config.toml", "abbreviation = \"WEB\"\n"), CHILD]))
        .expect("load");
    assert_eq!(index.detail("p", 2).expect("detail").id, "WEB-2");
    index.load(&plan(&[CHILD])).expect("load");
    assert!(
        index.list("p").is_empty(),
        "a project with no config has no keys"
    );
}

#[test]
fn updated_is_the_newest_change() {
    let mut index = Index::new();
    index.load(&plan(&[CONFIG, PARENT])).expect("load");
    let row = index.item("p", 1).expect("row");
    assert!(matches!(
        row.updated,
        op_api::Field::Error(op_api::FieldError::Missing)
    ));
    let entry = |at: &str| LogEntry {
        revision: Revision {
            id: RevisionId::new(at),
            parents: Vec::new(),
            author: Actor::new("Ada"),
            at: at.parse().expect("time"),
            message: String::new(),
        },
        changes: vec![Change::new("tasks/00001-parent.md", ChangeKind::Modified)],
    };
    index.date(&[entry("2026-03-01T00:00:00Z"), entry("2026-02-01T00:00:00Z")]);
    let row = index.item("p", 1).expect("row");
    assert!(
        matches!(row.updated, op_api::Field::Value(at) if at.0.to_string() == "2026-03-01T00:00:00Z")
    );
    index.touch(1, "2026-04-01T00:00:00Z".parse().expect("time"));
    let row = index.item("p", 1).expect("row");
    assert!(
        matches!(row.updated, op_api::Field::Value(at) if at.0.to_string() == "2026-04-01T00:00:00Z")
    );
}
