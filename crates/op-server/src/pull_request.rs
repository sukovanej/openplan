use std::path::Path;
use std::process::Command;

const TITLE: &str = "Rolling task updates";
const BODY: &str = "Task edits collected on the rolling-updates branch.";

// `gh` is optional. Without it a person still gets the URL that opens the request by hand, and
// without a GitHub remote they get nothing but the branch name.
pub fn open(dir: &Path, remote_url: Option<&str>, head: &str, base: &str) -> Option<String> {
    existing(dir, head)
        .or_else(|| create(dir, head, base))
        .or_else(|| {
            Some(format!(
                "{}/compare/{base}...{head}?expand=1",
                web_url(remote_url?)?
            ))
        })
}

pub fn web_url(remote_url: &str) -> Option<String> {
    let path = remote_url
        .strip_prefix("git@github.com:")
        .or_else(|| remote_url.strip_prefix("https://github.com/"))
        .or_else(|| remote_url.strip_prefix("ssh://git@github.com/"))?
        .trim_end_matches('/')
        .trim_end_matches(".git");
    (!path.is_empty()).then(|| format!("https://github.com/{path}"))
}

// `pr view <branch>` falls back to a merged or closed request when no open one exists; only an
// open one may stand in for a new one.
fn existing(dir: &Path, head: &str) -> Option<String> {
    gh(
        dir,
        &[
            "pr", "list", "--head", head, "--state", "open", "--json", "url", "--jq", ".[0].url",
        ],
    )
}

fn create(dir: &Path, head: &str, base: &str) -> Option<String> {
    gh(
        dir,
        &[
            "pr", "create", "--head", head, "--base", base, "--title", TITLE, "--body", BODY,
        ],
    )
}

fn gh(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("gh")
        .current_dir(dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .rev()
        .find(|line| line.starts_with("http"))
        .map(|line| line.trim().to_owned())
}
