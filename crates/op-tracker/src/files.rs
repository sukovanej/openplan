use std::collections::HashSet;

use op_forge::PullRequest;
use op_task::doc::Doc;
use op_task::layout;
use op_task::reference::{self, Target};
use op_task::{FieldConflict, Frontmatter, Task, TaskLink, conflict, parse_id, rank, ref_target};

use crate::{Plan, TrackerError};

// A task's title is its single `# H1`: zero, empty, or several would leave a file no reader can
// name.
pub(crate) fn single_title(body: &str) -> Result<String, TrackerError> {
    let mut h1s = op_md::headings(&conflict::published(body))
        .into_iter()
        .filter(|heading| heading.level == 1);
    match (h1s.next(), h1s.next()) {
        (Some(h1), None) if !h1.text.trim().is_empty() => Ok(h1.text),
        _ => Err(TrackerError::Invalid(
            "a task must have exactly one non-empty `# ` title heading".to_owned(),
        )),
    }
}

// Only sync writes a conflict. A write may keep one exactly as it is, or resolve all of it, so no
// write adds a conflict or edits inside one.
pub(crate) fn keeps_conflicts(old: Option<&Task>, new: &Task) -> Result<(), TrackerError> {
    keeps(
        old.map_or(&[][..], |old| old.conflicts.as_slice()),
        &new.conflicts,
        &old.map(|old| task_blocks(&old.body)).unwrap_or_default(),
        &task_blocks(&new.body),
    )
}

pub(crate) fn keeps_blocks(old: &str, new: &str) -> Result<(), TrackerError> {
    keeps(&[], &[], &task_blocks(old), &task_blocks(new))
}

pub(crate) fn doc_keeps_blocks(old: &str, new: &str) -> Result<(), TrackerError> {
    keeps(&[], &[], &doc_blocks(old), &doc_blocks(new))
}

// A doc has no comment log, so every block in its body counts.
pub(crate) fn doc_keeps_conflicts(old: Option<&Doc>, new: &Doc) -> Result<(), TrackerError> {
    keeps(
        old.map_or(&[][..], |old| old.conflicts.as_slice()),
        &new.conflicts,
        &old.map(|old| doc_blocks(&old.body)).unwrap_or_default(),
        &doc_blocks(&new.body),
    )
}

fn keeps(
    old_fields: &[FieldConflict],
    new_fields: &[FieldConflict],
    old_blocks: &[String],
    new_blocks: &[String],
) -> Result<(), TrackerError> {
    let added_field = new_fields.iter().any(|field| !old_fields.contains(field));
    let added_block = new_blocks.iter().any(|block| !old_blocks.contains(block));
    match added_field || added_block {
        true => Err(TrackerError::Invalid(
            "a write cannot add a conflict or edit inside one; keep each conflict block as it \
             is, or replace the whole block with the text you want"
                .to_owned(),
        )),
        false => Ok(()),
    }
}

fn task_blocks(body: &str) -> Vec<String> {
    block_texts(body, conflict::in_body(body))
}

fn doc_blocks(body: &str) -> Vec<String> {
    block_texts(body, conflict::blocks(body))
}

fn block_texts(body: &str, blocks: Vec<conflict::Block>) -> Vec<String> {
    blocks
        .into_iter()
        .map(|block| body[block.range].to_owned())
        .collect()
}

// The text that takes a block's place ends its last line, as the block did.
pub(crate) fn line_ended(text: &str) -> String {
    match text.is_empty() || text.ends_with('\n') {
        true => text.to_owned(),
        false => format!("{text}\n"),
    }
}

// Only the references a write adds are checked: a parent or dependency deleted since it was set
// must not block an unrelated edit.
pub(crate) fn validate(
    plan: &Plan,
    number: Option<u64>,
    old: Option<&Frontmatter>,
    new: &Frontmatter,
) -> Result<(), TrackerError> {
    if let Some(parent) = &new.parent
        && old.and_then(|old| old.parent.as_ref()).map(TaskLink::id) != Some(parent.id())
    {
        let target = parent.number;
        if Some(target) == number {
            return Err(TrackerError::Invalid(format!(
                "task {} cannot be its own parent",
                plan.key(target)
            )));
        }
        if !plan.exists(target) {
            return Err(TrackerError::Invalid(format!(
                "parent {} does not exist",
                plan.key(target)
            )));
        }
        if let Some(number) = number {
            refuse_parent_cycle(plan, number, target)?;
        }
    }
    if let Some(value) = &new.rank
        && old.and_then(|old| old.rank.as_deref()) != Some(value.as_str())
        && !rank::is_valid(value)
    {
        return Err(TrackerError::Invalid(format!(
            "rank {value} is not a base-36 key (0-9a-z)"
        )));
    }
    for dependency in &new.dependencies {
        if old.is_some_and(|old| {
            old.dependencies
                .iter()
                .any(|held| held.id() == dependency.id())
        }) {
            continue;
        }
        let target = dependency.number;
        if Some(target) == number {
            return Err(TrackerError::Invalid(format!(
                "task cannot depend on itself: {}",
                plan.key(target)
            )));
        }
        if !plan.exists(target) {
            return Err(TrackerError::Invalid(format!(
                "dependency {} does not exist",
                plan.key(target)
            )));
        }
    }
    // Every tag, not only the new ones: a task holding an unregistered tag drops it in the same
    // write, so no edit carries a dangling name forward.
    for tag in &new.tags {
        if !plan.tag_names().contains(tag) {
            return Err(TrackerError::TagUnregistered { name: tag.clone() });
        }
    }
    for address in &new.pull_requests {
        PullRequest::parse(address).map_err(|err| TrackerError::Invalid(err.to_string()))?;
    }
    Ok(())
}

// Every page of a pull request names it, and the task keeps the one address of the pull request
// itself. An address that names none stays as it is, and `validate` refuses it.
pub(crate) fn with_canonical_pull_requests(task: &Task) -> Task {
    let mut task = task.clone();
    task.frontmatter.pull_requests = op_task::sorted_set(
        task.frontmatter
            .pull_requests
            .iter()
            .map(|address| {
                PullRequest::parse(address).map_or_else(|_| address.clone(), |found| found.url())
            })
            .collect(),
    );
    task
}

// Walks up from the new parent; reaching the task itself would close a cycle. The visited set
// stops a cycle among other tasks from looping forever.
fn refuse_parent_cycle(plan: &Plan, number: u64, parent: u64) -> Result<(), TrackerError> {
    let mut cursor = Some(parent);
    let mut seen = HashSet::new();
    while let Some(current) = cursor {
        if current == number {
            return Err(TrackerError::Invalid(format!(
                "cannot reparent {} under its own descendant {}",
                plan.key(number),
                plan.key(parent)
            )));
        }
        if !seen.insert(current) {
            break;
        }
        cursor = match plan.task(current) {
            Ok(task) => task.frontmatter.parent.map(|parent| parent.number),
            Err(TrackerError::NotFound { .. }) => None,
            Err(err) => return Err(err),
        };
    }
    Ok(())
}

// A link that a write sets names only the number, and the file names the target by its path. A link
// to a deleted task keeps its number.
pub(crate) fn in_file_form(plan: &Plan, task: &Task) -> Task {
    let with_path = |link: &TaskLink| match plan.path_of(link.number) {
        Some(path) if link.slug.is_none() => link.clone().with_file_name(layout::file_name(path)),
        _ => link.clone(),
    };
    // `tasks get` prints the other version of a field conflict with ids too, so a text written back
    // from it holds them.
    let value_with_path = |value: serde_yaml::Value| {
        TaskLink::from_value(&value)
            .and_then(|link| serde_yaml::to_value(with_path(&link)).ok())
            .unwrap_or(value)
    };
    let mut task = task.clone();
    for conflict in &mut task.conflicts {
        conflict.other = match (conflict.field.as_str(), conflict.other.take()) {
            ("parent", Some(value)) => Some(value_with_path(value)),
            ("dependencies", Some(serde_yaml::Value::Sequence(items))) => {
                Some(items.into_iter().map(value_with_path).collect())
            }
            (_, other) => other,
        };
    }
    task.frontmatter.parent = task.frontmatter.parent.as_ref().map(with_path);
    task.frontmatter.dependencies = task
        .frontmatter
        .dependencies
        .iter()
        .map(with_path)
        .collect();
    task.body = body_in_file_form(plan, layout::TASKS, &task.body);
    task
}

// A body as a file under `dir` carries it: every reference is the path of its target. A task that
// no file holds keeps the key, so the reference resolves once that task exists.
pub(crate) fn body_in_file_form(plan: &Plan, dir: &str, body: &str) -> String {
    let abbreviation = plan.abbreviation().ok();
    renamed_body_refs(body, |reference| {
        let number = match parse_id(ref_target(reference)) {
            Some(number) => Some(number),
            None => match reference::body_target(abbreviation, dir, reference) {
                Some(Target::Task(number)) => Some(number),
                Some(Target::Doc(name)) => return reference::doc_ref(dir, &name, reference),
                None => None,
            },
        };
        let Some(number) = number else {
            return reference.to_owned();
        };
        match plan.path_of(number) {
            Some(path) => reference::task_file_ref(dir, path, reference),
            None if reference::is_path(dir, reference) => reference.to_owned(),
            None => with_section(&plan.key(number), reference),
        }
    })
}

// `text`, from a file under `dir`, with each link to the doc `from` pointing at the doc `to`.
pub(crate) fn relinked_doc(
    abbreviation: Option<op_task::Abbreviation>,
    dir: &str,
    text: &str,
    from: &str,
    to: &str,
) -> String {
    renamed_body_refs(text, |inner| {
        match reference::body_target(abbreviation, dir, inner) {
            Some(Target::Doc(name)) if name == from => reference::doc_ref(dir, to, inner),
            _ => inner.to_owned(),
        }
    })
}

pub(crate) fn doc_in_file_form(plan: &Plan, doc: &Doc) -> Doc {
    let mut doc = doc.clone();
    doc.body = body_in_file_form(plan, layout::DOCS, &doc.body);
    doc
}

fn with_section(target: &str, reference: &str) -> String {
    match reference.split_once('#') {
        Some((_, section)) => format!("{target}#{section}"),
        None => target.to_owned(),
    }
}

// Text inside `[[…]]` that names no task is ordinary bracketed prose and stays as written.
fn renamed_body_refs(body: &str, named: impl Fn(&str) -> String) -> String {
    let mut out = String::new();
    let mut last = 0;
    for (span, inner) in op_task::body_ref_spans(body) {
        let renamed = named(inner);
        if renamed == inner {
            continue;
        }
        out.push_str(&body[last..span.start]);
        out.push_str("[[");
        out.push_str(&renamed);
        out.push_str("]]");
        last = span.end;
    }
    if last == 0 {
        return body.to_owned();
    }
    out.push_str(&body[last..]);
    out
}
