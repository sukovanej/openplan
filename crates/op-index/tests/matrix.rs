use std::path::Path;
use std::process::Command;

use op_api::{ChangeKind, Field, Rfc3339, Status};
use op_git::Repo;
use op_index::Index;
use op_store::Store;
use op_task::Timestamp;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git {args:?} failed");
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

// Named the way the store names files (§3.1): the number the name starts with allocates the task,
// and the slug after it is decoration the index must look past.
fn task(root: &Path, number: u64, contents: &str) {
    write(&task_path(root, number), contents);
}

fn task_path(root: &Path, number: u64) -> std::path::PathBuf {
    root.join(format!(".plan/tasks/{number:05}-task-{number}.md"))
}

// The id everything above the store speaks (§3.1).
fn key(number: u64) -> String {
    format!("OPP-{number}")
}

fn init(root: &Path) {
    git(root, &["init", "-q", "-b", "main"]);
    identify(root);
}

fn identify(root: &Path) {
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    write(&root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n");
}

fn commit(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", message]);
}

fn built(root: &Path) -> Index {
    let repo = Repo::discover(root).unwrap();
    let store = Store::discover(root).unwrap();
    let mut index = Index::new(store.abbreviation());
    index.rebuild(&repo, &store).unwrap();
    index
}

fn find<'a>(index: &'a Index, branch: &str, number: u64) -> Option<&'a op_api::MatrixCell> {
    index
        .matrix()
        .cells
        .iter()
        .find(|c| c.branch == branch && c.task.id == key(number))
}

#[test]
fn a_branch_unchanged_against_main_is_not_listed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);

    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Shared\n",
    );
    commit(root, "add shared");
    git(root, &["branch", "feature"]);
    task(
        root,
        2,
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Only main\n",
    );
    commit(root, "add only-main");

    let index = built(root);
    let cells = &index.matrix().cells;

    // `feature` forked before `only-main` and never touched `shared`, so it diverges from main on
    // nothing — it contributes no cells. Only main's two Base rows remain.
    assert!(
        cells.iter().all(|c| c.branch == "main"),
        "no feature rows: {cells:?}"
    );
    assert_eq!(cells.len(), 2, "two Base rows on main: {cells:?}");
    assert!(cells.iter().all(|c| c.kind == ChangeKind::Base));
}

#[test]
fn a_committed_edit_on_a_branch_is_modified() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# A\n",
    );
    commit(root, "add a");
    git(root, &["checkout", "-q", "-b", "feature"]);
    task(
        root,
        1,
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# A done\n",
    );
    commit(root, "edit a");
    git(root, &["checkout", "-q", "main"]);

    let index = built(root);

    let main = find(&index, "main", 1).expect("main base row");
    assert_eq!(main.kind, ChangeKind::Base);
    assert_eq!(main.task.metadata.status(), Some(Status::Todo));

    let feature = find(&index, "feature", 1).expect("feature row");
    assert_eq!(feature.kind, ChangeKind::Modified);
    assert_eq!(feature.task.metadata.status(), Some(Status::Done));
    assert_eq!(feature.task.title, "A done");
    assert!(!feature.dirty);
}

#[test]
fn a_new_task_on_a_branch_is_added() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# A\n",
    );
    commit(root, "add a");
    git(root, &["checkout", "-q", "-b", "feature"]);
    task(
        root,
        2,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# New\n",
    );
    commit(root, "add new");
    git(root, &["checkout", "-q", "main"]);

    let index = built(root);

    assert!(find(&index, "main", 2).is_none(), "new is not on main");
    let feature = find(&index, "feature", 2).expect("feature added row");
    assert_eq!(feature.kind, ChangeKind::Added);
    // `a` is unchanged on feature, so it is not repeated there.
    assert!(find(&index, "feature", 1).is_none());
}

#[test]
fn a_deletion_is_tagged_while_main_still_has_the_task() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# A\n",
    );
    task(
        root,
        2,
        "---\nstatus: in_progress\ncreated: 2026-01-01T00:00:00Z\n---\n# B\n",
    );
    commit(root, "add a and b");
    git(root, &["checkout", "-q", "-b", "feature"]);
    std::fs::remove_file(task_path(root, 2)).unwrap();
    commit(root, "remove b");
    git(root, &["checkout", "-q", "main"]);

    let index = built(root);

    let deleted = find(&index, "feature", 2).expect("feature deletion row");
    assert_eq!(deleted.kind, ChangeKind::Deleted);
    assert!(!deleted.dirty, "a committed removal is not dirty");
    // The row carries the pre-deletion version, so the UI can show what is being removed.
    assert_eq!(deleted.task.metadata.status(), Some(Status::InProgress));
    assert_eq!(deleted.task.title, "B");

    // A deleted task is not "on" the branch for a plain per-branch listing.
    let summaries = index.branch_summaries("feature");
    assert!(summaries.iter().all(|s| s.id != key(2)), "{summaries:?}");
}

#[test]
fn a_deletion_main_already_made_is_suppressed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# X\n",
    );
    commit(root, "add x");
    git(root, &["checkout", "-q", "-b", "feature"]);
    std::fs::remove_file(task_path(root, 1)).unwrap();
    commit(root, "feature removes x");
    git(root, &["checkout", "-q", "main"]);
    std::fs::remove_file(task_path(root, 1)).unwrap();
    commit(root, "main removes x too");

    let index = built(root);
    assert!(
        index.matrix().cells.is_empty(),
        "settled deletion shows nowhere: {:?}",
        index.matrix().cells
    );
}

#[test]
fn an_uncommitted_edit_on_a_feature_worktree_is_modified_and_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n",
    );
    commit(root, "add t");
    git(root, &["branch", "feature"]);

    let wt = tempfile::tempdir().unwrap();
    let wt_path = wt.path().join("feature");
    git(
        root,
        &[
            "worktree",
            "add",
            "-q",
            wt_path.to_str().unwrap(),
            "feature",
        ],
    );
    // The feature commit still matches main, but the working copy is edited. Divergence is judged
    // on the effective (working) state, so this surfaces as a dirty Modified — not hidden until
    // commit (the app's `oplan set` edits are uncommitted by nature).
    task(
        &wt_path,
        1,
        "---\nstatus: in_progress\ncreated: 2026-01-01T00:00:00Z\n---\n# T edited\n",
    );

    let index = built(root);
    let feature = find(&index, "feature", 1).expect("uncommitted WIP must surface");
    assert_eq!(feature.kind, ChangeKind::Modified);
    assert!(feature.dirty, "uncommitted edit is dirty");
    assert_eq!(feature.task.metadata.status(), Some(Status::InProgress));
    assert_eq!(feature.task.title, "T edited");
    let main = find(&index, "main", 1).expect("main base row");
    assert_eq!(main.kind, ChangeKind::Base);
}

#[test]
fn an_uncommitted_deletion_on_a_feature_worktree_is_deleted_and_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: in_progress\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n",
    );
    commit(root, "add t");
    git(root, &["branch", "feature"]);

    let wt = tempfile::tempdir().unwrap();
    let wt_path = wt.path().join("feature");
    git(
        root,
        &[
            "worktree",
            "add",
            "-q",
            wt_path.to_str().unwrap(),
            "feature",
        ],
    );
    // The commit still has `t`; only the working tree removed it. Effective state is absent, so it
    // surfaces as a dirty Deleted (pre-deletion version shown), consistent with uncommitted edits.
    std::fs::remove_file(task_path(&wt_path, 1)).unwrap();

    let index = built(root);
    let feature = find(&index, "feature", 1).expect("uncommitted deletion must surface");
    assert_eq!(feature.kind, ChangeKind::Deleted);
    assert!(
        feature.dirty,
        "working-tree removal without commit is dirty"
    );
    assert_eq!(
        feature.task.metadata.status(),
        Some(Status::InProgress),
        "pre-deletion version"
    );
    assert!(find(&index, "main", 1).is_some(), "main still carries t");
}

#[test]
fn a_criss_cross_history_does_not_mislabel_an_unchanged_task() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n",
    );
    write(&root.join("other.txt"), "o0\n");
    commit(root, "c0");
    git(root, &["branch", "a"]);
    git(root, &["branch", "b"]);

    // `a` edits the task; `b` edits an unrelated file. The two edits are disjoint, so the branches
    // merge without conflict and share two merge-bases (a1 and b1) — a criss-cross.
    git(root, &["checkout", "-q", "a"]);
    task(
        root,
        1,
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# T done\n",
    );
    commit(root, "a1: edit task");
    git(root, &["checkout", "-q", "b"]);
    write(&root.join("other.txt"), "oB\n");
    commit(root, "b1: edit other");
    git(root, &["checkout", "-q", "a"]);
    git(root, &["merge", "-q", "--no-edit", "b"]);
    git(root, &["checkout", "-q", "b"]);
    git(root, &["merge", "-q", "--no-edit", "a"]);
    // main fast-forwards to the merged tip: `t` is now identical on main and on `a`.
    git(root, &["checkout", "-q", "main"]);
    git(root, &["merge", "-q", "--ff-only", "b"]);

    let index = built(root);
    // `a`'s task blob matches one of the two merge-bases, so it is unchanged — not `Modified`,
    // which a single-arbitrary-merge-base comparison would wrongly report.
    assert!(
        find(&index, "a", 1).is_none(),
        "unchanged across a criss-cross must not read as modified: {:?}",
        index.matrix().cells
    );
    let main = find(&index, "main", 1).expect("main base row");
    assert_eq!(main.task.metadata.status(), Some(Status::Done));
}

#[test]
fn without_a_default_branch_headline_prefers_the_checked_out_branch() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // No main/master exists, so there is no default branch to anchor the headline.
    git(root, &["init", "-q", "-b", "alpha"]);
    identify(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n",
    );
    commit(root, "add t on alpha");
    git(root, &["checkout", "-q", "-b", "zeta"]);
    task(
        root,
        1,
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# T done\n",
    );
    commit(root, "edit t on zeta");
    // Serve root stays on `zeta`, which sorts after `alpha`.

    let index = built(root);
    let items = index.aggregated_tasks();
    let item = items.iter().find(|i| i.id == key(1)).expect("task t");
    // The headline reflects the checked-out branch (zeta), not the alphabetically-first (alpha).
    assert_eq!(item.metadata.status(), Some(Status::Done));
    assert_eq!(item.title, "T done");
}

#[test]
fn an_uncommitted_change_on_the_default_branch_is_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n",
    );
    commit(root, "add t");
    // Serve root is checked out on main, so its working copy overlays the base row.
    task(
        root,
        1,
        "---\nstatus: in_progress\ncreated: 2026-01-01T00:00:00Z\n---\n# T edited\n",
    );

    let index = built(root);
    let main = find(&index, "main", 1).expect("main base row");
    assert_eq!(main.kind, ChangeKind::Base);
    assert!(main.dirty, "uncommitted working-copy edit is dirty");
    assert_eq!(main.task.metadata.status(), Some(Status::InProgress));
    assert_eq!(main.task.title, "T edited");
}

#[test]
fn a_new_uncommitted_task_on_the_default_branch_appears() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# A\n",
    );
    commit(root, "add a");
    // A freshly created (still uncommitted) task must show up right away, not vanish until commit.
    task(
        root,
        2,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Fresh\n",
    );

    let index = built(root);
    let fresh = find(&index, "main", 2).expect("uncommitted new task row");
    assert_eq!(fresh.kind, ChangeKind::Base);
    assert!(fresh.dirty);
}

#[test]
fn oplan_default_branch_config_overrides_autodetect() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# A\n",
    );
    commit(root, "add a");
    git(root, &["checkout", "-q", "-b", "dev"]);
    task(
        root,
        1,
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# A done\n",
    );
    commit(root, "edit a on dev");
    git(root, &["checkout", "-q", "main"]);
    git(root, &["config", "oplan.defaultBranch", "dev"]);

    let index = built(root);

    // dev is now the baseline; main is unchanged since the fork, so it contributes nothing.
    let dev = find(&index, "dev", 1).expect("dev base row");
    assert_eq!(dev.kind, ChangeKind::Base);
    assert_eq!(dev.task.metadata.status(), Some(Status::Done));
    assert!(find(&index, "main", 1).is_none(), "main unchanged vs dev");
}

#[test]
fn without_a_default_branch_every_branch_is_a_presence_row() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "-b", "trunk"]);
    identify(root);
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# A\n",
    );
    commit(root, "add a");
    git(root, &["checkout", "-q", "-b", "other"]);
    task(
        root,
        1,
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# A done\n",
    );
    commit(root, "edit a on other");
    git(root, &["checkout", "-q", "trunk"]);

    let index = built(root);

    // No main/master and no config → nothing to diff against, so both branches list their tasks.
    let trunk = find(&index, "trunk", 1).expect("trunk row");
    let other = find(&index, "other", 1).expect("other row");
    assert_eq!(trunk.kind, ChangeKind::Base);
    assert_eq!(other.kind, ChangeKind::Base);
    assert_eq!(trunk.task.metadata.status(), Some(Status::Todo));
    assert_eq!(other.task.metadata.status(), Some(Status::Done));
}

#[test]
fn unparseable_frontmatter_does_not_abort_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    write(
        &task_path(root, 1),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Body\n\n## Plan\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> other\n",
    );
    write(
        &task_path(root, 2),
        "---\n<<<<<<< HEAD\nstatus: todo\n=======\nstatus: done\n>>>>>>> other\n---\n# Head\n",
    );
    commit(root, "broken frontmatter");

    let index = built(root);
    let cells = &index.matrix().cells;
    assert_eq!(cells.len(), 2, "rebuild kept both cells: {cells:?}");

    let body = cells.iter().find(|c| c.task.id == key(1)).unwrap();
    assert_eq!(body.task.title, "Body");
    let head = cells.iter().find(|c| c.task.id == key(2)).unwrap();
    assert_eq!(head.task.title, "Head", "best-effort title survives");
}

fn commit_at(root: &Path, seconds: i64, message: &str) {
    let date = format!("@{seconds} +0000");
    git(root, &["add", "-A"]);
    let status = Command::new("git")
        .current_dir(root)
        .args(["commit", "-qm", message])
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git commit failed");
}

fn commit_replayed(root: &Path, authored: i64, committed: i64, message: &str) {
    git(root, &["add", "-A"]);
    let status = Command::new("git")
        .current_dir(root)
        .args(["commit", "-qm", message])
        .env("GIT_AUTHOR_DATE", format!("@{authored} +0000"))
        .env("GIT_COMMITTER_DATE", format!("@{committed} +0000"))
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git commit failed");
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_second(seconds).unwrap()
}

// Older than the commit dates below, so the view's `updated = max(created, updated)` clamp does not
// mask what the history walk found.
const CREATED_SECONDS: i64 = 900_000_000;

fn dated(status: &str) -> String {
    format!(
        "---\nstatus: {status}\ncreated: {}\n---\n# T\n",
        at(CREATED_SECONDS)
    )
}

#[test]
fn updated_follows_the_last_commit_to_touch_the_task() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(root, 1, &dated("todo"));
    commit_at(root, 1_000_000_000, "add t");

    assert_eq!(
        built(root).task_updated(&key(1), None),
        Ok(at(1_000_000_000))
    );

    // A status flip is a blob change like any other, so it counts as an update.
    task(root, 1, &dated("done"));
    commit_at(root, 1_000_000_500, "finish t");

    let index = built(root);
    assert_eq!(index.task_updated(&key(1), None), Ok(at(1_000_000_500)));
    let view = index
        .effective_view(&Repo::discover(root).unwrap(), &key(1), "main")
        .unwrap()
        .unwrap();
    assert_eq!(view.updated, Field::Value(Rfc3339(at(1_000_000_500))));
    assert_eq!(view.metadata.created(), Some(at(CREATED_SECONDS)));
}

#[test]
fn an_uncommitted_edit_reads_as_updated_now() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(root, 1, &dated("todo"));
    commit_at(root, 1_000_000_000, "add t");
    let before = Timestamp::now();
    task(root, 1, &dated("done"));

    let updated = built(root).task_updated(&key(1), None).unwrap();

    assert!(
        updated >= before,
        "a working-tree edit belongs to no commit, so it reads as now: {updated}"
    );
}

#[test]
fn the_aggregated_list_dates_a_task_like_its_detail_does() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(root, 1, &dated("todo"));
    commit_at(root, 1_000_000_000, "add t");

    let index = built(root);
    let listed = index
        .aggregated_tasks()
        .into_iter()
        .find(|item| item.id == key(1))
        .unwrap();

    // The board renders `updated` from here, so it must agree with the task's own page.
    assert_eq!(listed.updated, Field::Value(Rfc3339(at(1_000_000_000))));
    assert_eq!(
        listed.updated,
        Field::from(index.task_updated(&key(1), None).map(Rfc3339))
    );
}

#[test]
fn the_aggregated_list_clamps_updated_up_to_created() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    // A hand-set `created` after the last commit: the list must not report an age older than the
    // task itself, the same backstop the detail view applies.
    task(
        root,
        1,
        "---\nstatus: todo\ncreated: 2030-01-01T00:00:00Z\n---\n# T\n",
    );
    commit_at(root, 1_000_000_000, "add t");

    let listed = built(root)
        .aggregated_tasks()
        .into_iter()
        .find(|item| item.id == key(1))
        .unwrap();

    assert_eq!(
        listed.updated,
        Field::Value(Rfc3339("2030-01-01T00:00:00Z".parse().unwrap()))
    );
}

// The case that made this model necessary: a file written before `created` existed is not corrupt,
// and must not be reported as if its status were `backlog`.
#[test]
fn a_field_the_strict_parser_rejects_costs_only_that_field() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(root, 1, &dated("todo"));
    task(
        root,
        2,
        "---\nstatus: in_progress\nparent: '1'\n---\n# Legacy\n",
    );
    commit_at(root, 1_000_000_000, "add tasks");

    let index = built(root);
    let cell = find(&index, "main", 2).unwrap();

    assert_eq!(cell.task.metadata.status(), Some(Status::InProgress));
    assert_eq!(cell.task.metadata.parent(), Some(key(1).as_str()));
    assert_eq!(cell.task.title, "Legacy");
    // Only `created` failed, and it says so rather than going missing silently.
    let fields = cell.task.metadata.fields().unwrap();
    assert_eq!(
        fields.created,
        op_api::Field::Error(op_api::FieldError::Missing)
    );
    assert_eq!(index.task_updated(&key(2), None), Ok(at(1_000_000_000)));
}

#[test]
fn a_file_with_no_readable_metadata_reports_that_instead_of_a_status() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        1,
        "---\n<<<<<<< HEAD\nstatus: todo\n=======\nstatus: done\n>>>>>>> other\n---\n# Broken\n",
    );
    commit_at(root, 1_000_000_000, "add broken");

    let index = built(root);
    let cell = find(&index, "main", 1).unwrap();

    assert_eq!(cell.task.metadata.status(), None, "no status is claimed");
    assert!(matches!(cell.task.metadata, op_api::Metadata::Error { .. }));
    assert_eq!(cell.task.title, "Broken", "best-effort title still shows");

    // The board gives it its own group rather than filing it under a status it never had.
    let board = op_api::Board::build(&index.aggregated_tasks());
    assert_eq!(board.groups[0].status, None);
    assert_eq!(board.groups[0].rows[0].task.id, key(1));
}

#[test]
fn a_replayed_branch_does_not_outrank_newer_work_elsewhere() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(root, 1, &dated("todo"));
    commit_at(root, 1_000_000_000, "add t");

    // `feat` does the newest real work on the task.
    git(root, &["checkout", "-qb", "feat"]);
    task(root, 1, &dated("in_review"));
    commit_at(root, 1_000_200_000, "feat moves t forward");

    // `chore` only touches t incidentally — a backfill, a reformat — and is then rebased, which
    // rewrites its commit date to now while leaving the author date it was written with.
    git(root, &["checkout", "-q", "main"]);
    git(root, &["checkout", "-qb", "chore"]);
    task(root, 1, &dated("backlog"));
    commit_replayed(
        root,
        1_000_100_000,
        1_000_900_000,
        "chore rewrites t (replayed)",
    );

    let listed = built(root)
        .aggregated_tasks()
        .into_iter()
        .find(|item| item.id == key(1))
        .unwrap();

    // Ranking by commit date would headline `chore` — and every other task it carries — purely
    // because it was the last branch replayed.
    assert_eq!(listed.headline, "feat");
    assert_eq!(listed.metadata.status(), Some(Status::InReview));
}

#[test]
fn a_rebased_branch_headlines_over_the_branch_it_was_rebased_onto() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(root, 1, &dated("todo"));
    commit_at(root, 1_000_000_000, "add t");

    // `feat` writes its version first, `main` writes a different one after it.
    git(root, &["checkout", "-qb", "feat"]);
    task(root, 1, &dated("in_progress"));
    commit_at(root, 1_000_100_000, "feat moves t forward");
    git(root, &["checkout", "-q", "main"]);
    task(root, 1, &dated("done"));
    commit_at(root, 1_000_200_000, "main closes t");

    // Rebasing `feat` replays its edit over main's and the conflict is resolved by hand, so
    // `feat` now holds main's version *and* the resolution — while still carrying the author date
    // it was originally written with, older than the version it just superseded.
    git(root, &["checkout", "-q", "feat"]);
    rebase_resolving(root, "main", 1, &dated("in_progress"), 1_000_900_000);

    let listed = built(root)
        .aggregated_tasks()
        .into_iter()
        .find(|item| item.id == key(1))
        .unwrap();

    // Ranking by author date would headline `main`, showing the version the resolution superseded.
    assert_eq!(listed.headline, "feat");
    assert_eq!(listed.metadata.status(), Some(Status::InProgress));
}

#[test]
fn a_superseded_version_stays_beaten_when_a_third_branch_is_newer() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(root, 1, &dated("todo"));
    commit_at(root, 1_000_000_000, "add t");

    git(root, &["checkout", "-qb", "aa"]);
    task(root, 1, &dated("in_progress"));
    commit_at(root, 1_000_100_000, "aa moves t forward");

    // `bb` knows nothing of the other two: no version supersedes it and none of it.
    git(root, &["checkout", "-q", "main"]);
    git(root, &["checkout", "-qb", "bb"]);
    task(root, 1, &dated("in_review"));
    commit_at(root, 1_000_150_000, "bb moves t elsewhere");

    git(root, &["checkout", "-q", "main"]);
    task(root, 1, &dated("done"));
    commit_at(root, 1_000_200_000, "main closes t");
    git(root, &["checkout", "-q", "aa"]);
    rebase_resolving(root, "main", 1, &dated("in_progress"), 1_000_900_000);

    let listed = built(root)
        .aggregated_tasks()
        .into_iter()
        .find(|item| item.id == key(1))
        .unwrap();

    // `aa` supersedes `main` and only loses to `bb` on date, so dropping the superseded versions
    // has to come first: weighing one pair at a time lets `bb` knock `aa` out and `main` — the
    // version `aa` already buried — take the row.
    assert_eq!(listed.headline, "bb");
    assert_eq!(listed.metadata.status(), Some(Status::InReview));
}

fn rebase_resolving(root: &Path, onto: &str, number: u64, resolved: &str, committed: i64) {
    let conflicted = Command::new("git")
        .current_dir(root)
        .args(["rebase", onto])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("git must be installed for this test");
    assert!(
        !conflicted.success(),
        "the rebase must conflict for the resolution to be the branch's own version"
    );
    write(&task_path(root, number), resolved);
    git(root, &["add", "-A"]);
    let status = Command::new("git")
        .current_dir(root)
        .args(["rebase", "--continue"])
        .env("GIT_EDITOR", "true")
        .env("GIT_COMMITTER_DATE", format!("@{committed} +0000"))
        .status()
        .expect("git must be installed for this test");
    assert!(status.success(), "git rebase --continue failed");
}

#[test]
fn the_id_floor_counts_a_task_no_commit_holds_yet() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        7,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
    );

    // Nothing is committed, so HEAD is unborn and there are no branches to walk. Reading the floor
    // from branch tips alone would call 7 free and mint a second task 7 on the next create.
    assert_eq!(built(root).max_id(), Some(7));
}

#[test]
fn the_id_floor_counts_a_worktree_mid_merge() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    task(
        root,
        7,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
    );
    commit(root, "add alpha");
    git(root, &["branch", "feature"]);

    let wt = tempfile::tempdir().unwrap();
    let wt_path = wt.path().join("feature");
    git(
        root,
        &[
            "worktree",
            "add",
            "-q",
            wt_path.to_str().unwrap(),
            "feature",
        ],
    );
    task(
        &wt_path,
        9,
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Beta\n",
    );
    // A merge in flight keeps the worktree out of `live`, so its files reach the index nowhere
    // else; the number is taken all the same, and the merge will commit it.
    write(&root.join(".git/worktrees/feature/MERGE_HEAD"), "");

    assert_eq!(built(root).max_id(), Some(9));
}

#[test]
fn ids_are_ordered_as_numbers_not_as_text() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    for number in [1, 2, 9, 10, 11, 100] {
        task(
            root,
            number,
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# T\n",
        );
    }
    commit(root, "add tasks");

    let index = built(root);
    let expected: Vec<String> = [1, 2, 9, 10, 11, 100].map(key).to_vec();
    assert_eq!(
        index
            .aggregated_tasks()
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        index
            .branch_summaries("main")
            .iter()
            .map(|summary| summary.id.clone())
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn two_files_of_one_number_resolve_to_the_lowest_name() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init(root);
    // A hand-made state a merge can produce: one task renamed on one side, both names kept. The
    // store and the git reader must pick the same file, or a read and a write disagree.
    write(
        &root.join(".plan/tasks/00007-alpha.md"),
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
    );
    write(
        &root.join(".plan/tasks/00007-beta.md"),
        "---\nstatus: done\ncreated: 2026-01-01T00:00:00Z\n---\n# Beta\n",
    );
    commit(root, "add both names");

    let items = built(root).aggregated_tasks();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, key(7));
    assert_eq!(items[0].title, "Alpha");
    assert_eq!(
        Store::discover(root).unwrap().read(7).unwrap().title(),
        Some("Alpha".to_owned())
    );
}
