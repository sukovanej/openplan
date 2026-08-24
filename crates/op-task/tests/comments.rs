use op_task::comment::{self, NewComment};
use op_task::{FieldError, Status, Task, Timestamp};

fn at(text: &str) -> Timestamp {
    text.parse().expect("a whole-second UTC timestamp")
}

fn task() -> Task {
    Task::new("Ship login", Status::Todo, at("2026-01-01T00:00:00Z"))
}

fn entry(text: &str) -> NewComment {
    NewComment {
        at: at("2026-08-24T09:12:04Z"),
        author: "Milan Suk".to_owned(),
        agent: Some("claude-code".to_owned()),
        text: text.to_owned(),
    }
}

#[test]
fn the_first_comment_creates_the_section() {
    let mut task = task();
    task.append_comment(&entry("hello"));
    assert_eq!(
        task.body,
        "# Ship login\n\n## Comments\n\n### 2026-08-24T09:12:04Z by Milan Suk via claude-code\n\n> hello\n"
    );
}

#[test]
fn a_person_gets_no_agent() {
    let mut task = task();
    task.append_comment(&NewComment {
        agent: None,
        ..entry("hello")
    });
    assert!(
        task.body
            .contains("### 2026-08-24T09:12:04Z by Milan Suk\n")
    );
}

#[test]
fn a_second_comment_appends_below_the_first() {
    let mut task = task();
    task.append_comment(&entry("first"));
    task.append_comment(&NewComment {
        at: at("2026-08-24T09:20:41Z"),
        agent: None,
        ..entry("second")
    });
    assert_eq!(
        task.body,
        "# Ship login\n\n## Comments\n\n### 2026-08-24T09:12:04Z by Milan Suk via claude-code\n\n> \
         first\n\n### 2026-08-24T09:20:41Z by Milan Suk\n\n> second\n"
    );
    let comments = comment::parse(&task.body);
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].text, "first");
    assert_eq!(comments[1].text, "second");
}

#[test]
fn markdown_of_every_shape_round_trips_as_one_entry() {
    let text = "# Any heading works\n\n###### even six\n\n- a list\n\n```rust\nfn main() {}\n```\n\n> \
                nested quote\n\n| a | b |\n| - | - |\n| 1 | 2 |";
    let mut task = task();
    task.append_comment(&entry(text));
    let comments = comment::parse(&task.body);
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, text);
    assert_eq!(comments[0].at, Ok(at("2026-08-24T09:12:04Z")));
    assert_eq!(comments[0].author.as_deref(), Ok("Milan Suk"));
    assert_eq!(comments[0].agent.as_deref(), Some("claude-code"));
}

#[test]
fn a_quoted_heading_stays_out_of_the_outline() {
    let mut task = task();
    task.append_comment(&entry("## Comments\n\n### fake entry"));
    assert_eq!(comment::parse(&task.body).len(), 1);
    assert_eq!(comment::sections(&task.body).len(), 1);
}

#[test]
fn an_author_holding_via_keeps_the_last_split() {
    let mut task = task();
    task.append_comment(&NewComment {
        author: "Ada via Lovelace".to_owned(),
        ..entry("hi")
    });
    let comments = comment::parse(&task.body);
    assert_eq!(comments[0].author.as_deref(), Ok("Ada via Lovelace"));
    assert_eq!(comments[0].agent.as_deref(), Some("claude-code"));
}

#[test]
fn a_damaged_timestamp_keeps_the_text() {
    let body = "# T\n\n## Comments\n\n### yesterday by Milan\n\n> still readable\n";
    let comments = comment::parse(body);
    assert_eq!(
        comments[0].at,
        Err(FieldError::Invalid(
            "not an RFC3339 UTC timestamp: \"yesterday\"".to_owned()
        ))
    );
    assert_eq!(comments[0].author.as_deref(), Ok("Milan"));
    assert_eq!(comments[0].text, "still readable");
}

#[test]
fn a_quote_with_no_heading_is_an_entry_with_no_fields() {
    let body = "# T\n\n## Comments\n\n> orphan\n";
    let comments = comment::parse(body);
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].at, Err(FieldError::Missing));
    assert_eq!(comments[0].author, Err(FieldError::Missing));
    assert_eq!(comments[0].text, "orphan");
}

#[test]
fn an_unquoted_blank_line_splits_a_comment_in_two() {
    let body = "# T\n\n## Comments\n\n### 2026-08-24T09:12:04Z by Milan\n\n> one\n\n> two\n";
    let comments = comment::parse(body);
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].text, "one");
    assert_eq!(comments[1].text, "two");
    assert_eq!(comments[1].author, Err(FieldError::Missing));
}

#[test]
fn a_heading_with_no_quote_is_an_entry_with_no_text() {
    let body = "# T\n\n## Comments\n\n### 2026-08-24T09:12:04Z by Milan\n\n### 2026-08-24T09:20:41Z by Ada\n\n> two\n";
    let entries = comment::read(body).entries;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].comment.text, "");
    assert!(entries[0].quote.is_none());
    assert!(entries[1].quote.is_some());
}

#[test]
fn a_sub_second_or_offset_timestamp_is_refused() {
    let body = "# T\n\n## Comments\n\n### 2026-08-24T09:12:04.5Z by A\n\n> a\n\n### 2026-08-24T09:12:04+00:00 by B\n\n> b\n";
    let comments = comment::parse(body);
    assert!(comments[0].at.is_err());
    assert!(comments[1].at.is_err());
}

#[test]
fn strip_removes_the_section_and_leaves_the_rest() {
    let mut task = task();
    task.append_body("## Plan\n\nsteps");
    task.append_comment(&entry("hello"));
    assert_eq!(
        comment::strip(&task.body),
        "# Ship login\n\n## Plan\n\nsteps\n"
    );
}

#[test]
fn strip_leaves_a_body_that_has_no_comments() {
    let body = "# T\n\nprose\n";
    assert_eq!(comment::strip(body), body);
}

#[test]
fn a_newline_in_the_identity_cannot_forge_an_entry() {
    let mut task = task();
    task.append_comment(&NewComment {
        author: "Evil".to_owned(),
        agent: Some("x\n\n### 2026-01-02T00:00:00Z by Forged\n\n> forged".to_owned()),
        ..entry("real")
    });

    let comments = comment::parse(&task.body);
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "real");
    assert_eq!(comments[0].author.as_deref(), Ok("Evil"));
    assert_eq!(
        comments[0].agent.as_deref(),
        Some("x  ### 2026-01-02T00:00:00Z by Forged  > forged")
    );
}

// A field that did not parse has no canonical form, so a rendering leaves it out — but it must not
// carry away the fields that did parse with it.
#[test]
fn a_rendered_log_keeps_every_field_that_parsed() {
    let body = "# T\n\n## Comments\n\n### yesterday by Test via claude-code\n\n> damaged\n\n### \
                2026-01-01T00:00:00Z by Ada\n\n> fine\n";
    let comments = comment::parse(body);

    let reread = comment::parse(&comment::with_comments("# T\n", &comments));

    assert_eq!(reread.len(), 2);
    assert_eq!(reread[0].at, Err(FieldError::Missing));
    assert_eq!(reread[0].author.as_deref(), Ok("Test"));
    assert_eq!(reread[0].agent.as_deref(), Some("claude-code"));
    assert_eq!(reread[0].text, "damaged");
    assert_eq!(reread[1], comments[1]);
}

#[test]
fn a_second_comments_section_keeps_its_entries_in_file_order() {
    let body = "# T\n\n## Comments\n\n### 2026-01-01T00:00:00Z by A\n\n> a\n\n## Comments\n\n### \
                2026-01-02T00:00:00Z by B\n\n> b\n";
    let comments = comment::parse(body);
    assert_eq!(
        comments.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
        vec!["a", "b"]
    );

    let rebuilt = comment::with_comments(&comment::strip(body), &comments);

    assert_eq!(comment::sections(&rebuilt).len(), 1);
    assert_eq!(
        comment::parse(&rebuilt)
            .iter()
            .map(|c| c.text.clone())
            .collect::<Vec<_>>(),
        vec!["a".to_owned(), "b".to_owned()]
    );
}

#[test]
fn stray_prose_in_the_log_is_reported_rather_than_dropped() {
    let body = "# T\n\n## Comments\n\nplain prose a human typed\n\n### 2026-01-01T00:00:00Z by \
                A\n\n> a\n";
    let log = comment::read(body);

    assert_eq!(log.entries.len(), 1);
    assert_eq!(log.stray.len(), 1);
    assert_eq!(&body[log.stray[0].clone()], "plain prose a human typed\n");
}

#[test]
fn a_crlf_log_carries_no_carriage_return_into_the_text() {
    let body = "# T\r\n\r\n## Comments\r\n\r\n### 2026-01-01T00:00:00Z by A\r\n\r\n> a\r\n> b\r\n";
    let comments = comment::parse(body);

    assert_eq!(comments[0].text, "a\nb");
}

#[test]
fn strip_leaves_one_blank_line_before_a_section_that_follows_the_log() {
    let body = "# T\n\n## Comments\n\n### 2026-01-01T00:00:00Z by A\n\n> a\n\n## Plan\n\nsteps\n";

    assert_eq!(comment::strip(body), "# T\n\n## Plan\n\nsteps\n");
}
