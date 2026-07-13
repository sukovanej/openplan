use std::time::Duration;

use op_api::DaemonInfo;

const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

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
