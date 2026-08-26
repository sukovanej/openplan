use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    fn skills_dir(self, root: &Path) -> PathBuf {
        match self {
            Self::Claude => root.join(".claude/skills"),
            Self::Codex => root.join(".agents/skills"),
        }
    }
}

struct Skill {
    name: &'static str,
    contents: &'static str,
}

const SKILLS: &[Skill] = &[
    Skill {
        name: "task-comments",
        contents: include_str!("../skills/task-comments/SKILL.md"),
    },
    Skill {
        name: "task-management-merge",
        contents: include_str!("../skills/task-management-merge/SKILL.md"),
    },
    Skill {
        name: "task-management",
        contents: include_str!("../skills/task-management/SKILL.md"),
    },
];

pub fn setup(root: &Path, agents: &[Agent]) -> Result<()> {
    for agent in agents {
        let skills_dir = agent.skills_dir(root);
        for skill in SKILLS {
            let skill_dir = skills_dir.join(skill.name);
            fs::create_dir_all(&skill_dir)
                .with_context(|| format!("create {}", skill_dir.display()))?;
            let path = skill_dir.join("SKILL.md");
            fs::write(&path, skill.contents)
                .with_context(|| format!("write {}", path.display()))?;
        }
    }
    Ok(())
}
