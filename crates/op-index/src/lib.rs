use std::collections::{BTreeMap, HashMap, HashSet};

use op_api::{
    Comment, Metadata, SearchHit, SearchMatch, TaskChild, TaskDetail, TaskListItem, TaskRef,
    hit_cmp, list_item_cmp, updated_field,
};
use op_backend::{LogEntry, Timestamp};
use op_task::{Abbreviation, FieldError, layout};
use op_tracker::{Plan, TrackerError};

#[derive(Debug, Default)]
pub struct Index {
    abbreviation: Option<Abbreviation>,
    tasks: BTreeMap<u64, Entry>,
    updated: HashMap<u64, Timestamp>,
}

#[derive(Debug, Clone)]
struct Entry {
    raw: String,
    title: String,
    metadata: Metadata,
    comment_count: usize,
    conflicts: usize,
    haystack: Haystack,
}

// Lowercased once, so a query re-cases no field per keystroke. Split where the ranking cuts: a
// title hit outranks one that only the body or the frontmatter carries.
#[derive(Debug, Clone)]
struct Haystack {
    title: String,
    rest: String,
}

pub struct HierarchyContext {
    pub parent_title: Option<String>,
    pub children: Vec<TaskChild>,
    pub refs: Vec<TaskRef>,
    pub depends_on: Vec<TaskRef>,
    pub blocks: Vec<TaskRef>,
}

impl Index {
    pub fn new() -> Self {
        Self::default()
    }

    // Parses again only the tasks whose text changed since the last load.
    pub fn load(&mut self, plan: &Plan) -> Result<(), TrackerError> {
        let abbreviation = plan.abbreviation().ok();
        if abbreviation != self.abbreviation {
            self.tasks.clear();
            self.abbreviation = abbreviation;
        }
        let Some(abbreviation) = abbreviation else {
            self.tasks.clear();
            return Ok(());
        };
        let raw = plan.raw_all()?;
        self.tasks.retain(|number, _| raw.contains_key(number));
        for (number, text) in raw {
            if self
                .tasks
                .get(&number)
                .is_some_and(|entry| entry.raw == text)
            {
                continue;
            }
            self.tasks.insert(number, Entry::parse(text, abbreviation));
        }
        Ok(())
    }

    // A log lists the newest revision first, so the first entry to name a task dates it.
    pub fn date(&mut self, log: &[LogEntry]) {
        for entry in log {
            for change in &entry.changes {
                if let Some(number) = layout::task_number(&change.path) {
                    self.updated.entry(number).or_insert(entry.revision.at);
                }
            }
        }
    }

    pub fn touch(&mut self, number: u64, at: Timestamp) {
        self.updated.insert(number, at);
    }

    pub fn abbreviation(&self) -> Option<Abbreviation> {
        self.abbreviation
    }

    pub fn key(&self, number: u64) -> String {
        match self.abbreviation {
            Some(abbreviation) => abbreviation.format_key(number),
            None => number.to_string(),
        }
    }

    pub fn number(&self, key: &str) -> Option<u64> {
        self.abbreviation?.parse_key(key)
    }

    pub fn contains(&self, number: u64) -> bool {
        self.tasks.contains_key(&number)
    }

    pub fn max_number(&self) -> Option<u64> {
        self.tasks.keys().next_back().copied()
    }

    pub fn list(&self, project: &str) -> Vec<TaskListItem> {
        self.tasks
            .iter()
            .map(|(number, entry)| self.row(project, *number, entry))
            .collect()
    }

    pub fn item(&self, project: &str, number: u64) -> Option<TaskListItem> {
        Some(self.row(project, number, self.tasks.get(&number)?))
    }

    pub fn raw(&self, number: u64) -> Option<&str> {
        Some(self.tasks.get(&number)?.raw.as_str())
    }

    pub fn detail(&self, project: &str, number: u64) -> Option<TaskDetail> {
        let entry = self.tasks.get(&number)?;
        let partial = op_task::parse_partial(&entry.raw);
        let id = self.key(number);
        let hierarchy = self.hierarchy(project, &id, &entry.metadata, &partial.body);
        Some(TaskDetail {
            project: project.to_owned(),
            id,
            title: entry.title.clone(),
            metadata: entry.metadata.clone(),
            comments: comments_of(&partial.body),
            conflicts: entry.conflicts,
            body: op_task::comment::strip(&partial.body),
            updated: self.updated_of(number, entry),
            parent_title: hierarchy.parent_title,
            children: hierarchy.children,
            refs: hierarchy.refs,
            depends_on: hierarchy.depends_on,
            blocks: hierarchy.blocks,
        })
    }

    pub fn comments(&self, number: u64) -> Option<Vec<Comment>> {
        let entry = self.tasks.get(&number)?;
        Some(comments_of(&op_task::parse_partial(&entry.raw).body))
    }

    // A case-insensitive substring over the key and the whole file. A query of nothing but spaces
    // matches nothing: a palette that opens on every task is a list, not a search.
    pub fn search(&self, project: &str, query: &str) -> Vec<SearchHit> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        let needle = query.to_lowercase();
        let mut hits: Vec<SearchHit> = self
            .tasks
            .iter()
            .filter_map(|(number, entry)| {
                let id = self.key(*number);
                let matched = if id.to_lowercase().contains(&needle) {
                    SearchMatch::Key
                } else if entry.haystack.title.contains(&needle) {
                    SearchMatch::Title
                } else if entry.haystack.rest.contains(&needle) {
                    SearchMatch::Text
                } else {
                    return None;
                };
                Some(SearchHit {
                    task: self.row(project, *number, entry),
                    matched,
                })
            })
            .collect();
        hits.sort_by(hit_cmp);
        hits
    }

    // The immediate neighbourhood of a task: the parent's title, the direct children in sibling
    // order, every `[[id]]` in the body, and both directions of its dependencies.
    pub fn hierarchy(
        &self,
        project: &str,
        id: &str,
        metadata: &Metadata,
        body: &str,
    ) -> HierarchyContext {
        let rows = self.list(project);
        let by_id: HashMap<&str, &TaskListItem> =
            rows.iter().map(|row| (row.id.as_str(), row)).collect();
        let parent_title = metadata
            .parent()
            .and_then(|parent| by_id.get(parent))
            .map(|row| row.title.clone());
        let mut kids: Vec<&TaskListItem> = rows
            .iter()
            .filter(|row| row.metadata.parent() == Some(id))
            .collect();
        kids.sort_by(|a, b| list_item_cmp(a, b));
        let children = kids
            .into_iter()
            .map(|row| TaskChild {
                id: row.id.clone(),
                title: row.title.clone(),
                status: row.metadata.status_field(),
                rank: row.metadata.rank().map(str::to_owned),
            })
            .collect();
        let mut waited_for = HashSet::new();
        let depends_on = metadata
            .dependencies()
            .iter()
            .filter_map(|entry| by_id.get(op_task::ref_target(entry)))
            .filter(|row| waited_for.insert(row.id.as_str()))
            .map(|row| task_ref(row))
            .collect();
        let mut blocked: Vec<&TaskListItem> =
            rows.iter().filter(|row| depends_on_id(row, id)).collect();
        blocked.sort_by(|a, b| list_item_cmp(a, b));
        HierarchyContext {
            parent_title,
            children,
            refs: match self.abbreviation {
                Some(abbreviation) => body_refs(abbreviation, body, &by_id),
                None => Vec::new(),
            },
            depends_on,
            blocks: blocked.into_iter().map(task_ref).collect(),
        }
    }

    fn row(&self, project: &str, number: u64, entry: &Entry) -> TaskListItem {
        TaskListItem {
            project: project.to_owned(),
            id: self.key(number),
            title: entry.title.clone(),
            metadata: entry.metadata.clone(),
            comment_count: entry.comment_count,
            conflicts: entry.conflicts,
            updated: self.updated_of(number, entry),
        }
    }

    fn updated_of(&self, number: u64, entry: &Entry) -> op_api::Field<op_api::Rfc3339> {
        updated_field(
            entry.metadata.created(),
            self.updated
                .get(&number)
                .copied()
                .ok_or(FieldError::Missing),
        )
    }
}

impl Entry {
    fn parse(raw: String, abbreviation: Abbreviation) -> Self {
        let partial = op_task::parse_partial(&raw);
        let title = partial.title.clone().unwrap_or_default();
        let conflicts = partial.conflict_count();
        let metadata = Metadata::from_partial(partial.metadata, &partial.conflicts, abbreviation);
        Self {
            haystack: haystack(&title, &partial.body, &metadata),
            comment_count: op_task::comment::parse(&partial.body).len(),
            conflicts,
            title,
            metadata,
            raw,
        }
    }
}

fn haystack(title: &str, body: &str, metadata: &Metadata) -> Haystack {
    let mut rest = format!("{body}\n");
    if let Some(status) = metadata.status() {
        rest.push_str(status.as_str());
        rest.push('\n');
    }
    if let Some(parent) = metadata.parent() {
        rest.push_str(parent);
        rest.push('\n');
    }
    for dependency in metadata.dependencies() {
        rest.push_str(dependency);
        rest.push('\n');
    }
    Haystack {
        title: title.to_lowercase(),
        rest: rest.to_lowercase(),
    }
}

fn task_ref(row: &TaskListItem) -> TaskRef {
    TaskRef {
        id: row.id.clone(),
        title: row.title.clone(),
        status: row.metadata.status_field(),
    }
}

// A dependency may aim at a section (`OPP-42#Design`), which names the same task as the bare key.
fn depends_on_id(row: &TaskListItem, id: &str) -> bool {
    row.metadata
        .dependencies()
        .iter()
        .any(|entry| op_task::ref_target(entry) == id)
}

// Every `[[…]]` in `body` that resolves to a known task, once each, in first-seen order.
fn body_refs(
    abbreviation: Abbreviation,
    body: &str,
    by_id: &HashMap<&str, &TaskListItem>,
) -> Vec<TaskRef> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();
    for (_, inner) in op_task::body_ref_spans(body) {
        let Some(number) = op_task::body_ref_id(abbreviation, inner) else {
            continue;
        };
        let key = abbreviation.format_key(number);
        if let Some(row) = by_id.get(key.as_str())
            && seen.insert(key)
        {
            refs.push(task_ref(row));
        }
    }
    refs
}

pub fn comments_of(body: &str) -> Vec<Comment> {
    op_task::comment::parse(body)
        .iter()
        .map(Comment::from)
        .collect()
}
