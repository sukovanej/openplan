use std::collections::BTreeMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::{Forge, ForgeKind, PullRequest};

// A search page holds at most 100 results on both forges.
const PAGE: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestState {
    Open,
    Draft,
    Merged,
    Closed,
}

impl PullRequestState {
    pub fn as_str(self) -> &'static str {
        match self {
            PullRequestState::Open => "open",
            PullRequestState::Draft => "draft",
            PullRequestState::Merged => "merged",
            PullRequestState::Closed => "closed",
        }
    }

    // A merged or closed pull request seldom changes again.
    pub fn is_settled(self) -> bool {
        matches!(self, PullRequestState::Merged | PullRequestState::Closed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestRead {
    pub pull_request: PullRequest,
    pub state: PullRequestState,
    pub title: String,
    pub head: String,
    pub author: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

// One page of the pull requests of a repository that changed since a time. `viewer` is the login
// the forge CLI uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedPage {
    pub viewer: Option<String>,
    pub pull_requests: Vec<PullRequestRead>,
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnswerError {
    #[error("the forge answered with no JSON: {0}")]
    NotJson(String),
    #[error("the forge answered with no data: {0}")]
    NoData(String),
}

impl Forge {
    // The GitHub search index lags behind a change, so the caller asks for a window that overlaps
    // the last one.
    pub fn changed_query(&self, since: Timestamp, after: Option<&str>) -> String {
        let after = after.map_or(String::new(), |cursor| {
            format!(", after: {}", literal(cursor))
        });
        match self.kind {
            ForgeKind::Github => {
                let search = format!(
                    "repo:{} is:pr updated:>={} sort:updated-asc",
                    self.repo,
                    since.strftime("%Y-%m-%dT%H:%M:%S+00:00")
                );
                format!(
                    "query {{ viewer {{ login }} search(type: ISSUE, first: {PAGE}, query: {}{after}) \
                     {{ pageInfo {{ hasNextPage endCursor }} nodes {{ ... on PullRequest {{ {GITHUB_FIELDS} }} }} }} }}",
                    literal(&search)
                )
            }
            ForgeKind::Gitlab => format!(
                "query {{ currentUser {{ username }} project(fullPath: {}) {{ mergeRequests(updatedAfter: {}, \
                 sort: UPDATED_ASC, first: {PAGE}{after}) {{ pageInfo {{ hasNextPage endCursor }} nodes {{ {GITLAB_FIELDS} }} }} }} }}",
                literal(&self.repo),
                literal(&since.to_string())
            ),
        }
    }

    pub fn parse_changed(&self, answer: &str) -> Result<ChangedPage, AnswerError> {
        let data = data_of(answer)?;
        let (viewer, connection) = match self.kind {
            ForgeKind::Github => (&data["viewer"]["login"], &data["search"]),
            ForgeKind::Gitlab => (
                &data["currentUser"]["username"],
                &data["project"]["mergeRequests"],
            ),
        };
        if connection.is_null() {
            return Err(AnswerError::NoData(errors_of(answer)));
        }
        let pull_requests = connection["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|node| read(self, node))
            .collect();
        let page = &connection["pageInfo"];
        let next = match page["hasNextPage"].as_bool() {
            Some(true) => page["endCursor"].as_str().map(str::to_owned),
            _ => None,
        };
        Ok(ChangedPage {
            viewer: viewer.as_str().map(str::to_owned),
            pull_requests,
            next,
        })
    }
}

// One query for the pull requests of every repository on one host. They must share the forge kind
// and the host.
pub fn states_query(pull_requests: &[PullRequest]) -> Option<String> {
    let first = pull_requests.first()?;
    let fields = match first.forge.kind {
        ForgeKind::Github => GITHUB_FIELDS,
        ForgeKind::Gitlab => GITLAB_FIELDS,
    };
    let repositories: Vec<String> = repositories(pull_requests)
        .into_iter()
        .enumerate()
        .map(|(n, (forge, numbers))| {
            let fetches: String = numbers
                .iter()
                .map(|number| match forge.kind {
                    ForgeKind::Github => {
                        format!(" p{number}: pullRequest(number: {number}) {{ {fields} }}")
                    }
                    ForgeKind::Gitlab => format!(
                        " p{number}: mergeRequest(iid: {}) {{ {fields} }}",
                        literal(&number.to_string())
                    ),
                })
                .collect();
            match forge.kind {
                ForgeKind::Github => {
                    let (owner, name) = forge.repo.split_once('/').unwrap_or((&forge.repo, ""));
                    format!(
                        "r{n}: repository(owner: {}, name: {}) {{{fetches} }}",
                        literal(owner),
                        literal(name)
                    )
                }
                ForgeKind::Gitlab => format!(
                    "r{n}: project(fullPath: {}) {{{fetches} }}",
                    literal(&forge.repo)
                ),
            }
        })
        .collect();
    Some(format!("query {{ {} }}", repositories.join(" ")))
}

// `None` for a pull request that the forge does not show: deleted, or in a repository the login
// cannot read.
pub fn parse_states(
    pull_requests: &[PullRequest],
    answer: &str,
) -> Result<Vec<(PullRequest, Option<PullRequestRead>)>, AnswerError> {
    let data = data_of(answer)?;
    let repositories = repositories(pull_requests);
    let mut states = Vec::new();
    for (n, (forge, numbers)) in repositories.iter().enumerate() {
        let repository = &data[format!("r{n}")];
        for number in numbers {
            let node = &repository[format!("p{number}")];
            states.push((forge.pull_request(*number), read(forge, node)));
        }
    }
    Ok(states)
}

const GITHUB_FIELDS: &str =
    "number title state isDraft headRefName createdAt updatedAt author { login }";
const GITLAB_FIELDS: &str =
    "iid title state draft sourceBranch createdAt updatedAt author { username }";

// Repositories in the order they first appear, each with its numbers once.
fn repositories(pull_requests: &[PullRequest]) -> Vec<(Forge, Vec<u64>)> {
    let mut grouped: Vec<(Forge, Vec<u64>)> = Vec::new();
    for pull_request in pull_requests {
        match grouped
            .iter_mut()
            .find(|(forge, _)| forge.is(&pull_request.forge))
        {
            Some((_, numbers)) if numbers.contains(&pull_request.number) => {}
            Some((_, numbers)) => numbers.push(pull_request.number),
            None => grouped.push((pull_request.forge.clone(), vec![pull_request.number])),
        }
    }
    grouped
}

fn read(forge: &Forge, node: &Value) -> Option<PullRequestRead> {
    let (number, state, head, author) = match forge.kind {
        ForgeKind::Github => {
            let state = match (node["state"].as_str()?, node["isDraft"].as_bool()) {
                ("MERGED", _) => PullRequestState::Merged,
                ("CLOSED", _) => PullRequestState::Closed,
                (_, Some(true)) => PullRequestState::Draft,
                _ => PullRequestState::Open,
            };
            (
                node["number"].as_u64()?,
                state,
                &node["headRefName"],
                &node["author"]["login"],
            )
        }
        ForgeKind::Gitlab => {
            let state = match (node["state"].as_str()?, node["draft"].as_bool()) {
                ("merged", _) => PullRequestState::Merged,
                ("closed", _) => PullRequestState::Closed,
                (_, Some(true)) => PullRequestState::Draft,
                _ => PullRequestState::Open,
            };
            (
                node["iid"].as_str()?.parse().ok()?,
                state,
                &node["sourceBranch"],
                &node["author"]["username"],
            )
        }
    };
    Some(PullRequestRead {
        pull_request: forge.pull_request(number),
        state,
        title: node["title"].as_str().unwrap_or_default().to_owned(),
        head: head.as_str().unwrap_or_default().to_owned(),
        author: author.as_str().map(str::to_owned),
        created_at: node["createdAt"].as_str()?.parse().ok()?,
        updated_at: node["updatedAt"].as_str()?.parse().ok()?,
    })
}

// A forge answers a query that names a missing pull request with the data it has and an error for
// the rest, so `data` counts even beside `errors`.
fn data_of(answer: &str) -> Result<Value, AnswerError> {
    let value: Value = serde_json::from_str(answer)
        .map_err(|_| AnswerError::NotJson(answer.trim().chars().take(200).collect()))?;
    match value.get("data") {
        Some(data) if data.is_object() => Ok(data.clone()),
        _ => Err(AnswerError::NoData(errors_of(answer))),
    }
}

fn errors_of(answer: &str) -> String {
    let messages: Vec<String> = serde_json::from_str::<Value>(answer)
        .ok()
        .and_then(|value| value["errors"].as_array().cloned())
        .into_iter()
        .flatten()
        .filter_map(|error| error["message"].as_str().map(str::to_owned))
        .collect();
    match messages.is_empty() {
        true => "no error message".to_owned(),
        false => messages.join("; "),
    }
}

// A GraphQL string reads as a JSON string.
fn literal(text: &str) -> String {
    serde_json::to_string(text).expect("a string serializes")
}

// Changes the forge sends twice keep the newest.
pub fn newest(reads: impl IntoIterator<Item = PullRequestRead>) -> Vec<PullRequestRead> {
    let mut by_number: BTreeMap<u64, PullRequestRead> = BTreeMap::new();
    for read in reads {
        match by_number.get(&read.pull_request.number) {
            Some(held) if held.updated_at >= read.updated_at => {}
            _ => {
                by_number.insert(read.pull_request.number, read);
            }
        }
    }
    by_number.into_values().collect()
}
