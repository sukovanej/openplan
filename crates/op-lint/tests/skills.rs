use std::fs;
use std::path::{Path, PathBuf};

use op_lint::{Code, Diagnostic, Snapshot, Uncommitted, lint};
use op_skills::Agent;
use op_store::Store;
use op_task::Abbreviation;

fn abbr() -> Abbreviation {
    "OPP".parse().unwrap()
}

fn snapshot(root: &Path) -> Snapshot {
    Snapshot::from_files(root, abbr(), Vec::new()).with_skills(op_skills::installed(root).unwrap())
}

fn skill_path(root: &Path, agent_dir: &str, name: &str) -> PathBuf {
    root.join(agent_dir).join(name).join("SKILL.md")
}

fn only(diags: Vec<Diagnostic>) -> Diagnostic {
    assert_eq!(diags.len(), 1, "expected one diagnostic, got {diags:#?}");
    diags.into_iter().next().unwrap()
}

fn install_all(root: &Path) {
    op_skills::setup(root, &Agent::ALL).unwrap();
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn installed_skills_that_match_the_binary_lint_clean() {
    let root = tempfile::tempdir().unwrap();
    install_all(root.path());
    write(
        &skill_path(root.path(), ".claude/skills", "personal"),
        "mine",
    );

    assert_eq!(lint(&snapshot(root.path())), Vec::new());
}

// `.claude/skills/` is where a repository keeps its own skills. A directory of them is not an
// install of this binary's skills, and lint must not report three files the repository never asked
// for — nor may `--fix` write them.
#[test]
fn a_skills_directory_of_other_skills_is_not_an_install() {
    let root = tempfile::tempdir().unwrap();
    write(
        &skill_path(root.path(), ".claude/skills", "personal"),
        "mine",
    );

    assert_eq!(lint(&snapshot(root.path())), Vec::new());
}

#[test]
fn an_agent_without_a_skills_directory_is_not_reported() {
    let root = tempfile::tempdir().unwrap();
    op_skills::setup(root.path(), &[Agent::Claude]).unwrap();

    assert_eq!(lint(&snapshot(root.path())), Vec::new());
    assert!(!root.path().join(".agents").exists());
}

#[test]
fn an_edited_skill_is_reported_and_repaired() {
    let root = tempfile::tempdir().unwrap();
    op_skills::setup(root.path(), &[Agent::Claude]).unwrap();
    let path = skill_path(root.path(), ".claude/skills", "task-management");
    fs::write(&path, "hand-written\n").unwrap();

    let diagnostic = only(lint(&snapshot(root.path())));
    assert_eq!(diagnostic.code, Code::Skill);
    assert_eq!(diagnostic.path, path);
    assert!(diagnostic.message.contains("differs"), "{diagnostic}");
    assert!(diagnostic.fixable);

    repair(root.path());
    assert_eq!(lint(&snapshot(root.path())), Vec::new());
    assert_ne!(fs::read_to_string(&path).unwrap(), "hand-written\n");
}

// A file that is not text still differs from the skill the binary carries; reading it must not
// report it absent, and must not fail the whole lint.
#[test]
fn a_skill_that_is_not_text_differs() {
    let root = tempfile::tempdir().unwrap();
    op_skills::setup(root.path(), &[Agent::Claude]).unwrap();
    let path = skill_path(root.path(), ".claude/skills", "task-management");
    fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();

    let diagnostic = only(lint(&snapshot(root.path())));
    assert!(diagnostic.message.contains("differs"), "{diagnostic}");
}

#[test]
fn a_missing_skill_of_an_installed_agent_is_reported_and_repaired() {
    let root = tempfile::tempdir().unwrap();
    op_skills::setup(root.path(), &[Agent::Codex]).unwrap();
    let path = skill_path(root.path(), ".agents/skills", "task-comments");
    fs::remove_file(&path).unwrap();

    let diagnostic = only(lint(&snapshot(root.path())));
    assert_eq!(diagnostic.code, Code::Skill);
    assert_eq!(diagnostic.path, path);
    assert!(diagnostic.message.contains("missing"), "{diagnostic}");

    repair(root.path());
    assert_eq!(lint(&snapshot(root.path())), Vec::new());
}

// The repair writes the skills that drifted and no others: an untouched file keeps the bytes and
// the modification time it had.
#[test]
fn a_repair_rewrites_only_the_skill_that_drifted() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join(".plan/tasks")).unwrap();
    fs::write(root.join(".plan/config.toml"), "abbreviation = \"OPP\"\n").unwrap();
    install_all(root);
    let drifted = skill_path(root, ".claude/skills", "task-management");
    fs::write(&drifted, "hand-written\n").unwrap();

    let store = Store::discover(root).unwrap();
    let changed = op_lint::fix_store(&store, &Uncommitted).unwrap();

    assert_eq!(changed.len(), 1, "{changed:?}");
    assert!(
        changed[0].ends_with("task-management/SKILL.md"),
        "{changed:?}"
    );
}

fn repair(root: &Path) {
    for skill in snapshot(root).skills() {
        if !skill.matches() {
            op_skills::install(skill).unwrap();
        }
    }
}
