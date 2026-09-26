use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use op_backend::{Actor, Backend};
use op_backend_git::{GitBackend, Options};
use op_task::comment::NewComment;
use op_task::content::Text;
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
    let merge = history
        .iter()
        .find(|entry| entry.revision.parents.len() > 1)
        .expect("a merge");
    let described = bob.tracker.describe(merge).expect("describe");
    assert_eq!(
        described.lines(Some("OPP".parse().expect("abbr"))),
        ["OPP-3: moved from OPP-2"],
        "the history finds the move in the documents too"
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
    let resolved = |text: &str| {
        bob.tracker.update_task(&bob.actor, 1, |task| {
            task.body = task.body.replace(block, text);
            Ok(())
        })
    };
    let marked = resolved("<<<<<<< me\nPlan C.\n=======\nPlan D.\n>>>>>>> you\n");
    assert!(
        matches!(marked, Err(TrackerError::Invalid(_))),
        "{marked:?}"
    );
    resolved("Plan A, then plan B.\n").expect("resolve");
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

#[test]
fn edits_to_one_doc_on_both_sides_both_survive() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    let mut doc = op_task::doc::Doc::new(
        "Architecture",
        "2026-01-01T00:00:00Z".parse().expect("time"),
    )
    .expect("name");
    doc.set_content("line one\n\nline two");
    alice
        .tracker
        .create_doc(&alice.actor, &doc)
        .expect("create");
    alice.sync();
    bob.sync();
    let edit = |member: &Member, content: &'static str| {
        member
            .tracker
            .update_doc(&member.actor, "architecture", |doc| {
                doc.set_content(content);
                Ok(())
            })
            .expect("update");
    };
    edit(&alice, "line one from alice\n\nline two");
    edit(&bob, "line one\n\nline two from bob");
    alice.sync();
    bob.sync();
    let text = bob
        .tracker
        .plan()
        .expect("plan")
        .raw_doc("architecture")
        .expect("raw");
    assert!(text.contains("line one from alice"), "{text}");
    assert!(text.contains("line two from bob"), "{text}");
}

fn docs(member: &Member, names: &[&str]) {
    for name in names {
        let doc = op_task::doc::Doc::new(name, "2026-01-01T00:00:00Z".parse().expect("time"))
            .expect("name");
        member
            .tracker
            .create_doc(&member.actor, &doc)
            .expect("create");
    }
}

fn nest(member: &Member, name: &str, parent: &str) {
    member
        .tracker
        .update_doc(&member.actor, name, |doc| {
            doc.set_parent(Some(parent)).expect("parent");
            Ok(())
        })
        .expect("nest");
}

#[test]
fn a_parent_both_sides_changed_keeps_the_doc_readable_until_one_is_picked() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    docs(&alice, &["Guides", "Reference", "Storage"]);
    alice.sync();
    bob.sync();
    nest(&alice, "storage", "guides");
    nest(&bob, "storage", "reference");
    alice.sync();
    bob.sync();

    let storage = bob
        .tracker
        .plan()
        .expect("plan")
        .doc("storage")
        .expect("readable");
    assert_eq!(storage.frontmatter.parent.as_deref(), Some("guides"));
    assert_eq!(storage.conflicts.len(), 1);
    assert_eq!(storage.conflicts[0].other, Some("reference".into()));

    nest(&bob, "storage", "reference");
    let storage = bob
        .tracker
        .plan()
        .expect("plan")
        .doc("storage")
        .expect("readable");
    assert_eq!(storage.frontmatter.parent.as_deref(), Some("reference"));
    assert!(storage.conflicts.is_empty());
}

#[test]
fn a_doc_both_sides_created_merges_into_one_readable_doc() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    docs(&alice, &["Storage"]);
    docs(&bob, &["Storage"]);
    alice.sync();
    bob.sync();

    let storage = bob
        .tracker
        .plan()
        .expect("plan")
        .doc("storage")
        .expect("readable");
    assert_eq!(storage.title().as_deref(), Some("Storage"));
    assert!(storage.conflicts.is_empty());
}

#[test]
fn a_block_in_a_doc_resolves_and_an_edit_inside_it_is_refused() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    docs(&alice, &["Storage"]);
    alice.sync();
    bob.sync();
    let edit = |member: &Member, content: &str| {
        member.tracker.update_doc(&member.actor, "storage", |doc| {
            doc.set_content(content);
            Ok(())
        })
    };
    edit(&alice, "Keep the index in memory.").expect("edit");
    edit(&bob, "Keep the index in SQLite.").expect("edit");
    alice.sync();
    bob.sync();
    let storage = bob
        .tracker
        .plan()
        .expect("plan")
        .doc("storage")
        .expect("readable");
    let block = op_task::conflict::blocks(&storage.body)
        .into_iter()
        .map(|block| storage.body[block.range].to_owned())
        .next()
        .expect("a block");

    let inside = storage.content().replace("in memory", "on disk");
    assert!(edit(&bob, &inside).is_err());
    let read = Text {
        title: "Storage".to_owned(),
        description: storage.content(),
    };
    let edited = |description: String| Text {
        title: "Storage".to_owned(),
        description,
    };
    assert!(
        bob.tracker
            .edit_doc_text(&bob.actor, "storage", &read, &edited(inside))
            .is_err()
    );

    let resolved = bob
        .tracker
        .edit_doc_text(
            &bob.actor,
            "storage",
            &read,
            &edited(
                storage
                    .content()
                    .replace(&block, "Keep the index in SQLite.\n"),
            ),
        )
        .expect("resolve");
    assert_eq!(resolved.value.conflict_count(), 0);
    assert_eq!(resolved.value.content(), "Keep the index in SQLite.\n");
}

#[test]
fn a_doc_text_merges_with_a_write_made_since_it_was_read() {
    let team = Team::new();
    let (alice, _) = started(&team);
    docs(&alice, &["Storage"]);
    write_doc(
        &alice,
        "storage",
        "Keep the index in memory.\n\nSync it by hand.",
    );
    let read = Text {
        title: "Storage".to_owned(),
        description: "Keep the index in memory.\n\nSync it by hand.\n".to_owned(),
    };
    write_doc(
        &alice,
        "storage",
        "Keep the index in memory.\n\nSync it on every write.",
    );

    let written = alice
        .tracker
        .edit_doc_text(
            &alice.actor,
            "storage",
            &read,
            &Text {
                title: "Storage".to_owned(),
                description: "Keep the index in SQLite.\n\nSync it by hand.\n".to_owned(),
            },
        )
        .expect("write");

    assert_eq!(
        written.value.content(),
        "Keep the index in SQLite.\n\nSync it on every write.\n"
    );
}

#[test]
fn a_new_title_in_a_doc_text_renames_the_doc() {
    let team = Team::new();
    let (alice, _) = started(&team);
    docs(&alice, &["Storage"]);
    let read = Text {
        title: "Storage".to_owned(),
        description: String::new(),
    };

    let written = alice
        .tracker
        .edit_doc_text(
            &alice.actor,
            "storage",
            &read,
            &Text {
                title: "Storage Layout".to_owned(),
                description: String::new(),
            },
        )
        .expect("write");

    assert_eq!(written.value.name, "storage-layout");
    let plan = alice.tracker.plan().expect("plan");
    assert!(plan.doc_names().contains("storage-layout"));
    assert!(!plan.doc_names().contains("storage"));
}

fn write_doc(member: &Member, name: &str, content: &str) {
    member
        .tracker
        .update_doc(&member.actor, name, |doc| {
            doc.set_content(content);
            Ok(())
        })
        .expect("write");
}

#[test]
fn a_doc_link_to_a_renumbered_task_follows_it_and_a_link_to_the_other_task_stays() {
    let team = Team::new();
    let (alice, bob) = started(&team);
    docs(&alice, &["Roadmap"]);
    write_doc(&alice, "roadmap", "Alice's part.\n\nBob's part.");
    alice.sync();
    bob.sync();
    assert_eq!(alice.create("From Alice"), 2);
    assert_eq!(bob.create("From Bob"), 2);
    write_doc(&alice, "roadmap", "Alice's part: [[OPP-2]].\n\nBob's part.");
    write_doc(
        &bob,
        "roadmap",
        "Alice's part.\n\nBob's part: [[OPP-2#Plan]].",
    );
    docs(&bob, &["Plan"]);
    write_doc(&bob, "plan", "Start with [[OPP-2]].");

    alice.sync();
    bob.sync();

    let plan = bob.tracker.plan().expect("plan");
    assert_eq!(
        plan.task(3).expect("task").title().as_deref(),
        Some("From Bob")
    );
    assert_eq!(
        plan.doc("plan").expect("doc").content(),
        "Start with [[../tasks/00003-from-bob.md]].\n"
    );
    assert_eq!(
        plan.doc("roadmap").expect("doc").content(),
        "Alice's part: [[../tasks/00002-from-alice.md]].\n\nBob's part: [[../tasks/00003-from-bob.md#Plan]].\n"
    );
}
