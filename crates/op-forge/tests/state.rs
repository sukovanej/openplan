use op_forge::{
    AnswerError, Forge, ForgeKind, PullRequest, PullRequestState, parse_states, states_query,
};

fn github(repo: &str) -> Forge {
    Forge {
        kind: ForgeKind::Github,
        host: "github.com".to_owned(),
        repo: repo.to_owned(),
    }
}

fn gitlab(repo: &str) -> Forge {
    Forge {
        kind: ForgeKind::Gitlab,
        host: "gitlab.example.com".to_owned(),
        repo: repo.to_owned(),
    }
}

fn at(text: &str) -> jiff::Timestamp {
    text.parse().unwrap()
}

#[test]
fn the_github_change_query_searches_the_repository_since_a_time() {
    let query = github("acme/widgets").changed_query(at("2026-11-13T10:00:00Z"), None);
    assert!(query.contains("viewer { login }"), "{query}");
    assert!(
        query.contains(
            r#"query: "repo:acme/widgets is:pr updated:>=2026-11-13T10:00:00+00:00 sort:updated-asc""#
        ),
        "{query}"
    );
    assert!(!query.contains("after:"), "{query}");
    let next = github("acme/widgets").changed_query(at("2026-11-13T10:00:00Z"), Some("Y3Vy"));
    assert!(next.contains(r#"after: "Y3Vy""#), "{next}");
}

#[test]
fn the_gitlab_change_query_lists_merge_requests_updated_after_a_time() {
    let query = gitlab("group/sub/app").changed_query(at("2026-11-13T10:00:00Z"), None);
    assert!(
        query.contains(r#"project(fullPath: "group/sub/app")"#),
        "{query}"
    );
    assert!(
        query.contains(r#"mergeRequests(updatedAfter: "2026-11-13T10:00:00Z""#),
        "{query}"
    );
    assert!(query.contains("currentUser { username }"), "{query}");
}

#[test]
fn a_github_change_page_reads_each_state_and_the_next_cursor() {
    let answer = r#"{"data":{"viewer":{"login":"ann"},"search":{
        "pageInfo":{"hasNextPage":true,"endCursor":"Y3Vy"},
        "nodes":[
          {"number":214,"title":"Fix the parser (OPP-42)","state":"OPEN","isDraft":false,"headRefName":"opp-42","createdAt":"2026-11-13T10:01:00Z","updatedAt":"2026-11-13T10:02:00Z","author":{"login":"ann"}},
          {"number":215,"title":"Draft","state":"OPEN","isDraft":true,"headRefName":"x","createdAt":"2026-11-13T10:01:00Z","updatedAt":"2026-11-13T10:02:00Z","author":null},
          {"number":216,"title":"Done","state":"MERGED","isDraft":false,"headRefName":"y","createdAt":"2026-11-13T10:01:00Z","updatedAt":"2026-11-13T10:02:00Z","author":{"login":"ben"}},
          {"number":217,"title":"No","state":"CLOSED","isDraft":true,"headRefName":"z","createdAt":"2026-11-13T10:01:00Z","updatedAt":"2026-11-13T10:02:00Z","author":{"login":"ben"}},
          {}
        ]}}}"#;
    let page = github("acme/widgets").parse_changed(answer).unwrap();
    assert_eq!(page.viewer.as_deref(), Some("ann"));
    assert_eq!(page.next.as_deref(), Some("Y3Vy"));
    let states: Vec<(u64, PullRequestState)> = page
        .pull_requests
        .iter()
        .map(|read| (read.pull_request.number, read.state))
        .collect();
    assert_eq!(
        states,
        vec![
            (214, PullRequestState::Open),
            (215, PullRequestState::Draft),
            (216, PullRequestState::Merged),
            (217, PullRequestState::Closed),
        ]
    );
    let first = &page.pull_requests[0];
    assert_eq!(first.pull_request, github("acme/widgets").pull_request(214));
    assert_eq!(first.head, "opp-42");
    assert_eq!(first.author.as_deref(), Some("ann"));
    assert_eq!(first.updated_at, at("2026-11-13T10:02:00Z"));
    assert_eq!(page.pull_requests[1].author, None);
}

#[test]
fn a_gitlab_change_page_reads_the_iid_and_the_state() {
    let answer = r#"{"data":{"currentUser":{"username":"ann"},"project":{"mergeRequests":{
        "pageInfo":{"hasNextPage":false,"endCursor":"x"},
        "nodes":[
          {"iid":"7","title":"T","state":"opened","draft":true,"sourceBranch":"opp-1","createdAt":"2026-11-13T10:01:00Z","updatedAt":"2026-11-13T10:02:00Z","author":{"username":"ann"}},
          {"iid":"8","title":"T","state":"merged","draft":false,"sourceBranch":"b","createdAt":"2026-11-13T10:01:00Z","updatedAt":"2026-11-13T10:02:00Z","author":{"username":"ann"}},
          {"iid":"9","title":"T","state":"locked","draft":false,"sourceBranch":"b","createdAt":"2026-11-13T10:01:00Z","updatedAt":"2026-11-13T10:02:00Z","author":{"username":"ann"}}
        ]}}}}"#;
    let page = gitlab("group/app").parse_changed(answer).unwrap();
    assert_eq!(page.next, None);
    let states: Vec<(u64, PullRequestState)> = page
        .pull_requests
        .iter()
        .map(|read| (read.pull_request.number, read.state))
        .collect();
    assert_eq!(
        states,
        vec![
            (7, PullRequestState::Draft),
            (8, PullRequestState::Merged),
            (9, PullRequestState::Open),
        ]
    );
}

#[test]
fn a_change_page_with_no_repository_is_an_error_with_the_forge_message() {
    let answer = r#"{"data":{"viewer":{"login":"ann"},"search":null},"errors":[{"message":"Bad credentials"}]}"#;
    assert_eq!(
        github("acme/widgets").parse_changed(answer),
        Err(AnswerError::NoData("Bad credentials".to_owned()))
    );
    assert!(matches!(
        github("acme/widgets").parse_changed("gh: not logged in"),
        Err(AnswerError::NotJson(_))
    ));
}

#[test]
fn one_query_reads_the_pull_requests_of_many_repositories() {
    let pull_requests = [
        github("acme/widgets").pull_request(214),
        github("rust-lang/cargo").pull_request(1234),
        github("ACME/Widgets").pull_request(300),
        github("acme/widgets").pull_request(214),
    ];
    let query = states_query(&pull_requests).unwrap();
    assert_eq!(
        query.matches("repository(").count(),
        2,
        "one alias for each repository: {query}"
    );
    assert!(
        query.contains(
            r#"r0: repository(owner: "acme", name: "widgets") { p214: pullRequest(number: 214)"#
        ),
        "{query}"
    );
    assert!(query.contains("p300: pullRequest(number: 300)"), "{query}");
    assert!(
        query.contains(r#"r1: repository(owner: "rust-lang", name: "cargo")"#),
        "{query}"
    );
    assert_eq!(states_query(&[]), None);
}

#[test]
fn a_missing_pull_request_reads_as_none_beside_the_others() {
    let pull_requests = [
        github("acme/widgets").pull_request(214),
        github("acme/widgets").pull_request(999),
    ];
    let answer = r#"{"data":{"r0":{
        "p214":{"number":214,"title":"T","state":"MERGED","isDraft":false,"headRefName":"b","createdAt":"2026-11-13T10:01:00Z","updatedAt":"2026-11-13T10:02:00Z","author":{"login":"ann"}},
        "p999":null}},
        "errors":[{"message":"Could not resolve to a PullRequest with the number of 999."}]}"#;
    let states = parse_states(&pull_requests, answer).unwrap();
    assert_eq!(states.len(), 2);
    assert_eq!(states[0].0, pull_requests[0]);
    assert_eq!(
        states[0].1.as_ref().map(|read| read.state),
        Some(PullRequestState::Merged)
    );
    assert_eq!(states[1], (pull_requests[1].clone(), None));
}

#[test]
fn gitlab_reads_merge_requests_by_iid() {
    let query = states_query(&[gitlab("group/app").pull_request(7)]).unwrap();
    assert!(
        query.contains(r#"r0: project(fullPath: "group/app") { p7: mergeRequest(iid: "7")"#),
        "{query}"
    );
}

#[test]
fn a_quote_in_a_repository_cannot_break_the_query() {
    let query = states_query(&[PullRequest {
        forge: github(r#"ac"me/w"#),
        number: 1,
    }])
    .unwrap();
    assert!(query.contains(r#"owner: "ac\"me""#), "{query}");
}
