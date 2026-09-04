use std::fs;
use std::path::{Path, PathBuf};

use op_lint::{Code, Diagnostic, Snapshot, lint, write_skill};
use op_task::Abbreviation;

fn abbr() -> Abbreviation {
    "OPP".parse().unwrap()
}

fn snapshot(root: &Path) -> Snapshot {
    Snapshot::from_files(root, abbr(), Vec::new()).with_installed_skills()
}

fn skill_path(root: &Path, agent_dir: &str, name: &str) -> PathBuf {
    root.join(agent_dir).join(name).join("SKILL.md")
}

fn install(root: &Path, agents: &[op_skills::Agent]) {
    op_skills::setup(root, agents).unwrap();
}

fn only(diags: Vec<Diagnostic>) -> Diagnostic {
    assert_eq!(diags.len(), 1, "expected one diagnostic, got {diags:#?}");
    diags.into_iter().next().unwrap()
}

#[test]
fn installed_skills_that_match_the_binary_lint_clean() {
    let root = tempfile::tempdir().unwrap();
    install(root.path(), &op_skills::Agent::ALL);
    let personal = skill_path(root.path(), ".claude/skills", "personal");
    fs::create_dir_all(personal.parent().unwrap()).unwrap();
    fs::write(&personal, "mine").unwrap();

    assert_eq!(lint(&snapshot(root.path())), Vec::new());
}

// A repository that never ran `setup-skills` for an agent owes that agent nothing, so an absent
// skills directory is not a missing skill.
#[test]
fn an_agent_without_a_skills_directory_is_not_reported() {
    let root = tempfile::tempdir().unwrap();
    install(root.path(), &[op_skills::Agent::Claude]);

    assert_eq!(lint(&snapshot(root.path())), Vec::new());
    assert!(!root.path().join(".agents").exists());
}

#[test]
fn an_edited_skill_is_reported_and_repaired() {
    let root = tempfile::tempdir().unwrap();
    install(root.path(), &[op_skills::Agent::Claude]);
    let path = skill_path(root.path(), ".claude/skills", "task-management");
    fs::write(&path, "hand-written\n").unwrap();

    let before = snapshot(root.path());
    let diagnostic = only(lint(&before));
    assert_eq!(diagnostic.code, Code::Skill);
    assert_eq!(diagnostic.path, path);
    assert!(diagnostic.message.contains("differs"), "{diagnostic}");
    assert!(diagnostic.fixable);

    for skill in before.skills() {
        write_skill(skill).unwrap();
    }
    assert_eq!(lint(&snapshot(root.path())), Vec::new());
    assert_ne!(fs::read_to_string(&path).unwrap(), "hand-written\n");
}

#[test]
fn a_missing_skill_is_reported_and_repaired() {
    let root = tempfile::tempdir().unwrap();
    install(root.path(), &[op_skills::Agent::Codex]);
    let path = skill_path(root.path(), ".agents/skills", "task-comments");
    fs::remove_file(&path).unwrap();

    let before = snapshot(root.path());
    let diagnostic = only(lint(&before));
    assert_eq!(diagnostic.code, Code::Skill);
    assert_eq!(diagnostic.path, path);
    assert!(diagnostic.message.contains("missing"), "{diagnostic}");

    for skill in before.skills() {
        write_skill(skill).unwrap();
    }
    assert_eq!(lint(&snapshot(root.path())), Vec::new());
}
