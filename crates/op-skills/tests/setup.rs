use std::fs;

use op_skills::Agent;

#[test]
fn setup_installs_all_embedded_skills_for_each_agent() {
    let root = tempfile::tempdir().unwrap();

    op_skills::setup(root.path(), &[Agent::Claude, Agent::Codex]).unwrap();

    for agent_dir in [".claude/skills", ".agents/skills"] {
        for skill in ["openplan", "openplan-docs"] {
            let path = root.path().join(agent_dir).join(skill).join("SKILL.md");
            let contents = fs::read_to_string(path).unwrap();
            assert!(contents.starts_with(&format!("---\nname: {skill}\n")));
        }
    }
}

#[test]
fn setup_updates_owned_skills_and_preserves_other_skills() {
    let root = tempfile::tempdir().unwrap();
    let skills = root.path().join(".claude/skills");
    fs::create_dir_all(skills.join("openplan")).unwrap();
    fs::write(skills.join("openplan/SKILL.md"), "old").unwrap();
    fs::create_dir_all(skills.join("personal")).unwrap();
    fs::write(skills.join("personal/SKILL.md"), "keep").unwrap();

    op_skills::setup(root.path(), &[Agent::Claude]).unwrap();

    assert_ne!(
        fs::read_to_string(skills.join("openplan/SKILL.md")).unwrap(),
        "old"
    );
    assert_eq!(
        fs::read_to_string(skills.join("personal/SKILL.md")).unwrap(),
        "keep"
    );
    assert!(!root.path().join(".agents").exists());
}

#[test]
fn setup_removes_retired_skills_and_keeps_the_users_files_beside_them() {
    let root = tempfile::tempdir().unwrap();
    let skills = root.path().join(".claude/skills");
    for retired in ["task-comments", "task-management-merge", "task-management"] {
        fs::create_dir_all(skills.join(retired)).unwrap();
        fs::write(skills.join(retired).join("SKILL.md"), "old").unwrap();
    }
    fs::write(skills.join("task-comments/notes.md"), "keep").unwrap();

    op_skills::setup(root.path(), &[Agent::Claude]).unwrap();

    assert!(!skills.join("task-management").exists());
    assert!(!skills.join("task-management-merge").exists());
    assert!(!skills.join("task-comments/SKILL.md").exists());
    assert_eq!(
        fs::read_to_string(skills.join("task-comments/notes.md")).unwrap(),
        "keep"
    );
    assert!(
        op_skills::installed(root.path())
            .unwrap()
            .iter()
            .all(|file| file.matches())
    );
}

#[test]
fn setup_names_the_browser_of_each_agent() {
    let root = tempfile::tempdir().unwrap();

    op_skills::setup(root.path(), &[Agent::Claude, Agent::Codex]).unwrap();

    for skill in ["openplan", "openplan-docs"] {
        let read = |agent_dir: &str| {
            fs::read_to_string(root.path().join(agent_dir).join(skill).join("SKILL.md")).unwrap()
        };
        let claude = read(".claude/skills");
        let codex = read(".agents/skills");
        assert!(claude.contains("`mcp__Claude_Browser__navigate`"));
        assert!(codex.contains("`@Browser`"));
        assert!(!codex.contains("Claude"));
        for skill in [claude, codex] {
            assert!(!skill.contains("{{"));
        }
    }
}
