mod common;

use std::io::Write as _;
use std::process::{Output, Stdio};

use common::{AGENT, Project, json, ok, stderr, stdout, write};

fn piped(project: &Project, args: &[&str], input: &str) -> Output {
    let mut child = project
        .cmd()
        .arg("--root")
        .arg(project.path())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn revision_ids(project: &Project, args: &[&str]) -> Vec<String> {
    let mut all = vec!["history"];
    all.extend_from_slice(args);
    all.push("--json");
    json(project.run(&all))
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["revision"]["id"].as_str().unwrap().to_owned())
        .collect()
}

// A task has no file in a git checkout, so `get` and `write` are how a person or an agent edits one
// whole: print it, change the text, write it back.
#[test]
fn write_replaces_a_task_with_the_file_get_printed() {
    let project = Project::git();
    let parent = project.create("Parent");
    let kid = project.child("Kid", &parent);
    let printed = ok(project.run(&["get", &kid]));
    let edited = format!(
        "{}\nMore detail.\n",
        printed.replace("status: backlog", "status: todo")
    );
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("kid.md");
    write(&file, &edited);

    let written = ok(project.run(&["write", &kid, "--file", file.to_str().unwrap()]));

    assert_eq!(written.trim(), format!("{kid}: Kid"));
    let shown = ok(project.run(&["show", &kid]));
    assert!(shown.contains("status: todo"), "{shown}");
    assert!(
        shown.contains(&format!("parent: {parent}")),
        "the parent survives the round trip: {shown}"
    );
    assert!(ok(project.run(&["get", &kid])).contains("More detail."));
    let last = ok(project.run(&["history", &kid, "--limit", "1"]));
    assert!(
        last.contains(&format!("{kid}: status → todo, description")),
        "{last}"
    );
}

// The rendering names other tasks the way the stored file does, so writing it back changes nothing.
#[test]
fn write_of_an_unchanged_task_keeps_its_references() {
    let project = Project::git();
    let parent = project.create("Parent");
    let kid = project.child("Kid", &parent);
    let dependent = ok(project.run(&["create", "Dependent", "--dependency", &kid]))
        .trim()
        .to_owned();
    let revisions = revision_ids(&project, &[]);

    for id in [&kid, &dependent] {
        let printed = ok(project.run(&["get", id]));
        ok(piped(&project, &["write", id, "--file", "-"], &printed));
    }

    let kid_again = ok(project.run(&["show", &kid]));
    assert!(
        kid_again.contains(&format!("parent: {parent}")),
        "{kid_again}"
    );
    assert!(
        !kid_again.contains('!'),
        "no field is unreadable: {kid_again}"
    );
    let dependent_again = ok(project.run(&["show", &dependent]));
    assert!(
        dependent_again.contains(&format!("dependencies: {kid}")),
        "{dependent_again}"
    );
    assert_eq!(
        revision_ids(&project, &[]),
        revisions,
        "a write that changes nothing makes no revision"
    );
}

#[test]
fn write_refuses_a_file_that_drops_a_comment() {
    let project = Project::git();
    let id = project.create("Ship it");
    ok(project.run(&["comment", &id, "keep me"]));
    let printed = ok(project.run(&["get", &id]));
    let without = format!(
        "{}\n",
        printed.split("## Comments").next().unwrap().trim_end()
    );

    let out = piped(&project, &["write", &id, "--file", "-"], &without);

    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(
        stderr(&out).contains("the comment log is append-only"),
        "{}",
        stderr(&out)
    );
    assert!(ok(project.run(&["comments", &id])).contains("keep me"));
}

#[test]
fn write_refuses_text_that_is_not_a_task_file() {
    let project = Project::git();
    let id = project.create("Ship it");

    let out = piped(
        &project,
        &["write", &id, "--file", "-"],
        "just some prose\n",
    );

    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(stderr(&out).contains("not a task file"), "{}", stderr(&out));
    assert!(ok(project.run(&["show", &id])).contains("title:  Ship it"));

    let missing = piped(
        &project,
        &["write", "OPP-9", "--file", "-"],
        &ok(project.run(&["get", &id])),
    );
    assert!(!missing.status.success());
    assert!(
        stderr(&missing).contains("no such task: OPP-9"),
        "{}",
        stderr(&missing)
    );
}

#[test]
fn get_revision_prints_the_task_as_it_stood_then() {
    for project in [Project::git(), Project::local()] {
        let id = project.create("Ship it");
        ok(project.run(&["set", &id, "status", "done"]));
        let revisions = revision_ids(&project, &[&id]);
        assert_eq!(revisions.len(), 2, "{revisions:?}");

        let then = ok(project.run(&["get", &id, "--revision", &revisions[1]]));
        assert!(then.contains("status: backlog"), "{then}");
        assert!(then.contains("# Ship it"), "{then}");
        let now = ok(project.run(&["get", &id, "--revision", &revisions[0]]));
        assert!(now.contains("status: done"), "{now}");

        let view = json(project.run(&["get", &id, "--json", "--revision", &revisions[1]]));
        assert_eq!(view["id"], id.as_str());
        assert_eq!(view["revision"], revisions[1].as_str());
        assert_eq!(view["task"]["metadata"]["status"], "backlog");
        assert!(
            view["task"]["raw"]
                .as_str()
                .unwrap()
                .starts_with("---\nstatus: backlog\n"),
            "{view}"
        );
    }
}

#[test]
fn get_revision_before_the_task_existed_says_so() {
    let project = Project::git();
    let start = revision_ids(&project, &[]).remove(0);
    let id = project.create("Ship it");

    let out = project.run(&["get", &id, "--revision", &start]);
    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(
        stderr(&out).contains(&format!("{id} did not exist at revision {start}")),
        "{}",
        stderr(&out)
    );

    let view = json(project.run(&["get", &id, "--json", "--revision", &start]));
    assert!(view.get("task").is_none(), "{view}");

    let unknown = project.run(&["get", &id, "--revision", &"0".repeat(40)]);
    assert!(!unknown.status.success());
    assert!(
        stderr(&unknown).contains("no such revision"),
        "{}",
        stderr(&unknown)
    );
}

#[test]
fn history_prints_the_revisions_newest_first() {
    let project = Project::git();
    let id = project.create("Ship it");
    ok(project.run(&["set", &id, "status", "done"]));
    ok(project.run(&["comment", &id, "hello"]));

    let printed = ok(project.run(&["history"]));

    let lines: Vec<&str> = printed.lines().collect();
    let signed = format!("Test via {AGENT}");
    let entries = [
        (signed.as_str(), "OPP-1: comment"),
        (signed.as_str(), "OPP-1: status → done"),
        (signed.as_str(), "OPP-1: create \"Ship it\""),
        (signed.as_str(), "Start the OPP tasks"),
    ];
    assert_eq!(lines.len(), entries.len(), "{printed}");
    for (line, (author, message)) in lines.iter().zip(entries) {
        assert!(
            line.ends_with(&format!("  {author}  {message}")),
            "each line names the author, the agent, and the message: {line}"
        );
        let revision = line.split("  ").next().unwrap();
        assert!(
            revision.len() >= 7 && revision.bytes().all(|b| b.is_ascii_hexdigit()),
            "{line}"
        );
    }
}

// Every revision id `history` prints is one the other commands take back.
#[test]
fn a_revision_history_prints_names_a_task_as_it_stood() {
    let project = Project::git();
    let id = project.create("Ship it");
    ok(project.run(&["set", &id, "status", "done"]));
    let printed = ok(project.run(&["history", &id]));
    let oldest = printed
        .lines()
        .last()
        .unwrap()
        .split("  ")
        .next()
        .unwrap()
        .to_owned();

    let then = ok(project.run(&["get", &id, "--revision", &oldest]));
    assert!(then.contains("status: backlog"), "{then}");

    let older = ok(project.run(&["history", "--before", &oldest]));
    assert!(older.contains("Start the OPP tasks"), "{older}");
}

#[test]
fn history_of_one_task_leaves_the_others_out() {
    let project = Project::git();
    let alpha = project.create("Alpha");
    let beta = project.create("Beta");
    ok(project.run(&["set", &alpha, "status", "todo"]));

    let printed = ok(project.run(&["history", &alpha]));

    assert_eq!(printed.lines().count(), 2, "{printed}");
    assert!(!printed.contains(&beta), "{printed}");
    assert!(!printed.contains("Start the OPP tasks"), "{printed}");

    let none = ok(project.run(&["history", "OPP-7"]));
    assert_eq!(none.trim(), "no history yet");
}

#[test]
fn history_pages_with_before() {
    for project in [Project::git(), Project::local()] {
        for title in ["One", "Two", "Three", "Four"] {
            project.create(title);
        }
        let all = revision_ids(&project, &[]);
        assert_eq!(all.len(), 5, "{all:?}");

        let first = ok(project.run(&["history", "--limit", "2"]));
        let hint = format!("(older: --before {})", all[1]);
        assert_eq!(first.lines().count(), 3, "{first}");
        assert_eq!(first.lines().last(), Some(hint.as_str()), "{first}");

        let second = revision_ids(&project, &["--limit", "2", "--before", &all[1]]);
        assert_eq!(second, all[2..4].to_vec());

        let last = ok(project.run(&["history", "--limit", "2", "--before", &all[3]]));
        assert_eq!(
            last.lines().count(),
            1,
            "the last page holds no hint: {last}"
        );
        assert!(last.contains("Start the OPP tasks"), "{last}");
    }
}

#[test]
fn history_json_carries_the_revision_and_its_changes() {
    let project = Project::git();
    let id = project.create("Ship it");

    let entries = json(project.run(&["history", &id, "--json"]));

    let entry = &entries[0];
    assert_eq!(entry["revision"]["author"], "Test");
    assert_eq!(entry["revision"]["email"], "t@example.com");
    assert_eq!(entry["revision"]["agent"], AGENT);
    assert_eq!(entry["revision"]["message"], "OPP-1: create \"Ship it\"");
    assert_eq!(entry["revision"]["parents"].as_array().unwrap().len(), 1);
    assert_eq!(
        entry["changes"],
        serde_json::json!([{"path": "tasks/00001-ship-it.md", "kind": "added", "task": "OPP-1"}])
    );
}
