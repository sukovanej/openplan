mod common;

use std::path::Path;

use common::{
    Home, Project, Remote, commit_at, git, git_output, git_repo, git_stdout, json, ok, stderr,
    stdout, write,
};

fn canonical(path: &Path) -> String {
    path.canonicalize().unwrap().display().to_string()
}

fn tasks_branch_exists(root: &Path) -> bool {
    git_output(
        root,
        &["rev-parse", "--verify", "-q", "refs/openplan/tasks"],
    )
    .status
    .success()
}

// A repository with no commit at all is enough: the tasks branch has a history of its own.
#[test]
fn init_in_a_repository_starts_the_tasks_on_a_git_branch() {
    let home = Home::new();
    let repo = tempfile::tempdir().unwrap();
    git_repo(repo.path());

    let started = ok(home.run(repo.path(), &["init", "--abbreviation", "OPP"]));

    assert!(
        started.contains("keeps its OPP tasks in the git ref refs/openplan/tasks"),
        "{started}"
    );
    assert!(tasks_branch_exists(repo.path()));
    assert_eq!(
        ok(home.run(repo.path(), &["create", "Ship it"])).trim(),
        "OPP-1"
    );
    assert!(ok(home.run(repo.path(), &["list"])).contains("Ship it"));
    assert!(
        !repo.path().join(".plan").exists(),
        "no task file lands in the checkout"
    );
    let log = git_stdout(
        repo.path(),
        &["log", "--format=%an %s", "openplan/tasks", "--"],
    );
    assert!(log.contains("Test OPP-1: create \"Ship it\""), "{log}");
    assert!(log.contains("Test Start the OPP tasks"), "{log}");
    assert!(
        !git_output(repo.path(), &["rev-parse", "--verify", "-q", "HEAD"])
            .status
            .success(),
        "the code branch is still unborn"
    );
}

#[test]
fn init_outside_a_repository_starts_a_local_directory() {
    let home = Home::new();
    let dir = tempfile::tempdir().unwrap();

    let started = ok(home.run(dir.path(), &["init", "--abbreviation", "LOC"]));

    let store = dir.path().canonicalize().unwrap().join(".plan");
    assert!(
        started.contains(&format!("keeps its LOC tasks in {}", store.display())),
        "{started}"
    );
    assert!(store.join(".history.sqlite").is_file());
    assert_eq!(
        std::fs::read_to_string(store.join("config.toml")).unwrap(),
        "abbreviation = \"LOC\"\n"
    );
    let id = ok(home.run(dir.path(), &["create", "Ship it"]));
    assert_eq!(id.trim(), "LOC-1");
    assert!(store.join("tasks/00001-ship-it.md").is_file());
    let listed = ok(home.run(dir.path(), &["project", "list"]));
    assert!(listed.contains("LOC  local"), "{listed}");
}

#[test]
fn init_with_the_local_backend_keeps_the_tasks_of_a_repository_in_a_directory() {
    let home = Home::new();
    let repo = tempfile::tempdir().unwrap();
    git_repo(repo.path());

    let started = ok(home.run(
        repo.path(),
        &["init", "--abbreviation", "OPP", "--backend", "local"],
    ));

    assert!(started.contains("/.plan"), "{started}");
    assert!(repo.path().join(".plan/.history.sqlite").is_file());
    assert!(!tasks_branch_exists(repo.path()));
    ok(home.run(repo.path(), &["create", "Ship it"]));
    let history = ok(home.run(repo.path(), &["history"]));
    assert!(
        history.contains(&format!("Test via {}  OPP-1: create", common::AGENT)),
        "the git identity signs a local write too: {history}"
    );
}

#[test]
fn init_refuses_an_abbreviation_that_is_not_three_letters() {
    let home = Home::new();
    let dir = tempfile::tempdir().unwrap();

    let out = home.run(dir.path(), &["init", "--abbreviation", "Op"]);

    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("three uppercase letters"),
        "{}",
        stderr(&out)
    );
    assert!(!dir.path().join(".plan/config.toml").exists());
}

#[test]
fn init_again_keeps_the_abbreviation_the_project_has() {
    let home = Home::new();
    let local = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    git_repo(repo.path());
    for root in [local.path(), repo.path()] {
        ok(home.run(root, &["init", "--abbreviation", "OPP"]));
        let id = ok(home.run(root, &["create", "Ship it"])).trim().to_owned();

        ok(home.run(root, &["init", "--abbreviation", "OPP"]));
        let refused = home.run(root, &["init", "--abbreviation", "XYZ"]);

        assert!(!refused.status.success(), "{}", stdout(&refused));
        assert!(
            stderr(&refused).contains("this project already uses the abbreviation OPP"),
            "{}",
            stderr(&refused)
        );
        assert!(ok(home.run(root, &["show", &id])).contains("title:  Ship it"));
    }
}

// A second person who clones the repository joins the tasks the first one pushed, rather than start
// a rival set under the same name.
#[test]
fn init_without_an_abbreviation_joins_the_tasks_on_the_remote() {
    let remote = Remote::new();
    let ann = remote.founder("ann", "Ann");
    let id = remote.create(&ann, "From Ann");
    ok(remote.run(&ann, &["sync"]));

    let ben = remote.clone("ben", "Ben");
    let joined = ok(remote.run(&ben, &["init"]));
    assert!(
        joined.contains("keeps its OPP tasks in the git ref refs/openplan/tasks"),
        "{joined}"
    );
    assert!(ok(remote.run(&ben, &["show", &id])).contains("title:  From Ann"));
}

#[test]
fn init_without_an_abbreviation_refuses_a_remote_with_no_tasks() {
    let remote = Remote::new();
    let ann = remote.clone("ann", "Ann");

    let refused = remote.run(&ann, &["init"]);
    assert!(!refused.status.success(), "{}", stdout(&refused));
    assert!(
        stderr(&refused).contains("openplan init --abbreviation <ABC>"),
        "{}",
        stderr(&refused)
    );
}

#[test]
fn init_in_a_clone_joins_the_tasks_on_the_remote() {
    let remote = Remote::new();
    let ann = remote.founder("ann", "Ann");
    let id = remote.create(&ann, "From Ann");
    ok(remote.run(&ann, &["sync"]));

    let ben = remote.clone("ben", "Ben");
    let joined = ok(remote.run(&ben, &["init", "--abbreviation", "OPP"]));

    assert!(
        joined.contains("keeps its OPP tasks in the git ref refs/openplan/tasks"),
        "{joined}"
    );
    assert!(
        ok(remote.run(&ben, &["show", &id])).contains("title:  From Ann"),
        "the clone reads the tasks it joined"
    );

    let cat = remote.clone("cat", "Cat");
    let refused = remote.run(&cat, &["init", "--abbreviation", "XYZ"]);
    assert!(!refused.status.success(), "{}", stdout(&refused));
    assert!(
        stderr(&refused).contains("already uses the abbreviation OPP"),
        "{}",
        stderr(&refused)
    );
    assert!(
        ok(remote.run(&cat, &["list"])).contains("From Ann"),
        "a refused abbreviation leaves the tasks the clone joined"
    );
}

// Three commits of a repository that kept `.plan/` beside the code: the tasks, a change to the code
// alone, and a change to one task.
fn repository_with_plan(root: &Path) {
    git_repo(root);
    write(&root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n");
    write(
        &root.join(".plan/tasks/00001-alpha.md"),
        "---\nstatus: todo\ncreated: 2001-01-01T00:00:00Z\n---\n# Alpha\n",
    );
    write(
        &root.join(".plan/tasks/00002-beta.md"),
        "---\nstatus: todo\ncreated: 2001-01-01T00:00:00Z\n---\n# Beta\n",
    );
    commit_at(root, 1_000_000_000, "Plan alpha and beta");
    write(&root.join("README.md"), "# Code\n");
    commit_at(root, 1_050_000_000, "Write the README");
    write(
        &root.join(".plan/tasks/00001-alpha.md"),
        "---\nstatus: done\ncreated: 2001-01-01T00:00:00Z\n---\n# Alpha\n",
    );
    commit_at(root, 1_100_000_000, "Finish alpha");
}

#[test]
fn migrate_copies_the_history_of_the_plan_directory_to_the_tasks_branch() {
    let home = Home::new();
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    repository_with_plan(root);
    write(
        &root.join(".plan/tasks/00001-alpha.md"),
        "---\nstatus: done\ncreated: 2001-01-01T00:00:00Z\n---\n# Alpha\n\nEdited by hand.\n",
    );

    let before = home.run(root, &["list"]);
    assert!(!before.status.success(), "{}", stdout(&before));
    assert!(
        stderr(&before).contains("openplan migrate"),
        "{}",
        stderr(&before)
    );

    let migrated = ok(home.run(root, &["migrate"]));

    assert!(
        migrated.contains("copied 2 revisions of .plan/ to the git ref refs/openplan/tasks"),
        "one revision for each commit that changed .plan/: {migrated}"
    );
    assert!(
        migrated.contains("copied the edits in .plan/ that no commit holds yet"),
        "{migrated}"
    );
    assert!(
        migrated.contains("keeps its OPP tasks in the git ref refs/openplan/tasks"),
        "{migrated}"
    );
    assert!(migrated.contains("git rm -r .plan"), "{migrated}");

    let history = ok(home.run(root, &["history", "OPP-1"]));
    let messages: Vec<&str> = history
        .lines()
        .map(|line| line.rsplit("  ").next().unwrap())
        .collect();
    assert_eq!(
        messages,
        vec![
            "Import task edits not yet committed",
            "Finish alpha",
            "Plan alpha and beta"
        ],
        "{history}"
    );
    assert!(
        history.contains("2004-11-09T11:33:20Z  Test"),
        "a copied revision keeps its author and time: {history}"
    );

    let alpha = ok(home.run(root, &["get", "OPP-1"]));
    assert!(alpha.contains("status: done"), "{alpha}");
    assert!(
        alpha.contains("Edited by hand."),
        "the uncommitted edit arrives: {alpha}"
    );

    let oldest = json(home.run(root, &["history", "OPP-1", "--json"]))[2]["revision"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let then = ok(home.run(root, &["get", "OPP-1", "--revision", &oldest]));
    assert!(then.contains("status: todo"), "{then}");

    assert!(tasks_branch_exists(root));
    assert_eq!(
        git_stdout(root, &["log", "--format=%s", "main"])
            .lines()
            .count(),
        3,
        "the code branch keeps its own history"
    );
}

// The index dates a task by the last revision that changed it, and a migration keeps those dates.
#[test]
fn migrate_keeps_the_date_each_task_last_changed() {
    let home = Home::new();
    let repo = tempfile::tempdir().unwrap();
    repository_with_plan(repo.path());

    ok(home.run(repo.path(), &["migrate"]));

    let alpha = json(home.run(repo.path(), &["get", "OPP-1", "--json"]));
    let beta = json(home.run(repo.path(), &["get", "OPP-2", "--json"]));
    assert_eq!(alpha["updated"], "2004-11-09T11:33:20Z");
    assert_eq!(beta["updated"], "2001-09-09T01:46:40Z");
    let listed = json(home.run(repo.path(), &["list", "--json"]));
    let row = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "OPP-2")
        .unwrap_or_else(|| panic!("OPP-2 is listed: {listed}"));
    assert_eq!(row["updated"], "2001-09-09T01:46:40Z");
}

// Once `.plan/` leaves the code branch, the checkout still finds its tasks, on the branch.
#[test]
fn a_migrated_repository_answers_after_plan_leaves_the_code() {
    let home = Home::new();
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    repository_with_plan(root);
    ok(home.run(root, &["migrate"]));

    git(root, &["rm", "-r", "-q", ".plan"]);
    git(root, &["commit", "-qm", "Move the tasks to their branch"]);
    home.stop();

    let listed = ok(home.run(root, &["list"]));
    assert!(
        listed.contains("Alpha") && listed.contains("Beta"),
        "{listed}"
    );
    assert_eq!(ok(home.run(root, &["create", "Gamma"])).trim(), "OPP-3");
}

#[test]
fn migrate_to_a_local_directory_keeps_the_files_where_they_are() {
    let home = Home::new();
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    repository_with_plan(root);

    let migrated = ok(home.run(root, &["migrate", "--backend", "local"]));

    let store = root.canonicalize().unwrap().join(".plan");
    assert!(
        migrated.contains(&format!("keeps its OPP tasks in {}", store.display())),
        "{migrated}"
    );
    assert!(migrated.contains("git rm -r --cached .plan"), "{migrated}");
    assert!(store.join(".history.sqlite").is_file());
    assert!(!tasks_branch_exists(root));
    let listed = ok(home.run(root, &["list"]));
    assert!(
        listed.contains("Alpha") && listed.contains("Beta"),
        "{listed}"
    );
    let history = ok(home.run(root, &["history"]));
    assert!(history.contains("Edit outside openplan"), "{history}");
}

#[test]
fn migrate_refuses_a_project_with_nothing_to_migrate() {
    let project = Project::git();

    let out = project.run(&["migrate"]);

    assert!(!out.status.success(), "{}", stdout(&out));
    assert!(
        stderr(&out).contains(&format!(
            "{} already keeps its tasks in the git ref refs/openplan/tasks; there is nothing to migrate",
            canonical(project.path())
        )),
        "{}",
        stderr(&out)
    );

    let local = Project::local();
    let out = local.run(&["migrate"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("nothing to migrate"),
        "{}",
        stderr(&out)
    );

    let home = Home::new();
    let empty = tempfile::tempdir().unwrap();
    let out = home.run(empty.path(), &["migrate"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("openplan init --abbreviation"),
        "{}",
        stderr(&out)
    );
}
