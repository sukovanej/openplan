mod common;

use std::io::Write as _;
use std::path::Path;
use std::process::{Output, Stdio};

use common::{
    Home, Project, combined, git, git_repo, git_stdout, json, ok, stderr, stdout, task_body,
    task_count, write,
};
use op_task::tag::Color;
use op_task::{Status, Timestamp};

fn frontmatter_value(contents: &str, key: &str) -> String {
    let prefix = format!("{key}: ");
    contents
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("no {key} in {contents}"))
        .to_owned()
}

// A local project that no daemon serves yet: `.plan/config.toml` in a directory outside any git
// repository. The first command that reaches a daemon registers it.
fn unregistered_store(abbreviation: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(
        &dir.path().join(".plan/config.toml"),
        &format!("abbreviation = \"{abbreviation}\"\n"),
    );
    std::fs::create_dir_all(dir.path().join(".plan/tasks")).unwrap();
    dir
}

#[test]
fn setup_skills_installs_both_agents_by_default() {
    let home = Home::new();
    let root = tempfile::tempdir().unwrap();

    ok(home.run(root.path(), &["setup-skills"]));

    assert!(
        root.path()
            .join(".claude/skills/openplan/SKILL.md")
            .is_file()
    );
    assert!(
        root.path()
            .join(".agents/skills/openplan/SKILL.md")
            .is_file()
    );
}

#[test]
fn setup_skills_can_target_one_agent() {
    let home = Home::new();
    let root = tempfile::tempdir().unwrap();

    ok(home.run(root.path(), &["setup-skills", "--agent=codex"]));

    assert!(root.path().join(".agents/skills").is_dir());
    assert!(!root.path().join(".claude").exists());
}

#[test]
fn list_reports_real_status_and_title() {
    let project = Project::local();
    project.edit(
        "tasks/00001-ship-it.md",
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship it\n",
    );

    let listed = ok(project.run(&["list"]));

    assert!(listed.contains("OPP-1"), "the id is the key: {listed}");
    assert!(
        listed.contains("done"),
        "status must be read from the file: {listed}"
    );
    assert!(
        listed.contains("Ship it"),
        "title must be read from the body: {listed}"
    );
}

#[test]
fn search_finds_a_task_by_its_body() {
    let project = Project::local();
    project.edit(
        "tasks/00001-ship-it.md",
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship it\n\nIt needs a zeppelin.\n",
    );
    project.edit(
        "tasks/00002-paint-it.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Paint it\n",
    );

    let found = ok(project.run(&["search", "ZEPPELIN"]));

    assert!(found.contains("OPP-1"), "the key: {found}");
    assert!(found.contains("done"), "the status: {found}");
    assert!(found.contains("Ship it"), "the title: {found}");
    assert!(!found.contains("OPP-2"), "and nothing else: {found}");
}

#[test]
fn search_reports_no_matches_rather_than_nothing() {
    let project = Project::local();
    project.create("Ship it");

    let found = ok(project.run(&["search", "kubernetes"]));

    assert!(found.contains("no matching tasks"), "{found}");
}

#[test]
fn search_json_carries_the_hit() {
    let project = Project::local();
    let id = project.create("Ship it");
    ok(project.run(&["set", &id, "status", "done"]));

    let hits = json(project.run(&["search", "ship", "--json"]));

    let hits = hits.as_array().unwrap();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0]["task"]["id"], "OPP-1");
    assert_eq!(hits[0]["task"]["metadata"]["status"], "done");
    assert_eq!(hits[0]["matched"], "title");
}

// The documented way to use openplan is to stand somewhere inside the project and type the command,
// so `--root` defaults to `.`, and the tasks are found in an ancestor of it.
#[test]
fn list_finds_the_tasks_from_a_subdirectory() {
    for project in [Project::local(), Project::git()] {
        project.create("Ship it");
        let nested = project.path().join("crates/thing/src");
        std::fs::create_dir_all(&nested).unwrap();

        let out = project
            .cmd()
            .current_dir(&nested)
            .arg("list")
            .output()
            .unwrap();

        assert!(ok(out).contains("Ship it"));
    }
}

#[test]
fn a_write_from_inside_the_project_needs_no_root_flag() {
    let project = Project::local();

    let out = project
        .cmd()
        .current_dir(project.path())
        .args(["create", "Ship login page"])
        .output()
        .unwrap();

    let id = ok(out).trim().to_owned();
    assert_eq!(id, "OPP-1");
    assert!(project.task_file(&id).is_file());
}

#[test]
fn create_names_the_file_after_the_id_and_the_title() {
    let project = Project::local();
    let before = op_task::now();
    let id = project.create("Wire the parser");
    assert_eq!(id, "OPP-1", "the id printed is the key");

    let path = project.task_file(&id);
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        "00001-wire-the-parser.md",
        "the file name pads the id for sorting and carries the title for reading"
    );
    let contents = std::fs::read_to_string(&path).unwrap();
    let created = frontmatter_value(&contents, "created");
    assert_eq!(
        contents,
        format!("---\nstatus: backlog\ncreated: {created}\n---\n# Wire the parser\n")
    );
    assert!(
        created.parse::<Timestamp>().unwrap() >= before,
        "created must come from the clock at creation: {created}"
    );
    assert!(
        !created.contains('.'),
        "a stored timestamp carries whole seconds: {created}"
    );

    // The number is allocated, not derived from the title, so the same title yields the next id.
    assert_eq!(project.create("Wire the parser"), "OPP-2");
}

// A git project keeps its tasks on a branch of its own, so every command answers from there and no
// file lands in the checkout.
#[test]
fn the_task_commands_work_the_same_on_a_git_branch() {
    let project = Project::git();
    let parent = project.create("Parent");
    let out = project.run(&[
        "create",
        "Kid",
        "--parent",
        &parent,
        "--body",
        "It needs a zeppelin.",
    ]);
    let kid = ok(out).trim().to_owned();
    ok(project.run(&["set", &kid, "status", "in_progress"]));
    ok(project.run(&["comment", &kid, "hello"]));
    ok(project.run(&["tag", "create", "backend"]));
    ok(project.run(&["set", &kid, "tags", "backend"]));

    let shown = ok(project.run(&["show", &kid]));
    assert!(shown.contains("status: in_progress"), "{shown}");
    assert!(shown.contains(&format!("parent: {parent}")), "{shown}");
    assert!(shown.contains("tags: backend"), "{shown}");
    let printed = ok(project.run(&["get", &kid]));
    assert!(printed.contains("It needs a zeppelin."), "{printed}");
    assert!(printed.contains("> hello"), "{printed}");
    assert!(ok(project.run(&["search", "zeppelin"])).contains(&kid));
    assert!(ok(project.run(&["tree", &parent])).contains("Kid"));

    ok(project.run(&["delete", &kid, "--yes"]));
    assert!(!ok(project.run(&["list"])).contains("Kid"));

    assert!(
        !project.path().join(".plan").exists(),
        "no task file lands in the checkout"
    );
    assert!(
        git_stdout(project.path(), &["status", "--porcelain"]).is_empty(),
        "the working tree stays clean"
    );
    let log = git_stdout(
        project.path(),
        &["log", "--format=%s", "openplan/tasks", "--"],
    );
    assert!(log.contains("OPP-2: delete \"Kid\""), "{log}");
    assert!(log.contains("OPP-1: create \"Parent\""), "{log}");
}

// Every worktree of a repository reaches the one set of tasks, and the project is rooted at the main
// checkout, so a worktree that goes away takes nothing with it.
#[test]
fn a_linked_worktree_reaches_the_tasks_of_its_repository() {
    let project = Project::git();
    git(
        project.path(),
        &["commit", "-q", "--allow-empty", "-m", "Start the code"],
    );
    let anchor = project.create("Anchor");
    let feature = project.path().join(".worktrees/feature");
    git(
        project.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            feature.to_str().unwrap(),
        ],
    );
    let name = ok(project.run(&["project", "list"]))
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    ok(project.run(&["project", "remove", &name]));

    let from_feature = project.home.run(&feature, &["create", "From the worktree"]);
    assert!(
        stderr(&from_feature).contains("registered project"),
        "the worktree's first command registers the repository: {}",
        stderr(&from_feature)
    );
    let id = ok(from_feature).trim().to_owned();
    assert_ne!(id, anchor, "one counter serves every worktree");
    assert!(ok(project.home.run(&feature, &["list"])).contains("Anchor"));
    let registry = project.home.registry();
    assert!(
        registry.contains(&format!(
            "path = \"{}\"",
            project.path().canonicalize().unwrap().display()
        )),
        "the project is rooted at the main checkout: {registry}"
    );

    git(
        project.path(),
        &["worktree", "remove", "--force", feature.to_str().unwrap()],
    );
    let from_main = project.run(&["create", "From main"]);
    assert!(
        !stderr(&from_main).contains("registered"),
        "the project outlives the worktree that registered it: {}",
        stderr(&from_main)
    );
    ok(from_main);
    assert!(ok(project.run(&["show", &id])).contains("From the worktree"));
}

#[test]
fn a_write_with_no_reachable_daemon_fails_explicitly() {
    let project = Project::local();

    let out = project.run(&["--daemon", "http://127.0.0.1:1", "create", "Ship login"]);

    assert!(!out.status.success(), "an unreachable daemon must not pass");
    assert!(
        stderr(&out).contains("no openplan daemon at http://127.0.0.1:1"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(ok(project.run(&["list"])).contains("no tasks yet"));
}

#[test]
fn a_command_where_no_tasks_live_says_how_to_start_them() {
    let home = Home::new();
    let empty = tempfile::tempdir().unwrap();

    for args in [&["create", "Ship login"][..], &["list"]] {
        let out = home.run(empty.path(), args);
        assert!(!out.status.success(), "{args:?}: {}", stdout(&out));
        assert!(
            stderr(&out).contains("openplan init --abbreviation"),
            "{args:?}: {}",
            stderr(&out)
        );
    }
    assert!(
        !home.path().join("daemon.json").exists(),
        "a command with no project to serve starts no daemon"
    );
}

#[test]
fn a_repository_with_tasks_beside_the_code_asks_for_a_migration() {
    let home = Home::new();
    let repo = tempfile::tempdir().unwrap();
    git_repo(repo.path());
    write(
        &repo.path().join(".plan/config.toml"),
        "abbreviation = \"OPP\"\n",
    );

    let out = home.run(repo.path(), &["create", "Ship login"]);

    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("openplan migrate"),
        "stderr: {}",
        stderr(&out)
    );
    assert_eq!(task_count(repo.path()), 0, "nothing was written");
}

// One daemon serves every project on the machine. A write from a project it does not yet know
// registers that project and lands, with no setup step and no restart.
#[test]
fn writes_from_two_projects_land_in_their_own_stores() {
    let first = Project::local();
    let second = Project::local();
    first.create("Anchor");

    let out = first.home.run(second.path(), &["create", "Ship login"]);

    // Each project has its own id counter, so both first tasks are number one.
    assert_eq!(ok(out).trim(), "OPP-1");
    assert_eq!(
        task_count(second.path()),
        1,
        "the write lands in the project --root names"
    );
    assert_eq!(
        task_count(first.path()),
        1,
        "and nothing leaks into the other one"
    );
}

#[test]
fn only_the_first_write_from_a_project_reports_a_registration() {
    let home = Home::new();
    let store = unregistered_store("OPP");

    let first = home.run(store.path(), &["create", "Anchor"]);
    assert!(first.status.success(), "stderr: {}", stderr(&first));
    assert!(
        stderr(&first).contains("registered project"),
        "stderr: {}",
        stderr(&first)
    );

    let second = home.run(store.path(), &["create", "Ship login"]);
    assert!(second.status.success(), "stderr: {}", stderr(&second));
    assert!(
        !stderr(&second).contains("registered"),
        "a project already served is registered once, and said so once"
    );
    assert_eq!(
        stdout(&first).trim(),
        "OPP-1",
        "the id stays the only thing on stdout"
    );
}

// A daemon older than the project routes answers /health and falls every unknown path through to
// the SPA. A write cannot learn which project it is talking to there, so it stops and says how to
// fix it rather than write into whatever the daemon happens to serve.
#[test]
fn a_daemon_without_project_routes_asks_for_a_restart() {
    let project = Project::local();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut head = [0u8; 1024];
            let read = std::io::Read::read(&mut stream, &mut head).unwrap_or(0);
            let (kind, body) = match head[..read].starts_with(b"GET /health") {
                true => (
                    "application/json",
                    r#"{"pid":1,"port":1,"version":"0.0.1","started_at":0}"#,
                ),
                false => ("text/html", "<!doctype html>"),
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: \
                 close\r\n\r\n{body}",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        }
    });

    let out = project.run(&[
        "--daemon",
        &format!("http://127.0.0.1:{port}"),
        "create",
        "Ship login",
    ]);

    assert!(!out.status.success(), "the write must not be attempted");
    assert!(
        stderr(&out).contains("predates") && stderr(&out).contains("openplan server stop"),
        "stderr: {}",
        stderr(&out)
    );
    assert_eq!(task_count(project.path()), 0, "nothing was written");
}

// `--daemon` borrows a daemon for one command. Registering there would leave it serving a project
// the caller never asked it to serve.
#[test]
fn a_named_daemon_is_not_registered_into_by_a_write() {
    let served = Project::local();
    let other = Project::local();
    served.create("Anchor");
    let port = served.home.port().unwrap();

    let out = other.run(&[
        "--daemon",
        &format!("http://127.0.0.1:{port}"),
        "create",
        "Ship login",
    ]);

    assert!(!out.status.success(), "the write must be refused");
    assert!(
        stderr(&out).contains("openplan project add --daemon"),
        "the refusal names the explicit way in: {}",
        stderr(&out)
    );
    let registry = served.home.registry();
    assert_eq!(
        registry.matches("[[project]]").count(),
        1,
        "the named daemon keeps the projects it had: {registry}"
    );
    assert_eq!(task_count(other.path()), 0, "and nothing was written");
}

// A daemon whose home sits inside a project must not answer for that project. A write from another
// project must land where the caller stands.
#[test]
fn a_home_inside_a_project_never_becomes_the_project_written_to() {
    let project = Project::local();
    let elsewhere = unregistered_store("ELS");
    let home = elsewhere.path().join("openplanhome");
    std::fs::create_dir_all(&home).unwrap();

    let out = common::openplan(&home)
        .current_dir(project.path())
        .args(["create", "Ship login page"])
        .output()
        .unwrap();
    let _ = common::openplan(&home).args(["server", "stop"]).output();

    assert_eq!(
        ok(out).trim(),
        "OPP-1",
        "the key of the project we stand in"
    );
    assert!(project.task_file("OPP-1").is_file());
    assert_eq!(
        task_count(elsewhere.path()),
        0,
        "nothing may be written to the project the daemon's home happens to sit in"
    );
}

#[test]
fn get_show_and_missing_id() {
    let project = Project::local();
    let id = project.create("Ship it");

    let shown = ok(project.run(&["show", &id]));
    assert!(shown.contains("status: backlog"), "{shown}");
    assert!(shown.contains("title:  Ship it"), "{shown}");

    let view = json(project.run(&["get", &id, "--json"]));
    assert_eq!(view["title"], "Ship it");
    assert_eq!(view["metadata"]["status"], "backlog");

    for missing in ["does-not-exist", "OPP-99"] {
        let out = project.run(&["get", missing]);
        assert!(
            !out.status.success(),
            "get on a missing id must exit non-zero"
        );
        assert!(stderr(&out).contains(missing), "stderr: {}", stderr(&out));
    }
}

#[test]
fn set_updates_only_frontmatter() {
    let project = Project::local();
    let id = project.create("Ship it");
    let path = project.task_file(&id);
    let body_before = task_body(&path);

    ok(project.run(&["set", &id, "status", "in_progress"]));

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(
        contents.starts_with("---\nstatus: in_progress\ncreated: "),
        "{contents}"
    );
    assert_eq!(
        task_body(&path),
        body_before,
        "body must be byte-for-byte unchanged"
    );

    let bad_status = project.run(&["set", &id, "status", "bogus"]);
    assert!(
        !bad_status.status.success(),
        "invalid status must be rejected"
    );

    let bad_parent = project.run(&["set", &id, "parent", "OPP-99"]);
    assert!(
        !bad_parent.status.success(),
        "non-existent parent must be rejected"
    );

    let bad_field = project.run(&["set", &id, "colour", "red"]);
    assert!(
        stderr(&bad_field).contains("expected status | parent | dependencies | tags"),
        "stderr: {}",
        stderr(&bad_field)
    );
}

#[test]
fn delete_removes_the_file() {
    let project = Project::local();
    let id = project.create("Temporary");
    let path = project.task_file(&id);

    let deleted = ok(project.run(&["delete", &id, "--yes"]));

    assert!(deleted.contains(&format!("deleted {id}")), "{deleted}");
    assert!(!path.exists(), "delete must remove the file");
    assert!(
        !ok(project.run(&["list"])).contains(&id),
        "deleted id must not appear in list"
    );
}

#[test]
fn delete_asks_before_it_deletes() {
    let project = Project::local();
    let id = project.create("Temporary");

    let mut child = project
        .cmd()
        .arg("--root")
        .arg(project.path())
        .args(["delete", &id])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"n\n").unwrap();
    let out = child.wait_with_output().unwrap();

    let printed = ok(out);
    assert!(
        printed.contains(&format!("delete {id}? [y/N]")),
        "{printed}"
    );
    assert!(printed.contains("aborted"), "{printed}");
    assert!(project.task_file(&id).is_file(), "the task is still there");
}

#[test]
fn delete_of_a_missing_id_fails_before_it_prompts() {
    let project = Project::local();

    // No `--yes`: a typo must be refused outright rather than put to the reader as a question.
    let out = project.run(&["delete", "OPP-99"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("no such task: OPP-99"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        !stdout(&out).contains("delete OPP-99?"),
        "stdout: {}",
        stdout(&out)
    );
}

#[test]
fn list_json_filters_by_status() {
    let project = Project::local();
    let todo = project.create("Still to do");
    let done = project.create("Already done");
    for (id, status) in [(&todo, "todo"), (&done, "done")] {
        ok(project.run(&["set", id, "status", status]));
    }

    let tasks = json(project.run(&["list", "--json", "--status", "todo"]));

    let tasks = tasks.as_array().unwrap();
    assert_eq!(tasks.len(), 1, "only the todo task should match: {tasks:?}");
    assert_eq!(tasks[0]["id"], todo.as_str());
    assert_eq!(tasks[0]["metadata"]["status"], "todo");
}

#[test]
fn list_filters_by_parent() {
    let project = Project::local();
    let parent = project.create("Parent");
    let kid = project.child("Kid", &parent);
    project.create("Loose");

    let listed = ok(project.run(&["list", "--parent", &parent]));

    assert!(listed.contains(&kid), "{listed}");
    assert!(!listed.contains("Loose"), "{listed}");
}

#[test]
fn set_status_survives_a_deleted_dependency() {
    let project = Project::local();
    let a = project.create("Task A");
    let b = ok(project.run(&["create", "Task B", "--dependency", &a]))
        .trim()
        .to_owned();

    ok(project.run(&["delete", &a, "--yes"]));

    // B still lists A as a dependency, but changing B's status is unrelated and must succeed.
    ok(project.run(&["set", &b, "status", "done"]));
    assert!(ok(project.run(&["show", &b])).contains("status: done"));
}

#[test]
fn set_preserves_unknown_frontmatter_keys() {
    let project = Project::local();
    let id = project.create("Task C");
    let path = project.task_file(&id);
    project.edit(
        "tasks/00001-task-c.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nestimate: 3.5\nassignee: milan\n---\n# Task C\n",
    );

    ok(project.run(&["set", &id, "status", "done"]));

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(
        contents.contains("estimate: 3.5"),
        "estimate dropped: {contents}"
    );
    assert!(
        contents.contains("assignee: milan"),
        "assignee dropped: {contents}"
    );
    assert!(contents.contains("status: done"));
}

// The daemon holds the task parsed, not the bytes it parsed, so `get` renders that state back to
// markdown. Key order, spacing, and keys no field names normalize; the task itself does not change.
#[test]
fn get_renders_the_daemons_state_rather_than_the_file() {
    let project = Project::local();
    let id = project.create("Task D");
    project.edit(
        "tasks/00001-task-d.md",
        "---\nassignee: milan\nrank:   '7'\ncreated: 2026-01-01T00:00:00Z\nstatus: todo\n---\n# Task D\n\nsome body\n",
    );

    let printed = ok(project.run(&["get", &id]));

    assert_eq!(
        printed,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nrank: '7'\n---\n# Task D\n\nsome body\n"
    );
}

// The rendering is a task file, and `status` and `created` are what make one. A task missing either
// cannot be rendered at all: emitting the rest would look like a task file and destroy the task if
// it were written over one.
#[test]
fn get_refuses_to_render_a_task_missing_a_required_field() {
    let project = Project::local();
    project.edit(
        "tasks/00001-legacy.md",
        "---\nstatus: todo\n---\n# Legacy\n",
    );

    let out = project.run(&["get", "OPP-1"]);
    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    assert!(
        stdout(&out).is_empty(),
        "nothing that reads as a task file: {}",
        stdout(&out)
    );
    assert!(
        stderr(&out).contains("created: missing"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("cannot be rendered as a task file"),
        "stderr: {}",
        stderr(&out)
    );

    // `--json` answers with the state the daemon holds, which is exactly what a broken file needs
    // read, so it keeps working.
    let view = json(project.run(&["get", "OPP-1", "--json"]));
    assert_eq!(view["metadata"]["created"]["kind"], "missing");
}

// A field that is not required has no canonical form when it fails to parse, so it is left out and
// named on stderr rather than dropped in silence.
#[test]
fn get_reports_a_field_it_cannot_render() {
    let project = Project::local();
    project.edit(
        "tasks/00001-legacy.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nrank: [1, 2]\n---\n# Legacy\n",
    );

    let out = project.run(&["get", "OPP-1"]);

    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Legacy\n"
    );
    assert!(stderr(&out).contains("rank:"), "stderr: {}", stderr(&out));
}

#[test]
fn set_on_a_task_without_created_explains_what_to_add() {
    let project = Project::local();
    project.edit(
        "tasks/00002-legacy.md",
        "---\nstatus: todo\n---\n# Legacy\n",
    );

    let out = project.run(&["set", "OPP-2", "status", "done"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("no `created:` field"),
        "{}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("Add the field to its frontmatter"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn create_with_body_places_content_below_title() {
    let project = Project::local();

    let out = project.run(&[
        "create",
        "Ship login",
        "--body",
        "Support OAuth and email login.",
    ]);

    let id = ok(out).trim().to_owned();
    let contents = std::fs::read_to_string(project.task_file(&id)).unwrap();
    let created = frontmatter_value(&contents, "created");
    assert_eq!(
        contents,
        format!(
            "---\nstatus: backlog\ncreated: {created}\n---\n# Ship login\n\nSupport OAuth and email login.\n"
        )
    );
    let view = json(project.run(&["get", &id, "--json"]));
    assert_eq!(view["title"], "Ship login");
    assert_eq!(view["description"], "Support OAuth and email login.\n");
    assert_eq!(ok(project.run(&["get", &id])), contents);
}

#[test]
fn create_with_body_file_reads_the_file() {
    let project = Project::local();
    let notes = project.path().join("notes.md");
    write(&notes, "## Goals\n- OAuth\n- Email + password\n");

    let out = project.run(&[
        "create",
        "Ship login",
        "--body-file",
        notes.to_str().unwrap(),
    ]);

    let id = ok(out).trim().to_owned();
    assert_eq!(
        task_body(&project.task_file(&id)),
        "# Ship login\n\n## Goals\n- OAuth\n- Email + password\n"
    );
}

#[test]
fn create_with_body_file_dash_reads_stdin() {
    let project = Project::local();

    let out = piped(
        &project,
        &["create", "Ship login", "--body-file", "-"],
        b"## Goals\n- OAuth\n- Email + password\n",
    );

    let id = ok(out).trim().to_owned();
    assert_eq!(
        task_body(&project.task_file(&id)),
        "# Ship login\n\n## Goals\n- OAuth\n- Email + password\n"
    );
}

fn piped(project: &Project, args: &[&str], input: &[u8]) -> Output {
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
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn create_rejects_body_with_body_file() {
    let project = Project::local();

    let out = project.run(&[
        "create",
        "Ship login",
        "--body",
        "x",
        "--body-file",
        "notes.md",
    ]);

    assert!(
        !out.status.success(),
        "--body and --body-file are mutually exclusive"
    );
    assert!(ok(project.run(&["list"])).contains("no tasks yet"));
}

#[test]
fn create_rejects_malformed_title() {
    let project = Project::local();

    assert!(
        !project.run(&["create", ""]).status.success(),
        "an empty title must be rejected"
    );
    assert!(
        !project
            .run(&["create", "line one\n# line two"])
            .status
            .success(),
        "a title producing two H1 headings must be rejected"
    );
    assert!(ok(project.run(&["list"])).contains("no tasks yet"));
}

#[test]
fn list_distinguishes_empty_store_from_empty_filter() {
    let project = Project::local();
    assert!(ok(project.run(&["list"])).contains("no tasks yet"));

    project.create("A todo");

    let filtered = ok(project.run(&["list", "--status", "done"]));
    assert!(filtered.contains("no matching tasks"), "{filtered}");
    assert!(!filtered.contains("no tasks yet"), "{filtered}");
}

#[test]
fn list_json_carries_an_unreadable_task_rather_than_dropping_it() {
    let project = Project::local();
    let good = project.create("Good one");
    project.edit("tasks/00002-broken.md", "this file has no frontmatter\n");
    project.edit(
        "tasks/00003-legacy.md",
        "---\nstatus: in_progress\n---\n# Legacy\n",
    );

    let tasks = json(project.run(&["list", "--json"]));

    let tasks = tasks.as_array().unwrap();
    let by_id = |id: &str| {
        tasks
            .iter()
            .find(|task| task["id"] == id)
            .unwrap_or_else(|| panic!("{id} missing from {tasks:?}"))
            .clone()
    };
    assert_eq!(by_id(&good)["metadata"]["status"], "backlog");
    // A file with no readable frontmatter says so, instead of borrowing a status it never claimed.
    assert_eq!(by_id("OPP-2")["metadata"]["kind"], "error");
    // One unreadable field costs only itself: the status is still the file's own.
    assert_eq!(by_id("OPP-3")["metadata"]["status"], "in_progress");
    assert_eq!(by_id("OPP-3")["metadata"]["created"]["kind"], "missing");
}

#[test]
fn list_filters_by_status_across_an_unreadable_task() {
    let project = Project::local();
    project.edit("tasks/00001-broken.md", "this file has no frontmatter\n");
    let good = project.create("Good one");

    let tasks = json(project.run(&["list", "--json", "--status", "backlog"]));

    // A task with no readable status matches no status filter, rather than matching the default.
    let tasks = tasks.as_array().unwrap();
    assert_eq!(tasks.len(), 1, "{tasks:?}");
    assert_eq!(tasks[0]["id"], good.as_str());
}

fn tree_ids(project: &Project, id: &str) -> Vec<String> {
    let tree = json(project.run(&["tree", id, "--json"]));
    tree["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|child| child["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn set_parent_empty_clears_to_top_level() {
    let project = Project::local();
    let parent = project.create("Parent");
    let kid = project.child("Kid", &parent);

    ok(project.run(&["set", &kid, "parent", ""]));

    let shown = ok(project.run(&["show", &kid]));
    assert!(shown.contains("parent: -"), "{shown}");
    assert!(
        !std::fs::read_to_string(project.task_file(&kid))
            .unwrap()
            .contains("parent"),
        "cleared parent key must drop from the file"
    );
}

#[test]
fn tree_bounds_by_depth_and_reports_json() {
    let project = Project::local();
    let root = project.create("Root");
    let a = project.child("A", &root);
    project.child("A1", &a);

    let tree = json(project.run(&["tree", &root, "--depth", "1", "--json"]));

    assert_eq!(tree["children"][0]["id"], a.as_str());
    assert!(
        tree["children"][0]["children"]
            .as_array()
            .unwrap()
            .is_empty(),
        "depth 1 must not expand grandchildren"
    );

    let printed = ok(project.run(&["tree", &root]));
    assert!(
        printed.contains("  OPP-3"),
        "a grandchild is indented: {printed}"
    );
}

#[test]
fn move_reorders_siblings_via_before_and_after() {
    let project = Project::local();
    let root = project.create("Root");
    let a = project.child("A", &root);
    let b = project.child("B", &root);
    let c = project.child("C", &root);

    ok(project.run(&["move", &c, "--parent", &root, "--before", &a]));
    ok(project.run(&["move", &b, "--parent", &root, "--after", &c]));

    assert_eq!(tree_ids(&project, &root), vec![c, b, a]);
}

#[test]
fn move_reparents_across_parents() {
    let project = Project::local();
    let root = project.create("Root");
    let other = project.create("Other");
    let kid = project.child("Kid", &root);

    ok(project.run(&["move", &kid, "--parent", &other]));

    assert_eq!(tree_ids(&project, &root), Vec::<String>::new());
    assert_eq!(tree_ids(&project, &other), vec![kid]);
}

#[test]
fn move_under_own_descendant_is_refused() {
    let project = Project::local();
    let a = project.create("A");
    let b = project.child("B", &a);
    let c = project.child("C", &b);

    let out = project.run(&["move", &a, "--parent", &c]);

    assert!(!out.status.success(), "cycle must be refused");
    assert!(
        stderr(&out).contains("descendant"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn move_unranked_siblings_then_reorder_lists_in_new_order() {
    let project = Project::local();
    let root = project.create("Root");
    // Created without explicit order — unranked, so they list by id first.
    let a = project.child("A", &root);
    let z = project.child("Z", &root);
    assert_eq!(tree_ids(&project, &root), vec![a.clone(), z.clone()]);

    // Reorder Z before A; the group migrates to ranks and the new order sticks.
    ok(project.run(&["move", &z, "--parent", &root, "--before", &a]));

    assert_eq!(tree_ids(&project, &root), vec![z, a]);
}

fn set_rank(project: &Project, id: &str, rank: &str) {
    let path = project.task_file(id);
    let raw = std::fs::read_to_string(&path).unwrap();
    let patched = raw.replacen("---\n", &format!("---\nrank: {rank}\n"), 1);
    let relative = path.strip_prefix(project.store()).unwrap();
    project.edit(relative.to_str().unwrap(), &patched);
}

fn rank_of(project: &Project, id: &str) -> Option<String> {
    std::fs::read_to_string(project.task_file(id))
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("rank: "))
        .map(|value| value.trim().to_owned())
}

#[test]
fn move_between_neighbours_naming_the_same_point_rebalances() {
    // Hand-edited frontmatter can give two siblings ranks that differ as text but name one point
    // (`a` and `a0`). There is no key between them, so the group has to be rebalanced rather than
    // searched forever for a gap that cannot exist.
    let project = Project::local();
    let root = project.create("Root");
    let a = project.child("A", &root);
    let b = project.child("B", &root);
    let c = project.child("C", &root);
    set_rank(&project, &a, "a");
    set_rank(&project, &b, "a0");

    ok(project.run(&["move", &c, "--parent", &root, "--after", &a]));

    assert_eq!(tree_ids(&project, &root), vec![a, c, b]);
}

#[test]
fn move_within_a_group_holding_a_malformed_rank_rebalances() {
    let project = Project::local();
    let root = project.create("Root");
    let a = project.child("A", &root);
    let b = project.child("B", &root);
    set_rank(&project, &a, "NOT-BASE36");

    ok(project.run(&["move", &b, "--parent", &root, "--before", &a]));

    assert_eq!(tree_ids(&project, &root), vec![b.clone(), a.clone()]);
    for id in [&a, &b] {
        let rank = rank_of(&project, id).expect("rebalance ranks the whole group");
        assert!(
            rank.bytes()
                .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase()),
            "{id} kept a malformed rank: {rank}"
        );
    }
}

#[test]
fn a_refused_move_leaves_sibling_ranks_untouched() {
    // The rebalance path rewrites every sibling, so the moved task is written first: its write is
    // the one that fails the cycle check, and a refused command must not have edited anything.
    let project = Project::local();
    let a = project.create("A");
    let b = project.child("B", &a);
    let c = project.child("C", &b);
    let sibling = project.child("Sibling", &c);
    let before = rank_of(&project, &sibling);

    let out = project.run(&["move", &a, "--parent", &c]);

    assert!(!out.status.success(), "cycle must be refused");
    assert_eq!(
        rank_of(&project, &sibling),
        before,
        "a refused move must not rewrite the target group"
    );
}

fn create_tag(project: &Project, name: &str) -> String {
    ok(project.run(&["tag", "create", name])).trim().to_owned()
}

fn tag_file(project: &Project, name: &str) -> std::path::PathBuf {
    project.store().join("tags").join(format!("{name}.md"))
}

fn tags_of(project: &Project, key: &str) -> Vec<String> {
    std::fs::read_to_string(project.task_file(key))
        .unwrap()
        .lines()
        .skip_while(|line| *line != "tags:")
        .skip(1)
        .map_while(|line| line.strip_prefix("- ").map(str::to_owned))
        .collect()
}

#[test]
fn tag_create_normalizes_the_name_and_writes_the_file() {
    let project = Project::local();

    let out = project.run(&["tag", "create", "Front End", "--desc", "the SPA"]);

    assert_eq!(
        ok(out).trim(),
        "front-end",
        "create prints the name the tag is registered under"
    );
    let contents = std::fs::read_to_string(tag_file(&project, "front-end")).unwrap();
    assert!(contents.contains("# Front End"), "contents: {contents}");
    assert!(contents.contains("the SPA"), "contents: {contents}");
    assert!(
        contents.contains("color: "),
        "a create always materializes the color: {contents}"
    );
}

#[test]
fn init_registers_the_default_tags() {
    let project = Project::local();

    let listed = ok(project.run(&["tag", "list"]));

    for name in ["bug", "feature", "draft"] {
        assert!(listed.contains(name), "{name} is missing: {listed}");
    }
}

#[test]
fn tag_create_refuses_a_name_it_cannot_normalize() {
    let project = Project::local();

    let out = project.run(&["tag", "create", "C++"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("lowercase letters"),
        "the refusal carries the naming rule: {}",
        stderr(&out)
    );
}

#[test]
fn tag_create_refuses_a_second_tag_of_the_same_name() {
    let project = Project::local();
    create_tag(&project, "backend");

    let out = project.run(&["tag", "create", "Backend"]);

    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("backend"),
        "the refusal names the tag: {}",
        stderr(&out)
    );
}

#[test]
fn tag_create_refuses_a_color_outside_the_palette() {
    let project = Project::local();

    let out = project.run(&["tag", "create", "backend", "--color", "turquoise"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("teal"),
        "the refusal lists the palette: {}",
        stderr(&out)
    );
    assert!(
        !tag_file(&project, "backend").exists(),
        "a refused color registers nothing"
    );
}

#[test]
fn tag_colors_lists_the_palette() {
    let project = Project::local();

    let palette = ok(project.run(&["tag", "colors"]));

    let names: Vec<&str> = palette.lines().collect();
    assert_eq!(names.len(), 12, "the palette is closed: {names:?}");
    assert!(names.contains(&"teal"), "{names:?}");

    let home = Home::new();
    let elsewhere = tempfile::tempdir().unwrap();
    let outside = home.run(elsewhere.path(), &["tag", "colors"]);
    assert_eq!(
        ok(outside),
        palette,
        "the palette needs no project and no daemon"
    );
    assert!(!home.path().join("daemon.json").exists());
}

#[test]
fn tag_list_and_show_report_the_registry() {
    let project = Project::local();
    ok(project.run(&["tag", "create", "Backend", "--color", "teal"]));
    ok(project.run(&["tag", "create", "wip", "--desc", "in flight"]));

    let listed = ok(project.run(&["tag", "list"]));
    assert!(listed.contains("backend"), "{listed}");
    assert!(listed.contains("teal"), "{listed}");
    assert!(listed.contains("in flight"), "{listed}");

    let shown = ok(project.run(&["tag", "show", "Backend"]));
    assert!(
        shown.contains("name:    backend"),
        "show takes any spelling that normalizes: {shown}"
    );
    assert!(shown.contains("display: Backend"), "{shown}");
    assert!(shown.contains("color:   teal"), "{shown}");

    let tag = json(project.run(&["tag", "show", "wip", "--json"]));
    assert_eq!(tag["name"], "wip");
    assert_eq!(tag["description"], "in flight");
}

#[test]
fn tag_set_recolors_and_redescribes() {
    let project = Project::local();
    create_tag(&project, "backend");

    ok(project.run(&["tag", "set", "backend", "color", "pink"]));
    ok(project.run(&["tag", "set", "backend", "desc", "server work"]));

    let shown = ok(project.run(&["tag", "show", "backend"]));
    assert!(shown.contains("color:   pink"), "{shown}");
    assert!(shown.contains("desc:    server work"), "{shown}");

    ok(project.run(&["tag", "set", "backend", "desc", ""]));
    let shown = ok(project.run(&["tag", "show", "backend"]));
    assert!(
        shown.contains("desc:    -"),
        "an empty value clears the description: {shown}"
    );
}

#[test]
fn tag_set_rejects_an_unknown_field() {
    let project = Project::local();
    create_tag(&project, "backend");

    let out = project.run(&["tag", "set", "backend", "colour", "pink"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("expected color | desc"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn tag_rename_moves_the_file_and_rewrites_the_tasks_that_carry_it() {
    let project = Project::local();
    create_tag(&project, "backend");
    let id = ok(project.run(&["create", "Wire the parser", "--tag", "backend"]))
        .trim()
        .to_owned();

    let renamed = ok(project.run(&["tag", "rename", "backend", "Infra"]));

    assert_eq!(renamed.trim(), "infra");
    assert!(!tag_file(&project, "backend").exists());
    assert!(tag_file(&project, "infra").exists());
    assert_eq!(tags_of(&project, &id), vec!["infra".to_owned()]);
}

#[test]
fn tag_delete_refuses_a_referenced_tag_until_it_is_forced() {
    let project = Project::local();
    create_tag(&project, "backend");
    ok(project.run(&["create", "Wire the parser", "--tag", "backend"]));

    let refused = project.run(&["tag", "delete", "backend", "--yes"]);
    assert!(!refused.status.success(), "stdout: {}", stdout(&refused));
    assert!(
        stderr(&refused).contains("tag backend is used by 1 task(s); pass --force to delete it"),
        "the refusal says how to override it: {}",
        stderr(&refused)
    );
    assert!(tag_file(&project, "backend").exists());

    let forced = project.run(&["tag", "delete", "backend", "--force", "--yes"]);
    assert!(forced.status.success(), "stderr: {}", stderr(&forced));
    assert!(!tag_file(&project, "backend").exists());
    assert!(
        stderr(&forced).contains("1 task still carries backend"),
        "a forced delete says what it left behind: {}",
        stderr(&forced)
    );
}

#[test]
fn tag_names_reach_the_store_as_the_identity_they_normalize_to() {
    let project = Project::local();
    assert_eq!(create_tag(&project, "Front End"), "front-end");

    let id = ok(project.run(&["create", "Wire the parser", "--tag", "Front End"]))
        .trim()
        .to_owned();
    assert_eq!(tags_of(&project, &id), vec!["front-end".to_owned()]);

    ok(project.run(&["set", &id, "tags", "FRONT_END"]));
    assert_eq!(tags_of(&project, &id), vec!["front-end".to_owned()]);

    let shown = ok(project.run(&["tag", "show", "Front_End"]));
    assert!(shown.contains("name:    front-end"), "{shown}");
}

#[test]
fn a_name_no_tag_can_have_is_refused_with_the_rule() {
    let project = Project::local();

    for args in [
        vec!["create", "Wire the parser", "--tag", "C++"],
        vec!["tag", "show", ""],
        vec!["tag", "delete", ""],
    ] {
        let out = project.run(&args);
        assert!(!out.status.success(), "{args:?} stdout: {}", stdout(&out));
        assert!(
            stderr(&out).contains("lowercase letters"),
            "{args:?} answers with the naming rule: {}",
            stderr(&out)
        );
        assert!(
            !stderr(&out).contains("openplan server stop"),
            "{args:?} must not blame the daemon: {}",
            stderr(&out)
        );
    }
}

#[test]
fn tag_delete_of_an_unknown_name_fails_before_it_prompts() {
    let project = Project::local();

    // No `--yes`: a typo must be refused outright rather than put to the reader as a question.
    let out = project.run(&["tag", "delete", "backend"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("no such tag: backend"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        !stdout(&out).contains("delete tag backend?"),
        "stdout: {}",
        stdout(&out)
    );
}

#[test]
fn create_with_tags_writes_a_sorted_set_and_leaves_the_body_alone() {
    let project = Project::local();
    create_tag(&project, "backend");
    create_tag(&project, "wip");

    let out = project.run(&[
        "create",
        "Wire the parser",
        "--tag",
        "wip",
        "--tag",
        "backend",
        "--tag",
        "wip",
        "--body",
        "## Goals\n- Parse it\n",
    ]);

    let id = ok(out).trim().to_owned();
    assert_eq!(
        tags_of(&project, &id),
        vec!["backend".to_owned(), "wip".to_owned()],
        "the set is sorted and deduplicated"
    );
    assert_eq!(
        task_body(&project.task_file(&id)),
        "# Wire the parser\n\n## Goals\n- Parse it\n"
    );
}

#[test]
fn create_with_an_unknown_tag_names_the_command_that_registers_it() {
    let project = Project::local();

    let out = project.run(&["create", "Wire the parser", "--tag", "wip"]);

    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    // The daemon states the fact and names the refusal; the command that answers it is this
    // binary's own spelling, so this binary is what adds it.
    assert!(
        stderr(&out)
            .contains("tag wip is not registered; register it with `openplan tag create <name>`"),
        "the refusal says how to register the tag: {}",
        stderr(&out)
    );
    assert!(
        !ok(project.run(&["list"])).contains("Wire the parser"),
        "a refused write creates no task"
    );
}

#[test]
fn set_tags_replaces_the_whole_set_and_an_empty_value_clears_it() {
    let project = Project::local();
    create_tag(&project, "backend");
    create_tag(&project, "wip");
    let id = project.create("Wire the parser");

    ok(project.run(&["set", &id, "tags", "wip, backend"]));
    assert_eq!(
        tags_of(&project, &id),
        vec!["backend".to_owned(), "wip".to_owned()]
    );

    ok(project.run(&["set", &id, "tags", "wip"]));
    assert_eq!(tags_of(&project, &id), vec!["wip".to_owned()]);

    ok(project.run(&["set", &id, "tags", ""]));
    assert!(
        tags_of(&project, &id).is_empty(),
        "an empty value clears the set and omits the field"
    );
    assert!(
        !std::fs::read_to_string(project.task_file(&id))
            .unwrap()
            .contains("tags:")
    );
    assert!(ok(project.run(&["show", &id])).contains("tags: -"));
}

#[test]
fn set_tags_refuses_a_name_the_project_does_not_register() {
    let project = Project::local();
    create_tag(&project, "backend");
    let id = project.create("Wire the parser");

    let out = project.run(&["set", &id, "tags", "backend, wip"]);

    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("openplan tag create"),
        "stderr: {}",
        stderr(&out)
    );
    assert!(
        tags_of(&project, &id).is_empty(),
        "a refused set leaves the task alone"
    );
}

#[test]
fn comment_appends_an_entry_and_creates_the_section() {
    let project = Project::local();
    let id = project.create("Ship login");

    let printed = ok(project.run(&["comment", &id, "hello"]));

    assert!(printed.starts_with(&format!("{id}: ")), "{printed}");
    assert!(
        printed.contains(&format!("by Test via {}", common::AGENT)),
        "{printed}"
    );
    let body = task_body(&project.task_file(&id));
    assert!(body.contains("## Comments\n"), "{body}");
    assert!(body.contains(" by Test"), "{body}");
    assert!(body.trim_end().ends_with("> hello"), "{body}");
}

#[test]
fn a_comment_holding_markdown_stays_one_entry() {
    let project = Project::local();
    let id = project.create("Ship login");
    let text = "# Any heading works\n\n```rust\nfn main() {}\n```\n\n> nested";

    ok(project.run(&["comment", &id, text]));

    let comments = json(project.run(&["comments", &id, "--json"]));
    assert_eq!(comments.as_array().unwrap().len(), 1);
    assert_eq!(comments[0]["text"], text);
}

#[test]
fn comments_print_oldest_first() {
    let project = Project::local();
    let id = project.create("Ship login");
    ok(project.run(&["comment", &id, "first"]));
    ok(project.run(&["comment", &id, "second"]));

    let printed = ok(project.run(&["comments", &id]));

    let first = printed.find("first").expect("the first entry");
    let second = printed.find("second").expect("the second entry");
    assert!(first < second, "file order is the true order: {printed}");
    assert!(
        printed.contains("    first"),
        "the text is indented: {printed}"
    );
}

#[test]
fn comments_json_carries_the_four_fields() {
    let project = Project::local();
    let id = project.create("Ship login");
    ok(project.run(&["comment", &id, "hello"]));

    let comments = json(project.run(&["comments", &id, "--json"]));

    let entry = comments[0].as_object().unwrap();
    let mut keys: Vec<&String> = entry.keys().collect();
    keys.sort();
    assert_eq!(keys, vec!["agent", "at", "author", "text"]);
    assert_eq!(entry["author"], "Test");
    assert_eq!(entry["agent"], common::AGENT);
    assert_eq!(entry["text"], "hello");
}

#[test]
fn comment_refuses_empty_text() {
    let project = Project::local();
    let id = project.create("Ship login");

    let out = project.run(&["comment", &id, "   \n"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("needs text"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn comment_refuses_an_unsigned_entry() {
    let project = Project::local();
    let id = project.create("Ship login");
    git(project.path(), &["config", "--unset", "user.name"]);

    let out = project.run(&["comment", &id, "hello"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("user.name"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn comment_reads_its_text_from_stdin() {
    let project = Project::local();
    let id = project.create("Ship login");

    ok(piped(
        &project,
        &["comment", &id, "--body-file", "-"],
        b"from a pipe",
    ));

    assert!(task_body(&project.task_file(&id)).contains("> from a pipe"));
}

#[test]
fn a_damaged_entry_keeps_its_text_and_reports_the_field() {
    let project = Project::local();
    project.edit(
        "tasks/00001-ship-it.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship it\n\n## Comments\n\n### \
         yesterday by Test\n\n> still readable\n",
    );

    let comments = json(project.run(&["comments", "OPP-1", "--json"]));

    assert_eq!(comments[0]["text"], "still readable");
    assert_eq!(comments[0]["at"]["kind"], "invalid");
    assert!(
        comments[0]["at"]["message"]
            .as_str()
            .unwrap()
            .contains("\"yesterday\""),
        "the message carries the offending text: {}",
        comments[0]["at"]["message"]
    );
    let printed = ok(project.run(&["comments", "OPP-1"]));
    assert!(
        printed.contains("\"yesterday\"") && printed.contains("by Test"),
        "the heading names why the time is unreadable: {printed}"
    );
}

#[test]
fn get_renders_the_comment_log_with_the_file() {
    let project = Project::local();
    let id = project.create("Ship login");
    ok(project.run(&["comment", &id, "hello"]));

    let printed = ok(project.run(&["get", &id]));

    assert!(printed.contains("## Comments"), "{printed}");
    assert!(printed.contains("> hello"), "{printed}");
}

// A store `lint` can read: `.plan/config.toml` plus `.plan/tasks/` in a directory outside any git
// repository. Lint opens the tasks itself and never starts a daemon.
struct LintStore {
    home: Home,
    dir: tempfile::TempDir,
}

impl LintStore {
    fn new() -> Self {
        Self {
            home: Home::new(),
            dir: unregistered_store("OPP"),
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn put(&self, relative: &str, contents: &str) {
        write(&self.path().join(".plan").join(relative), contents);
    }

    fn lint(&self, args: &[&str]) -> Output {
        let mut all = vec!["lint"];
        all.extend_from_slice(args);
        let out = self.home.run(self.path(), &all);
        assert!(
            !self.home.path().join("daemon.json").exists(),
            "lint never starts a daemon"
        );
        out
    }
}

const VALID: &str = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Title\n";

#[test]
fn lint_clean_project_exits_zero() {
    let store = LintStore::new();
    store.put("tasks/00001-clean.md", VALID);
    store.put("tasks/00002-clean.md", VALID);

    let out = store.lint(&[]);

    assert!(out.status.success(), "{}", combined(&out));
    assert!(
        stdout(&out).contains("checked 2 tasks and 0 skill files, found 0 problems"),
        "{}",
        stdout(&out)
    );
}

#[test]
fn lint_reports_a_problem_by_its_task_and_fails() {
    let store = LintStore::new();
    store.put("tasks/00001-clean.md", VALID);
    store.put(
        "tasks/00002-orphan.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: ./00009-gone.md\n---\n# Orphan\n",
    );

    let out = store.lint(&[]);

    assert!(!out.status.success(), "{}", combined(&out));
    assert!(
        stdout(&out).contains("OPP-2: error[reference]: the parent OPP-9 does not exist"),
        "{}",
        stdout(&out)
    );
    assert!(stdout(&out).contains("found 1 problem"), "{}", stdout(&out));
}

#[test]
fn lint_json_names_the_task_the_code_and_the_help() {
    let store = LintStore::new();
    store.put(
        "tasks/00001-defect.md",
        "---\nstatus: bogus\ncreated: 2026-01-01T00:00:00Z\n---\n# Defect\n",
    );

    let out = store.lint(&["--json"]);

    assert!(!out.status.success());
    let findings: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0]["task"], "OPP-1");
    assert_eq!(findings[0]["code"], "field");
    assert!(
        findings[0]["help"]
            .as_str()
            .is_some_and(|help| !help.is_empty())
    );
}

#[test]
fn lint_keys_filter_the_report_and_an_unknown_key_fails() {
    let store = LintStore::new();
    let broken = |title: &str| {
        format!("---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags: [gone]\n---\n# {title}\n")
    };
    store.put("tasks/00001-alpha.md", &broken("Alpha"));
    store.put("tasks/00002-beta.md", &broken("Beta"));

    let out = store.lint(&["OPP-1"]);
    assert!(!out.status.success());
    assert!(
        stdout(&out).contains("OPP-1: error[tag]"),
        "{}",
        stdout(&out)
    );
    assert!(!stdout(&out).contains("OPP-2"), "{}", stdout(&out));
    assert!(
        stdout(&out).contains("checked 1 task and"),
        "{}",
        stdout(&out)
    );

    let unknown = store.lint(&["OPP-7"]);
    assert!(!unknown.status.success());
    assert!(
        stderr(&unknown).contains("no task matches OPP-7"),
        "{}",
        stderr(&unknown)
    );
}

#[test]
fn lint_reports_a_conflict_left_by_a_sync() {
    let store = LintStore::new();
    store.put(
        "tasks/00001-login.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Login\n\n<<<<<<< Ann (1111111)\nOAuth only.\n=======\nOAuth and email.\n>>>>>>> Ben (2222222)\n",
    );

    let out = store.lint(&[]);

    assert!(!out.status.success());
    assert!(
        stdout(&out).contains("OPP-1: error[conflict]: 1 unresolved conflict from a sync"),
        "{}",
        stdout(&out)
    );
}

// CI checks a code change, and the tasks are not part of one: `--skills` needs no tasks at all.
#[test]
fn lint_skills_checks_the_skill_files_alone() {
    let home = Home::new();
    let root = tempfile::tempdir().unwrap();
    ok(home.run(root.path(), &["setup-skills", "--agent=claude"]));

    let clean = home.run(root.path(), &["lint", "--skills"]);
    assert!(clean.status.success(), "{}", combined(&clean));

    let skill = root.path().join(".claude/skills/openplan/SKILL.md");
    std::fs::write(&skill, "stale\n").unwrap();
    let stale = home.run(root.path(), &["lint", "--skills"]);
    assert!(!stale.status.success());
    let report = stdout(&stale);
    assert!(
        report.contains("error[skill]: skill openplan differs from the openplan binary"),
        "{report}"
    );
    assert!(report.contains("run `openplan setup-skills`"), "{report}");
    assert!(
        !home.path().join("daemon.json").exists(),
        "lint never starts a daemon"
    );
}

#[test]
fn lint_skills_reports_retired_skills_until_setup_skills_removes_them() {
    let home = Home::new();
    let root = tempfile::tempdir().unwrap();
    let retired = root.path().join(".claude/skills/task-management");
    std::fs::create_dir_all(&retired).unwrap();
    std::fs::write(retired.join("SKILL.md"), "old\n").unwrap();

    let before = home.run(root.path(), &["lint", "--skills"]);
    assert!(!before.status.success());
    let report = stdout(&before);
    assert!(
        report.contains("error[skill]: skill task-management is retired"),
        "{report}"
    );
    assert!(
        report.contains("error[skill]: skill openplan is missing"),
        "{report}"
    );

    ok(home.run(root.path(), &["setup-skills", "--agent=claude"]));

    let after = home.run(root.path(), &["lint", "--skills"]);
    assert!(after.status.success(), "{}", combined(&after));
    assert!(!retired.exists());
}

// `lint` reads the skills under the project root it discovers, so an install run from a
// subdirectory has to write them there; otherwise lint reports as missing what the user just
// installed.
#[test]
fn setup_skills_installs_at_the_project_root() {
    let store = LintStore::new();
    let sub = store.path().join("crates/op-cli");
    std::fs::create_dir_all(&sub).unwrap();

    ok(store.home.run(&sub, &["setup-skills", "--agent=claude"]));

    assert!(store.path().join(".claude/skills").is_dir());
    assert!(!sub.join(".claude").exists());
    assert!(
        store.lint(&[]).status.success(),
        "what setup-skills installed must lint clean"
    );
}

#[test]
fn lint_reports_every_comment_failure() {
    let project = Project::local();
    project.edit(
        "tasks/00001-ship-it.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Ship it\n\n## Comments\n\n### \
         yesterday by Test\n\n> a\n\n### 2026-01-01T00:00:00Z by Test\n\n### 2026-01-02T00:00:00Z \
         by Test\n\n> c\n\n> orphan\n\n## Comments\n\n### 2026-01-03T00:00:00Z by Test\n\n> d\n",
    );

    let out = project.run(&["lint"]);
    let report = combined(&out);

    assert!(!out.status.success(), "{report}");
    let reported: Vec<&str> = report
        .lines()
        .filter(|line| line.starts_with("OPP-1: error[comment]"))
        .collect();
    assert_eq!(reported.len(), 5, "{report}");
    for expected in [
        "not an RFC3339 UTC timestamp: \"yesterday\"",
        "needs a blockquote below its heading",
        "needs an entry heading above it",
        "one `## Comments` section",
        "must be the last section",
    ] {
        assert!(
            reported.iter().any(|line| line.contains(expected)),
            "{expected} is not reported: {report}"
        );
    }
}

#[test]
fn lint_reports_a_broken_mermaid_fence_in_the_body_and_in_a_comment() {
    let store = LintStore::new();
    store.put(
        "tasks/00001-drawn.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Drawn\n\n```mermaid\npie\n```\n\n\
         ## Comments\n\n### 2026-01-01T00:00:00Z by Test\n\n> ```mermaid\n> flowchart LR\n>   a -->\n> ```\n",
    );

    let out = store.lint(&[]);
    let report = stdout(&out);

    assert!(!out.status.success(), "{}", combined(&out));
    for expected in [
        "OPP-1: error[diagram]: the Mermaid diagram fails at line 8, column 1: `pie` is not a supported diagram type",
        "OPP-1: error[diagram]: the Mermaid diagram fails at line 17, column 10: expected a node id",
    ] {
        assert!(
            report.contains(expected),
            "{expected} is not reported: {report}"
        );
    }
}

// A worktree is a checkout of its own, so the skills a person installs there stay there.
#[test]
fn setup_skills_in_a_worktree_writes_into_that_worktree() {
    let home = Home::new();
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    git_repo(&main);
    write(&main.join("README.md"), "# Code\n");
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-qm", "Start"]);
    let worktree = dir.path().join("feature");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            worktree.to_str().unwrap(),
            "-b",
            "feature",
        ],
    );
    let sub = worktree.join("src");
    std::fs::create_dir_all(&sub).unwrap();

    ok(home.run(&sub, &["setup-skills", "--agent=claude"]));

    assert!(worktree.join(".claude/skills").is_dir());
    assert!(!main.join(".claude").exists());
    assert!(!sub.join(".claude").exists());
    let checked = home.run(&sub, &["lint", "--skills"]);
    assert!(checked.status.success(), "{}", combined(&checked));
}

#[test]
fn help_lists_every_status() {
    let home = Home::new();
    let root = tempfile::tempdir().unwrap();

    for command in [&["create", "--help"][..], &["list", "--help"]] {
        let help = ok(home.run(root.path(), command));
        let expected = format!(
            "[possible values: {}]",
            Status::ALL.map(|s| s.as_str()).join(", ")
        );
        assert!(help.contains(&expected), "{command:?} help: {help}");
    }
}

#[test]
fn help_lists_every_tag_color() {
    let home = Home::new();
    let root = tempfile::tempdir().unwrap();

    let help = ok(home.run(root.path(), &["tag", "create", "--help"]));

    let expected = format!(
        "[possible values: {}]",
        Color::ALL.map(|c| c.as_str()).join(", ")
    );
    assert!(help.contains(&expected), "{help}");
}
