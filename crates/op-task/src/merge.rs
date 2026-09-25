use std::collections::BTreeSet;

use serde_yaml::{Mapping, Value};

use crate::comment::{self, Comment};
use crate::conflict::{self, Labels};
use crate::{FieldConflict, Frontmatter, Task, names, sorted_set, three_way};

// Both sides may add to and remove from a set, so the two edits compose and never conflict.
const SETS: [&str; 2] = ["dependencies", "tags"];

// A field or lines that both sides changed differently become a conflict that keeps both versions,
// with the published one (`theirs`) in force until someone picks.
pub fn task(base: &Task, ours: &Task, theirs: &Task, labels: &Labels) -> Task {
    let (frontmatter, conflicts) = frontmatter(base, ours, theirs, labels);
    let body = conflict::merge(
        &comment::strip(&base.body),
        &comment::strip(&ours.body),
        &comment::strip(&theirs.body),
        labels,
    );
    Task {
        frontmatter,
        conflicts,
        body: comment::with_comments(&body, &comments(&ours.body, &theirs.body)),
    }
}

fn frontmatter(
    base: &Task,
    ours: &Task,
    theirs: &Task,
    labels: &Labels,
) -> (Frontmatter, Vec<FieldConflict>) {
    let tasks = [base, ours, theirs];
    let maps = tasks.map(|task| mapping(&task.frontmatter));
    let keys: BTreeSet<&str> = maps
        .iter()
        .flat_map(Mapping::keys)
        .filter_map(Value::as_str)
        .filter(|key| !SETS.contains(key))
        .collect();
    let mut merged = Mapping::new();
    let mut conflicts = Vec::new();
    for key in keys {
        let [b, o, t] = [0, 1, 2].map(|side| {
            (
                maps[side].get(key).cloned(),
                tasks[side]
                    .conflicts
                    .iter()
                    .find(|conflict| conflict.field == key)
                    .cloned(),
            )
        });
        let (value, conflict) = match o.0 == t.0 {
            // Both sides hold one value: a conflict one side resolved stays resolved.
            true => (t.0.clone(), three_way(&b.1, &o.1, &t.1).flatten()),
            false => three_way(&b, &o, &t).unwrap_or_else(|| {
                let conflict = FieldConflict {
                    field: key.to_owned(),
                    other: o.0.clone(),
                    other_label: labels.ours.clone(),
                    label: labels.theirs.clone(),
                };
                (t.0.clone(), Some(conflict))
            }),
        };
        if let Some(value) = value {
            merged.insert(Value::String(key.to_owned()), value);
        }
        conflicts.extend(conflict);
    }
    let mut frontmatter: Frontmatter = serde_yaml::from_value(Value::Mapping(merged))
        .unwrap_or_else(|_| theirs.frontmatter.clone());
    let [base, ours, theirs] = tasks.map(|task| &task.frontmatter);
    frontmatter.dependencies = names(&base.dependencies, &ours.dependencies, &theirs.dependencies);
    frontmatter.tags = sorted_set(names(&base.tags, &ours.tags, &theirs.tags));
    (frontmatter, conflicts)
}

fn mapping(frontmatter: &Frontmatter) -> Mapping {
    match serde_yaml::to_value(frontmatter) {
        Ok(Value::Mapping(map)) => map,
        _ => Mapping::new(),
    }
}

// Append-only on both sides, so the union loses nothing and orders by the time each was written.
fn comments(ours: &str, theirs: &str) -> Vec<Comment> {
    let mut all = comment::parse(theirs);
    for entry in comment::parse(ours) {
        if !all.contains(&entry) {
            all.push(entry);
        }
    }
    all.sort_by_key(|entry| entry.at.clone().ok());
    all
}
