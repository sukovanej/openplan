use std::collections::BTreeMap;
use std::sync::Arc;

use op_api::ProblemCode;
use op_backend::MemorySnapshot;
use op_index::Index;
use op_tracker::Plan;

fn plan(files: &[(&str, &str)]) -> Plan {
    let mut documents: BTreeMap<String, Vec<u8>> = files
        .iter()
        .map(|(path, text)| ((*path).to_owned(), text.as_bytes().to_vec()))
        .collect();
    documents.insert(
        "config.toml".to_owned(),
        b"abbreviation = \"OPP\"\n".to_vec(),
    );
    documents.insert("tags/bug.md".to_owned(), b"# bug\n".to_vec());
    Plan::read(Arc::new(MemorySnapshot::new(None, documents))).expect("plan")
}

fn task(number: u64, extra: &str, body: &str) -> (String, String) {
    (
        format!("tasks/{number:05}-t.md"),
        format!("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n{extra}---\n{body}"),
    )
}

fn problems(files: &[(String, String)]) -> BTreeMap<String, Vec<(ProblemCode, String)>> {
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let mut index = Index::new();
    index.load(&plan(&files)).expect("load");
    index
        .list("p")
        .into_iter()
        .filter(|row| !row.problems.is_empty())
        .map(|row| {
            (
                row.id,
                row.problems
                    .into_iter()
                    .map(|problem| (problem.code, problem.message))
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn a_sound_set_of_tasks_has_no_problems() {
    let found = problems(&[
        task(1, "tags: [bug]\n", "# One\n\nSee [[./00002-t.md]].\n"),
        task(
            2,
            "parent: ./00001-t.md\ndependencies: [./00001-t.md]\n",
            "# Two\n",
        ),
    ]);

    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_reference_a_file_spells_as_a_key_or_a_number_is_a_problem() {
    let found = problems(&[
        task(1, "parent: 2\n", "# One\n\nSee [[OPP-2]] and [[OPP-9]].\n"),
        task(2, "", "# Two\n"),
    ]);

    assert_eq!(
        found["OPP-1"],
        vec![
            (
                ProblemCode::Reference,
                "the text names OPP-9, which does not exist".to_owned()
            ),
            (
                ProblemCode::ReferencePath,
                "`2` names OPP-2; a file names it by the path to the task file".to_owned()
            ),
            (
                ProblemCode::ReferencePath,
                "`OPP-2` names OPP-2; a file names it by the path to the task file".to_owned()
            ),
        ]
    );
}

#[test]
fn references_to_tasks_that_do_not_exist_are_problems() {
    let found = problems(&[task(
        1,
        "parent: ./00009-gone.md\ndependencies: [./00008-gone.md]\n",
        "# One\n\nSee [[./00007-gone.md]].\n\n## Comments\n\n### 2026-01-02T00:00:00Z by Ada\n\n> Old note about [[./00006-gone.md]].\n",
    )]);

    assert_eq!(
        found["OPP-1"],
        vec![
            (
                ProblemCode::Reference,
                "the dependency OPP-8 does not exist".to_owned()
            ),
            (
                ProblemCode::Reference,
                "the parent OPP-9 does not exist".to_owned()
            ),
            (
                ProblemCode::Reference,
                "the text names OPP-7, which does not exist".to_owned()
            ),
        ],
        "a reference in the comment log is history, not a problem"
    );
}

#[test]
fn a_parent_cycle_is_a_problem_of_each_task_in_it() {
    let found = problems(&[
        task(1, "parent: ./00002-t.md\n", "# One\n"),
        task(2, "parent: ./00001-t.md\n", "# Two\n"),
        task(3, "parent: ./00001-t.md\n", "# Three\n"),
    ]);

    assert_eq!(
        found["OPP-1"],
        vec![(
            ProblemCode::ParentCycle,
            "the task is its own ancestor: OPP-1 → OPP-2 → OPP-1".to_owned()
        )]
    );
    assert_eq!(found["OPP-2"][0].0, ProblemCode::ParentCycle);
    assert!(
        !found.contains_key("OPP-3"),
        "a task below a cycle is not in it"
    );
}

#[test]
fn a_dependency_cycle_is_a_problem_of_each_task_in_it() {
    let found = problems(&[
        task(1, "dependencies: [./00002-t.md]\n", "# One\n"),
        task(2, "dependencies: [./00003-t.md]\n", "# Two\n"),
        task(3, "dependencies: [./00001-t.md]\n", "# Three\n"),
        task(4, "dependencies: [./00004-t.md]\n", "# Four\n"),
    ]);

    assert_eq!(
        found["OPP-1"],
        vec![(
            ProblemCode::DependencyCycle,
            "the task waits for itself: OPP-1 → OPP-2 → OPP-3 → OPP-1".to_owned()
        )]
    );
    assert_eq!(found["OPP-3"][0].0, ProblemCode::DependencyCycle);
    assert_eq!(
        found["OPP-4"][0].1,
        "the task waits for itself: OPP-4 → OPP-4"
    );
}

#[test]
fn an_unregistered_tag_a_bad_field_and_a_bad_title_are_problems() {
    let found = problems(&[
        task(1, "tags: [bug, gone]\n", "# One\n"),
        task(2, "rank: \"!\"\n", "No title here.\n"),
        (
            "tasks/00003-t.md".to_owned(),
            "---\nstatus: nope\ncreated: 2026-01-01T00:00:00Z\n---\n# Three\n\n# Again\n"
                .to_owned(),
        ),
    ]);

    assert_eq!(
        found["OPP-1"],
        vec![(
            ProblemCode::Tag,
            "the tag gone is not registered".to_owned()
        )]
    );
    assert_eq!(found["OPP-2"][0].0, ProblemCode::Title);
    let three: Vec<ProblemCode> = found["OPP-3"].iter().map(|problem| problem.0).collect();
    assert_eq!(three, vec![ProblemCode::Field, ProblemCode::Title]);
}

#[test]
fn a_title_both_sides_of_a_conflict_changed_is_one_title() {
    let found = problems(&[task(
        1,
        "",
        "<<<<<<< Ann (1111111)\n# One for Ann\n=======\n# One for Ben\n>>>>>>> Ben (2222222)\n",
    )]);

    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn two_files_of_one_number_are_a_problem() {
    let found = problems(&[
        task(1, "", "# One\n"),
        (
            "tasks/00001-other.md".to_owned(),
            task(1, "", "# Other\n").1,
        ),
    ]);

    assert_eq!(
        found["OPP-1"],
        vec![(
            ProblemCode::DuplicateNumber,
            "tasks/00001-t.md also takes this number, so openplan reads only one of the files"
                .to_owned()
        )]
    );
}

#[test]
fn a_problem_goes_away_with_its_cause() {
    let mut index = Index::new();
    let (path, text) = task(1, "parent: ./00002-t.md\n", "# One\n");
    index
        .load(&plan(&[(path.as_str(), text.as_str())]))
        .expect("load");
    assert_eq!(index.detail("p", 1).expect("detail").problems.len(), 1);

    let (other, other_text) = task(2, "", "# Two\n");
    index
        .load(&plan(&[
            (path.as_str(), text.as_str()),
            (other.as_str(), other_text.as_str()),
        ]))
        .expect("load");
    assert!(index.detail("p", 1).expect("detail").problems.is_empty());
}

#[test]
fn a_broken_mermaid_fence_names_its_line_in_the_task_file() {
    let found = problems(&[
        task(
            1,
            "",
            "# One\n\n```mermaid\nflowchart LR\n  a --> b\n```\n\n```mermaid\nflowchart LR\n  a -->\n```\n",
        ),
        task(2, "", "# Two\n\n```d2\na -> b\n```\n"),
    ]);
    let codes: Vec<(&str, ProblemCode)> = found
        .iter()
        .flat_map(|(id, problems)| problems.iter().map(move |(code, _)| (id.as_str(), *code)))
        .collect();
    assert_eq!(codes, vec![("OPP-1", ProblemCode::Diagram)]);
    let message = &found["OPP-1"][0].1;
    assert!(
        message.starts_with("the Mermaid diagram fails at line 14, column "),
        "{message}"
    );
}
