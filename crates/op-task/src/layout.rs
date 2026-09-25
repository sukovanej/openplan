use crate::{file_id, task_filename};

pub const CONFIG: &str = "config.toml";
pub const TASKS: &str = "tasks";
pub const TAGS: &str = "tags";
pub const ASSETS: &str = "assets";

const MARKDOWN: &str = ".md";

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Document {
    Config,
    Task(u64),
    Tag(String),
    Asset(String),
    Other(String),
}

impl Document {
    pub fn of(path: &str) -> Self {
        if path == CONFIG {
            return Self::Config;
        }
        if let Some(number) = task_number(path) {
            return Self::Task(number);
        }
        if let Some(name) = tag_name(path) {
            return Self::Tag(name.to_owned());
        }
        if let Some(name) = path
            .strip_prefix(ASSETS)
            .and_then(|rest| rest.strip_prefix('/'))
        {
            return Self::Asset(name.to_owned());
        }
        Self::Other(path.to_owned())
    }
}

pub fn task_path(number: u64, title: &str) -> String {
    format!("{TASKS}/{}", task_filename(number, title))
}

pub fn task_number(path: &str) -> Option<u64> {
    let stem = path
        .strip_prefix(TASKS)?
        .strip_prefix('/')?
        .strip_suffix(MARKDOWN)?;
    if stem.contains('/') {
        return None;
    }
    file_id(stem)
}

// Every path a task's file can have, whatever its slug says.
pub fn task_prefix(number: u64) -> String {
    let named = task_filename(number, "x");
    let digits = named
        .split_once('-')
        .map_or(named.as_str(), |(digits, _)| digits);
    format!("{TASKS}/{digits}-")
}

pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn tag_path(name: &str) -> String {
    format!("{TAGS}/{name}{MARKDOWN}")
}

pub fn tag_name(path: &str) -> Option<&str> {
    let name = path
        .strip_prefix(TAGS)?
        .strip_prefix('/')?
        .strip_suffix(MARKDOWN)?;
    (!name.contains('/')).then_some(name)
}
