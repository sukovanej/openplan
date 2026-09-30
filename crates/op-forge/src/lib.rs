use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const GITHUB_HOST: &str = "github.com";
const GITLAB_HOST: &str = "gitlab.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForgeKind {
    Github,
    Gitlab,
}

// One repository on a forge. `repo` is `owner/repo` on GitHub and `group/sub/project` on GitLab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Forge {
    pub kind: ForgeKind,
    pub host: String,
    pub repo: String,
}

impl Forge {
    // `is_gitlab_host` answers for a host that is neither github.com nor gitlab.com: a GitLab that a
    // company runs has a host of its own, and only the caller can know it.
    pub fn of_remote(url: &str, is_gitlab_host: impl FnOnce(&str) -> bool) -> Option<Self> {
        let (host, repo) = remote_parts(url)?;
        let kind = match host.as_str() {
            GITHUB_HOST => ForgeKind::Github,
            GITLAB_HOST => ForgeKind::Gitlab,
            host if is_gitlab_host(host) => ForgeKind::Gitlab,
            _ => return None,
        };
        let segments = repo.split('/').count();
        let named = match kind {
            ForgeKind::Github => segments == 2,
            ForgeKind::Gitlab => segments >= 2,
        };
        named.then_some(Self { kind, host, repo })
    }

    pub fn pull_request(&self, number: u64) -> PullRequest {
        PullRequest {
            forge: self.clone(),
            number,
        }
    }

    // Both forges compare a host and a repository path without case.
    fn is(&self, other: &Forge) -> bool {
        self.kind == other.kind
            && self.host.eq_ignore_ascii_case(&other.host)
            && self.repo.eq_ignore_ascii_case(&other.repo)
    }
}

// `https://<host>/<path>`, `ssh://git@<host>/<path>`, and `git@<host>:<path>`, each with or without
// `.git`. A local path and a `file://` address name no host.
fn remote_parts(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let (authority, path) = match url.split_once("://") {
        Some(("https" | "http" | "ssh", rest)) => rest.split_once('/')?,
        Some(_) => return None,
        None => url.split_once(':').filter(|(user, _)| user.contains('@'))?,
    };
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split(':').next().unwrap_or_default();
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    (!host.is_empty() && !path.is_empty()).then(|| (host.to_ascii_lowercase(), path.to_owned()))
}

// `glab auth status` names each host on a line of its own, and indents what it reports about it.
pub fn glab_hosts(auth_status: &str) -> Vec<String> {
    let names_a_host = |line: &&str| {
        !line.is_empty()
            && line
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'))
    };
    auth_status
        .lines()
        .filter(names_a_host)
        .map(str::to_ascii_lowercase)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    pub forge: Forge,
    pub number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PullRequestError {
    #[error(
        "not a pull request: {got:?}; expected https://github.com/<owner>/<repo>/pull/<n>, \
         https://<host>/<group>/<project>/-/merge_requests/<n>, or a number"
    )]
    NotAnAddress { got: String },
    #[error(
        "{got} names a pull request by its number, and this project has no GitHub or GitLab \
         remote; give the full address"
    )]
    NoForge { got: String },
}

impl PullRequest {
    // The address of any page of the pull request: the conversation, the files, the commits.
    pub fn parse(address: &str) -> Result<Self, PullRequestError> {
        parsed(address).ok_or_else(|| PullRequestError::NotAnAddress {
            got: address.to_owned(),
        })
    }

    // An address, or `214` or `#214` for a pull request of the project repository.
    pub fn resolve(input: &str, project: Option<&Forge>) -> Result<Self, PullRequestError> {
        let input = input.trim();
        match number(input.strip_prefix('#').unwrap_or(input)) {
            Some(number) => project
                .map(|forge| forge.pull_request(number))
                .ok_or_else(|| PullRequestError::NoForge {
                    got: input.to_owned(),
                }),
            None => Self::parse(input),
        }
    }

    pub fn is(&self, other: &PullRequest) -> bool {
        self.number == other.number && self.forge.is(&other.forge)
    }

    pub fn url(&self) -> String {
        let Forge { kind, host, repo } = &self.forge;
        match kind {
            ForgeKind::Github => format!("https://{host}/{repo}/pull/{}", self.number),
            ForgeKind::Gitlab => format!("https://{host}/{repo}/-/merge_requests/{}", self.number),
        }
    }

    // `#214` in the project repository, and `owner/repo#214` in every other one.
    pub fn short(&self, project: Option<&Forge>) -> String {
        match project.is_some_and(|project| project.is(&self.forge)) {
            true => format!("#{}", self.number),
            false => format!("{}#{}", self.forge.repo, self.number),
        }
    }
}

fn parsed(address: &str) -> Option<PullRequest> {
    let rest = address.trim().strip_prefix("https://")?;
    let rest = rest.split(['?', '#']).next()?;
    let (host, path) = rest.split_once('/')?;
    let host = host.to_ascii_lowercase();
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let (kind, repo, number) = match host.as_str() {
        GITHUB_HOST => match segments.as_slice() {
            [owner, repo, "pull", n, ..] => (ForgeKind::Github, format!("{owner}/{repo}"), *n),
            _ => return None,
        },
        _ => {
            let dash = segments.iter().position(|part| *part == "-")?;
            match &segments[dash..] {
                ["-", "merge_requests", n, ..] if dash >= 2 => {
                    (ForgeKind::Gitlab, segments[..dash].join("/"), *n)
                }
                _ => return None,
            }
        }
    };
    (!host.is_empty()).then_some(PullRequest {
        forge: Forge { kind, host, repo },
        number: self::number(number)?,
    })
}

fn number(text: &str) -> Option<u64> {
    let number: u64 = text.parse().ok()?;
    (number > 0 && number.to_string() == text).then_some(number)
}
