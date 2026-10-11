use op_api::{
    CreateTask, Forge, ForgeKind, Metadata, PullRequestError, PullRequestView, TaskPatch,
    WriteError, render_task_file,
};
use op_task::content::split;
use op_task::{ProjectCode, Status, Task, Timestamp};

const OWN: &str = "https://github.com/sukovanej/openplan/pull/214";
const OTHER: &str = "https://github.com/rust-lang/cargo/pull/1234";

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().unwrap()
}

fn project_code() -> ProjectCode {
    "OPP".parse().unwrap()
}

fn project() -> Forge {
    Forge {
        kind: ForgeKind::Github,
        host: "github.com".to_owned(),
        repo: "sukovanej/openplan".to_owned(),
    }
}

fn task(pull_requests: &[&str]) -> Task {
    let mut task = Task::new("Parser", Status::Todo, stamp());
    task.set_pull_requests(pull_requests.iter().map(|p| (*p).to_owned()).collect());
    task
}

fn patched(task: &mut Task, patch: TaskPatch) -> Result<(), WriteError> {
    patch.apply(task, project_code(), Some(&project()))
}

fn entries(entries: &[&str]) -> Vec<String> {
    entries.iter().map(|entry| (*entry).to_owned()).collect()
}

#[test]
fn create_takes_an_address_or_a_number_of_the_project_repository() {
    let created = CreateTask {
        title: "Parser".to_owned(),
        status: None,
        parent: None,
        dependencies: Vec::new(),
        tags: Vec::new(),
        pull_requests: entries(&["#214", "https://github.com/rust-lang/cargo/pull/1234/files"]),
        body: None,
    }
    .into_task(stamp(), project_code(), Some(&project()))
    .unwrap();

    assert_eq!(created.frontmatter.pull_requests, [OTHER, OWN]);
}

#[test]
fn add_and_remove_change_the_set_the_task_holds() {
    let mut task = task(&[OTHER]);

    patched(
        &mut task,
        TaskPatch {
            add_pull_requests: entries(&["214"]),
            ..TaskPatch::default()
        },
    )
    .unwrap();
    assert_eq!(task.frontmatter.pull_requests, [OTHER, OWN]);

    patched(
        &mut task,
        TaskPatch {
            add_pull_requests: entries(&["https://github.com/Sukovanej/OpenPlan/pull/214"]),
            remove_pull_requests: entries(&[OTHER]),
            ..TaskPatch::default()
        },
    )
    .unwrap();
    assert_eq!(
        task.frontmatter.pull_requests,
        ["https://github.com/Sukovanej/OpenPlan/pull/214"]
    );

    patched(
        &mut task,
        TaskPatch {
            remove_pull_requests: entries(&["#214", "#9"]),
            ..TaskPatch::default()
        },
    )
    .unwrap();
    assert!(task.frontmatter.pull_requests.is_empty());
}

#[test]
fn the_whole_set_is_replaced_and_an_empty_one_clears_it() {
    let mut task = task(&[OTHER]);

    patched(
        &mut task,
        TaskPatch {
            pull_requests: Some(entries(&["214"])),
            ..TaskPatch::default()
        },
    )
    .unwrap();
    assert_eq!(task.frontmatter.pull_requests, [OWN]);

    patched(
        &mut task,
        TaskPatch {
            pull_requests: Some(Vec::new()),
            ..TaskPatch::default()
        },
    )
    .unwrap();
    assert!(task.frontmatter.pull_requests.is_empty());
}

#[test]
fn a_number_is_refused_in_a_project_with_no_forge() {
    let mut task = task(&[]);
    let refused = TaskPatch {
        add_pull_requests: entries(&["214"]),
        ..TaskPatch::default()
    }
    .apply(&mut task, project_code(), None);

    assert_eq!(
        refused,
        Err(WriteError::PullRequest(PullRequestError::NoForge {
            got: "214".to_owned()
        }))
    );
}

#[test]
fn a_view_names_the_repository_only_when_it_is_another_one() {
    let views = PullRequestView::of_addresses(
        &entries(&[OTHER, OWN, "https://example.com/not-a-pull-request"]),
        Some(&project()),
    );

    assert_eq!(
        views,
        vec![
            PullRequestView {
                url: OTHER.to_owned(),
                forge: ForgeKind::Github,
                repo: "rust-lang/cargo".to_owned(),
                number: 1234,
                short: "rust-lang/cargo#1234".to_owned(),
            },
            PullRequestView {
                url: OWN.to_owned(),
                forge: ForgeKind::Github,
                repo: "sukovanej/openplan".to_owned(),
                number: 214,
                short: "#214".to_owned(),
            },
        ]
    );
}

#[test]
fn a_rendered_file_carries_the_pull_requests_the_task_holds() {
    let task = task(&[OWN, OTHER]);
    let metadata = Metadata::from_frontmatter(&task.frontmatter, project_code());
    let rendered = render_task_file(
        &metadata,
        &task.title().unwrap_or_default(),
        &split(&task.body),
        &[],
    )
    .unwrap();

    assert_eq!(rendered, task.to_file_string().unwrap());
    assert!(rendered.contains(&format!("pull_requests:\n- {OTHER}\n- {OWN}\n")));
}
