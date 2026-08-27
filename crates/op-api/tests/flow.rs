use op_api::{Field, Flow, FlowNode, FlowQuery, FrontmatterFields, Metadata, Status, TaskListItem};

struct Build {
    project: String,
    id: String,
    status: Status,
    parent: Option<String>,
    rank: Option<String>,
    dependencies: Vec<String>,
    tags: Vec<String>,
}

fn task(id: &str, status: Status) -> Build {
    Build {
        project: "one".to_owned(),
        id: id.to_owned(),
        status,
        parent: None,
        rank: None,
        dependencies: Vec::new(),
        tags: Vec::new(),
    }
}

impl Build {
    fn project(mut self, project: &str) -> Self {
        self.project = project.to_owned();
        self
    }

    fn parent(mut self, parent: &str) -> Self {
        self.parent = Some(parent.to_owned());
        self
    }

    fn rank(mut self, rank: &str) -> Self {
        self.rank = Some(rank.to_owned());
        self
    }

    fn needs(mut self, dependencies: &[&str]) -> Self {
        self.dependencies = dependencies.iter().map(|d| (*d).to_owned()).collect();
        self
    }

    fn tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|t| (*t).to_owned()).collect();
        self
    }

    fn item(self) -> TaskListItem {
        TaskListItem {
            project: self.project,
            title: format!("title of {}", self.id),
            id: self.id,
            metadata: Metadata::Fields(FrontmatterFields {
                status: Field::Value(self.status),
                created: Field::Value(op_api::Rfc3339("2026-01-01T00:00:00Z".parse().unwrap())),
                parent: Field::Value(self.parent),
                rank: Field::Value(self.rank),
                dependencies: Field::Value(self.dependencies),
                tags: Field::Value(self.tags),
            }),
            comment_count: 0,
            updated: Field::Error(op_api::FieldError::Missing),
            headline: "main".to_owned(),
            branches: Vec::new(),
            write_target: None,
        }
    }
}

fn tasks(builds: Vec<Build>) -> Vec<TaskListItem> {
    builds.into_iter().map(Build::item).collect()
}

fn flow(builds: Vec<Build>) -> Flow {
    Flow::build(&tasks(builds), &FlowQuery::default()).expect("no cycle")
}

fn kind(node: &FlowNode) -> &'static str {
    match node {
        FlowNode::Leaf { .. } => "leaf",
        FlowNode::Box { .. } => "box",
        FlowNode::Unresolved { .. } => "unresolved",
    }
}

fn id(node: &FlowNode) -> &str {
    match node {
        FlowNode::Leaf { id, .. } | FlowNode::Box { id, .. } | FlowNode::Unresolved { id, .. } => {
            id
        }
    }
}

fn kinds(flow: &Flow) -> Vec<(&str, &str)> {
    flow.nodes
        .iter()
        .map(|node| (id(node), kind(node)))
        .collect()
}

// wave, position and how much waits for the leaf — the three fields a box does not carry.
fn place(flow: &Flow, wanted: &str) -> (usize, usize, usize) {
    flow.nodes
        .iter()
        .find_map(|node| match node {
            FlowNode::Leaf {
                id,
                wave,
                position,
                blocks_count,
                ..
            } if id == wanted => Some((*wave, *position, *blocks_count)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{wanted} is no leaf of the flow"))
}

fn edges(flow: &Flow) -> Vec<(&str, &str)> {
    flow.edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect()
}

fn ids(flow: &Flow) -> Vec<&str> {
    flow.nodes.iter().map(id).collect()
}

#[test]
fn a_chain_lands_in_one_wave_for_each_step() {
    let flow = flow(vec![
        task("OPP-92", Status::Todo).needs(&["OPP-91"]),
        task("OPP-91", Status::Todo),
    ]);

    assert_eq!(place(&flow, "OPP-91").0, 0);
    assert_eq!(place(&flow, "OPP-92").0, 1);
    assert_eq!(edges(&flow), [("OPP-91", "OPP-92")]);
}

#[test]
fn two_projects_share_the_waves_and_no_edge_crosses_them() {
    let flow = flow(vec![
        task("OPP-91", Status::Todo),
        task("OPP-92", Status::Todo).needs(&["OPP-91"]),
        task("WEB-3", Status::Todo).project("two"),
    ]);

    assert_eq!(place(&flow, "OPP-91").0, 0);
    assert_eq!(place(&flow, "WEB-3").0, 0);
    assert_eq!(place(&flow, "OPP-92").0, 1);
    assert!(flow.edges.iter().all(|edge| edge.project == "one"));
}

#[test]
fn a_key_of_another_project_stays_unresolved() {
    let flow = flow(vec![
        task("OPP-70", Status::Todo).needs(&["WEB-7"]),
        task("WEB-7", Status::Todo).project("two"),
    ]);

    assert!(kinds(&flow).contains(&("WEB-7", "unresolved")));
    assert_eq!(place(&flow, "OPP-70").0, 0);
}

#[test]
fn a_parent_is_a_box_and_its_children_hold_the_places() {
    let flow = flow(vec![
        task("OPP-40", Status::Todo),
        task("OPP-41", Status::Todo).parent("OPP-40"),
        task("OPP-42", Status::Todo)
            .parent("OPP-40")
            .needs(&["OPP-41"]),
        task("OPP-43", Status::Done).parent("OPP-40"),
        task("OPP-50", Status::Todo).needs(&["OPP-40"]),
    ]);

    assert_eq!(
        kinds(&flow),
        [
            ("OPP-40", "box"),
            ("OPP-41", "leaf"),
            ("OPP-42", "leaf"),
            ("OPP-50", "leaf"),
        ]
    );
    assert_eq!(place(&flow, "OPP-41"), (0, 0, 2));
    assert_eq!(place(&flow, "OPP-42"), (1, 0, 1));
    assert_eq!(place(&flow, "OPP-50"), (2, 0, 0));
    assert_eq!(edges(&flow), [("OPP-41", "OPP-42"), ("OPP-40", "OPP-50")]);
}

#[test]
fn a_child_inherits_the_dependency_of_its_parent() {
    let flow = flow(vec![
        task("OPP-60", Status::Todo),
        task("OPP-40", Status::Todo).needs(&["OPP-60"]),
        task("OPP-41", Status::Todo).parent("OPP-40"),
    ]);

    assert_eq!(place(&flow, "OPP-60").0, 0);
    assert_eq!(place(&flow, "OPP-41").0, 1);
    assert_eq!(edges(&flow), [("OPP-60", "OPP-40")]);
}

#[test]
fn a_box_with_no_remaining_child_is_a_plain_node() {
    let flow = flow(vec![
        task("OPP-40", Status::Todo),
        task("OPP-43", Status::Cancelled).parent("OPP-40"),
    ]);

    assert_eq!(kinds(&flow), [("OPP-40", "leaf")]);
    assert_eq!(place(&flow, "OPP-40"), (0, 0, 0));
}

#[test]
fn a_backlog_child_of_an_included_parent_joins_the_box() {
    let flow = flow(vec![
        task("OPP-40", Status::Todo),
        task("OPP-41", Status::Todo).parent("OPP-40"),
        task("OPP-44", Status::Backlog).parent("OPP-40"),
    ]);

    assert_eq!(
        kinds(&flow),
        [("OPP-40", "box"), ("OPP-41", "leaf"), ("OPP-44", "leaf")]
    );
}

#[test]
fn an_unresolved_dependency_keeps_its_edge_and_moves_nothing() {
    let flow = flow(vec![
        task("OPP-70", Status::Todo).needs(&["WEB-7", "OPP-99#Design"]),
    ]);

    assert_eq!(
        kinds(&flow),
        [
            ("OPP-70", "leaf"),
            ("OPP-99#Design", "unresolved"),
            ("WEB-7", "unresolved"),
        ]
    );
    assert_eq!(place(&flow, "OPP-70"), (0, 0, 0));
    assert_eq!(
        edges(&flow),
        [("OPP-99#Design", "OPP-70"), ("WEB-7", "OPP-70")]
    );
}

#[test]
fn a_dependency_on_a_section_resolves_to_the_task() {
    let flow = flow(vec![
        task("OPP-80", Status::Todo).needs(&["OPP-79#Design"]),
        task("OPP-79", Status::Todo),
    ]);

    assert_eq!(edges(&flow), [("OPP-79", "OPP-80")]);
    assert_eq!(place(&flow, "OPP-80").0, 1);
}

#[test]
fn a_satisfied_dependency_drops_the_edge_and_the_node() {
    let flow = flow(vec![
        task("OPP-80", Status::Todo).needs(&["OPP-79"]),
        task("OPP-79", Status::Done),
    ]);

    assert_eq!(kinds(&flow), [("OPP-80", "leaf")]);
    assert_eq!(place(&flow, "OPP-80"), (0, 0, 0));
    assert!(flow.edges.is_empty());
}

#[test]
fn the_wave_holds_the_task_that_unblocks_the_most_work_first() {
    let flow = flow(vec![
        task("OPP-1", Status::Todo),
        task("OPP-2", Status::Todo),
        task("OPP-3", Status::Todo).needs(&["OPP-2"]),
        task("OPP-4", Status::Todo).needs(&["OPP-3"]),
    ]);

    assert_eq!(place(&flow, "OPP-2"), (0, 0, 2));
    assert_eq!(place(&flow, "OPP-1"), (0, 1, 0));
}

#[test]
fn rank_orders_the_tasks_that_unblock_as_much_as_each_other() {
    let flow = flow(vec![
        task("OPP-1", Status::Todo),
        task("OPP-2", Status::Todo).rank("a"),
        task("OPP-3", Status::Todo).rank("b"),
    ]);

    assert_eq!(place(&flow, "OPP-2").1, 0);
    assert_eq!(place(&flow, "OPP-3").1, 1);
    assert_eq!(place(&flow, "OPP-1").1, 2);
}

#[test]
fn the_key_orders_the_rest_by_its_number() {
    let flow = flow(vec![
        task("OPP-10", Status::Todo),
        task("OPP-9", Status::Todo),
    ]);

    assert_eq!(place(&flow, "OPP-9").1, 0);
    assert_eq!(place(&flow, "OPP-10").1, 1);
}

#[test]
fn no_parameter_seeds_every_todo_task_of_every_project() {
    let flow = flow(vec![
        task("OPP-1", Status::Todo),
        task("OPP-2", Status::InProgress),
        task("WEB-1", Status::Todo).project("two"),
    ]);

    assert_eq!(ids(&flow), ["OPP-1", "WEB-1"]);
}

#[test]
fn a_status_replaces_the_default_seed_status() {
    let query = FlowQuery {
        statuses: vec![Status::InProgress],
        ..FlowQuery::default()
    };
    let flow = Flow::build(
        &tasks(vec![
            task("OPP-1", Status::Todo),
            task("OPP-2", Status::InProgress),
        ]),
        &query,
    )
    .expect("no cycle");

    assert_eq!(ids(&flow), ["OPP-2"]);
}

#[test]
fn the_growth_takes_a_dependency_of_any_status() {
    let flow = flow(vec![
        task("OPP-1", Status::Todo).needs(&["OPP-2"]),
        task("OPP-2", Status::Backlog).needs(&["OPP-3"]),
        task("OPP-3", Status::InReview),
    ]);

    assert_eq!(ids(&flow), ["OPP-3", "OPP-2", "OPP-1"]);
}

#[test]
fn the_flow_does_not_grow_towards_what_waits_for_a_seed() {
    let query = FlowQuery {
        tasks: vec![("one".to_owned(), "OPP-1".to_owned())],
        ..FlowQuery::default()
    };
    let flow = Flow::build(
        &tasks(vec![
            task("OPP-1", Status::Todo),
            task("OPP-2", Status::Todo).needs(&["OPP-1"]),
        ]),
        &query,
    )
    .expect("no cycle");

    assert_eq!(ids(&flow), ["OPP-1"]);
}

#[test]
fn the_fields_of_a_query_narrow_each_other() {
    let all = tasks(vec![
        task("OPP-1", Status::Todo).tags(&["api"]),
        task("OPP-2", Status::Todo).tags(&["web"]),
        task("WEB-1", Status::Todo).project("two").tags(&["api"]),
    ]);
    let query = FlowQuery {
        projects: vec!["one".to_owned()],
        tags: vec!["api".to_owned()],
        ..FlowQuery::default()
    };

    assert_eq!(
        ids(&Flow::build(&all, &query).expect("no cycle")),
        ["OPP-1"]
    );
}

#[test]
fn the_values_of_one_field_are_alternatives() {
    let all = tasks(vec![
        task("OPP-1", Status::Todo).tags(&["api"]),
        task("OPP-2", Status::Todo).tags(&["web"]),
        task("OPP-3", Status::Todo).tags(&["cli"]),
    ]);
    let query = FlowQuery {
        tags: vec!["api".to_owned(), "web".to_owned()],
        ..FlowQuery::default()
    };

    assert_eq!(
        ids(&Flow::build(&all, &query).expect("no cycle")),
        ["OPP-1", "OPP-2"]
    );
}

#[test]
fn a_cycle_fails_the_request_and_names_its_members() {
    let cycles = Flow::build(
        &tasks(vec![
            task("OPP-12", Status::Todo).needs(&["OPP-31"]),
            task("OPP-19", Status::Todo).needs(&["OPP-12"]),
            task("OPP-31", Status::Todo).needs(&["OPP-19"]),
        ]),
        &FlowQuery::default(),
    )
    .expect_err("a cycle");

    assert_eq!(cycles.cycles, [["OPP-12", "OPP-19", "OPP-31"]]);
    assert_eq!(
        cycles.to_string(),
        "dependencies form a cycle: OPP-12 -> OPP-19 -> OPP-31 -> OPP-12"
    );
}

#[test]
fn a_cycle_the_seeds_cannot_reach_leaves_the_flow_alone() {
    let query = FlowQuery {
        tasks: vec![("one".to_owned(), "OPP-1".to_owned())],
        ..FlowQuery::default()
    };
    let flow = Flow::build(
        &tasks(vec![
            task("OPP-1", Status::Todo),
            task("OPP-12", Status::Todo).needs(&["OPP-19"]),
            task("OPP-19", Status::Todo).needs(&["OPP-12"]),
        ]),
        &query,
    )
    .expect("no cycle");

    assert_eq!(ids(&flow), ["OPP-1"]);
}

#[test]
fn a_parent_that_depends_on_its_own_child_is_a_cycle() {
    let cycles = Flow::build(
        &tasks(vec![
            task("OPP-40", Status::Todo).needs(&["OPP-41"]),
            task("OPP-41", Status::Todo).parent("OPP-40"),
        ]),
        &FlowQuery::default(),
    )
    .expect_err("a cycle");

    assert_eq!(cycles.cycles, [["OPP-41"]]);
}

#[test]
fn a_leaf_carries_its_place_and_a_box_carries_none() {
    let flow = flow(vec![
        task("OPP-40", Status::Todo),
        task("OPP-41", Status::Todo).parent("OPP-40"),
    ]);
    let json = serde_json::to_value(&flow).expect("serializes");

    assert_eq!(
        json,
        serde_json::json!({
            "nodes": [
                {
                    "kind": "box",
                    "project": "one",
                    "id": "OPP-40",
                    "title": "title of OPP-40",
                    "status": "todo",
                },
                {
                    "kind": "leaf",
                    "project": "one",
                    "id": "OPP-41",
                    "title": "title of OPP-41",
                    "status": "todo",
                    "parent": "OPP-40",
                    "wave": 0,
                    "position": 0,
                    "blocks_count": 0,
                },
            ],
            "edges": [],
        })
    );
}

#[test]
fn a_parent_cycle_keeps_both_tasks_in_the_flow() {
    let flow = flow(vec![
        task("OPP-1", Status::Todo).parent("OPP-2"),
        task("OPP-2", Status::Todo).parent("OPP-1"),
        task("OPP-3", Status::Todo).needs(&["OPP-1"]),
    ]);

    assert_eq!(
        kinds(&flow),
        [("OPP-1", "leaf"), ("OPP-2", "leaf"), ("OPP-3", "leaf")],
        "neither task can hold the other, so both stand alone"
    );
    let drawn: Vec<&str> = ids(&flow);
    for edge in &flow.edges {
        assert!(
            drawn.contains(&edge.from.as_str()) && drawn.contains(&edge.to.as_str()),
            "{edge:?} points at a node the flow does not send"
        );
    }
}

#[test]
fn a_task_in_a_parent_cycle_reports_no_parent() {
    let flow = flow(vec![
        task("OPP-1", Status::Todo).parent("OPP-2"),
        task("OPP-2", Status::Todo).parent("OPP-1"),
    ]);

    assert!(
        flow.nodes
            .iter()
            .all(|node| matches!(node, FlowNode::Leaf { parent: None, .. }))
    );
}

#[test]
fn a_finished_parent_blocks_nothing_it_declared() {
    let flow = flow(vec![
        task("OPP-40", Status::Done).needs(&["OPP-60"]),
        task("OPP-41", Status::Todo).parent("OPP-40"),
        task("OPP-60", Status::Backlog),
    ]);

    assert_eq!(place(&flow, "OPP-41"), (0, 0, 0));
    assert!(
        !ids(&flow).contains(&"OPP-60"),
        "a complete parent pulls nothing else in"
    );
    assert!(flow.edges.is_empty());
}

#[test]
fn two_cycles_through_one_task_are_both_reported() {
    let cycles = Flow::build(
        &tasks(vec![
            task("OPP-1", Status::Todo).needs(&["OPP-3"]),
            task("OPP-2", Status::Todo).needs(&["OPP-1"]),
            task("OPP-3", Status::Todo).needs(&["OPP-2", "OPP-4"]),
            task("OPP-4", Status::Todo).needs(&["OPP-3"]),
        ]),
        &FlowQuery::default(),
    )
    .expect_err("two cycles");

    assert_eq!(
        cycles.cycles,
        [vec!["OPP-1", "OPP-2", "OPP-3"], vec!["OPP-3", "OPP-4"]]
    );
}
