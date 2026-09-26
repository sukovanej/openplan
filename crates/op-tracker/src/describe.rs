use std::collections::{BTreeMap, BTreeSet};

use op_backend::{BackendError, Change, ChangeKind, Snapshot};
use op_task::layout::{self, Document};
use op_task::{
    Abbreviation, FieldResult, PartialFrontmatter, PartialMetadata, PartialTask, Status, comment,
};

pub(crate) type Read<'a> = &'a dyn Fn(&str) -> Result<Option<Vec<u8>>, BackendError>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Described {
    pub config: Option<ChangeKind>,
    pub tags: Vec<TagChange>,
    pub tasks: Vec<TaskChange>,
    pub docs: Vec<DocChange>,
    pub others: Vec<Change>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagChange {
    pub name: String,
    pub kind: ChangeKind,
    // A rename moves the file, so the revision removes one tag file and adds another. A pair that
    // differs only in the heading is one tag, and it is `Modified`.
    pub renamed_from: Option<String>,
}

// A rename moves the file too, and the doc keeps the time it was created, which pairs the two files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocChange {
    pub name: String,
    pub kind: ChangeKind,
    pub renamed_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskChange {
    pub number: u64,
    pub kind: ChangeKind,
    // For a removed task, the title it had before the revision.
    pub title: Option<String>,
    pub fields: Vec<FieldChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldChange {
    Number {
        from: u64,
        to: u64,
    },
    Status {
        from: Status,
        to: Status,
    },
    Parent {
        from: Option<u64>,
        to: Option<u64>,
    },
    Order,
    Dependencies {
        from: Vec<u64>,
        to: Vec<u64>,
    },
    Tags {
        from: Vec<String>,
        to: Vec<String>,
    },
    Title {
        from: Option<String>,
        to: Option<String>,
    },
    Description,
    Comments {
        added: usize,
        removed: usize,
    },
    Conflicts {
        from: usize,
        to: usize,
    },
    // A field that openplan does not model, such as `created`, or a modeled field that one
    // version cannot read.
    Other(String),
    // One version of the frontmatter does not parse, so no field of it can be compared.
    Frontmatter,
}

const MODELED: [&str; 5] = ["status", "parent", "rank", "dependencies", "tags"];

// `changes` names the documents that differ; `before` and `after` read one of them on each side.
// `moved_from` is the first parent of a sync merge: a merge moves a task that another task took the
// number of, and there the task still has its old number.
pub(crate) fn describe(
    changes: &[Change],
    before: Read<'_>,
    after: Read<'_>,
    moved_from: Option<&dyn Snapshot>,
) -> Result<Described, BackendError> {
    let mut described = Described::default();
    let mut tasks: BTreeMap<u64, Vec<&Change>> = BTreeMap::new();
    let mut tags = Vec::new();
    let mut docs = Vec::new();
    for change in changes {
        match Document::of(&change.path) {
            Document::Config => described.config = Some(change.kind),
            Document::Task(number) => tasks.entry(number).or_default().push(change),
            Document::Tag(name) => tags.push((name, change.kind)),
            Document::Doc(name) => docs.push((name, change.kind)),
            Document::Asset(_) | Document::Other(_) => described.others.push(change.clone()),
        }
    }
    for (number, files) in tasks {
        described
            .tasks
            .extend(task_change(number, &files, before, after, moved_from)?);
    }
    described.tags = tag_changes(tags, before, after)?;
    described.docs = doc_changes(docs, before, after)?;
    Ok(described)
}

// A new title gives the task a new file name, so one task can arrive as a removed and an added path.
fn task_change(
    number: u64,
    files: &[&Change],
    before: Read<'_>,
    after: Read<'_>,
    moved_from: Option<&dyn Snapshot>,
) -> Result<Option<TaskChange>, BackendError> {
    let side = |skip: ChangeKind, read: Read<'_>| -> Result<Option<PartialTask>, BackendError> {
        let Some(change) = files.iter().find(|change| change.kind != skip) else {
            return Ok(None);
        };
        Ok(read(&change.path)?
            .map(|bytes| op_task::parse_partial(&String::from_utf8_lossy(&bytes))))
    };
    let (mut old, new) = (
        side(ChangeKind::Added, before)?,
        side(ChangeKind::Removed, after)?,
    );
    let mut moves = Vec::new();
    if let (None, Some(new), Some(earlier)) = (&old, &new, moved_from)
        && let Some((from, task)) = old_number(earlier, number, new)?
    {
        moves.push(FieldChange::Number { from, to: number });
        old = Some(task);
    }
    let (kind, fields) = match (&old, &new) {
        (None, None) => return Ok(None),
        (None, Some(_)) => (ChangeKind::Added, Vec::new()),
        (Some(_), None) => (ChangeKind::Removed, Vec::new()),
        (Some(old), Some(new)) => (ChangeKind::Modified, [moves, fields(old, new)].concat()),
    };
    Ok(Some(TaskChange {
        number,
        kind,
        title: new.or(old).and_then(|task| task.title),
        fields,
    }))
}

// A task is the one it was created as: the same creation time and the same title.
fn old_number(
    earlier: &dyn Snapshot,
    number: u64,
    task: &PartialTask,
) -> Result<Option<(u64, PartialTask)>, BackendError> {
    let created = |task: &PartialTask| task.frontmatter.as_ref()?.get("created").cloned();
    let Some(identity) = created(task).zip(task.title.clone()) else {
        return Ok(None);
    };
    for path in earlier.list(layout::TASKS)? {
        let Some(other) = layout::task_number(&path).filter(|other| *other != number) else {
            continue;
        };
        let Some(bytes) = earlier.read(&path)? else {
            continue;
        };
        let candidate = op_task::parse_partial(&String::from_utf8_lossy(&bytes));
        if created(&candidate).zip(candidate.title.clone()).as_ref() == Some(&identity) {
            return Ok(Some((other, candidate)));
        }
    }
    Ok(None)
}

fn fields(old: &PartialTask, new: &PartialTask) -> Vec<FieldChange> {
    let mut fields = Vec::new();
    let mut others = Vec::new();
    match (
        &old.frontmatter,
        &new.frontmatter,
        &old.metadata,
        &new.metadata,
    ) {
        (
            Some(old_map),
            Some(new_map),
            PartialMetadata::Fields(old_fields),
            PartialMetadata::Fields(new_fields),
        ) => {
            let changed = |key: &str| old_map.get(key) != new_map.get(key);
            let mut modeled = |key: &str, change: Option<Option<FieldChange>>| {
                if changed(key) {
                    match change {
                        Some(change) => fields.extend(change),
                        None => others.push(FieldChange::Other(key.to_owned())),
                    }
                }
            };
            modeled("status", status(old_fields, new_fields));
            modeled("parent", parent(old_fields, new_fields));
            modeled("rank", Some(Some(FieldChange::Order)));
            modeled("dependencies", dependencies(old_fields, new_fields));
            modeled("tags", tags(old_fields, new_fields));
            let keys: BTreeSet<&str> = old_map
                .keys()
                .chain(new_map.keys())
                .filter_map(serde_yaml::Value::as_str)
                .filter(|key| !MODELED.contains(key))
                .collect();
            others.extend(
                keys.into_iter()
                    .filter(|key| changed(key))
                    .map(|key| FieldChange::Other(key.to_owned())),
            );
        }
        _ if old.metadata != new.metadata => others.push(FieldChange::Frontmatter),
        _ => {}
    }
    if old.title != new.title {
        fields.push(FieldChange::Title {
            from: old.title.clone(),
            to: new.title.clone(),
        });
    }
    if description(&old.body) != description(&new.body) {
        fields.push(FieldChange::Description);
    }
    let (old_comments, new_comments) = (comment::parse(&old.body), comment::parse(&new.body));
    let added = new_comments
        .iter()
        .filter(|entry| !old_comments.contains(entry))
        .count();
    let removed = old_comments
        .iter()
        .filter(|entry| !new_comments.contains(entry))
        .count();
    if added + removed > 0 {
        fields.push(FieldChange::Comments { added, removed });
    }
    let (from, to) = (old.conflict_count(), new.conflict_count());
    if from != to {
        fields.push(FieldChange::Conflicts { from, to });
    }
    fields.extend(others);
    fields
}

// `None` where one version cannot be read, and `Some(None)` where both read the same, such as a
// reference that names the same task under an older file name.
type Modeled = Option<Option<FieldChange>>;

fn status(old: &PartialFrontmatter, new: &PartialFrontmatter) -> Modeled {
    let (from, to) = (old.status.as_ref().ok()?, new.status.as_ref().ok()?);
    Some((from != to).then_some(FieldChange::Status {
        from: *from,
        to: *to,
    }))
}

fn parent(old: &PartialFrontmatter, new: &PartialFrontmatter) -> Modeled {
    let id = |parent: &FieldResult<Option<String>>| match parent.as_ref().ok()? {
        Some(reference) => op_task::ref_id(reference).map(Some),
        None => Some(None),
    };
    let (from, to) = (id(&old.parent)?, id(&new.parent)?);
    Some((from != to).then_some(FieldChange::Parent { from, to }))
}

fn dependencies(old: &PartialFrontmatter, new: &PartialFrontmatter) -> Modeled {
    let ids = |dependencies: &FieldResult<Vec<String>>| {
        dependencies
            .as_ref()
            .ok()?
            .iter()
            .map(|reference| op_task::ref_id(reference))
            .collect::<Option<Vec<u64>>>()
    };
    let (from, to) = (ids(&old.dependencies)?, ids(&new.dependencies)?);
    Some((from != to).then_some(FieldChange::Dependencies { from, to }))
}

fn tags(old: &PartialFrontmatter, new: &PartialFrontmatter) -> Modeled {
    let (from, to) = (old.tags.as_ref().ok()?, new.tags.as_ref().ok()?);
    Some((from != to).then(|| FieldChange::Tags {
        from: from.clone(),
        to: to.clone(),
    }))
}

// The body less its title and its comment log, which have changes of their own.
fn description(body: &str) -> String {
    without_title(&comment::strip(body)).trim().to_owned()
}

fn without_title(text: &str) -> String {
    match op_md::headings(text).into_iter().find(|h| h.level == 1) {
        Some(title) => format!("{}{}", &text[..title.start], &text[title.end..]),
        None => text.to_owned(),
    }
}

fn tag_changes(
    tags: Vec<(String, ChangeKind)>,
    before: Read<'_>,
    after: Read<'_>,
) -> Result<Vec<TagChange>, BackendError> {
    let (mut removed, rest): (Vec<_>, Vec<_>) = tags
        .into_iter()
        .partition(|(_, kind)| *kind == ChangeKind::Removed);
    let renames_possible =
        !removed.is_empty() && rest.iter().any(|(_, kind)| *kind == ChangeKind::Added);
    let mut gone = Vec::new();
    if renames_possible {
        for (name, _) in removed.drain(..) {
            let content = tag_content(before, &name)?;
            gone.push((name, content));
        }
    }
    let mut changes = Vec::new();
    for (name, kind) in rest {
        let mut renamed_from = None;
        if kind == ChangeKind::Added && !gone.is_empty() {
            let content = tag_content(after, &name)?;
            if let Some(at) = gone.iter().position(|(_, old)| *old == content) {
                renamed_from = Some(gone.remove(at).0);
            }
        }
        changes.push(TagChange {
            kind: match renamed_from {
                Some(_) => ChangeKind::Modified,
                None => kind,
            },
            name,
            renamed_from,
        });
    }
    changes.extend(
        removed
            .into_iter()
            .chain(
                gone.into_iter()
                    .map(|(name, _)| (name, ChangeKind::Removed)),
            )
            .map(|(name, kind)| TagChange {
                name,
                kind,
                renamed_from: None,
            }),
    );
    changes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(changes)
}

// The doc a revision renamed, created, or deleted leads, so it names the revision; the docs whose
// `parent:` a rename or a delete moved follow it.
fn doc_changes(
    docs: Vec<(String, ChangeKind)>,
    before: Read<'_>,
    after: Read<'_>,
) -> Result<Vec<DocChange>, BackendError> {
    let created = |read: Read<'_>, name: &str| -> Result<Option<String>, BackendError> {
        Ok(read(&layout::doc_path(name))?.and_then(|bytes| {
            let partial = op_task::doc::parse_partial(&String::from_utf8_lossy(&bytes));
            partial.created().ok().map(|at| at.to_string())
        }))
    };
    let (removed, rest): (Vec<_>, Vec<_>) = docs
        .into_iter()
        .partition(|(_, kind)| *kind == ChangeKind::Removed);
    let mut gone = Vec::new();
    for (name, _) in removed {
        let at = created(before, &name)?;
        gone.push((name, at));
    }
    let mut changes = Vec::new();
    for (name, kind) in rest {
        let mut renamed_from = None;
        if kind == ChangeKind::Added
            && let Some(at) = created(after, &name)?
            && let Some(found) = gone.iter().position(|(_, old)| old.as_ref() == Some(&at))
        {
            renamed_from = Some(gone.remove(found).0);
        }
        changes.push(DocChange {
            kind: match renamed_from {
                Some(_) => ChangeKind::Modified,
                None => kind,
            },
            name,
            renamed_from,
        });
    }
    changes.extend(gone.into_iter().map(|(name, _)| DocChange {
        name,
        kind: ChangeKind::Removed,
        renamed_from: None,
    }));
    let led =
        |change: &DocChange| change.kind == ChangeKind::Modified && change.renamed_from.is_none();
    changes.sort_by(|a, b| led(a).cmp(&led(b)).then_with(|| a.name.cmp(&b.name)));
    Ok(changes)
}

fn tag_content(read: Read<'_>, name: &str) -> Result<Option<String>, BackendError> {
    Ok(read(&layout::tag_path(name))?.map(|bytes| {
        without_title(&String::from_utf8_lossy(&bytes))
            .trim()
            .to_owned()
    }))
}

impl Described {
    // One line for each document, in the words of the commit messages that openplan writes.
    pub fn lines(&self, abbreviation: Option<Abbreviation>) -> Vec<String> {
        let key = |number: u64| match abbreviation {
            Some(abbreviation) => abbreviation.format_key(number),
            None => number.to_string(),
        };
        let mut lines = Vec::new();
        if let Some(kind) = self.config {
            lines.push(match (kind, abbreviation) {
                (ChangeKind::Added, Some(abbreviation)) => {
                    format!("Start the {abbreviation} tasks")
                }
                (ChangeKind::Added, None) => "Start the tasks".to_owned(),
                (kind, _) => format!("{}: {}", layout::CONFIG, verb(kind)),
            });
        }
        for tag in &self.tags {
            lines.push(match &tag.renamed_from {
                Some(from) => format!("tag {from}: rename to {}", tag.name),
                None => format!("tag {}: {}", tag.name, verb(tag.kind)),
            });
        }
        for task in &self.tasks {
            lines.push(format!("{}: {}", key(task.number), task_words(task, &key)));
        }
        for doc in &self.docs {
            lines.push(match &doc.renamed_from {
                Some(from) => format!("doc {from}: rename to {}", doc.name),
                None => format!("doc {}: {}", doc.name, verb(doc.kind)),
            });
        }
        for other in &self.others {
            lines.push(format!("{}: {}", other.path, verb(other.kind)));
        }
        lines
    }
}

fn verb(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "create",
        ChangeKind::Modified => "edit",
        ChangeKind::Removed => "delete",
    }
}

fn task_words(task: &TaskChange, key: &dyn Fn(u64) -> String) -> String {
    let title = task.title.as_deref().unwrap_or_default();
    match task.kind {
        ChangeKind::Added => format!("create \"{title}\""),
        ChangeKind::Removed => format!("delete \"{title}\""),
        ChangeKind::Modified if task.fields.is_empty() => verb(task.kind).to_owned(),
        ChangeKind::Modified => task
            .fields
            .iter()
            .map(|field| field_words(field, key))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn field_words(field: &FieldChange, key: &dyn Fn(u64) -> String) -> String {
    let list = |name: &str, items: Vec<String>| match items.is_empty() {
        true => format!("no {name}"),
        false => format!("{name} → {}", items.join(", ")),
    };
    match field {
        FieldChange::Number { from, .. } => format!("moved from {}", key(*from)),
        FieldChange::Status { to, .. } => format!("status → {}", to.as_str()),
        FieldChange::Parent { to: Some(to), .. } => format!("parent → {}", key(*to)),
        FieldChange::Parent { to: None, .. } => "no parent".to_owned(),
        FieldChange::Order => "order".to_owned(),
        FieldChange::Dependencies { to, .. } => list(
            "dependencies",
            to.iter().map(|number| key(*number)).collect(),
        ),
        FieldChange::Tags { to, .. } => list("tags", to.clone()),
        FieldChange::Title { to: Some(to), .. } => format!("title → \"{to}\""),
        FieldChange::Title { to: None, .. } => "no title".to_owned(),
        FieldChange::Description => "description".to_owned(),
        FieldChange::Comments {
            added: 1,
            removed: 0,
        } => "comment".to_owned(),
        FieldChange::Comments { added, removed: 0 } => format!("{added} comments"),
        FieldChange::Comments { .. } => "comments".to_owned(),
        FieldChange::Conflicts { to: 0, .. } => "conflicts resolved".to_owned(),
        FieldChange::Conflicts { to, .. } => format!("conflicts → {to}"),
        FieldChange::Other(name) => name.clone(),
        FieldChange::Frontmatter => "frontmatter".to_owned(),
    }
}
