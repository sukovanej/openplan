use std::fs;

use op_skills::Agent;

#[test]
fn setup_installs_all_embedded_skills_for_each_agent() {
    let root = tempfile::tempdir().unwrap();

    op_skills::setup(root.path(), &[Agent::Claude, Agent::Codex]).unwrap();

    for agent_dir in [".claude/skills", ".agents/skills"] {
        for skill in ["task-comments", "task-management-merge", "task-management"] {
            let contents =
                fs::read_to_string(root.path().join(agent_dir).join(skill).join("SKILL.md"))
                    .unwrap();
            assert!(contents.starts_with("---\nname: "));
        }
    }
}

#[test]
fn setup_updates_owned_skills_and_preserves_other_skills() {
    let root = tempfile::tempdir().unwrap();
    let skills = root.path().join(".claude/skills");
    fs::create_dir_all(skills.join("task-management")).unwrap();
    fs::write(skills.join("task-management/SKILL.md"), "old").unwrap();
    fs::create_dir_all(skills.join("personal")).unwrap();
    fs::write(skills.join("personal/SKILL.md"), "keep").unwrap();

    op_skills::setup(root.path(), &[Agent::Claude]).unwrap();

    assert_ne!(
        fs::read_to_string(skills.join("task-management/SKILL.md")).unwrap(),
        "old"
    );
    assert_eq!(
        fs::read_to_string(skills.join("personal/SKILL.md")).unwrap(),
        "keep"
    );
    assert!(!root.path().join(".agents").exists());
}
