use std::path::{Path, PathBuf};
use std::process::Command;

use op_lint::{Code, CreatedSource, Snapshot, Uncommitted, fix, lint};
use op_task::{Abbreviation, PartialMetadata, Timestamp, parse_partial};

const ROOT: &str = "/repo";
const TARGET_NAME: &str = "00042-write-the-parser.md";
const TARGET_SRC: &str =
    "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Write the parser\n";

fn abbr() -> Abbreviation {
    "OPP".parse().unwrap()
}

fn tpath(name: &str) -> PathBuf {
    Path::new(ROOT).join(".plan/tasks").join(name)
}

fn snapshot(files: &[(PathBuf, String)]) -> Snapshot {
    Snapshot::from_files(PathBuf::from(ROOT), abbr(), files.to_vec())
}

// A ref written in a non-canonical but resolvable spelling round-trips: fix rewrites it to the
// target's file form, a re-lint is clean, and a second fix is byte-identical.
fn assert_reference_round_trips(referrer_name: &str, referrer_src: &str) {
    let referrer = tpath(referrer_name);
    let target = tpath(TARGET_NAME);
    let files = vec![
        (target.clone(), TARGET_SRC.to_owned()),
        (referrer.clone(), referrer_src.to_owned()),
    ];
    let snap = snapshot(&files);

    let before = lint(&snap);
    assert!(
        before.iter().any(|d| d.path == referrer && d.fixable),
        "expected a fixable diagnostic on {referrer_name} before fixing, got {before:?}"
    );

    let fixed = fix(&snap, &Uncommitted);
    let fixed_src = fixed[&referrer].clone();
    assert_ne!(
        fixed_src, referrer_src,
        "fix should rewrite the reference in {referrer_name}"
    );
    assert_eq!(
        fixed[&target], TARGET_SRC,
        "the referenced file must be left untouched"
    );

    let refixed = snapshot(&fixed.into_iter().collect::<Vec<_>>());
    let after = lint(&refixed);
    assert!(
        !after.iter().any(|d| d.path == referrer),
        "re-lint after fixing {referrer_name} should report nothing for it, got {after:?}"
    );

    let again = fix(&refixed, &Uncommitted);
    assert_eq!(
        again[&referrer], fixed_src,
        "fixing {referrer_name} a second time must be byte-identical"
    );
}

// A diagnostic with more than one valid repair is reported but never touched by fix.
fn assert_left_untouched(name: &str, src: &str, others: &[(PathBuf, String)]) {
    let path = tpath(name);
    let mut files = vec![(path.clone(), src.to_owned())];
    files.extend(others.iter().cloned());
    let snap = snapshot(&files);

    let before = lint(&snap);
    assert!(
        before.iter().any(|d| d.path == path),
        "expected a diagnostic on {name}, got {before:?}"
    );

    let fixed = fix(&snap, &Uncommitted);
    assert_eq!(
        fixed[&path], src,
        "the non-fixable case {name} must be left byte-identical"
    );
}

#[test]
fn parent_bare_number_canonicalizes() {
    assert_reference_round_trips(
        "00007-bare-number-parent.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: 42\n---\n# Child\n",
    );
}

#[test]
fn body_ref_missing_dot_slash_canonicalizes() {
    assert_reference_round_trips(
        "00008-missing-dot-slash.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Child\n\nSee [[00042-write-the-parser.md]] for details.\n",
    );
}

#[test]
fn stale_slug_canonicalizes() {
    assert_reference_round_trips(
        "00009-stale-slug.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: ./00042-old-name.md\n---\n# Child\n",
    );
}

#[test]
fn body_key_form_canonicalizes() {
    assert_reference_round_trips(
        "00010-key-form-body.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Child\n\nBlocked by [[OPP-42]].\n",
    );
}

#[test]
fn dangling_ref_left_untouched() {
    assert_left_untouched(
        "00011-dangling.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: 99\n---\n# Orphaned parent\n",
        &[],
    );
}

#[test]
fn invalid_status_left_untouched() {
    assert_left_untouched(
        "00012-bad-status.md",
        "---\nstatus: nonsense\ncreated: 2026-01-01T00:00:00Z\n---\n# Bad status\n",
        &[],
    );
}

#[test]
fn duplicate_h1_left_untouched() {
    assert_left_untouched(
        "00013-duplicate-h1.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# First title\n\n# Second title\n",
        &[],
    );
}

#[test]
fn key_form_parent_left_untouched() {
    assert_left_untouched(
        "00014-key-parent.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: OPP-42\n---\n# Key-form parent\n",
        &[(tpath(TARGET_NAME), TARGET_SRC.to_owned())],
    );
}

#[test]
fn uncommitted_missing_created_left_untouched() {
    assert_left_untouched(
        "00015-uncommitted-created.md",
        "---\nstatus: todo\n---\n# No created, no commit\n",
        &[],
    );
}

struct FirstCommitDates {
    repo: op_git::Repo,
    root: PathBuf,
}

impl CreatedSource for FirstCommitDates {
    fn created(&self, path: &Path) -> Option<Timestamp> {
        let relative = path.strip_prefix(&self.root).ok()?;
        self.repo.first_commit(relative).ok().flatten()?.at.ok()
    }
}

fn git(dir: &Path, args: &[&str], envs: &[(&str, &str)]) {
    let mut command = Command::new("git");
    command.current_dir(dir).args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    let status = command.status().expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

#[test]
fn created_backfill_from_first_commit_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();

    let tasks = root.join(".plan/tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    std::fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    let file = tasks.join("00042-backfill-me.md");
    std::fs::write(&file, "---\nstatus: todo\n---\n# Backfill me\n").unwrap();

    // git records the author date only from these env vars, so the first commit's time is what the
    // backfill must reproduce.
    let date = "2026-01-15T08:30:00Z";
    git(&root, &["init", "-q"], &[]);
    git(&root, &["config", "user.email", "t@example.com"], &[]);
    git(&root, &["config", "user.name", "Test"], &[]);
    git(&root, &["add", "."], &[]);
    git(
        &root,
        &["commit", "-q", "-m", "add task"],
        &[("GIT_AUTHOR_DATE", date), ("GIT_COMMITTER_DATE", date)],
    );

    let store = op_store::Store::open(&root, abbr()).unwrap();
    let snap = Snapshot::from_store(&store).unwrap();
    let source = FirstCommitDates {
        repo: op_git::Repo::discover(&root).unwrap(),
        root: root.clone(),
    };

    let before = lint(&snap);
    assert!(
        before
            .iter()
            .any(|d| d.path == file && d.code == Code::Created),
        "expected a Created diagnostic before backfill, got {before:?}"
    );

    let fixed = fix(&snap, &source);
    let fixed_src = fixed[&file].clone();

    let expected: Timestamp = date.parse().unwrap();
    match parse_partial(&fixed_src).metadata {
        PartialMetadata::Fields(fields) => assert_eq!(
            fields.created,
            Ok(expected),
            "created must be backfilled from the first commit's author time"
        ),
        other => panic!("expected parsed frontmatter fields, got {other:?}"),
    }

    let refixed = Snapshot::from_files(root.clone(), abbr(), fixed);
    let after = lint(&refixed);
    assert!(
        !after
            .iter()
            .any(|d| d.path == file && d.code == Code::Created),
        "the Created diagnostic must be gone after backfill, got {after:?}"
    );

    let again = fix(&refixed, &source);
    assert_eq!(
        again[&file], fixed_src,
        "backfilling a second time must be byte-identical"
    );
}
