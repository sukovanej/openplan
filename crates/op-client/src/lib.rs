use std::time::Duration;

use op_api::{ApiErrorBody, CreateTask, DaemonInfo, TaskDetail, TaskPatch};
use reqwest::Url;
use reqwest::blocking::{RequestBuilder, Response};
use serde::Deserialize;
use serde::de::DeserializeOwned;

const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const READ_TIMEOUT: Duration = Duration::from_secs(2);
// A write waits for the target file's advisory lock, which another writer may hold for a while.
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("cannot reach the oplan daemon: {0}")]
    Unreachable(String),
    #[error("{message}")]
    Refused { status: u16, message: String },
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

    pub fn task(&self, base_url: &str, id: &str, branch: Option<&str>) -> Option<TaskDetail> {
        let mut url = reqwest::Url::parse(&format!("{base_url}/api/tasks/{id}")).ok()?;
        if let Some(branch) = branch {
            url.query_pairs_mut().append_pair("branch", branch);
        }
        let response = self.http.get(url).timeout(READ_TIMEOUT).send().ok()?;
        if !response.status().is_success() {
            return None;
        }
        response.json::<TaskDetail>().ok()
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
        branch: &str,
        task: &CreateTask,
    ) -> Result<String, ClientError> {
        let url = tasks_url(base_url, branch, None)?;
        let created: CreatedTask = self.json(self.http.post(url).json(task))?;
        Ok(created.id)
    }

    pub fn patch_task(
        &self,
        base_url: &str,
        branch: &str,
        id: &str,
        patch: &TaskPatch,
    ) -> Result<TaskDetail, ClientError> {
        let url = tasks_url(base_url, branch, Some(id))?;
        self.json(self.http.patch(url).json(patch))
    }

    pub fn delete_task(&self, base_url: &str, branch: &str, id: &str) -> Result<(), ClientError> {
        let url = tasks_url(base_url, branch, Some(id))?;
        accepted(send(self.http.delete(url))?).map(drop)
    }

    fn json<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, ClientError> {
        accepted(send(request)?)?
            .json()
            .map_err(|err| ClientError::Unreachable(err.to_string()))
    }
}

fn send(request: RequestBuilder) -> Result<Response, ClientError> {
    request
        .timeout(WRITE_TIMEOUT)
        .send()
        .map_err(|err| ClientError::Unreachable(err.to_string()))
}

// The daemon answers every refusal with an `ApiErrorBody`; anything else (a proxy's page, an empty
// body from a dropped connection) leaves the status as the only thing worth reporting.
fn accepted(response: Response) -> Result<Response, ClientError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let message = response
        .json::<ApiErrorBody>()
        .map(|body| body.message)
        .unwrap_or_else(|_| format!("request failed with status {status}"));
    Err(ClientError::Refused {
        status: status.as_u16(),
        message,
    })
}

fn tasks_url(base_url: &str, branch: &str, id: Option<&str>) -> Result<Url, ClientError> {
    let unusable = || ClientError::Unreachable(format!("{base_url} is not a usable daemon URL"));
    let mut url = Url::parse(&format!("{base_url}/api/tasks")).map_err(|_| unusable())?;
    if let Some(id) = id {
        url.path_segments_mut().map_err(|_| unusable())?.push(id);
    }
    url.query_pairs_mut().append_pair("branch", branch);
    Ok(url)
}
