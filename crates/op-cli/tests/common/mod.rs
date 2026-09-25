#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use tempfile::TempDir;

pub const AGENT: &str = "test-agent";

// The binary with every input that could reach outside the test pinned down. Each caller passes its
// own OPENPLAN_HOME, so a command that starts a daemon starts the test's own one, and port 0 keeps
// that daemon off the developer's 7373 and off every other test's port.
pub fn openplan(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_openplan"));
    cmd.env("OPENPLAN_HOME", home).env("OPENPLAN_PORT", "0");
    isolate_git(&mut cmd);
    // The agent is detected from the environment and from the processes above this one, so a test
    // run from inside a coding agent would sign its writes with that agent. A named one is the
    // same everywhere.
    for (key, _) in std::env::vars_os() {
        if key
            .to_str()
            .is_some_and(|key| key.starts_with("CODEX_SANDBOX"))
        {
            cmd.env_remove(key);
        }
    }
    cmd.env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .env("AI_AGENT", AGENT);
    cmd
}

// The repository under test is the only git config a test may read. Without this a developer's own
// `~/.gitconfig` reaches the command, and a test about an unset field passes here and fails on a
// machine that sets it.
fn isolate_git(cmd: &mut Command) {
    cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
}

pub fn git(dir: &Path, args: &[&str]) {
    let output = git_output(dir, args);
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        stderr(&output)
    );
}

pub fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let output = git_output(dir, args);
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        stderr(&output)
    );
    stdout(&output)
}

pub fn git_output(dir: &Path, args: &[&str]) -> Output {
    let mut cmd = Command::new("git");
    isolate_git(&mut cmd);
    cmd.current_dir(dir)
        .args(args)
        .output()
        .expect("git must be installed for this test")
}

pub fn git_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@example.com"]);
    git(dir, &["config", "user.name", "Test"]);
}

pub fn commit_at(dir: &Path, seconds: i64, message: &str) {
    let date = format!("@{seconds} +0000");
    git(dir, &["add", "-A"]);
    let mut cmd = Command::new("git");
    isolate_git(&mut cmd);
    let status = cmd
        .current_dir(dir)
        .args(["commit", "-qm", message])
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git commit failed");
}

pub fn write(path: &Path, contents: &str) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub fn combined(output: &Output) -> String {
    format!("{}{}", stdout(output), stderr(output))
}

// The stdout of a command that must succeed.
pub fn ok(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        stdout(&output),
        stderr(&output)
    );
    stdout(&output)
}

pub fn wait_until(limit: Duration, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < limit, "condition not met in time");
        std::thread::sleep(Duration::from_millis(20));
    }
}

// One OPENPLAN_HOME, so one daemon, which every command the test runs through it shares and which
// stops when the test ends.
pub struct Home {
    dir: TempDir,
}

impl Home {
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn cmd(&self) -> Command {
        openplan(self.path())
    }

    pub fn run(&self, root: &Path, args: &[&str]) -> Output {
        self.cmd()
            .arg("--root")
            .arg(root)
            .args(args)
            .output()
            .unwrap()
    }

    pub fn port(&self) -> Option<u16> {
        self.info_field("port").map(|port| port as u16)
    }

    pub fn pid(&self) -> Option<u32> {
        self.info_field("pid").map(|pid| pid as u32)
    }

    fn info_field(&self, key: &str) -> Option<u64> {
        let text = std::fs::read_to_string(self.path().join("daemon.json")).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        value.get(key)?.as_u64()
    }

    pub fn stop(&self) {
        let _ = self.cmd().args(["server", "stop"]).output();
    }

    pub fn registry(&self) -> String {
        std::fs::read_to_string(self.path().join("registry.toml")).unwrap_or_default()
    }
}

impl Default for Home {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        self.stop();
    }
}

// A project the test's daemon serves, started with `openplan init`. The home is declared first, so
// the daemon stops before the checkout it serves goes away.
pub struct Project {
    pub home: Home,
    root: TempDir,
}

impl Project {
    // Tasks as files in `.plan/` of a git checkout, so a test reads what a write left on disk and
    // the git identity signs every write.
    pub fn local() -> Self {
        Self::started(&["--backend", "local"])
    }

    // Tasks on the `openplan/tasks` branch of a git checkout that has no remote.
    pub fn git() -> Self {
        Self::started(&[])
    }

    fn started(backend: &[&str]) -> Self {
        let project = Self {
            home: Home::new(),
            root: tempfile::tempdir().unwrap(),
        };
        git_repo(project.path());
        let mut args = vec!["init", "--abbreviation", "OPP"];
        args.extend_from_slice(backend);
        ok(project.run(&args));
        project
    }

    pub fn path(&self) -> &Path {
        self.root.path()
    }

    pub fn cmd(&self) -> Command {
        self.home.cmd()
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.home.run(self.path(), args)
    }

    pub fn create(&self, title: &str) -> String {
        ok(self.run(&["create", title])).trim().to_owned()
    }

    pub fn child(&self, title: &str, parent: &str) -> String {
        ok(self.run(&["create", title, "--parent", parent]))
            .trim()
            .to_owned()
    }

    pub fn store(&self) -> PathBuf {
        self.path().join(".plan")
    }

    // A hand edit of a local project. The daemon records it when it opens the project again, before
    // it answers anything, so no command can race the file watcher to it.
    pub fn edit(&self, relative: &str, contents: &str) {
        self.home.stop();
        write(&self.store().join(relative), contents);
    }

    pub fn task_file(&self, key: &str) -> PathBuf {
        task_file(self.path(), key)
    }
}

// A task file's name carries a title slug its key does not, so a test finds it by the number behind
// the key.
pub fn task_file(root: &Path, key: &str) -> PathBuf {
    let dir = root.join(".plan/tasks");
    let prefix = format!("{:0>5}-", number(key));
    std::fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("no task directory {}: {err}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix))
        })
        .unwrap_or_else(|| panic!("no file for task {key} in {}", dir.display()))
}

pub fn task_count(root: &Path) -> usize {
    std::fs::read_dir(root.join(".plan/tasks")).map_or(0, |entries| entries.count())
}

pub fn number(key: &str) -> u64 {
    key.split_once('-')
        .and_then(|(_, number)| number.parse().ok())
        .unwrap_or_else(|| panic!("not a key: {key}"))
}

pub fn task_body(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap()
        .splitn(3, "---\n")
        .nth(2)
        .unwrap()
        .to_owned()
}

pub fn json(output: Output) -> serde_json::Value {
    serde_json::from_str(&ok(output)).unwrap()
}

// A bare repository on disk as `origin`, and the clones of it that one daemon serves. Nothing here
// reaches a network or a repository outside the test's own directory.
pub struct Remote {
    pub home: Home,
    dir: TempDir,
}

impl Remote {
    pub fn new() -> Self {
        let remote = Self {
            home: Home::new(),
            dir: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(remote.origin()).unwrap();
        git(&remote.origin(), &["init", "-q", "--bare"]);
        remote
    }

    pub fn origin(&self) -> PathBuf {
        self.dir.path().join("origin.git")
    }

    fn away(&self) -> PathBuf {
        self.dir.path().join("origin.away")
    }

    // The first clone: it pushes the code branch, then starts the tasks and pushes them too.
    pub fn founder(&self, name: &str, person: &str) -> PathBuf {
        let root = self.dir.path().join(name);
        git_repo(&root);
        git(&root, &["config", "user.name", person]);
        write(&root.join("README.md"), "# Code\n");
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-qm", "Start the code"]);
        git(
            &root,
            &["remote", "add", "origin", self.origin().to_str().unwrap()],
        );
        git(&root, &["push", "-q", "origin", "main"]);
        ok(self.run(&root, &["init", "--abbreviation", "OPP"]));
        ok(self.run(&root, &["sync"]));
        root
    }

    pub fn clone(&self, name: &str, person: &str) -> PathBuf {
        let root = self.dir.path().join(name);
        git(
            self.dir.path(),
            &["clone", "-q", self.origin().to_str().unwrap(), name],
        );
        git(&root, &["config", "user.email", "t@example.com"]);
        git(&root, &["config", "user.name", person]);
        root
    }

    pub fn run(&self, root: &Path, args: &[&str]) -> Output {
        self.home.run(root, args)
    }

    pub fn create(&self, root: &Path, title: &str) -> String {
        ok(self.run(root, &["create", title])).trim().to_owned()
    }

    // The remote goes missing, as it does for a laptop on a train.
    pub fn offline(&self) {
        std::fs::rename(self.origin(), self.away()).unwrap();
    }

    pub fn online(&self) {
        std::fs::rename(self.away(), self.origin()).unwrap();
    }
}

impl Default for Remote {
    fn default() -> Self {
        Self::new()
    }
}
