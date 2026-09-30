use op_task::{FieldError, PartialMetadata, Status, Task, parse_partial};

const FIRST: &str = "https://github.com/sukovanej/openplan/pull/214";
const SECOND: &str = "https://gitlab.example.com/group/sub/app/-/merge_requests/7";

fn task(frontmatter: &str) -> Task {
    Task::from_file_string(&format!(
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n{frontmatter}---\n# Title\n\nbody\n"
    ))
    .unwrap()
}

fn fields(input: &str) -> op_task::PartialFrontmatter {
    match parse_partial(input).metadata {
        PartialMetadata::Fields(fields) => fields,
        PartialMetadata::Error(message) => panic!("expected recoverable fields, got {message}"),
    }
}

#[test]
fn pull_requests_are_sorted_and_deduped_on_write() {
    let mut parsed = task("");
    parsed.set_pull_requests(vec![SECOND.to_owned(), FIRST.to_owned(), SECOND.to_owned()]);

    let written = parsed.to_file_string().unwrap();
    assert!(
        written.contains(&format!("pull_requests:\n- {FIRST}\n- {SECOND}\n")),
        "{written}"
    );
    assert_eq!(
        Task::from_file_string(&written)
            .unwrap()
            .frontmatter
            .pull_requests,
        vec![FIRST.to_owned(), SECOND.to_owned()]
    );
}

#[test]
fn an_empty_set_omits_the_field() {
    let mut parsed = task(&format!("pull_requests:\n- {FIRST}\n"));
    parsed.set_pull_requests(Vec::new());
    assert!(!parsed.to_file_string().unwrap().contains("pull_requests"));
}

#[test]
fn an_entry_that_is_not_text_is_refused() {
    assert!(
        Task::from_file_string(
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\npull_requests:\n- 214\n---\n# Title\n"
        )
        .is_err()
    );
    let numbered = fields("---\nstatus: todo\npull_requests:\n- 214\n---\n# Title\n");
    assert!(matches!(
        numbered.pull_requests,
        Err(FieldError::Invalid(_))
    ));
    assert_eq!(numbered.status, Ok(Status::Todo));
}

#[test]
fn the_lenient_path_sorts_the_set_and_reads_an_absent_field_as_empty() {
    let sorted = fields(&format!(
        "---\nstatus: todo\npull_requests:\n- {SECOND}\n- {FIRST}\n---\n# Title\n"
    ));
    assert_eq!(
        sorted.pull_requests,
        Ok(vec![FIRST.to_owned(), SECOND.to_owned()])
    );
    assert_eq!(
        fields("---\nstatus: todo\n---\n# Title\n").pull_requests,
        Ok(Vec::new())
    );
}
