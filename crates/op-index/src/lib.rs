use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

mod docs;
mod problems;

use op_api::{
    Author, Comment, Metadata, Problem, SearchHit, SearchMatch, TaskChild, TaskDetail,
    TaskListItem, TaskRef, hit_cmp, list_item_cmp, updated_field,
};
use op_backend::{Actor, Change, ChangeKind, LogEntry, Timestamp};
use op_task::reference::{self, Target};
use op_task::{Abbreviation, FieldError, layout};
use op_tracker::{Plan, TrackerError, doc_moves};

#[derive(Debug, Default)]
pub struct Index {
    abbreviation: Option<Abbreviation>,
    tasks: BTreeMap<u64, Entry>,
    updated: HashMap<u64, Timestamp>,
    authors: HashMap<u64, Author>,
    problems: HashMap<u64, Vec<Problem>>,
    docs: BTreeMap<String, docs::DocEntry>,
    doc_updated: HashMap<String, Timestamp>,
    doc_authors: HashMap<String, Author>,
}

#[derive(Debug, Clone)]
struct Entry {
    raw: String,
    title: String,
    metadata: Metadata,
    comment_count: usize,
    conflicts: usize,
    // `# ` headings in the published version of the text, outside the comment log.
    titles: usize,
    // The tasks and the docs the text names with `[[…]]`, found or not.
    body_refs: Vec<u64>,
    doc_refs: Vec<String>,
    // References the file spells as a key, a number, or a name rather than as a path.
    unpathed: Vec<(String, Target)>,
    comment_problems: Vec<String>,
    diagram_problems: Vec<String>,
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
            self.docs.clear();
            self.abbreviation = abbreviation;
        }
        let Some(abbreviation) = abbreviation else {
            self.tasks.clear();
            self.problems.clear();
            self.docs.clear();
            return Ok(());
        };
        self.load_docs(plan)?;
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
        self.problems = self.find_problems(plan.tag_names(), plan.shadowed());
        Ok(())
    }

    // Reads again only the tasks `numbers` names. The problems span tasks, so they cover all again.
    pub fn update(&mut self, plan: &Plan, numbers: &BTreeSet<u64>) -> Result<(), TrackerError> {
        let held = self
            .abbreviation
            .filter(|held| plan.abbreviation().ok() == Some(*held));
        let Some(abbreviation) = held else {
            return self.load(plan);
        };
        for &number in numbers {
            let text = match plan.path_of(number) {
                Some(path) => plan.snapshot().read_text(path)?,
                None => None,
            };
            let Some(text) = text else {
                self.tasks.remove(&number);
                continue;
            };
            if self
                .tasks
                .get(&number)
                .is_none_or(|entry| entry.raw != text)
            {
                self.tasks.insert(number, Entry::parse(text, abbreviation));
            }
        }
        self.problems = self.find_problems(plan.tag_names(), plan.shadowed());
        Ok(())
    }

    fn problems_of(&self, number: u64) -> Vec<Problem> {
        self.problems.get(&number).cloned().unwrap_or_default()
    }

    // A log lists the newest revision first, so the first entry to name a task dates it.
    pub fn date(&mut self, log: &[LogEntry]) {
        for entry in log {
            for change in &entry.changes {
                if let Some(number) = layout::task_number(&change.path) {
                    self.updated.entry(number).or_insert(entry.revision.at);
                }
                if let Some(name) = layout::doc_name(&change.path) {
                    self.doc_updated
                        .entry(name.to_owned())
                        .or_insert(entry.revision.at);
                }
            }
        }
    }

    pub fn touch(&mut self, number: u64, at: Timestamp) {
        self.updated.insert(number, at);
    }

    // A log lists the newest revision first, so the first entry to create a task names its author:
    // a task deleted and created again under the same number is a new task. A doc is read oldest
    // first, because a rename moves the author of the old name to the new one.
    pub fn credit(&mut self, log: &[LogEntry]) {
        let mut found = HashMap::new();
        for entry in log {
            for number in created(&entry.changes) {
                found
                    .entry(number)
                    .or_insert_with(|| author_of(&entry.revision.author));
            }
        }
        self.authors.extend(found);
        let mut docs: HashMap<String, Author> = HashMap::new();
        for entry in log.iter().rev() {
            let moves = doc_moves(&entry.changes);
            for (old, new) in moves.renamed {
                let author = docs
                    .remove(&old)
                    .or_else(|| self.doc_authors.get(&old).cloned());
                if let Some(author) = author {
                    docs.insert(new, author);
                }
            }
            for name in moves.created {
                docs.insert(name, author_of(&entry.revision.author));
            }
        }
        self.doc_authors.extend(docs);
    }

    // A rename between two loads keeps the author the old name had.
    pub fn carry_doc_authors(&mut self, changes: &[Change]) {
        for (old, new) in doc_moves(changes).renamed {
            if let Some(author) = self.doc_authors.remove(&old) {
                self.doc_authors.insert(new, author);
            }
        }
    }

    pub fn doc_exists(&self, name: &str) -> bool {
        self.docs.contains_key(name)
    }

    pub fn doc_credited(&self, name: &str) -> bool {
        self.doc_authors.contains_key(name)
    }

    pub fn credited(&self, number: u64) -> bool {
        self.authors.contains_key(&number)
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
            problems: self.problems_of(number),
            description: self.description_of(&partial.body),
            updated: self.updated_of(number, entry),
            author: self.authors.get(&number).cloned(),
            parent_title: hierarchy.parent_title,
            children: hierarchy.children,
            refs: hierarchy.refs,
            doc_refs: self.doc_refs_in(layout::TASKS, &partial.body),
            depends_on: hierarchy.depends_on,
            blocks: hierarchy.blocks,
        })
    }

    // Without the store's abbreviation no reference can be spelled as a key, so they keep the
    // spelling of the file.
    fn description_of(&self, body: &str) -> String {
        let description = op_task::content::split(&op_task::comment::strip(body));
        match self.abbreviation {
            Some(abbreviation) => op_api::body_to_keys(abbreviation, layout::TASKS, &description),
            None => description,
        }
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
                Some(abbreviation) => body_refs(abbreviation, layout::TASKS, body, &by_id),
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
            problems: self.problems_of(number),
            updated: self.updated_of(number, entry),
            author: self.authors.get(&number).cloned(),
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
        let text = op_task::comment::strip(&op_task::conflict::published(&partial.body));
        let titles = op_md::headings(&text)
            .iter()
            .filter(|heading| heading.level == 1)
            .count();
        let body_refs = op_task::body_ref_spans(&text)
            .into_iter()
            .filter_map(|(_, inner)| op_task::body_ref_id(abbreviation, layout::TASKS, inner))
            .collect();
        let mut unpathed = reference::unpathed(Some(abbreviation), layout::TASKS, &text);
        unpathed.extend(
            reference::frontmatter_spellings(&raw)
                .into_iter()
                .filter(|spelled| !reference::is_path(layout::TASKS, spelled))
                .filter_map(|spelled| {
                    let number = op_task::ref_id(&spelled)?;
                    Some((spelled, Target::Task(number)))
                }),
        );
        let metadata = Metadata::from_partial(partial.metadata, &partial.conflicts, abbreviation);
        Self {
            titles,
            body_refs,
            doc_refs: op_task::doc::body_doc_names(Some(abbreviation), layout::TASKS, &text),
            unpathed,
            comment_problems: op_task::comment::problems(&partial.body),
            diagram_problems: diagram_problems(&raw),
            haystack: haystack(&title, &partial.body, &metadata),
            comment_count: op_task::comment::parse(&partial.body).len(),
            conflicts,
            title,
            metadata,
            raw,
        }
    }
}

// The tasks whose file `changes` adds. A new title moves a task to a file with a new name, so a
// change that also removes a file of the same task only renames it.
pub fn created(changes: &[Change]) -> BTreeSet<u64> {
    let numbers = |kind: ChangeKind| -> BTreeSet<u64> {
        changes
            .iter()
            .filter(|change| change.kind == kind)
            .filter_map(|change| layout::task_number(&change.path))
            .collect()
    };
    &numbers(ChangeKind::Added) - &numbers(ChangeKind::Removed)
}

fn author_of(actor: &Actor) -> Author {
    Author {
        name: actor.name.clone(),
        email: actor.email.clone(),
        agent: actor.via.clone(),
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
    dir: &str,
    body: &str,
    by_id: &HashMap<&str, &TaskListItem>,
) -> Vec<TaskRef> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();
    for (_, inner) in op_task::body_ref_spans(body) {
        let Some(number) = op_task::body_ref_id(abbreviation, dir, inner) else {
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

// The place counts from the top of the task file, the text that `openplan get` prints and an agent
// edits. A fence in a comment sits in a blockquote, so its lines in the file carry a `> ` that the
// parser never saw.
pub(crate) fn diagram_problems(raw: &str) -> Vec<String> {
    op_md::fences(raw)
        .into_iter()
        .filter(|fence| fence.language.eq_ignore_ascii_case("mermaid"))
        .filter_map(|fence| {
            let err = op_diagram_mermaid::parse(&fence.text).err()?;
            let line = raw[..fence.text_start].matches('\n').count() + err.line;
            let in_file = raw.lines().nth(line - 1).unwrap_or("");
            let in_fence = fence.text.lines().nth(err.line - 1).unwrap_or("");
            let prefix = match in_file.ends_with(in_fence) {
                true => in_file.chars().count() - in_fence.chars().count(),
                false => 0,
            };
            Some(format!(
                "the Mermaid diagram fails at line {line}, column {}: {}",
                err.column + prefix,
                err.message
            ))
        })
        .collect()
}
