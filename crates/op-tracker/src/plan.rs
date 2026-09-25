use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use op_backend::{RevisionId, Snapshot};
use op_task::config::{Config, ConfigError};
use op_task::layout::{self, Document};
use op_task::tag::{Tag, normalize_name};
use op_task::{Abbreviation, Task};

use crate::TrackerError;

// The tasks and tags of one revision, typed. Cheap to hold: the documents stay in the snapshot
// until a caller reads one.
#[derive(Clone)]
pub struct Plan {
    snapshot: Arc<dyn Snapshot>,
    config: Option<Result<Config, ConfigError>>,
    tasks: BTreeMap<u64, String>,
    // The other files of a number that two files claim; readers see only the one in `tasks`.
    shadowed: BTreeMap<u64, Vec<String>>,
    tags: BTreeSet<String>,
}

impl Plan {
    pub fn read(snapshot: Arc<dyn Snapshot>) -> Result<Self, TrackerError> {
        let config = snapshot
            .read_text(layout::CONFIG)?
            .map(|text| Config::parse(&text));
        let mut tasks: BTreeMap<u64, String> = BTreeMap::new();
        let mut shadowed: BTreeMap<u64, Vec<String>> = BTreeMap::new();
        let mut tags = BTreeSet::new();
        for path in snapshot.files()? {
            match Document::of(&path) {
                // Two files of one number resolve to the lowest path every time.
                Document::Task(number) => match tasks.entry(number) {
                    Entry::Vacant(slot) => {
                        slot.insert(path);
                    }
                    Entry::Occupied(mut slot) => {
                        let hidden = match path < *slot.get() {
                            true => slot.insert(path),
                            false => path,
                        };
                        shadowed.entry(number).or_default().push(hidden);
                    }
                },
                Document::Tag(name)
                    if normalize_name(&name).is_ok_and(|normalized| normalized == name) =>
                {
                    tags.insert(name);
                }
                _ => {}
            }
        }
        Ok(Self {
            snapshot,
            config,
            tasks,
            shadowed,
            tags,
        })
    }

    pub fn snapshot(&self) -> &Arc<dyn Snapshot> {
        &self.snapshot
    }

    pub fn revision(&self) -> Option<&RevisionId> {
        self.snapshot.revision()
    }

    pub fn is_initialized(&self) -> bool {
        self.config.is_some()
    }

    pub fn config(&self) -> Result<&Config, TrackerError> {
        match &self.config {
            None => Err(TrackerError::NotInitialized),
            Some(Ok(config)) => Ok(config),
            Some(Err(err)) => Err(err.clone().into()),
        }
    }

    pub fn abbreviation(&self) -> Result<Abbreviation, TrackerError> {
        Ok(self.config()?.abbreviation)
    }

    pub fn key(&self, number: u64) -> String {
        match self.abbreviation() {
            Ok(abbreviation) => abbreviation.format_key(number),
            Err(_) => number.to_string(),
        }
    }

    pub fn numbers(&self) -> impl Iterator<Item = u64> + '_ {
        self.tasks.keys().copied()
    }

    pub fn shadowed(&self) -> &BTreeMap<u64, Vec<String>> {
        &self.shadowed
    }

    pub fn max_number(&self) -> Option<u64> {
        self.tasks.keys().next_back().copied()
    }

    pub fn exists(&self, number: u64) -> bool {
        self.tasks.contains_key(&number)
    }

    pub fn path_of(&self, number: u64) -> Option<&str> {
        self.tasks.get(&number).map(String::as_str)
    }

    pub fn paths(&self) -> &BTreeMap<u64, String> {
        &self.tasks
    }

    pub fn raw(&self, number: u64) -> Result<String, TrackerError> {
        let path = self.task_path(number)?;
        self.snapshot
            .read_text(path)?
            .ok_or_else(|| self.not_found(number))
    }

    pub fn raw_all(&self) -> Result<BTreeMap<u64, String>, TrackerError> {
        self.tasks
            .iter()
            .filter_map(|(number, path)| match self.snapshot.read_text(path) {
                Ok(Some(text)) => Some(Ok((*number, text))),
                Ok(None) => None,
                Err(err) => Some(Err(err.into())),
            })
            .collect()
    }

    pub fn task(&self, number: u64) -> Result<Task, TrackerError> {
        let path = self.task_path(number)?;
        let text = self.raw(number)?;
        Task::from_file_string(&text).map_err(|err| match err {
            op_task::TaskError::MissingCreated => TrackerError::MissingCreated {
                path: path.to_owned(),
                example: op_task::now().to_string(),
            },
            other => other.into(),
        })
    }

    pub fn tag_names(&self) -> &BTreeSet<String> {
        &self.tags
    }

    pub fn tags(&self) -> Result<Vec<Tag>, TrackerError> {
        self.tags.iter().map(|name| self.read_tag(name)).collect()
    }

    pub fn tag(&self, name: &str) -> Result<Tag, TrackerError> {
        let name = normalized(name)?;
        if !self.tags.contains(&name) {
            return Err(TrackerError::TagNotFound { name });
        }
        self.read_tag(&name)
    }

    pub fn raw_tags(&self) -> Result<BTreeMap<String, String>, TrackerError> {
        let mut raw = BTreeMap::new();
        for path in self.snapshot.list(layout::TAGS)? {
            if let (Some(name), Some(text)) =
                (layout::tag_name(&path), self.snapshot.read_text(&path)?)
            {
                raw.insert(name.to_owned(), text);
            }
        }
        Ok(raw)
    }

    fn read_tag(&self, name: &str) -> Result<Tag, TrackerError> {
        let path = layout::tag_path(name);
        let text = self
            .snapshot
            .read_text(&path)?
            .ok_or_else(|| TrackerError::TagNotFound {
                name: name.to_owned(),
            })?;
        Tag::from_file_string(name.to_owned(), &text).map_err(|err| TrackerError::Unreadable {
            path,
            reason: err.to_string(),
        })
    }

    fn task_path(&self, number: u64) -> Result<&str, TrackerError> {
        self.path_of(number).ok_or_else(|| self.not_found(number))
    }

    pub(crate) fn not_found(&self, number: u64) -> TrackerError {
        TrackerError::NotFound {
            id: self.key(number),
        }
    }
}

pub(crate) fn normalized(name: &str) -> Result<String, TrackerError> {
    normalize_name(name).map_err(|err| TrackerError::Invalid(err.to_string()))
}
