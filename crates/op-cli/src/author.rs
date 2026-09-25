use std::path::Path;

use anyhow::Result;
use op_client::Identity;

// Tools that drive this CLI from a shell of their own. A shell is an ancestor of every command, so
// only a known list tells a tool apart from the terminal that a person types in.
const AGENTS: &[(&str, &str)] = &[
    ("claude", "claude-code"),
    ("codex", "codex"),
    ("opencode", "opencode"),
    ("aider", "aider"),
    ("cursor-agent", "cursor-agent"),
    ("gemini", "gemini"),
    ("amp", "amp"),
];

// Who a write is for: git `user.name` and `user.email`, and the tool that ran this command. There
// is no flag and no environment override: the CLI signs with the name the repository already knows
// the writer by.
pub fn identity(root: &Path) -> Identity {
    let actor = op_backend_git::identity(root);
    Identity {
        name: actor.as_ref().map(|actor| actor.name.clone()),
        email: actor.and_then(|actor| actor.email),
        agent: agent(),
    }
}

// A comment is signed in the log itself, and an entry no one signed is worse than no entry.
pub fn author(root: &Path) -> Result<String> {
    match op_backend_git::identity(root) {
        Some(actor) => Ok(actor.name),
        None => anyhow::bail!(
            "a comment is signed with git `user.name`, and none is set here. Set it with \
             `git config --global user.name \"Your Name\"`."
        ),
    }
}

// The tool that typed the entry, as a claim rather than a proof: every signal below is spoofable,
// and the daemon writes what the CLI sends. The token carries no version — a log that records
// which build of a tool wrote each line ages into noise.
pub fn agent() -> Option<String> {
    if env_set("CLAUDECODE") || env_set("CLAUDE_CODE_ENTRYPOINT") {
        return Some("claude-code".to_owned());
    }
    if std::env::vars_os().any(|(key, _)| {
        key.to_str()
            .is_some_and(|key| key.starts_with("CODEX_SANDBOX"))
    }) {
        return Some("codex".to_owned());
    }
    if let Some(named) = std::env::var("AI_AGENT")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Some(named.trim().to_owned());
    }
    ancestor_agent(&process_table()?, std::process::id())
}

fn env_set(key: &str) -> bool {
    std::env::var_os(key).is_some_and(|value| !value.is_empty())
}

fn ancestor_agent(parents: &[(u32, u32, String)], mut pid: u32) -> Option<String> {
    let mut seen = 0;
    while seen < parents.len() {
        let (_, parent, name) = parents.iter().find(|(own, _, _)| *own == pid)?;
        if let Some((_, token)) = AGENTS.iter().find(|(binary, _)| binary == name) {
            return Some((*token).to_owned());
        }
        if *parent <= 1 {
            return None;
        }
        pid = *parent;
        seen += 1;
    }
    None
}

// One `ps` call for the whole table rather than one per ancestor: the walk is a handful of steps,
// and the process this runs in is short-lived enough that a snapshot cannot go stale under it.
// Reading the table needs no unsafe code, which no crate in this workspace is allowed to hold.
fn process_table() -> Option<Vec<(u32, u32, String)>> {
    let output = std::process::Command::new("ps")
        .args(["-Ao", "pid=,ppid=,comm="])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.lines().filter_map(process_row).collect())
}

fn process_row(line: &str) -> Option<(u32, u32, String)> {
    let mut fields = line.split_whitespace();
    let pid = fields.next()?.parse().ok()?;
    let parent = fields.next()?.parse().ok()?;
    let command = fields.next()?;
    let name = command.rsplit('/').next()?;
    Some((pid, parent, name.to_owned()))
}
