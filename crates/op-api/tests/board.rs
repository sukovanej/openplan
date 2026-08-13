use op_api::{Board, Field, FrontmatterFields, Metadata, Status, TaskListItem};

fn item(id: &str, status: Status, parent: Option<&str>, rank: Option<&str>) -> TaskListItem {
    with_metadata(id, fields(Field::Value(status), parent, rank))
}

fn fields(status: Field<Status>, parent: Option<&str>, rank: Option<&str>) -> Metadata {
    Metadata::Fields(FrontmatterFields {
        status,
        created: Field::Value(op_api::Rfc3339("2026-01-01T00:00:00Z".parse().unwrap())),
        parent: Field::Value(parent.map(str::to_owned)),
        rank: Field::Value(rank.map(str::to_owned)),
        dependencies: Field::Value(Vec::new()),
    })
}

fn with_metadata(id: &str, metadata: Metadata) -> TaskListItem {
    TaskListItem {
        project: "one".to_owned(),
        id: id.to_owned(),
        title: id.to_uppercase(),
        metadata,
        updated: op_api::Field::Error(op_api::FieldError::Missing),
        headline: "main".to_owned(),
        branches: Vec::new(),
    }
}

fn of(project: &str, task: TaskListItem) -> TaskListItem {
    TaskListItem {
        project: project.to_owned(),
        title: format!("{project} {}", task.title),
        ..task
    }
}

fn rows(board: &Board, status: Status) -> Vec<(&str, &str, usize)> {
    board
        .groups
        .iter()
        .find(|g| g.status == Some(status))
        .map(|g| {
            g.rows
                .iter()
                .map(|r| (r.task.project.as_str(), r.task.id.as_str(), r.depth))
                .collect()
        })
        .unwrap_or_default()
}

fn dated(id: &str, status: Status, parent: Option<&str>, at: &str) -> TaskListItem {
    TaskListItem {
        updated: Field::Value(op_api::Rfc3339(at.parse().unwrap())),
        ..item(id, status, parent, None)
    }
}

fn row_ids(board: &Board, status: Status) -> Vec<&str> {
    board
        .groups
        .iter()
        .find(|g| g.status == Some(status))
        .map(|g| g.rows.iter().map(|r| r.task.id.as_str()).collect())
        .unwrap_or_default()
}

#[test]
fn groups_appear_in_board_order_skipping_empties() {
    let board = Board::build(&[
        item("d", Status::Done, None, None),
        item("p", Status::InProgress, None, None),
        item("r", Status::InReview, None, None),
        item("t", Status::Todo, None, None),
    ]);
    let order: Vec<Status> = board.groups.iter().filter_map(|g| g.status).collect();
    assert_eq!(
        order,
        vec![
            Status::InReview,
            Status::InProgress,
            Status::Todo,
            Status::Done
        ]
    );
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
        .find(|g| g.status == Some(Status::InProgress))
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
        .find(|g| g.status == Some(Status::InProgress))
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
fn unranked_rows_lead_with_the_most_recently_updated() {
    let board = Board::build(&[
        dated("old", Status::Todo, None, "2026-01-01T00:00:00Z"),
        dated("newest", Status::Todo, None, "2026-03-01T00:00:00Z"),
        dated("middle", Status::Todo, None, "2026-02-01T00:00:00Z"),
    ]);
    assert_eq!(
        row_ids(&board, Status::Todo),
        vec!["newest", "middle", "old"]
    );
}

#[test]
fn nested_children_lead_with_the_most_recently_updated() {
    let board = Board::build(&[
        dated("root", Status::Todo, None, "2026-01-01T00:00:00Z"),
        dated("old", Status::Todo, Some("root"), "2026-01-02T00:00:00Z"),
        dated("new", Status::Todo, Some("root"), "2026-02-02T00:00:00Z"),
    ]);
    assert_eq!(row_ids(&board, Status::Todo), vec!["root", "new", "old"]);
}

#[test]
fn an_undated_task_sorts_after_every_dated_one() {
    let board = Board::build(&[
        item("undated", Status::Todo, None, None),
        dated("ancient", Status::Todo, None, "2020-01-01T00:00:00Z"),
    ]);
    assert_eq!(row_ids(&board, Status::Todo), vec!["ancient", "undated"]);
}

#[test]
fn a_rank_outranks_a_newer_update() {
    let board = Board::build(&[
        TaskListItem {
            metadata: fields(Field::Value(Status::Todo), None, Some("m")),
            ..dated("ranked", Status::Todo, None, "2020-01-01T00:00:00Z")
        },
        dated("fresh", Status::Todo, None, "2026-06-01T00:00:00Z"),
    ]);
    assert_eq!(row_ids(&board, Status::Todo), vec!["ranked", "fresh"]);
}

// Two stores can commit the same abbreviation, so the same key names a different task in each.
#[test]
fn one_key_in_two_projects_is_two_rows() {
    let board = Board::build(&[
        of("alpha", item("APP-3", Status::Todo, None, None)),
        of("beta", item("APP-3", Status::Todo, None, None)),
    ]);
    assert_eq!(
        rows(&board, Status::Todo),
        vec![("alpha", "APP-3", 0), ("beta", "APP-3", 0)]
    );
}

#[test]
fn a_parent_resolves_only_inside_its_own_project() {
    let board = Board::build(&[
        of("alpha", item("APP-1", Status::Todo, None, None)),
        of("beta", item("APP-2", Status::Todo, Some("APP-1"), None)),
    ]);
    // beta's APP-1 does not exist, so its child is a group-local root with no hint — never a row
    // nested under alpha's task of the same key.
    assert_eq!(
        rows(&board, Status::Todo),
        vec![("alpha", "APP-1", 0), ("beta", "APP-2", 0)]
    );
    let group = &board.groups[0];
    assert_eq!(group.rows[1].parent_title, None);
    assert!(!group.rows[0].has_children);
}

#[test]
fn a_child_nests_under_the_parent_in_its_own_project() {
    let board = Board::build(&[
        of("alpha", item("APP-1", Status::Todo, None, None)),
        of("beta", item("APP-1", Status::Todo, None, None)),
        of("beta", item("APP-2", Status::Todo, Some("APP-1"), None)),
    ]);
    assert_eq!(
        rows(&board, Status::Todo),
        vec![
            ("alpha", "APP-1", 0),
            ("beta", "APP-1", 0),
            ("beta", "APP-2", 1)
        ]
    );
}

// Rank and update time decide first; the project name only breaks what is left, which keeps a
// merged board deterministic rather than ordered by whichever project was read first.
#[test]
fn the_project_name_breaks_a_tie_before_the_id_does() {
    let board = Board::build(&[
        of("beta", item("APP-1", Status::Todo, None, None)),
        of("alpha", item("APP-2", Status::Todo, None, None)),
        of("alpha", item("APP-1", Status::Todo, None, None)),
    ]);
    assert_eq!(
        rows(&board, Status::Todo),
        vec![
            ("alpha", "APP-1", 0),
            ("alpha", "APP-2", 0),
            ("beta", "APP-1", 0)
        ]
    );
}

#[test]
fn a_newer_update_still_outranks_the_project_name() {
    let board = Board::build(&[
        of(
            "alpha",
            dated("APP-1", Status::Todo, None, "2026-01-01T00:00:00Z"),
        ),
        of(
            "beta",
            dated("APP-1", Status::Todo, None, "2026-03-01T00:00:00Z"),
        ),
    ]);
    assert_eq!(
        rows(&board, Status::Todo),
        vec![("beta", "APP-1", 0), ("alpha", "APP-1", 0)]
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
