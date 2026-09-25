mod common;

use std::path::Path;

use common::{Project, Remote, json, ok, stderr, stdout};

fn titles(remote: &Remote, root: &Path) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = json(remote.run(root, &["list", "--json"]))
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["id"].as_str().unwrap().to_owned(),
                row["title"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    rows.sort();
    rows
}

#[test]
fn sync_brings_the_tasks_of_one_clone_to_the_other() {
    let remote = Remote::new();
    let ann = remote.founder("ann", "Ann");
    let from_ann = remote.create(&ann, "From Ann");
    let sent = ok(remote.run(&ann, &["sync"]));
    assert!(
        sent.starts_with("received ") && sent.contains(", sent "),
        "{sent}"
    );

    // A fresh clone needs no setup step: its first command registers it and reads the tasks the
    // remote holds.
    let ben = remote.clone("ben", "Ben");
    let first = remote.run(&ben, &["sync"]);
    assert!(first.status.success(), "stderr: {}", stderr(&first));
    assert!(
        stderr(&first).contains("registered project ben"),
        "stderr: {}",
        stderr(&first)
    );
    assert!(
        ok(remote.run(&ben, &["show", &from_ann])).contains("title:  From Ann"),
        "the clone reads what the other one sent"
    );

    let from_ben = remote.create(&ben, "From Ben");
    assert_eq!(
        from_ben, "OPP-2",
        "the clone numbers after what it received"
    );
    ok(remote.run(&ben, &["sync"]));
    ok(remote.run(&ann, &["sync"]));

    assert_eq!(titles(&remote, &ann), titles(&remote, &ben));
    let history = ok(remote.run(&ann, &["history", &from_ben]));
    assert!(
        history.contains("Ben via") && history.contains("OPP-2: create \"From Ben\""),
        "the revision keeps who wrote it: {history}"
    );
    let listed = ok(remote.run(&ann, &["project", "list"]));
    assert!(
        listed.contains("ann ") && listed.contains("ben "),
        "one daemon serves both clones: {listed}"
    );
}

// Two clones that create a task while neither can reach the remote pick the same number. The sync
// keeps both tasks: the one that reaches the remote second moves to the next free number and says
// so in its comment log.
#[test]
fn sync_keeps_both_tasks_two_clones_created_under_one_number() {
    let remote = Remote::new();
    let ann = remote.founder("ann", "Ann");
    let ben = remote.clone("ben", "Ben");
    ok(remote.run(&ben, &["sync"]));

    remote.offline();
    assert_eq!(remote.create(&ann, "From Ann"), "OPP-1");
    assert_eq!(remote.create(&ben, "From Ben"), "OPP-1");
    let refused = remote.run(&ann, &["sync"]);
    assert!(!refused.status.success(), "{}", stdout(&refused));
    remote.online();

    for root in [&ann, &ben, &ann, &ben] {
        ok(remote.run(root, &["sync"]));
    }

    let seen = titles(&remote, &ann);
    assert_eq!(seen, titles(&remote, &ben), "both clones agree");
    let ids: Vec<&str> = seen.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["OPP-1", "OPP-2"], "{seen:?}");
    let mut names: Vec<&str> = seen.iter().map(|(_, title)| title.as_str()).collect();
    names.sort();
    assert_eq!(names, ["From Ann", "From Ben"], "no task is lost: {seen:?}");

    let history = ok(remote.run(&ann, &["history"]));
    let moved = seen
        .iter()
        .find(|(id, _)| id == "OPP-2")
        .map(|(_, title)| title.as_str())
        .expect("OPP-2");
    assert!(
        history.contains(&format!(
            "    OPP-1 \"{moved}\" is now OPP-2: another task took OPP-1 first."
        )),
        "the merge revision tells of the move: {history}"
    );
    assert!(
        ok(remote.run(&ann, &["comments", "OPP-2"]))
            .trim()
            .is_empty()
    );
}

#[test]
fn sync_status_reports_the_last_sync_and_why_it_failed() {
    let remote = Remote::new();
    let ann = remote.founder("ann", "Ann");

    let healthy = ok(remote.run(&ann, &["sync", "--status"]));
    assert!(healthy.contains("remote:       origin"), "{healthy}");
    assert!(!healthy.contains("last success: never"), "{healthy}");
    assert!(healthy.contains("ahead:        0"), "{healthy}");
    assert!(healthy.contains("behind:       0"), "{healthy}");
    let view = json(remote.run(&ann, &["sync", "--status", "--json"]));
    assert_eq!(view["remote"], "origin");
    assert!(view["last_success"].is_string(), "{view}");
    assert!(view.get("error").is_none(), "{view}");

    remote.offline();
    remote.create(&ann, "Written offline");
    let refused = remote.run(&ann, &["sync"]);
    assert!(!refused.status.success(), "{}", stdout(&refused));
    assert!(!stderr(&refused).is_empty());

    let failing = remote.run(&ann, &["sync", "--status"]);
    assert!(
        !failing.status.success(),
        "a failed last sync fails the status: {}",
        stdout(&failing)
    );
    assert!(
        stdout(&failing).contains("ahead:        1"),
        "{}",
        stdout(&failing)
    );
    assert!(
        stdout(&failing).lines().any(|line| line.starts_with('!')),
        "the reason is printed: {}",
        stdout(&failing)
    );
    let view: serde_json::Value =
        serde_json::from_str(&stdout(&remote.run(&ann, &["sync", "--status", "--json"]))).unwrap();
    assert!(view["error"].is_string(), "{view}");
    assert_eq!(view["ahead"], 1);

    remote.online();
    let sent = ok(remote.run(&ann, &["sync", "--json"]));
    let result: serde_json::Value = serde_json::from_str(&sent).unwrap();
    assert!(result["sent"].as_u64().unwrap() >= 1, "{result}");
    assert!(result["status"].get("error").is_none(), "{result}");
    ok(remote.run(&ann, &["sync", "--status"]));
}

#[test]
fn sync_refuses_a_project_with_no_remote() {
    let local = Project::local();
    for args in [&["sync"][..], &["sync", "--status"]] {
        let out = local.run(args);
        assert!(!out.status.success(), "{args:?}: {}", stdout(&out));
        assert!(
            stderr(&out).contains("has no remote to sync with: its tasks are in a local directory"),
            "{args:?}: {}",
            stderr(&out)
        );
    }

    let git = Project::git();
    let out = git.run(&["sync"]);
    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(
        stderr(&out).contains("on a git branch of a repository with no remote"),
        "{}",
        stderr(&out)
    );
}
