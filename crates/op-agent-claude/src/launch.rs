use std::path::Path;

use op_agent::{McpPolicy, Permissions, Persistence, SessionOptions};
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tools {
    All,
    Only(Vec<String>),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skills {
    Enabled,
    Disabled,
}

// `Inherit` runs the session the way the person who owns the machine configured Claude Code.
// `Ignore` drops the user, project and local settings files, so the session behaves the same on
// every machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settings {
    Inherit,
    Ignore,
}

pub fn command(
    binary: &Path,
    tools: &Tools,
    skills: &Skills,
    settings: &Settings,
    options: &SessionOptions,
) -> Command {
    let mut command = Command::new(binary);
    command
        .current_dir(&options.cwd)
        .arg("--print")
        .args(["--input-format", "stream-json"])
        .args(["--output-format", "stream-json"])
        .arg("--verbose")
        .arg("--include-partial-messages")
        .args(["--permission-prompts", "host"])
        // Without the tool named, the CLI answers its own prompts and denies anything it would
        // have asked about, so `--permission-prompts host` alone never reaches this process.
        .args(["--permission-prompt-tool", "stdio"])
        .args(["--permission-mode", permission_mode(options.permissions)])
        // Moves cwd, git status and memory paths out of the system prompt, which keeps the cached
        // prefix identical for every session this repository starts.
        .arg("--exclude-dynamic-system-prompt-sections");

    match tools {
        Tools::All => {}
        Tools::Only(names) => {
            command.args(["--tools", &names.join(",")]);
        }
        Tools::None => {
            command.args(["--tools", ""]);
        }
    }
    if *skills == Skills::Disabled {
        command.arg("--disable-slash-commands");
    }
    if *settings == Settings::Ignore {
        command.args(["--setting-sources", ""]);
    }
    if options.mcp == McpPolicy::Disabled {
        // With no --mcp-config alongside it, this leaves the session with no MCP server at all.
        command.arg("--strict-mcp-config");
    }
    if options.persistence == Persistence::Ephemeral {
        command.arg("--no-session-persistence");
    }
    if let Some(model) = &options.model {
        command.args(["--model", model]);
    }
    if let Some(effort) = options.effort {
        command.args(["--effort", effort.as_str()]);
    }
    if let Some(instructions) = &options.instructions {
        command.args(["--append-system-prompt", instructions]);
    }
    if let Some(session) = &options.resume {
        command.args(["--resume", &session.0]);
    }
    if let Some(schema) = &options.schema {
        command.args(["--json-schema", &schema.to_string()]);
    }
    if let Some(limit) = options.budget.max_cost_usd {
        command.args(["--max-budget-usd", &limit.to_string()]);
    }
    command
}

fn permission_mode(permissions: Permissions) -> &'static str {
    match permissions {
        Permissions::Ask => "default",
        Permissions::AcceptEdits => "acceptEdits",
        Permissions::Full => "bypassPermissions",
    }
}
