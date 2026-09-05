use std::path::Path;
use std::time::Duration;

use op_api::{
    ApiErrorBody, BranchComments, Comment, CreateComment, CreateTag, CreateTask, DaemonInfo,
    Matrix, ProjectView, Refusal, RegisterProject, RenameProject, SearchHit, TagPatch, TagView,
    TaskBranches, TaskDetail, TaskListItem, TaskPatch, TaskTreeView,
};
use reqwest::Url;
use reqwest::blocking::{RequestBuilder, Response};
use serde::Deserialize;
use serde::de::DeserializeOwned;

const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
// A read has no local answer to fall back on, and it asks the daemon to walk every branch before
// answering, so it waits as long as a write does.
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);
// A write waits for the target file's advisory lock, which another writer may hold for a while.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("cannot reach the openplan daemon: {0}")]
    Unreachable(String),
    // Giving up on the response says nothing about the write: the daemon is still waiting on the
    // file's lock and will finish. Retrying is what duplicates a task, so say so.
    #[error(
        "the openplan daemon did not answer within {}s (another writer may be holding the file); the write may still have completed — check before retrying",
        WRITE_TIMEOUT.as_secs()
    )]
    TimedOut,
    #[error(
        "the openplan daemon did not answer a read within {}s; it may still be walking the repository",
        READ_TIMEOUT.as_secs()
    )]
    ReadTimedOut,
    #[error("{message}")]
    Refused {
        status: u16,
        // The remedy is a spelling of the caller's own interface, so the daemon names the refusal
        // and leaves the sentence about it to whoever the caller is.
        reason: Option<Refusal>,
        message: String,
    },
    // The daemon answered, and the answer is not what this route returns. A daemon that predates a
    // route serves the web UI's index page from its fallback instead of 404-ing, so this is what an
    // out-of-date daemon looks like from here — not a transport failure.
    #[error("the openplan daemon answered {route} with a body this client cannot read: {message}")]
    Unreadable { route: String, message: String },
    // A body that is not JSON at all, which no route of this API ever answers with. Told apart from
    // `Unreadable` because that one covers JSON of the wrong shape too — a real schema mismatch,
    // which says nothing about the daemon's age.
    #[error("the openplan daemon answered {route} with {content_type}, not JSON")]
    NotJson { route: String, content_type: String },
}

pub struct Client {
    http: reqwest::blocking::Client,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            http: reqwest::blocking::Client::new(),
        }
    }
}

#[derive(Deserialize)]
struct CreatedTask {
    id: String,
}

impl Client {
    pub fn health(&self, base_url: &str) -> Option<DaemonInfo> {
        self.http
            .get(format!("{base_url}/health"))
            .timeout(HEALTH_TIMEOUT)
            .send()
            .ok()?
            .json::<DaemonInfo>()
            .ok()
    }

    // Every read below is a one-shot question from a caller with no change stream, so each asks the
    // daemon for a freshly walked index rather than one the watcher may not have invalidated yet.
    pub fn tasks(
        &self,
        base_url: &str,
        project: &str,
        branch: Option<&str>,
    ) -> Result<Vec<TaskListItem>, ClientError> {
        self.read(read_url(base_url, project, None, branch)?)
    }

    pub fn matrix(&self, base_url: &str, project: &str) -> Result<Matrix, ClientError> {
        let mut url = projects_url(base_url, project)?;
        url.path_segments_mut()
            .map_err(|_| unusable(base_url))?
            .push("matrix");
        self.read(fresh(url))
    }

    pub fn search(
        &self,
        base_url: &str,
        project: &str,
        query: &str,
    ) -> Result<Vec<SearchHit>, ClientError> {
        let mut url = projects_url(base_url, project)?;
        url.path_segments_mut()
            .map_err(|_| unusable(base_url))?
            .push("search");
        url.query_pairs_mut().append_pair("q", query);
        self.read(fresh(url))
    }

    pub fn task(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
        branch: Option<&str>,
    ) -> Result<TaskDetail, ClientError> {
        self.read(read_url(base_url, project, Some(id), branch)?)
    }

    pub fn task_tree(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
        branch: Option<&str>,
        depth: Option<usize>,
    ) -> Result<TaskTreeView, ClientError> {
        let mut url = read_url(base_url, project, Some(id), branch)?;
        url.path_segments_mut()
            .map_err(|_| unusable(base_url))?
            .push("tree");
        if let Some(depth) = depth {
            url.query_pairs_mut()
                .append_pair("depth", &depth.to_string());
        }
        self.read(url)
    }

    pub fn comments(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
        branch: Option<&str>,
    ) -> Result<Vec<Comment>, ClientError> {
        self.read(sub_url(
            read_url(base_url, project, Some(id), branch)?,
            base_url,
            &["comments"],
        )?)
    }

    pub fn branch_comments(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
    ) -> Result<Vec<BranchComments>, ClientError> {
        self.read(sub_url(
            read_url(base_url, project, Some(id), None)?,
            base_url,
            &["comments", "branches"],
        )?)
    }

    pub fn add_comment(
        &self,
        base_url: &str,
        project: &str,
        branch: &str,
        id: &str,
        comment: &CreateComment,
    ) -> Result<Comment, ClientError> {
        let url = sub_url(
            write_url(base_url, project, branch, Some(id))?,
            base_url,
            &["comments"],
        )?;
        self.json(self.http.post(url).json(comment))
    }

    pub fn task_branches(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
    ) -> Result<TaskBranches, ClientError> {
        let mut url = read_url(base_url, project, Some(id), None)?;
        url.path_segments_mut()
            .map_err(|_| unusable(base_url))?
            .push("branches");
        self.read(url)
    }

    pub fn tags(
        &self,
        base_url: &str,
        project: &str,
        branch: Option<&str>,
    ) -> Result<Vec<TagView>, ClientError> {
        self.read(tag_read_url(base_url, project, None, branch)?)
    }

    pub fn tag(
        &self,
        base_url: &str,
        project: &str,
        name: &str,
        branch: Option<&str>,
    ) -> Result<TagView, ClientError> {
        self.read(tag_read_url(base_url, project, Some(name), branch)?)
    }

    fn read<T: DeserializeOwned>(&self, url: Url) -> Result<T, ClientError> {
        let route = url.path().to_owned();
        let response = accepted(send_read(self.http.get(url))?)?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        if !content_type.starts_with("application/json") {
            return Err(ClientError::NotJson {
                route,
                content_type: match content_type.is_empty() {
                    true => "no content type".to_owned(),
                    false => content_type,
                },
            });
        }
        response.json().map_err(|err| ClientError::Unreadable {
            route,
            message: err.to_string(),
        })
    }

    // The caller names the wait it can afford: a write must know its project before it can start, a
    // read falls back to its own index rather than hold the terminal.
    pub fn projects(
        &self,
        base_url: &str,
        timeout: Duration,
    ) -> Result<Vec<ProjectView>, ClientError> {
        let request = self.http.get(format!("{base_url}/api/projects"));
        accepted(send_within(request, timeout)?)?
            .json()
            .map_err(|err| ClientError::Unreadable {
                route: "/api/projects".to_owned(),
                message: err.to_string(),
            })
    }

    // The bool says whether this call is what registered the repository; the daemon answers 200 for
    // one it already serves, so two concurrent first writes both get the project and only one of
    // them reports it.
    pub fn register_project(
        &self,
        base_url: &str,
        path: &Path,
    ) -> Result<(ProjectView, bool), ClientError> {
        let body = RegisterProject {
            path: path.display().to_string(),
        };
        let response = accepted(send(
            self.http
                .post(format!("{base_url}/api/projects"))
                .json(&body),
        )?)?;
        let created = response.status() == reqwest::StatusCode::CREATED;
        let view = response
            .json()
            .map_err(|err| ClientError::Unreachable(err.to_string()))?;
        Ok((view, created))
    }

    pub fn remove_project(&self, base_url: &str, name: &str) -> Result<(), ClientError> {
        accepted(send(self.http.delete(projects_url(base_url, name)?))?).map(drop)
    }

    pub fn rename_project(
        &self,
        base_url: &str,
        from: &str,
        to: &str,
    ) -> Result<ProjectView, ClientError> {
        let body = RenameProject {
            name: to.to_owned(),
        };
        self.json(self.http.patch(projects_url(base_url, from)?).json(&body))
    }

    pub fn shutdown(&self, base_url: &str) -> bool {
        self.http
            .post(format!("{base_url}/admin/shutdown"))
            .header(op_api::ADMIN_HEADER, "1")
            .timeout(SHUTDOWN_TIMEOUT)
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    // Every write names the branch it targets, so the daemon resolves the worktree at write time and
    // a branch switch underneath yields a refusal instead of a write to the wrong branch.
    pub fn create_task(
        &self,
        base_url: &str,
        project: &str,
        branch: &str,
        task: &CreateTask,
    ) -> Result<String, ClientError> {
        let url = write_url(base_url, project, branch, None)?;
        let created: CreatedTask = self.json(self.http.post(url).json(task))?;
        Ok(created.id)
    }

    pub fn patch_task(
        &self,
        base_url: &str,
        project: &str,
        branch: &str,
        id: &str,
        patch: &TaskPatch,
    ) -> Result<TaskDetail, ClientError> {
        let url = write_url(base_url, project, branch, Some(id))?;
        self.json(self.http.patch(url).json(patch))
    }

    pub fn delete_task(
        &self,
        base_url: &str,
        project: &str,
        branch: &str,
        id: &str,
    ) -> Result<(), ClientError> {
        let url = write_url(base_url, project, branch, Some(id))?;
        accepted(send(self.http.delete(url))?).map(drop)
    }

    pub fn create_tag(
        &self,
        base_url: &str,
        project: &str,
        branch: &str,
        tag: &CreateTag,
    ) -> Result<TagView, ClientError> {
        let url = tag_write_url(base_url, project, branch, None)?;
        self.json(self.http.post(url).json(tag))
    }

    pub fn patch_tag(
        &self,
        base_url: &str,
        project: &str,
        branch: &str,
        name: &str,
        patch: &TagPatch,
    ) -> Result<TagView, ClientError> {
        let url = tag_write_url(base_url, project, branch, Some(name))?;
        self.json(self.http.patch(url).json(patch))
    }

    pub fn delete_tag(
        &self,
        base_url: &str,
        project: &str,
        branch: &str,
        name: &str,
        force: bool,
    ) -> Result<(), ClientError> {
        let mut url = tag_write_url(base_url, project, branch, Some(name))?;
        if force {
            url.query_pairs_mut().append_pair("force", "true");
        }
        accepted(send(self.http.delete(url))?).map(drop)
    }

    fn json<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, ClientError> {
        accepted(send(request)?)?
            .json()
            .map_err(|err| ClientError::Unreachable(err.to_string()))
    }
}

fn send(request: RequestBuilder) -> Result<Response, ClientError> {
    send_within(request, WRITE_TIMEOUT)
}

// A read that times out has changed nothing, so it says so plainly rather than warn about a write
// that may have landed.
fn send_read(request: RequestBuilder) -> Result<Response, ClientError> {
    match send_within(request, READ_TIMEOUT) {
        Err(ClientError::TimedOut) => Err(ClientError::ReadTimedOut),
        other => other,
    }
}

fn send_within(request: RequestBuilder, timeout: Duration) -> Result<Response, ClientError> {
    request.timeout(timeout).send().map_err(|err| {
        if err.is_timeout() {
            ClientError::TimedOut
        } else {
            ClientError::Unreachable(err.to_string())
        }
    })
}

// The daemon answers every refusal with an `ApiErrorBody`; anything else (a proxy's page, an empty
// body from a dropped connection) leaves the status as the only thing worth reporting.
fn accepted(response: Response) -> Result<Response, ClientError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let (reason, message) = response
        .json::<ApiErrorBody>()
        .map(|body| (body.reason, body.message))
        .unwrap_or_else(|_| (None, format!("request failed with status {status}")));
    Err(ClientError::Refused {
        status: status.as_u16(),
        reason,
        message,
    })
}

fn unusable(base_url: &str) -> ClientError {
    ClientError::Unreachable(format!("{base_url} is not a usable daemon URL"))
}

fn projects_url(base_url: &str, project: &str) -> Result<Url, ClientError> {
    let mut url =
        Url::parse(&format!("{base_url}/api/projects")).map_err(|_| unusable(base_url))?;
    url.path_segments_mut()
        .map_err(|_| unusable(base_url))?
        .push(project);
    Ok(url)
}

fn tasks_url(base_url: &str, project: &str, id: Option<&str>) -> Result<Url, ClientError> {
    let mut url = projects_url(base_url, project)?;
    {
        let mut segments = url.path_segments_mut().map_err(|_| unusable(base_url))?;
        segments.push("tasks");
        if let Some(id) = id {
            segments.push(id);
        }
    }
    Ok(url)
}

fn tags_url(base_url: &str, project: &str, name: Option<&str>) -> Result<Url, ClientError> {
    let mut url = projects_url(base_url, project)?;
    {
        let mut segments = url.path_segments_mut().map_err(|_| unusable(base_url))?;
        segments.push("tags");
        if let Some(name) = name {
            segments.push(name);
        }
    }
    Ok(url)
}

fn tag_write_url(
    base_url: &str,
    project: &str,
    branch: &str,
    name: Option<&str>,
) -> Result<Url, ClientError> {
    let mut url = tags_url(base_url, project, name)?;
    url.query_pairs_mut().append_pair("branch", branch);
    Ok(url)
}

// A tag read names its branch the way a write does; the daemon walks the worktree either way, so
// there is no stale index for `fresh` to bypass.
fn tag_read_url(
    base_url: &str,
    project: &str,
    name: Option<&str>,
    branch: Option<&str>,
) -> Result<Url, ClientError> {
    let mut url = tags_url(base_url, project, name)?;
    if let Some(branch) = branch {
        url.query_pairs_mut().append_pair("branch", branch);
    }
    Ok(url)
}

fn write_url(
    base_url: &str,
    project: &str,
    branch: &str,
    id: Option<&str>,
) -> Result<Url, ClientError> {
    let mut url = tasks_url(base_url, project, id)?;
    url.query_pairs_mut().append_pair("branch", branch);
    Ok(url)
}

fn read_url(
    base_url: &str,
    project: &str,
    id: Option<&str>,
    branch: Option<&str>,
) -> Result<Url, ClientError> {
    let mut url = tasks_url(base_url, project, id)?;
    if let Some(branch) = branch {
        url.query_pairs_mut().append_pair("branch", branch);
    }
    Ok(fresh(url))
}

fn sub_url(mut url: Url, base_url: &str, segments: &[&str]) -> Result<Url, ClientError> {
    {
        let mut path = url.path_segments_mut().map_err(|_| unusable(base_url))?;
        for segment in segments {
            path.push(segment);
        }
    }
    Ok(url)
}

fn fresh(mut url: Url) -> Url {
    url.query_pairs_mut().append_pair("fresh", "true");
    url
}
