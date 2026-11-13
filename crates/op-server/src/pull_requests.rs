use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use op_api::{
    ChangeEvent, Forge, ForgeKind, PullRequest, PullRequestStatus, Rfc3339, TaskPatch, Timestamp,
};
use op_backend::Actor;
use op_forge::{PullRequestRead, parse_states, states_query};
use serde::{Deserialize, Serialize};

use crate::{Project, Publisher};

pub const CURSOR_FILE: &str = "pull-requests.json";
const DAEMON_AGENT: &str = "openplan-daemon";
const POLL_INTERVAL: Duration = Duration::from_secs(120);
const CLI_TIMEOUT: Duration = Duration::from_secs(30);
const CLI_POLL: Duration = Duration::from_millis(25);
// The GitHub search index shows a change some time after it happens, so each window overlaps the
// last one by this much.
const SEARCH_LAG: jiff::SignedDuration = jiff::SignedDuration::from_secs(300);
const MAX_PAGES: usize = 10;
// A daemon that the desktop app starts gets the short PATH of launchd, which holds no Homebrew.
const FALLBACK_DIRS: [&str; 2] = ["/opt/homebrew/bin", "/usr/local/bin"];

#[derive(Debug, Clone)]
pub struct PullRequestPolling {
    pub gh: OsString,
    pub glab: OsString,
    pub interval: Duration,
    // Absent for a state with no daemon home: the cursors then last as long as the process.
    pub cursor_file: Option<PathBuf>,
}

impl Default for PullRequestPolling {
    fn default() -> Self {
        Self {
            gh: program("gh"),
            glab: program("glab"),
            interval: POLL_INTERVAL,
            cursor_file: None,
        }
    }
}

// `OPENPLAN_GH` and `OPENPLAN_GLAB` name another program, so a test daemon never reaches a forge.
fn program(name: &str) -> OsString {
    if let Some(chosen) = std::env::var_os(format!("OPENPLAN_{}", name.to_ascii_uppercase()))
        .filter(|chosen| !chosen.is_empty())
    {
        return chosen;
    }
    let on_path = std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()));
    if on_path {
        return name.into();
    }
    FALLBACK_DIRS
        .iter()
        .map(|dir| Path::new(dir).join(name))
        .find(|path| path.is_file())
        .map_or_else(|| name.into(), PathBuf::into_os_string)
}

pub(crate) struct Poller {
    config: PullRequestPolling,
    cursors: Mutex<BTreeMap<String, Cursor>>,
}

// Where the last scan of a repository stopped. `since` only moves forward: a pull request created
// before it is old, and the scan never links it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cursor {
    updated: Timestamp,
    since: Timestamp,
    // The pull requests created after `since` that a scan already judged, so a removed link stays
    // removed.
    considered: BTreeMap<u64, Timestamp>,
}

impl Cursor {
    fn new(now: Timestamp) -> Self {
        Self {
            updated: now,
            since: now,
            considered: BTreeMap::new(),
        }
    }

    fn window(&self) -> Timestamp {
        self.updated - SEARCH_LAG
    }

    fn is_new(&self, read: &PullRequestRead) -> bool {
        read.created_at >= self.since && !self.considered.contains_key(&read.pull_request.number)
    }

    fn advance(&mut self, reads: &[PullRequestRead]) {
        if let Some(newest) = reads.iter().map(|read| read.updated_at).max() {
            self.updated = self.updated.max(newest);
        }
        self.since = self.since.max(self.window());
        let since = self.since;
        self.considered.retain(|_, created| *created >= since);
    }
}

impl Poller {
    pub(crate) fn new(config: PullRequestPolling) -> Self {
        let cursors = config
            .cursor_file
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            config,
            cursors: Mutex::new(cursors),
        }
    }

    fn program(&self, kind: ForgeKind) -> &OsStr {
        match kind {
            ForgeKind::Github => &self.config.gh,
            ForgeKind::Gitlab => &self.config.glab,
        }
    }

    fn lock_cursors(&self) -> MutexGuard<'_, BTreeMap<String, Cursor>> {
        self.cursors.lock().expect("cursor mutex poisoned")
    }

    fn cursor(&self, forge: &Forge) -> Cursor {
        self.lock_cursors()
            .entry(cursor_key(forge))
            .or_insert_with(|| Cursor::new(op_backend::now()))
            .clone()
    }

    fn keep(&self, forge: &Forge, cursor: Cursor) {
        let mut cursors = self.lock_cursors();
        cursors.insert(cursor_key(forge), cursor);
        let Some(path) = &self.config.cursor_file else {
            return;
        };
        let written = serde_json::to_vec_pretty(&*cursors)
            .map_err(std::io::Error::other)
            .and_then(|bytes| write_atomically(path, &bytes));
        if let Err(err) = written {
            tracing::warn!(path = %path.display(), error = %err, "cannot keep the pull request cursors");
        }
    }

    pub(crate) fn interval(&self) -> Duration {
        self.config.interval
    }
}

fn cursor_key(forge: &Forge) -> String {
    format!("{}/{}", forge.host, forge.repo).to_ascii_lowercase()
}

fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)
}

// Why a forge CLI cannot answer. Only a missing or a logged-out CLI is a fault: a person fixes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliProblem {
    Missing,
    LoggedOut,
}

impl CliProblem {
    pub(crate) fn message(&self, kind: ForgeKind) -> String {
        let (cli, name) = match kind {
            ForgeKind::Github => ("gh", "GitHub CLI"),
            ForgeKind::Gitlab => ("glab", "GitLab CLI"),
        };
        match self {
            CliProblem::Missing => format!(
                "the daemon cannot read the state of pull requests, because `{cli}` is not \
                 installed; install {name} and run `{cli} auth login`"
            ),
            CliProblem::LoggedOut => format!(
                "the daemon cannot read the state of pull requests, because `{cli}` is not logged \
                 in; run `{cli} auth login`"
            ),
        }
    }
}

enum CliError {
    Problem(CliProblem),
    Failed(String),
}

// What the poller remembers about a project between two polls.
#[derive(Debug, Default)]
pub(crate) struct Memory {
    // Pull requests the forge does not show. The daemon asks for each once in its run.
    missing: HashSet<String>,
    pub(crate) problems: HashMap<ForgeKind, CliProblem>,
}

// One poll of one project: what changed in the project repository, then each linked pull request
// that the first query cannot see. Publishes a change for each task whose pull request changed.
pub(crate) fn poll(project: &Project, poller: &Poller, publisher: &Publisher) {
    let mut problems: HashMap<ForgeKind, CliProblem> = HashMap::new();
    let mut changed: HashSet<String> = HashSet::new();
    let mut read_now: HashSet<String> = HashSet::new();
    if let Some(forge) = project.forge().cloned() {
        match changes(poller, &forge) {
            Ok((viewer, reads)) => {
                for read in &reads {
                    read_now.insert(read.pull_request.key());
                    if project
                        .index()
                        .set_pull_request_status(&read.pull_request, status_of(read))
                    {
                        changed.insert(read.pull_request.key());
                    }
                }
                let mut cursor = poller.cursor(&forge);
                link_new(project, &mut cursor, viewer.as_deref(), &reads);
                cursor.advance(&reads);
                poller.keep(&forge, cursor);
            }
            Err(CliError::Problem(problem)) => {
                problems.insert(forge.kind, problem);
            }
            Err(CliError::Failed(error)) => {
                tracing::warn!(project = %project.name(), %error, "cannot read the changed pull requests");
            }
        }
    }
    let wanted = wanted(project, &problems, &read_now);
    for ((kind, host), pull_requests) in by_host(wanted) {
        match states(poller, kind, &host, &pull_requests) {
            Ok(states) => {
                for (pull_request, read) in states {
                    match read {
                        Some(read) => {
                            if project
                                .index()
                                .set_pull_request_status(&pull_request, status_of(&read))
                            {
                                changed.insert(pull_request.key());
                            }
                        }
                        None => {
                            project
                                .lock_pull_requests()
                                .missing
                                .insert(pull_request.key());
                        }
                    }
                }
            }
            Err(CliError::Problem(problem)) => {
                problems.insert(kind, problem);
            }
            Err(CliError::Failed(error)) => {
                tracing::warn!(project = %project.name(), %host, %error, "cannot read the state of pull requests");
            }
        }
    }
    project.lock_pull_requests().problems = problems;
    publisher.report(project);
    let tasks: Vec<String> = {
        let index = project.index();
        let mut numbers: Vec<u64> = index
            .pull_request_links()
            .into_iter()
            .filter(|(_, pull_request)| changed.contains(&pull_request.key()))
            .map(|(number, _)| number)
            .collect();
        numbers.dedup();
        numbers
            .into_iter()
            .map(|number| index.key(number))
            .collect()
    };
    for id in tasks {
        publisher.publish(ChangeEvent::TaskChanged {
            project: project.name(),
            id,
        });
    }
}

// A link the daemon has not read yet, so a poll now has work to do.
pub(crate) fn has_unread(project: &Project) -> bool {
    let memory = project.lock_pull_requests();
    let index = project.index();
    index.pull_request_links().iter().any(|(_, pull_request)| {
        index.pull_request_status(pull_request).is_none()
            && !memory.missing.contains(&pull_request.key())
            && !memory.problems.contains_key(&pull_request.forge.kind)
    })
}

// Each linked pull request with no known state, and each open one in another repository, which
// the change query of the project does not cover.
fn wanted(
    project: &Project,
    problems: &HashMap<ForgeKind, CliProblem>,
    read_now: &HashSet<String>,
) -> Vec<PullRequest> {
    let memory = project.lock_pull_requests();
    let index = project.index();
    let in_project = |pull_request: &PullRequest| {
        project
            .forge()
            .is_some_and(|forge| cursor_key(forge) == cursor_key(&pull_request.forge))
    };
    let mut seen = HashSet::new();
    index
        .pull_request_links()
        .into_iter()
        .map(|(_, pull_request)| pull_request)
        .filter(|pull_request| seen.insert(pull_request.key()))
        .filter(|pull_request| {
            let key = pull_request.key();
            if problems.contains_key(&pull_request.forge.kind)
                || memory.missing.contains(&key)
                || read_now.contains(&key)
            {
                return false;
            }
            match index.pull_request_status(pull_request) {
                None => true,
                Some(status) => !in_project(pull_request) && !status.state.is_settled(),
            }
        })
        .collect()
}

fn by_host(pull_requests: Vec<PullRequest>) -> BTreeMap<(ForgeKind, String), Vec<PullRequest>> {
    let mut grouped: BTreeMap<(ForgeKind, String), Vec<PullRequest>> = BTreeMap::new();
    for pull_request in pull_requests {
        grouped
            .entry((pull_request.forge.kind, pull_request.forge.host.clone()))
            .or_default()
            .push(pull_request);
    }
    grouped
}

fn status_of(read: &PullRequestRead) -> PullRequestStatus {
    PullRequestStatus {
        state: read.state,
        title: read.title.clone(),
        updated_at: Rfc3339(read.updated_at),
    }
}

// A new pull request that the person opened links itself to each task its branch or title names.
fn link_new(
    project: &Project,
    cursor: &mut Cursor,
    viewer: Option<&str>,
    reads: &[PullRequestRead],
) {
    let Some(project_code) = project.index().project_code() else {
        return;
    };
    for read in reads {
        if !cursor.is_new(read) {
            continue;
        }
        cursor
            .considered
            .insert(read.pull_request.number, read.created_at);
        if viewer.is_none() || read.author.as_deref() != viewer {
            continue;
        }
        let mut numbers = project_code.keys_in(&read.head);
        for number in project_code.keys_in(&read.title) {
            if !numbers.contains(&number) {
                numbers.push(number);
            }
        }
        for number in numbers {
            if let Err(err) = link(project, number, &read.pull_request) {
                tracing::warn!(project = %project.name(), task = number, error = %err, "cannot link a new pull request");
            }
        }
    }
}

fn link(project: &Project, number: u64, pull_request: &PullRequest) -> Result<(), String> {
    let linked = {
        let index = project.index();
        if !index.contains(number) {
            return Ok(());
        }
        index
            .pull_request_links()
            .iter()
            .any(|(task, held)| *task == number && held.is(pull_request))
    };
    if linked {
        return Ok(());
    }
    let Some(project_code) = project.index().project_code() else {
        return Ok(());
    };
    let actor = Actor {
        via: Some(DAEMON_AGENT.to_owned()),
        ..project.sign().map_err(|err| err.to_string())?
    };
    let patch = TaskPatch {
        add_pull_requests: vec![pull_request.url()],
        ..TaskPatch::default()
    };
    let updated = project
        .tracker()
        .update_task(&actor, number, |task| {
            patch
                .clone()
                .apply(task, project_code, project.forge())
                .map_err(|err| op_tracker::TrackerError::Invalid(err.to_string()))
        })
        .map_err(|err| err.to_string())?;
    project.written(updated.committed.as_ref());
    tracing::info!(project = %project.name(), task = number, pull_request = %pull_request.url(), "pull request linked");
    Ok(())
}

fn changes(
    poller: &Poller,
    forge: &Forge,
) -> Result<(Option<String>, Vec<PullRequestRead>), CliError> {
    let since = poller.cursor(forge).window();
    let mut viewer = None;
    let mut reads = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let answer = graphql(
            poller.program(forge.kind),
            &forge.host,
            &forge.changed_query(since, after.as_deref()),
        )?;
        let page = forge
            .parse_changed(&answer)
            .map_err(|err| CliError::Failed(err.to_string()))?;
        viewer = viewer.or(page.viewer);
        reads.extend(page.pull_requests);
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    Ok((viewer, op_forge::newest(reads)))
}

fn states(
    poller: &Poller,
    kind: ForgeKind,
    host: &str,
    pull_requests: &[PullRequest],
) -> Result<Vec<(PullRequest, Option<PullRequestRead>)>, CliError> {
    let Some(query) = states_query(pull_requests) else {
        return Ok(Vec::new());
    };
    let answer = graphql(poller.program(kind), host, &query)?;
    parse_states(pull_requests, &answer).map_err(|err| CliError::Failed(err.to_string()))
}

// `gh` and `glab` exit with an error when a query names a pull request that does not exist, and
// still print the data of the rest.
fn graphql(program: &OsStr, host: &str, query: &str) -> Result<String, CliError> {
    let mut command = Command::new(program);
    command
        .args(["api", "graphql", "--hostname", host, "-f"])
        .arg(format!("query={query}"))
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("NO_PROMPT", "1")
        .env("NO_COLOR", "1");
    let output = match run(command, CLI_TIMEOUT) {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(CliError::Problem(CliProblem::Missing));
        }
        Err(err) => return Err(CliError::Failed(err.to_string())),
    };
    if output.success || output.stdout.contains("\"data\"") {
        return Ok(output.stdout);
    }
    let stderr = output.stderr.trim();
    let logged_out = ["auth login", "not logged", "401"]
        .iter()
        .any(|sign| stderr.contains(sign));
    match logged_out {
        true => Err(CliError::Problem(CliProblem::LoggedOut)),
        false => Err(CliError::Failed(stderr.to_owned())),
    }
}

pub(crate) struct Output {
    pub(crate) success: bool,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

// Both pipes drain on threads of their own, so a large answer cannot fill a pipe while this waits.
pub(crate) fn run(mut command: Command, timeout: Duration) -> std::io::Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let drain = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_string(&mut text);
            }
            text
        })
    };
    let stdout = drain(child.stdout.take().map(|pipe| Box::new(pipe) as _));
    let stderr = drain(child.stderr.take().map(|pipe| Box::new(pipe) as _));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("{:?} gave no answer in {timeout:?}", command.get_program()),
            ));
        }
        std::thread::sleep(CLI_POLL);
    };
    Ok(Output {
        success: status.success(),
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

// The thread that polls one project. It holds the project weakly, so it ends with the project.
pub(crate) struct PollLoop {
    shared: Arc<Shared>,
}

struct Shared {
    state: Mutex<LoopState>,
    signal: Condvar,
}

struct LoopState {
    due: Instant,
    stop: bool,
}

impl PollLoop {
    pub(crate) fn start(project: Weak<Project>, poller: Arc<Poller>, publisher: Publisher) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(LoopState {
                due: Instant::now(),
                stop: false,
            }),
            signal: Condvar::new(),
        });
        let waiting = Arc::clone(&shared);
        std::thread::spawn(move || {
            while waiting.wait_until_due(poller.interval()) {
                let Some(project) = project.upgrade() else {
                    return;
                };
                poll(&project, &poller, &publisher);
            }
        });
        Self { shared }
    }

    pub(crate) fn poll_soon(&self) {
        let mut state = self.shared.lock();
        state.due = Instant::now();
        self.shared.signal.notify_all();
    }
}

// The thread may be in the middle of a `gh` call that takes up to its timeout, so a stop does not
// wait for it. The thread ends at its next wake.
impl Drop for PollLoop {
    fn drop(&mut self) {
        self.shared.lock().stop = true;
        self.shared.signal.notify_all();
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, LoopState> {
        self.state.lock().expect("poll loop mutex poisoned")
    }

    fn wait_until_due(&self, interval: Duration) -> bool {
        let mut state = self.lock();
        loop {
            if state.stop {
                return false;
            }
            let now = Instant::now();
            if state.due <= now {
                state.due = now + interval;
                return true;
            }
            let wait = state.due - now;
            state = self
                .signal
                .wait_timeout(state, wait)
                .expect("poll loop mutex poisoned")
                .0;
        }
    }
}
