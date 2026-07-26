use op_api::{Board, Status, TaskListItem};

fn item(id: &str, status: Status, parent: Option<&str>, rank: Option<&str>) -> TaskListItem {
    TaskListItem {
        id: id.to_owned(),
        title: id.to_uppercase(),
        status,
        parent: parent.map(str::to_owned),
        rank: rank.map(str::to_owned),
        headline: "main".to_owned(),
        branches: Vec::new(),
    }
}

fn row_ids(board: &Board, status: Status) -> Vec<&str> {
    board
        .groups
        .iter()
        .find(|g| g.status == status)
        .map(|g| g.rows.iter().map(|r| r.task.id.as_str()).collect())
        .unwrap_or_default()
}

#[test]
fn groups_appear_in_board_order_skipping_empties() {
    let board = Board::build(&[
        item("d", Status::Done, None, None),
        item("p", Status::InProgress, None, None),
        item("t", Status::Todo, None, None),
    ]);
    let order: Vec<Status> = board.groups.iter().map(|g| g.status).collect();
    assert_eq!(order, vec![Status::InProgress, Status::Todo, Status::Done]);
}

#[test]
fn a_task_lands_in_its_own_status_group() {
    // Child shares no status with its parent, so it surfaces in its OWN group, not the parent's.
    let board = Board::build(&[
        item("epic", Status::Todo, None, None),
        item("sub", Status::InProgress, Some("epic"), None),
    ]);
    assert_eq!(row_ids(&board, Status::Todo), vec!["epic"]);
    assert_eq!(row_ids(&board, Status::InProgress), vec!["sub"]);
}

#[test]
fn same_status_child_nests_under_parent_with_depth() {
    let board = Board::build(&[
        item("epic", Status::InProgress, None, None),
        item("sub", Status::InProgress, Some("epic"), None),
    ]);
    let group = board
        .groups
        .iter()
        .find(|g| g.status == Status::InProgress)
        .unwrap();
    assert_eq!(
        group
            .rows
            .iter()
            .map(|r| (r.task.id.as_str(), r.depth))
            .collect::<Vec<_>>(),
        vec![("epic", 0), ("sub", 1)],
    );
    assert!(group.rows[0].has_children);
    assert!(!group.rows[1].has_children);
}

#[test]
fn cross_status_child_is_a_root_with_a_parent_hint() {
    let board = Board::build(&[
        item("epic", Status::Todo, None, None),
        item("sub", Status::InProgress, Some("epic"), None),
    ]);
    let group = board
        .groups
        .iter()
        .find(|g| g.status == Status::InProgress)
        .unwrap();
    let row = &group.rows[0];
    assert_eq!(row.task.id, "sub");
    assert_eq!(row.depth, 0);
    // The cut cross-status edge carries the parent's title so the client can show "under EPIC".
    assert_eq!(row.parent_title.as_deref(), Some("EPIC"));
}

#[test]
fn a_dangling_parent_yields_no_hint() {
    let board = Board::build(&[item("orphan", Status::Todo, Some("missing"), None)]);
    let row = &board.groups[0].rows[0];
    assert_eq!(row.task.id, "orphan");
    assert_eq!(row.parent_title, None);
}

#[test]
fn siblings_order_by_rank_then_unranked_by_id() {
    let board = Board::build(&[
        item("root", Status::Todo, None, None),
        item("b", Status::Todo, Some("root"), Some("m")),
        item("a", Status::Todo, Some("root"), Some("t")),
        item("z", Status::Todo, Some("root"), None),
        item("y", Status::Todo, Some("root"), None),
    ]);
    assert_eq!(
        row_ids(&board, Status::Todo),
        vec!["root", "b", "a", "y", "z"]
    );
}

#[test]
fn a_parent_cycle_still_shows_every_member_once() {
    let board = Board::build(&[
        item("x", Status::Todo, Some("y"), None),
        item("y", Status::Todo, Some("x"), None),
    ]);
    let ids = row_ids(&board, Status::Todo);
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"x") && ids.contains(&"y"));
}
