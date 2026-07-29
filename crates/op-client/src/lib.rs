use std::time::Duration;

use op_api::{DaemonInfo, TaskDetail};

const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const READ_TIMEOUT: Duration = Duration::from_secs(2);

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
}
