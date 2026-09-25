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
    expected: Expected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expected {
    Contents(&'static str),
    Retired,
}

const FILE_NAME: &str = "SKILL.md";

// Releases up to 0.0.3 split the openplan skill in three. An agent that kept those files would read
// two sets of rules, so `setup` removes them.
const SKILLS: &[Skill] = &[
    Skill {
        name: "openplan",
        expected: Expected::Contents(include_str!("../skills/openplan/SKILL.md")),
    },
    Skill {
        name: "task-comments",
        expected: Expected::Retired,
    },
    Skill {
        name: "task-management-merge",
        expected: Expected::Retired,
    },
    Skill {
        name: "task-management",
        expected: Expected::Retired,
    },
];

// `source` holds bytes, not text: a file the caller edited into something that is not UTF-8 still
// answers "this differs from the binary" rather than failing the read.
#[derive(Debug, Clone)]
pub struct SkillFile {
    pub name: &'static str,
    pub path: PathBuf,
    pub expected: Expected,
    pub source: Option<Vec<u8>>,
}

impl SkillFile {
    pub fn matches(&self) -> bool {
        match self.expected {
            Expected::Contents(contents) => self.source.as_deref() == Some(contents.as_bytes()),
            Expected::Retired => self.source.is_none(),
        }
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
                expected: skill.expected,
                path,
            });
        }
        if of_agent.iter().any(|file| file.source.is_some()) {
            files.append(&mut of_agent);
        }
    }
    Ok(files)
}

pub fn setup(root: &Path, agents: &[Agent]) -> Result<()> {
    for agent in agents {
        for skill in SKILLS {
            let path = agent.skill_path(root, skill);
            match skill.expected {
                Expected::Contents(contents) => {
                    write(&path, contents).with_context(|| format!("write {}", path.display()))
                }
                Expected::Retired => {
                    remove(&path).with_context(|| format!("remove {}", path.display()))
                }
            }?;
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

// The user may keep files of their own beside a retired skill, and those keep its directory.
fn remove(path: &Path) -> io::Result<()> {
    if let Err(error) = fs::remove_file(path) {
        return match error.kind() {
            io::ErrorKind::NotFound => Ok(()),
            _ => Err(error),
        };
    }
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    match fs::remove_dir(dir) {
        Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => Ok(()),
        result => result,
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
