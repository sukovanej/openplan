use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    fn skills_dir(self, root: &Path) -> PathBuf {
        match self {
            Self::Claude => root.join(".claude/skills"),
            Self::Codex => root.join(".agents/skills"),
        }
    }

    fn skill_path(self, root: &Path, skill: &Skill) -> PathBuf {
        self.skills_dir(root).join(skill.name).join(FILE_NAME)
    }
}

struct Skill {
    name: &'static str,
    contents: &'static str,
}

const FILE_NAME: &str = "SKILL.md";

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

// `source` holds bytes, not text: a file the caller edited into something that is not UTF-8 still
// answers "this differs from the binary" rather than failing the read.
#[derive(Debug, Clone)]
pub struct SkillFile {
    pub name: &'static str,
    pub path: PathBuf,
    pub contents: &'static str,
    pub source: Option<Vec<u8>>,
}

impl SkillFile {
    pub fn matches(&self) -> bool {
        self.source.as_deref() == Some(self.contents.as_bytes())
    }
}

// An agent owns these skills once one of them is written to it. A skills directory alone proves
// nothing — `.claude/skills/` is where a repository keeps its own skills, and a repository that
// never ran `setup-skills` owes this binary no file.
pub fn installed(root: &Path) -> io::Result<Vec<SkillFile>> {
    let mut files = Vec::new();
    for agent in Agent::ALL {
        let mut of_agent = Vec::new();
        for skill in SKILLS {
            let path = agent.skill_path(root, skill);
            of_agent.push(SkillFile {
                name: skill.name,
                source: read(&path)?,
                contents: skill.contents,
                path,
            });
        }
        if of_agent.iter().any(|file| file.source.is_some()) {
            files.append(&mut of_agent);
        }
    }
    Ok(files)
}

pub fn install(file: &SkillFile) -> io::Result<()> {
    write(&file.path, file.contents)
}

pub fn setup(root: &Path, agents: &[Agent]) -> Result<()> {
    for agent in agents {
        for skill in SKILLS {
            let path = agent.skill_path(root, skill);
            write(&path, skill.contents).with_context(|| format!("write {}", path.display()))?;
        }
    }
    Ok(())
}

fn read(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(source) => Ok(Some(source)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

// An agent reads a skill file whenever it starts, so a write that truncates the file first hands
// that agent an empty skill. The rename is what the reader sees, and it sees all of it.
fn write(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let temp = path.with_file_name(format!(".{FILE_NAME}.{}.tmp", std::process::id()));
    fs::write(&temp, contents)?;
    fs::rename(&temp, path)
}
