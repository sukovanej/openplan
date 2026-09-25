use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use op_backend::{Actor, Backend};
use op_backend_git::{GitBackend, Options};
use op_task::comment::NewComment;
use op_task::{Status, Task, Timestamp};
use op_tracker::{HistoryQuery, TaskMergePolicy, Tracker, TrackerError};

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Team {
    root: tempfile::TempDir,
}

struct Member {
    tracker: Tracker,
    backend: Arc<GitBackend>,
    actor: Actor,
}

impl Team {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("tempdir");
        git(
            root.path(),
            &["init", "--quiet", "--bare", "-b", "main", "remote.git"],
        );
        Self { root }
    }

    fn join(&self, name: &str) -> Member {
        let remote = self.root.path().join("remote.git");
        git(
            self.root.path(),
            &["clone", "--quiet", remote.to_str().expect("utf-8"), name],
        );
        let path: PathBuf = self.root.path().join(name);
        let policy = Arc::new(TaskMergePolicy);
        let backend = Arc::new(GitBackend::open(&path, Options::new(policy)).expect("open"));
        Member {
            tracker: Tracker::new(backend.clone()),
            backend,
            actor: Actor::new(name),
        }
    }
}

impl Member {
    fn sync(&self) {
        self.backend
            .remote()
            .expect("a remote")
            .sync()
            .expect("sync");
    }

    fn create(&self, title: &str) -> u64 {
        self.create_at(title, "2026-01-01T00:00:00Z")
    }

    fn create_at(&self, title: &str, at: &str) -> u64 {
        let created: Timestamp = at.parse().expect("time");
        self.tracker
            .create_task(&self.actor, &Task::new(title, Status::Todo, created))
            .expect("create")
            .number
    }

    fn task(&self, number: u64) -> Task {
        self.tracker
            .plan()
            .expect("plan")
            .task(number)
            .expect("task")
    }

    fn titles(&self) -> Vec<(u64, String)> {
        let plan = self.tracker.plan().expect("plan");
        plan.numbers()
            .map(|number| {
                (
                    number,
                    plan.task(number).expect("task").title().unwrap_or_default(),
                )
            })
            .collect()
    }
}

fn started(team: &Team) -> (Member, Member) {
    let alice = team.join("alice");
    alice
        .tracker
        .init(&alice.actor, "OPP".parse().expect("abbr"))
        .expect("init");
    alice.create("Shared");
    alice.sync();
    let bob = team.join("bob");
    bob.sync();
    (alice, bob)
}

#[test]
fn two_new_tasks_under_one_number_both_survive() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    assert_eq!(alice.create("From Alice"), 2);
    assert_eq!(bob.create("From Bob"), 2);
    alice.sync();
    bob.sync();
    alice.sync();

    let expected = vec![
        (1, "Shared".to_owned()),
        (2, "From Alice".to_owned()),
        (3, "From Bob".to_owned()),
    ];
    assert_eq!(alice.titles(), expected);
    assert_eq!(bob.titles(), expected);
    assert!(op_task::comment::parse(&bob.task(3).body).is_empty());
    let history = bob
        .tracker
        .history(&HistoryQuery::default())
        .expect("history");
    assert!(
        history.iter().any(|entry| entry
            .revision
            .message
            .contains("OPP-2 \"From Bob\" is now OPP-3")),
        "the merge revision tells of the move"
    );
}

#[test]
fn the_same_title_under_one_number_both_survive() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    alice.create_at("Fix login", "2026-03-01T10:00:00Z");
    bob.create_at("Fix login", "2026-03-01T10:00:07Z");
    alice.sync();
    bob.sync();
    assert_eq!(bob.titles().len(), 3);
    assert_eq!(
        bob.tracker.plan().expect("plan").path_of(3),
        Some("tasks/00003-fix-login.md")
    );
}

#[test]
fn a_reference_to_a_renumbered_task_follows_it() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    alice.create("From Alice");
    let target = bob.create("Target");
    let child = bob.create("Child");
    bob.tracker
        .update_task(&bob.actor, child, |task| {
            task.set_parent(Some(target.to_string()));
            Ok(())
        })
        .expect("parent");
    alice.sync();
    bob.sync();
    let titles = bob.titles();
    assert_eq!(titles.len(), 4, "{titles:?}");
    let target = titles
        .iter()
        .find(|(_, title)| title == "Target")
        .expect("target")
        .0;
    let child = titles
        .iter()
        .find(|(_, title)| title == "Child")
        .expect("child")
        .0;
    assert_eq!(bob.task(child).frontmatter.parent, Some(target.to_string()));
}

#[test]
fn edits_to_different_fields_of_one_task_combine() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    alice
        .tracker
        .update_task(&alice.actor, 1, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .expect("status");
    bob.tracker
        .update_task(&bob.actor, 1, |task| {
            task.append_body("More detail.");
            Ok(())
        })
        .expect("body");
    alice.sync();
    bob.sync();
    let task = bob.task(1);
    assert_eq!(task.frontmatter.status, Status::Done);
    assert!(task.body.contains("More detail."));
    assert!(op_task::comment::parse(&task.body).is_empty());
}

#[test]
fn a_field_both_changed_keeps_both_versions_until_someone_picks() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    let set = |member: &Member, status: Status| {
        member
            .tracker
            .update_task(&member.actor, 1, |task| {
                task.set_status(status);
                Ok(())
            })
            .expect("status");
    };
    set(&alice, Status::Done);
    set(&bob, Status::Cancelled);
    alice.sync();
    bob.sync();
    alice.sync();
    for member in [&alice, &bob] {
        let task = member.task(1);
        assert_eq!(task.frontmatter.status, Status::Done);
        assert_eq!(task.conflicts.len(), 1);
        let conflict = &task.conflicts[0];
        assert_eq!(conflict.field, "status");
        assert_eq!(conflict.other_fields().status, Ok(Status::Cancelled));
        assert!(conflict.other_label.starts_with("bob ("), "{conflict:?}");
        assert!(conflict.label.starts_with("alice ("), "{conflict:?}");
        assert!(op_task::comment::parse(&task.body).is_empty());
    }

    set(&bob, Status::Cancelled);
    bob.sync();
    alice.sync();
    for member in [&alice, &bob] {
        let task = member.task(1);
        assert_eq!(task.frontmatter.status, Status::Cancelled);
        assert!(task.conflicts.is_empty());
    }
}

#[test]
fn lines_both_changed_keep_both_versions_and_a_write_cannot_edit_inside_them() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    let append = |member: &Member, text: &str| {
        member
            .tracker
            .update_task(&member.actor, 1, |task| {
                task.append_body(text);
                Ok(())
            })
            .expect("body");
    };
    append(&alice, "Plan A.");
    append(&bob, "Plan B.");
    alice.sync();
    bob.sync();
    let task = bob.task(1);
    assert_eq!(task.conflict_count(), 1, "{}", task.body);
    assert!(task.body.contains("Plan A.") && task.body.contains("Plan B."));
    assert_eq!(task.title().as_deref(), Some("Shared"));

    let edited = bob.tracker.update_task(&bob.actor, 1, |task| {
        task.body = task.body.replace("Plan B.", "Plan C.");
        Ok(())
    });
    assert!(
        matches!(edited, Err(TrackerError::Invalid(_))),
        "{edited:?}"
    );

    let block = op_task::conflict::in_body(&task.body).remove(0);
    let block = &task.body[block.range];
    let marked = bob.tracker.resolve_block(
        &bob.actor,
        1,
        block,
        "<<<<<<< me\nPlan C.\n=======\nPlan D.\n>>>>>>> you\n",
    );
    assert!(
        matches!(marked, Err(TrackerError::Invalid(_))),
        "{marked:?}"
    );
    bob.tracker
        .resolve_block(&bob.actor, 1, block, "\nPlan A, then plan B.")
        .expect("resolve");
    let stale = bob.tracker.resolve_block(&bob.actor, 1, block, "Plan B.");
    assert!(
        matches!(stale, Err(TrackerError::ConflictGone)),
        "{stale:?}"
    );
    bob.sync();
    alice.sync();
    let task = alice.task(1);
    assert_eq!(task.conflict_count(), 0);
    assert!(task.body.contains("Plan A, then plan B."), "{}", task.body);
}

#[test]
fn comments_written_at_once_are_all_kept() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    let comment = |member: &Member, text: &str, at: &str| {
        member
            .tracker
            .add_comment(
                &member.actor,
                1,
                &NewComment {
                    at: at.parse().expect("time"),
                    author: member.actor.name.clone(),
                    agent: None,
                    text: text.to_owned(),
                },
            )
            .expect("comment");
    };
    comment(&alice, "from alice", "2026-02-02T00:00:00Z");
    comment(&bob, "from bob", "2026-02-01T00:00:00Z");
    alice.sync();
    bob.sync();
    let texts: Vec<String> = op_task::comment::parse(&bob.task(1).body)
        .into_iter()
        .map(|comment| comment.text)
        .collect();
    assert_eq!(texts, vec!["from bob", "from alice"]);
}

#[test]
fn an_edit_wins_over_a_delete() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    alice.tracker.delete_task(&alice.actor, 1).expect("delete");
    bob.tracker
        .update_task(&bob.actor, 1, |task| {
            task.set_status(Status::Done);
            Ok(())
        })
        .expect("edit");
    alice.sync();
    bob.sync();
    assert_eq!(bob.task(1).frontmatter.status, Status::Done);
}
