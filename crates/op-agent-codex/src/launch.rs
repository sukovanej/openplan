use std::path::Path;

use op_agent::{McpPolicy, Permissions, Persistence, SessionOptions};
use serde_json::{Value, json};
use tokio::process::Command;

pub fn command(binary: &Path, config: &[String], options: &SessionOptions) -> Command {
    let mut command = Command::new(binary);
    command
        .current_dir(&options.cwd)
        .arg("app-server")
        .args(["--listen", "stdio://"]);
    if options.mcp == McpPolicy::Disabled {
        // Every configured server puts its tool schemas in the prompt of every request.
        command.args(["--config", "mcp_servers={}"]);
    }
    for override_ in config {
        command.args(["--config", override_]);
    }
    command
}

// The notifications this backend never reads. Declining them up front keeps a busy turn from
// pushing thousands of lines through the pipe.
const IGNORED: [&str; 6] = [
    "thread/status/changed",
    "mcpServer/startupStatus/updated",
    "remoteControl/status/changed",
    "fs/changed",
    "hook/started",
    "hook/completed",
];

pub fn initialize() -> Value {
    json!({
        "clientInfo": { "name": "openplan", "version": env!("CARGO_PKG_VERSION") },
        "capabilities": { "optOutNotificationMethods": IGNORED },
    })
}

pub fn thread(options: &SessionOptions) -> (&'static str, Value) {
    let (sandbox, approval_policy) = policy(options.permissions);
    let mut params = json!({
        "cwd": options.cwd,
        "sandbox": sandbox,
        "approvalPolicy": approval_policy,
        "ephemeral": options.persistence == Persistence::Ephemeral,
    });
    if let Some(model) = &options.model {
        params["model"] = json!(model);
    }
    if let Some(instructions) = &options.instructions {
        params["developerInstructions"] = json!(instructions);
    }
    match &options.resume {
        Some(session) => {
            params["threadId"] = json!(session.0);
            ("thread/resume", params)
        }
        None => ("thread/start", params),
    }
}

pub fn turn(thread: &str, text: String, options: &SessionOptions) -> Value {
    let mut params = json!({
        "threadId": thread,
        "input": [{ "type": "text", "text": text }],
    });
    if let Some(effort) = options.effort {
        params["effort"] = json!(effort.as_str());
    }
    params
}

fn policy(permissions: Permissions) -> (&'static str, &'static str) {
    match permissions {
        Permissions::Ask => ("workspace-write", "untrusted"),
        Permissions::AcceptEdits => ("workspace-write", "on-request"),
        Permissions::Full => ("danger-full-access", "never"),
    }
}
