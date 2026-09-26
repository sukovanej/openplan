use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use op_backend::{
    Actor, Backend, BackendError, BackendEvent, Edit, LogQuery, MergeInput, MergePolicy, Op,
    Origin, PreferTheirs, Resolution,
};
use op_backend_git::{GitBackend, Options, TASKS_NAME, fetch_tasks, tracking_reference};

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Team {
    _root: tempfile::TempDir,
    remote: std::path::PathBuf,
    root: std::path::PathBuf,
}

impl Team {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("tempdir");
        let remote = root.path().join("remote.git");
        git(
            root.path(),
            &["init", "--quiet", "--bare", "-b", "main", "remote.git"],
        );
        let path = root.path().to_path_buf();
        Self {
            _root: root,
            remote,
            root: path,
        }
    }

    fn clone(&self, name: &str) -> std::path::PathBuf {
        git(
            &self.root,
            &[
                "clone",
                "--quiet",
                self.remote.to_str().expect("utf-8"),
                name,
            ],
        );
        self.root.join(name)
    }
}

fn open(path: &Path) -> GitBackend {
    open_with(path, Arc::new(PreferTheirs))
}

fn open_with(path: &Path, policy: Arc<dyn MergePolicy>) -> GitBackend {
    GitBackend::open(path, Options::new(policy)).expect("open")
}

fn put(backend: &dyn Backend, path: &str, text: &str) {
    let (path, text) = (path.to_owned(), text.to_owned());
    backend
        .commit(&Actor::new("Ada"), &mut |_| {
            Ok(Edit::new(
                format!("Write {path}"),
                vec![Op::put(&path, text.as_bytes())],
            ))
        })
        .expect("commit")
        .expect("a revision");
}

fn text(backend: &dyn Backend, path: &str) -> Option<String> {
    backend.head().expect("head").read_text(path).expect("read")
}

fn sync(backend: &GitBackend) -> op_backend::SyncReport {
    backend.remote().expect("a remote").sync().expect("sync")
}

#[test]
fn writes_touch_neither_the_checkout_nor_its_branch() {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "--quiet", "-b", "main"]);
    std::fs::write(dir.path().join("README"), "code").expect("write");
    git(dir.path(), &["add", "README"]);
    git(dir.path(), &["commit", "--quiet", "-m", "code"]);
    let main = git(dir.path(), &["rev-parse", "main"]);

    let backend = open(dir.path());
    put(&backend, "tasks/00001-a.md", "a");

    assert_eq!(git(dir.path(), &["rev-parse", "main"]), main);
    assert_eq!(git(dir.path(), &["status", "--porcelain"]), "");
    assert_eq!(
        git(
            dir.path(),
            &["show", &format!("{TASKS_NAME}:tasks/00001-a.md")]
        ),
        "a"
    );
    assert_eq!(
        git(dir.path(), &["rev-list", "--count", TASKS_NAME]),
        "1",
        "the tasks share no history with the code"
    );
    assert_eq!(
        git(dir.path(), &["branch", "--format=%(refname)"]),
        "refs/heads/main",
        "the tasks are no branch"
    );
}

#[test]
fn two_handles_on_one_repository_lose_no_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "--quiet", "-b", "main"]);
    put(&open(dir.path()), "count", "0");
    std::thread::scope(|scope| {
        for _ in 0..3 {
            let path = dir.path().to_path_buf();
            scope.spawn(move || {
                let backend = open(&path);
                for _ in 0..5 {
                    backend
                        .commit(&Actor::new("Ada"), &mut |head| {
                            let count: u32 = head
                                .read_text("count")?
                                .expect("count")
                                .parse()
                                .expect("number");
                            Ok(Edit::new(
                                "Count",
                                vec![Op::put("count", (count + 1).to_string())],
                            ))
                        })
                        .expect("commit");
                }
            });
        }
    });
    assert_eq!(text(&open(dir.path()), "count").as_deref(), Some("15"));
}

#[test]
fn removing_the_last_file_of_a_directory_leaves_no_empty_tree() {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "--quiet", "-b", "main"]);
    let backend = open(dir.path());
    put(&backend, "tags/bug.md", "bug");
    put(&backend, "config.toml", "x");
    backend
        .commit(&Actor::new("Ada"), &mut |_| {
            Ok(Edit::new("Drop", vec![Op::remove("tags/bug.md")]))
        })
        .expect("commit");
    assert_eq!(
        git(dir.path(), &["ls-tree", "--name-only", TASKS_NAME]),
        "config.toml"
    );
}

#[test]
fn a_repository_without_the_remote_has_nothing_to_sync() {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "--quiet", "-b", "main"]);
    assert!(open(dir.path()).remote().is_none());
}

#[test]
fn sync_publishes_and_a_second_clone_receives() {
    let team = Team::new();
    let alice = open(&team.clone("alice"));
    let bob = open(&team.clone("bob"));
    put(&alice, "tasks/00001-a.md", "from alice");
    let sent = sync(&alice);
    assert_eq!(sent.sent, 1);
    assert_eq!(
        git(&team.remote, &["for-each-ref", "--format=%(refname)"]),
        "refs/openplan/tasks",
        "the remote gets no branch"
    );

    let mut events = bob.subscribe();
    let received = sync(&bob);
    assert_eq!(received.received, 1);
    assert!(!received.merged);
    assert_eq!(
        text(&bob, "tasks/00001-a.md").as_deref(),
        Some("from alice")
    );
    let moved = std::iter::from_fn(|| events.try_recv().ok())
        .find_map(|event| match event {
            BackendEvent::HeadMoved(moved) => Some(moved),
            BackendEvent::Sync(_) => None,
        })
        .expect("a head move");
    assert_eq!(moved.origin, Origin::Remote);
    assert_eq!(moved.from, None);
}

#[test]
fn a_clone_joins_the_tasks_with_one_fetch() {
    let team = Team::new();
    let alice_path = team.clone("alice");
    assert!(!fetch_tasks(&alice_path, "origin").expect("fetch"));
    let alice = open(&alice_path);
    put(&alice, "tasks/00001-a.md", "from alice");
    sync(&alice);

    let path = team.clone("bob");
    assert!(
        op_backend_git::inspect(&path).is_some_and(|checkout| !checkout.has_tasks),
        "a clone fetches no ref outside refs/heads/"
    );
    assert!(fetch_tasks(&path, "origin").expect("fetch"));
    assert!(op_backend_git::inspect(&path).is_some_and(|checkout| checkout.has_tasks));
    let bob = open(&path);

    assert_eq!(
        text(&bob, "tasks/00001-a.md").as_deref(),
        Some("from alice")
    );
    assert_eq!(
        git(&path, &["rev-parse", TASKS_NAME]),
        git(&path, &["rev-parse", &tracking_reference("origin")])
    );
    let report = sync(&bob);
    assert_eq!((report.sent, report.received), (0, 0));
}

#[test]
fn diverged_clones_merge_and_keep_every_revision() {
    let team = Team::new();
    let alice = open(&team.clone("alice"));
    put(&alice, "config.toml", "base");
    sync(&alice);
    let bob = open(&team.clone("bob"));
    sync(&bob);

    put(&alice, "tasks/00001-a.md", "alice");
    put(&bob, "tasks/00002-b.md", "bob");
    sync(&alice);
    let report = sync(&bob);
    assert!(report.merged);
    assert_eq!(text(&bob, "tasks/00001-a.md").as_deref(), Some("alice"));
    assert_eq!(text(&bob, "tasks/00002-b.md").as_deref(), Some("bob"));

    sync(&alice);
    assert_eq!(text(&alice, "tasks/00002-b.md").as_deref(), Some("bob"));
    let alice_log = alice.log(&LogQuery::default()).expect("log");
    let messages: Vec<_> = alice_log
        .iter()
        .map(|entry| entry.revision.message.as_str())
        .collect();
    assert_eq!(messages.len(), 3, "{messages:?}");
    assert!(messages.contains(&"Write tasks/00001-a.md"));
    assert!(messages.contains(&"Write tasks/00002-b.md"));
    assert_eq!(alice.remote().expect("remote").status().ahead, 0);
    assert_eq!(alice.remote().expect("remote").status().error, None);
}

struct Concatenate;

impl MergePolicy for Concatenate {
    fn resolve(&self, input: &MergeInput<'_>) -> Result<Resolution, BackendError> {
        let mut ops = Vec::new();
        for path in input.conflicts {
            let ours = input.ours.read_text(path)?.unwrap_or_default();
            let theirs = input.theirs.read_text(path)?.unwrap_or_default();
            ops.push(Op::put(path.clone(), format!("{theirs}+{ours}")));
        }
        Ok(Resolution {
            ops,
            notes: vec![format!("Joined {}", input.conflicts.join(", "))],
        })
    }
}

#[test]
fn a_conflict_is_answered_by_the_policy() {
    let team = Team::new();
    let alice = open_with(&team.clone("alice"), Arc::new(Concatenate));
    put(&alice, "a.md", "base");
    sync(&alice);
    let bob = open_with(&team.clone("bob"), Arc::new(Concatenate));
    sync(&bob);

    put(&alice, "a.md", "alice");
    put(&bob, "a.md", "bob");
    sync(&alice);
    sync(&bob);
    assert_eq!(text(&bob, "a.md").as_deref(), Some("alice+bob"));
    let newest = bob
        .log(&LogQuery {
            prefix: String::new(),
            before: None,
            limit: Some(1),
        })
        .expect("log");
    let message = &newest[0].revision.message;
    assert!(
        message.starts_with("Merge origin openplan/tasks") && message.contains("Joined a.md"),
        "{message}"
    );
    sync(&alice);
    assert_eq!(text(&alice, "a.md").as_deref(), Some("alice+bob"));
}

#[test]
fn unrelated_histories_merge_from_an_empty_base() {
    let team = Team::new();
    let alice = open_with(&team.clone("alice"), Arc::new(Concatenate));
    let bob = open_with(&team.clone("bob"), Arc::new(Concatenate));
    put(&alice, "config.toml", "alice");
    put(&bob, "config.toml", "bob");
    sync(&alice);
    let report = sync(&bob);
    assert!(report.merged);
    assert_eq!(text(&bob, "config.toml").as_deref(), Some("alice+bob"));
}

#[test]
fn a_pre_push_hook_does_not_run() {
    let team = Team::new();
    let path = team.clone("alice");
    let hook = path.join(".git/hooks/pre-push");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").expect("hook");
    Command::new("chmod")
        .args(["+x", hook.to_str().expect("utf-8")])
        .status()
        .expect("chmod");
    let alice = open(&path);
    put(&alice, "a.md", "a");
    assert_eq!(sync(&alice).sent, 1);
}

#[test]
fn an_unreachable_remote_reports_the_failure_in_the_status() {
    let team = Team::new();
    let path = team.clone("alice");
    let alice = open(&path);
    put(&alice, "a.md", "a");
    std::fs::remove_dir_all(&team.remote).expect("remove remote");
    let mut events = alice.subscribe();
    assert!(alice.remote().expect("remote").sync().is_err());
    let status = alice.remote().expect("remote").status();
    assert!(status.error.is_some());
    assert_eq!(status.ahead, 1);
    assert_eq!(syncing_flags(&mut events), [true, false]);
}

#[test]
fn a_sync_announces_its_start_and_its_end() {
    let team = Team::new();
    let alice = open(&team.clone("alice"));
    put(&alice, "a.md", "a");
    let mut events = alice.subscribe();
    sync(&alice);
    assert_eq!(syncing_flags(&mut events), [true, false]);
    assert!(!alice.remote().expect("remote").status().syncing);
}

fn syncing_flags(events: &mut tokio::sync::broadcast::Receiver<BackendEvent>) -> Vec<bool> {
    std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            BackendEvent::Sync(status) => Some(status.syncing),
            BackendEvent::HeadMoved(_) => None,
        })
        .collect()
}

#[test]
fn refresh_notices_a_branch_moved_by_another_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "--quiet", "-b", "main"]);
    let watching = open(dir.path());
    put(&open(dir.path()), "a.md", "a");
    let moved = watching.refresh().expect("refresh").expect("a move");
    assert_eq!(moved.origin, Origin::External);
    assert_eq!(moved.changes.len(), 1);
    assert_eq!(watching.refresh().expect("refresh"), None);
}

#[test]
fn import_copies_the_history_of_a_directory_and_the_edits_not_yet_committed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    git(root, &["init", "--quiet", "-b", "main"]);
    std::fs::create_dir_all(root.join(".plan/tasks")).expect("mkdir");
    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").expect("write");
    std::fs::write(root.join(".plan/tasks/00001-a.md"), "one").expect("write");
    std::fs::write(root.join("README"), "code").expect("write");
    git(root, &["add", "-A"]);
    git(root, &["commit", "--quiet", "-m", "Add the first task"]);
    std::fs::write(root.join("README"), "more code").expect("write");
    git(root, &["commit", "--quiet", "-am", "Code only"]);
    std::fs::write(root.join(".plan/tasks/00001-a.md"), "two").expect("write");
    git(root, &["commit", "--quiet", "-am", "Edit the first task"]);
    std::fs::write(root.join(".plan/tasks/00002-b.md"), "draft").expect("write");

    let backend = open(root);
    let imported = backend.import(root, ".plan").expect("import");
    assert_eq!(imported.revisions, 2);
    assert!(imported.uncommitted);
    assert_eq!(text(&backend, "tasks/00001-a.md").as_deref(), Some("two"));
    assert_eq!(text(&backend, "tasks/00002-b.md").as_deref(), Some("draft"));
    let messages: Vec<String> = backend
        .log(&LogQuery::under("tasks/00001-"))
        .expect("log")
        .into_iter()
        .map(|entry| entry.revision.message)
        .collect();
    assert_eq!(messages, vec!["Edit the first task", "Add the first task"]);
    let log = backend.log(&LogQuery::default()).expect("log");
    assert_eq!(log[1].revision.author.name, "Test");
    assert!(
        backend.import(root, ".plan").is_err(),
        "a second import has nowhere to go"
    );
}

#[test]
fn inspect_finds_the_tasks_locally_or_fetched() {
    let team = Team::new();
    let alice_path = team.clone("alice");
    assert!(
        !op_backend_git::inspect(&alice_path)
            .expect("a checkout")
            .has_tasks
    );
    let alice = open(&alice_path);
    put(&alice, "a.md", "a");
    assert!(
        op_backend_git::inspect(&alice_path)
            .expect("a checkout")
            .has_tasks
    );
    sync(&alice);
    let bob_path = team.clone("bob");
    fetch_tasks(&bob_path, "origin").expect("fetch");
    let bob = op_backend_git::inspect(&bob_path).expect("a checkout");
    assert!(bob.has_tasks);
    assert_eq!(
        bob.root.as_deref(),
        Some(bob_path.canonicalize().expect("path").as_path())
    );
    assert!(op_backend_git::inspect(team.root.as_path()).is_none());
}
