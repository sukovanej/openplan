use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    pub fn skills_dir(self, root: &Path) -> PathBuf {
        match self {
            Self::Claude => root.join(".claude/skills"),
            Self::Codex => root.join(".agents/skills"),
        }
    }

    pub fn skill_path(self, root: &Path, skill: &Skill) -> PathBuf {
        self.skills_dir(root).join(skill.name).join(FILE_NAME)
    }
}

pub struct Skill {
    pub name: &'static str,
    pub contents: &'static str,
}

const FILE_NAME: &str = "SKILL.md";

pub const SKILLS: &[Skill] = &[
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

pub fn installed(root: &Path) -> Vec<Agent> {
    Agent::ALL
        .into_iter()
        .filter(|agent| agent.skills_dir(root).is_dir())
        .collect()
}

pub fn setup(root: &Path, agents: &[Agent]) -> Result<()> {
    for agent in agents {
        for skill in SKILLS {
            write(&agent.skill_path(root, skill), skill.contents)?;
        }
    }
    Ok(())
}

pub fn write(path: &Path, contents: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("write {}", path.display()))
}
