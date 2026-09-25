use std::path::Path;
use std::time::Duration;

use op_api::{
    ApiErrorBody, BackendKind, Comment, CreateComment, CreateTag, CreateTask, DaemonInfo,
    HistoryEntry, ProjectView, Refusal, RegisterProject, RenameProject, SearchHit, SyncResult,
    SyncView, TagPatch, TagView, TaskAtRevision, TaskDetail, TaskListItem, TaskPatch, TaskTreeView,
    WriteTaskFile,
};
use reqwest::Url;
use reqwest::blocking::{RequestBuilder, Response};
use serde::Deserialize;
use serde::de::DeserializeOwned;

pub const DEFAULT_PORT: u16 = 7373;

#[derive(Debug, thiserror::Error)]
#[error("OPENPLAN_PORT={0} is not a port number")]
pub struct InvalidPort(String);

// The port the daemon binds unless told otherwise. A write brings the daemon up itself, with no
// `--port` to carry, so the override has to be reachable from the environment too. A value that is
// not a port number gets no default: 7373 would attach the caller to a daemon it never named.
pub fn default_port() -> Result<u16, InvalidPort> {
    match std::env::var("OPENPLAN_PORT") {
        Ok(value) => value.parse().map_err(|_| InvalidPort(value)),
        Err(_) => Ok(DEFAULT_PORT),
    }
}

pub fn base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
// A read has no local answer to fall back on, so it waits as long as a write does.
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);
// A write can wait on another writer, or on a sync the daemon runs.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("cannot reach the openplan daemon: {0}")]
    Unreachable(String),
    // Giving up on the response says nothing about the write: the daemon may still finish it.
    // Retrying is what duplicates a task, so say so.
    #[error(
        "the openplan daemon did not answer within {}s; the write may still have completed — check before retrying",
        WRITE_TIMEOUT.as_secs()
    )]
    TimedOut,
    #[error(
        "the openplan daemon did not answer a read within {}s",
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

// Who the writes of this client are for. The daemon signs each revision with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Identity {
    pub name: Option<String>,
    pub email: Option<String>,
    pub agent: Option<String>,
}

pub struct Client {
    http: reqwest::blocking::Client,
    identity: Identity,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            http: reqwest::blocking::Client::new(),
            identity: Identity::default(),
        }
    }
}

#[derive(Deserialize)]
struct CreatedTask {
    id: String,
}

impl Client {
    pub fn with_identity(mut self, identity: Identity) -> Self {
        self.identity = identity;
        self
    }

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
    // daemon to read the writes of other processes first.
    pub fn tasks(&self, base_url: &str, project: &str) -> Result<Vec<TaskListItem>, ClientError> {
        self.read(fresh(tasks_url(base_url, project, None)?))
    }

    pub fn search(
        &self,
        base_url: &str,
        project: &str,
        query: &str,
    ) -> Result<Vec<SearchHit>, ClientError> {
        let mut url = sub_url(projects_url(base_url, project)?, base_url, &["search"])?;
        url.query_pairs_mut().append_pair("q", query);
        self.read(fresh(url))
    }

    pub fn task(&self, base_url: &str, project: &str, id: &str) -> Result<TaskDetail, ClientError> {
        self.read(fresh(tasks_url(base_url, project, Some(id))?))
    }

    pub fn task_tree(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
        depth: Option<usize>,
    ) -> Result<TaskTreeView, ClientError> {
        let mut url = sub_url(tasks_url(base_url, project, Some(id))?, base_url, &["tree"])?;
        if let Some(depth) = depth {
            url.query_pairs_mut()
                .append_pair("depth", &depth.to_string());
        }
        self.read(fresh(url))
    }

    pub fn comments(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
    ) -> Result<Vec<Comment>, ClientError> {
        let url = sub_url(
            tasks_url(base_url, project, Some(id))?,
            base_url,
            &["comments"],
        )?;
        self.read(fresh(url))
    }

    pub fn add_comment(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
        comment: &CreateComment,
    ) -> Result<Comment, ClientError> {
        let url = sub_url(
            tasks_url(base_url, project, Some(id))?,
            base_url,
            &["comments"],
        )?;
        self.json(self.write(self.http.post(url)).json(comment))
    }

    pub fn history(
        &self,
        base_url: &str,
        project: &str,
        before: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<HistoryEntry>, ClientError> {
        let url = sub_url(projects_url(base_url, project)?, base_url, &["history"])?;
        self.read(page(url, before, limit))
    }

    pub fn task_history(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
        before: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<HistoryEntry>, ClientError> {
        let url = sub_url(
            tasks_url(base_url, project, Some(id))?,
            base_url,
            &["history"],
        )?;
        self.read(page(url, before, limit))
    }

    pub fn task_revision(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
        revision: &str,
    ) -> Result<TaskAtRevision, ClientError> {
        let url = sub_url(
            tasks_url(base_url, project, Some(id))?,
            base_url,
            &["revisions", revision],
        )?;
        self.read(url)
    }

    pub fn sync_status(&self, base_url: &str, project: &str) -> Result<SyncView, ClientError> {
        self.read(sub_url(
            projects_url(base_url, project)?,
            base_url,
            &["sync"],
        )?)
    }

    pub fn sync(&self, base_url: &str, project: &str) -> Result<SyncResult, ClientError> {
        let url = sub_url(projects_url(base_url, project)?, base_url, &["sync"])?;
        self.json(self.write(self.http.post(url)))
    }

    pub fn tags(&self, base_url: &str, project: &str) -> Result<Vec<TagView>, ClientError> {
        self.read(tags_url(base_url, project, None)?)
    }

    pub fn tag(&self, base_url: &str, project: &str, name: &str) -> Result<TagView, ClientError> {
        self.read(tags_url(base_url, project, Some(name))?)
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

    // The caller names the wait it can afford.
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

    // The bool says whether this call is what registered the project; the daemon answers 200 for one
    // it already serves. An abbreviation starts the project's tasks.
    pub fn register_project(
        &self,
        base_url: &str,
        path: &Path,
        backend: Option<BackendKind>,
        abbreviation: Option<&str>,
    ) -> Result<(ProjectView, bool), ClientError> {
        let body = RegisterProject {
            path: path.display().to_string(),
            backend,
            abbreviation: abbreviation.map(str::to_owned),
        };
        let response = accepted(send(
            self.write(self.http.post(format!("{base_url}/api/projects")))
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

    pub fn create_task(
        &self,
        base_url: &str,
        project: &str,
        task: &CreateTask,
    ) -> Result<String, ClientError> {
        let url = tasks_url(base_url, project, None)?;
        let created: CreatedTask = self.json(self.write(self.http.post(url)).json(task))?;
        Ok(created.id)
    }

    pub fn patch_task(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
        patch: &TaskPatch,
    ) -> Result<TaskDetail, ClientError> {
        let url = tasks_url(base_url, project, Some(id))?;
        self.json(self.write(self.http.patch(url)).json(patch))
    }

    pub fn write_task_file(
        &self,
        base_url: &str,
        project: &str,
        id: &str,
        text: &str,
    ) -> Result<TaskDetail, ClientError> {
        let url = sub_url(tasks_url(base_url, project, Some(id))?, base_url, &["file"])?;
        let body = WriteTaskFile {
            text: text.to_owned(),
        };
        self.json(self.write(self.http.put(url)).json(&body))
    }

    pub fn delete_task(&self, base_url: &str, project: &str, id: &str) -> Result<(), ClientError> {
        let url = tasks_url(base_url, project, Some(id))?;
        accepted(send(self.write(self.http.delete(url)))?).map(drop)
    }

    pub fn create_tag(
        &self,
        base_url: &str,
        project: &str,
        tag: &CreateTag,
    ) -> Result<TagView, ClientError> {
        let url = tags_url(base_url, project, None)?;
        self.json(self.write(self.http.post(url)).json(tag))
    }

    pub fn patch_tag(
        &self,
        base_url: &str,
        project: &str,
        name: &str,
        patch: &TagPatch,
    ) -> Result<TagView, ClientError> {
        let url = tags_url(base_url, project, Some(name))?;
        self.json(self.write(self.http.patch(url)).json(patch))
    }

    pub fn delete_tag(
        &self,
        base_url: &str,
        project: &str,
        name: &str,
        force: bool,
    ) -> Result<(), ClientError> {
        let mut url = tags_url(base_url, project, Some(name))?;
        if force {
            url.query_pairs_mut().append_pair("force", "true");
        }
        accepted(send(self.write(self.http.delete(url)))?).map(drop)
    }

    fn write(&self, mut request: RequestBuilder) -> RequestBuilder {
        let headers = [
            (op_api::AUTHOR_HEADER, &self.identity.name),
            (op_api::EMAIL_HEADER, &self.identity.email),
            (op_api::AGENT_HEADER, &self.identity.agent),
        ];
        for (name, value) in headers {
            if let Some(value) = value {
                request = request.header(name, op_api::encode_header(value));
            }
        }
        request
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

fn page(mut url: Url, before: Option<&str>, limit: Option<usize>) -> Url {
    if let Some(before) = before {
        url.query_pairs_mut().append_pair("before", before);
    }
    if let Some(limit) = limit {
        url.query_pairs_mut()
            .append_pair("limit", &limit.to_string());
    }
    url
}
