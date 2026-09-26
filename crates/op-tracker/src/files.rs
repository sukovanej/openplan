use std::collections::HashSet;

use op_task::{Frontmatter, Task, conflict, parse_id, rank, ref_id, ref_target};

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
    let fields = old.map_or(&[][..], |old| old.conflicts.as_slice());
    if new
        .conflicts
        .iter()
        .any(|conflict| !fields.contains(conflict))
    {
        return Err(conflict_edited());
    }
    keeps_blocks(old.map_or("", |old| &old.body), &new.body)
}

pub(crate) fn keeps_blocks(old: &str, new: &str) -> Result<(), TrackerError> {
    let blocks = block_texts(old);
    match block_texts(new).iter().all(|block| blocks.contains(block)) {
        true => Ok(()),
        false => Err(conflict_edited()),
    }
}

fn conflict_edited() -> TrackerError {
    TrackerError::Invalid(
        "a write cannot add a conflict or edit inside one; keep each conflict block as it is, or \
         replace the whole block with the text you want"
            .to_owned(),
    )
}

fn block_texts(body: &str) -> Vec<String> {
    conflict::in_body(body)
        .into_iter()
        .map(|block| body[block.range].to_owned())
        .collect()
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
        && old.and_then(|old| old.parent.as_deref()) != Some(parent.as_str())
    {
        let target = reference(parent)?;
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
        if old.is_some_and(|old| old.dependencies.contains(dependency)) {
            continue;
        }
        let target = reference(dependency)?;
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
    Ok(())
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
            Ok(task) => task.frontmatter.parent.as_deref().and_then(ref_id),
            Err(TrackerError::NotFound { .. }) => None,
            Err(err) => return Err(err),
        };
    }
    Ok(())
}

fn reference(text: &str) -> Result<u64, TrackerError> {
    ref_id(text).ok_or_else(|| TrackerError::InvalidRef {
        reference: ref_target(text).to_owned(),
    })
}

// References as the file carries them: the target's file name, so a plain markdown reader can
// follow one. A reference to a deleted task keeps its number, and a body reference that names no
// file is written as the key, so it resolves once that task exists.
pub(crate) fn in_file_form(plan: &Plan, task: &Task) -> Task {
    let named = |reference: &str| named(plan, reference);
    let mut task = task.clone();
    for conflict in &mut task.conflicts {
        conflict.other = match (conflict.field.as_str(), conflict.other.take()) {
            ("parent", Some(value)) => Some(reference_value(value, named)),
            ("dependencies", Some(serde_yaml::Value::Sequence(items))) => Some(
                items
                    .into_iter()
                    .map(|item| reference_value(item, named))
                    .collect(),
            ),
            (_, other) => other,
        };
    }
    task.frontmatter.parent = task.frontmatter.parent.as_deref().map(named);
    task.frontmatter.dependencies = task
        .frontmatter
        .dependencies
        .iter()
        .map(|reference| named(reference))
        .collect();
    task.body = body_in_file_form(plan, &task.body);
    task
}

pub(crate) fn body_in_file_form(plan: &Plan, body: &str) -> String {
    renamed_body_refs(body, |reference| {
        let renamed = named(plan, reference);
        if renamed != reference {
            return renamed;
        }
        match parse_id(ref_target(reference)) {
            Some(number) => with_section(&plan.key(number), reference),
            None => renamed,
        }
    })
}

fn named(plan: &Plan, reference: &str) -> String {
    match ref_id(reference).and_then(|number| plan.path_of(number)) {
        None => reference.to_owned(),
        Some(path) => with_section(
            &op_task::task_ref(op_task::layout::file_name(path)),
            reference,
        ),
    }
}

// A reference YAML reads as a number is one all the same.
fn reference_value(value: serde_yaml::Value, named: impl Fn(&str) -> String) -> serde_yaml::Value {
    match &value {
        serde_yaml::Value::String(text) => named(text).into(),
        serde_yaml::Value::Number(number) => named(&number.to_string()).into(),
        _ => value,
    }
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
