use op_api::{Status, TaskPatch, TaskSummary, TaskTree};
use op_task::Task;

fn summary(id: &str, parent: Option<&str>, rank: Option<&str>) -> TaskSummary {
    TaskSummary {
        id: id.to_owned(),
        title: id.to_owned(),
        status: Status::Todo,
        parent: parent.map(str::to_owned),
        rank: rank.map(str::to_owned),
    }
}

fn child_ids(tree: &TaskTree) -> Vec<&str> {
    tree.children.iter().map(|c| c.id.as_str()).collect()
}

#[test]
fn orders_siblings_by_rank_then_unranked_by_id() {
    let summaries = vec![
        summary("root", None, None),
        summary("b", Some("root"), Some("m")),
        summary("a", Some("root"), Some("t")),
        summary("z", Some("root"), None),
        summary("y", Some("root"), None),
    ];
    let mut cycles = Vec::new();
    let tree = TaskTree::build(&summaries, "root", None, &mut cycles).unwrap();
    assert!(cycles.is_empty());
    // ranked "m" < "t" first, then unranked by id.
    assert_eq!(child_ids(&tree), vec!["b", "a", "y", "z"]);
}

#[test]
fn depth_one_bounds_to_direct_children() {
    let summaries = vec![
        summary("root", None, None),
        summary("child", Some("root"), Some("m")),
        summary("grandchild", Some("child"), Some("m")),
    ];
    let mut cycles = Vec::new();
    let tree = TaskTree::build(&summaries, "root", Some(1), &mut cycles).unwrap();
    assert_eq!(child_ids(&tree), vec!["child"]);
    assert!(tree.children[0].children.is_empty(), "depth 1 stops here");
}

#[test]
fn nested_subtree_is_built_recursively() {
    let summaries = vec![
        summary("root", None, None),
        summary("child", Some("root"), Some("m")),
        summary("grandchild", Some("child"), Some("m")),
    ];
    let mut cycles = Vec::new();
    let tree = TaskTree::build(&summaries, "root", None, &mut cycles).unwrap();
    assert_eq!(child_ids(&tree), vec!["child"]);
    assert_eq!(child_ids(&tree.children[0]), vec!["grandchild"]);
}

#[test]
fn cycle_is_reported_not_expanded() {
    let summaries = vec![
        summary("x", Some("y"), Some("m")),
        summary("y", Some("x"), Some("m")),
    ];
    let mut cycles = Vec::new();
    let tree = TaskTree::build(&summaries, "x", None, &mut cycles).unwrap();
    // x -> y is expanded once; y -> x is the cycle edge, reported and not re-expanded.
    assert_eq!(child_ids(&tree), vec!["y"]);
    assert!(tree.children[0].children.is_empty());
    assert_eq!(cycles, vec!["x".to_owned()]);
}

#[test]
fn unknown_root_yields_none() {
    let summaries = vec![summary("root", None, None)];
    let mut cycles = Vec::new();
    assert!(TaskTree::build(&summaries, "missing", None, &mut cycles).is_none());
}

#[test]
fn patch_parent_absent_leaves_it_unchanged() {
    let patch: TaskPatch = serde_json::from_value(serde_json::json!({ "status": "done" })).unwrap();
    assert_eq!(patch.parent, None);
    let mut task = Task::new("T", Status::Todo);
    task.set_parent(Some("p".to_owned()));
    patch.apply(&mut task);
    assert_eq!(task.frontmatter.parent.as_deref(), Some("p"));
}

#[test]
fn patch_parent_null_clears_it() {
    let patch: TaskPatch = serde_json::from_value(serde_json::json!({ "parent": null })).unwrap();
    assert_eq!(patch.parent, Some(None));
    let mut task = Task::new("T", Status::Todo);
    task.set_parent(Some("p".to_owned()));
    patch.apply(&mut task);
    assert_eq!(task.frontmatter.parent, None);
}

#[test]
fn patch_parent_id_sets_it() {
    let patch: TaskPatch =
        serde_json::from_value(serde_json::json!({ "parent": "epic-1" })).unwrap();
    assert_eq!(patch.parent, Some(Some("epic-1".to_owned())));
    let mut task = Task::new("T", Status::Todo);
    patch.apply(&mut task);
    assert_eq!(task.frontmatter.parent.as_deref(), Some("epic-1"));
}
