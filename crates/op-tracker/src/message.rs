use op_task::Task;

use crate::Plan;

pub(crate) fn created(key: &str, title: &str) -> String {
    format!("{key}: create \"{title}\"")
}

pub(crate) fn deleted(key: &str, title: &str) -> String {
    format!("{key}: delete \"{title}\"")
}

pub(crate) fn commented(key: &str) -> String {
    format!("{key}: comment")
}

pub(crate) fn updated(plan: &Plan, key: &str, old: &Task, new: &Task) -> String {
    let (before, after) = (&old.frontmatter, &new.frontmatter);
    let mut parts = Vec::new();
    if before.status != after.status {
        parts.push(format!("status → {}", after.status.as_str()));
    }
    if before.parent != after.parent {
        parts.push(match after.parent.as_deref().and_then(op_task::ref_id) {
            Some(parent) => format!("parent → {}", plan.key(parent)),
            None => "no parent".to_owned(),
        });
    }
    if before.rank != after.rank {
        parts.push("order".to_owned());
    }
    if before.dependencies != after.dependencies {
        parts.push("dependencies".to_owned());
    }
    if before.tags != after.tags {
        parts.push(match after.tags.is_empty() {
            true => "no tags".to_owned(),
            false => format!("tags → {}", after.tags.join(", ")),
        });
    }
    if old.title() != new.title() {
        parts.push(format!("title → \"{}\"", new.title().unwrap_or_default()));
    }
    if strip_title(&old.body) != strip_title(&new.body) {
        parts.push("description".to_owned());
    }
    if before.extra != after.extra || before.created != after.created {
        parts.push("fields".to_owned());
    }
    match parts.is_empty() {
        true => format!("{key}: edit"),
        false => format!("{key}: {}", parts.join(", ")),
    }
}

fn strip_title(body: &str) -> String {
    body.lines()
        .filter(|line| !line.starts_with("# "))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn tag_created(name: &str) -> String {
    format!("tag {name}: create")
}

pub(crate) fn tag_updated(name: &str) -> String {
    format!("tag {name}: edit")
}

pub(crate) fn tag_renamed(from: &str, to: &str) -> String {
    format!("tag {from}: rename to {to}")
}

pub(crate) fn tag_deleted(name: &str) -> String {
    format!("tag {name}: delete")
}
