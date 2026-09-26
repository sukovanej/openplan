use std::collections::BTreeSet;

use serde::Serialize;
use serde_yaml::{Mapping, Value};

use crate::comment::{self, Comment};
use crate::conflict::{self, Labels};
use crate::doc::{Doc, DocFrontmatter};
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

// A doc both sides created is one doc from the first time it was written, so `created` keeps the
// earlier time. No command sets a field the model does not name, so such a field keeps the
// published version rather than a conflict nobody could settle. Only `parent` can stay in conflict.
pub fn doc(base: Option<&Doc>, ours: &Doc, theirs: &Doc, labels: &Labels) -> Doc {
    let maps = [
        base.map_or_else(Mapping::new, |base| mapping(&base.frontmatter)),
        mapping(&ours.frontmatter),
        mapping(&theirs.frontmatter),
    ];
    let held = [
        base.map_or(&[][..], |base| base.conflicts.as_slice()),
        &ours.conflicts,
        &theirs.conflicts,
    ];
    let (merged, conflicts) = fields(&maps, held, &[], labels);
    let mut frontmatter: DocFrontmatter = serde_yaml::from_value(Value::Mapping(merged))
        .unwrap_or_else(|_| theirs.frontmatter.clone());
    frontmatter.created = ours.frontmatter.created.min(theirs.frontmatter.created);
    Doc {
        name: theirs.name.clone(),
        frontmatter,
        conflicts: conflicts
            .into_iter()
            .filter(|conflict| conflict.field == "parent")
            .collect(),
        body: conflict::merge(
            base.map_or("", |base| &base.body),
            &ours.body,
            &theirs.body,
            labels,
        ),
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
    let (merged, conflicts) = fields(
        &maps,
        tasks.map(|task| task.conflicts.as_slice()),
        &SETS,
        labels,
    );
    let mut frontmatter: Frontmatter = serde_yaml::from_value(Value::Mapping(merged))
        .unwrap_or_else(|_| theirs.frontmatter.clone());
    let [base, ours, theirs] = tasks.map(|task| &task.frontmatter);
    frontmatter.dependencies = names(&base.dependencies, &ours.dependencies, &theirs.dependencies);
    frontmatter.tags = sorted_set(names(&base.tags, &ours.tags, &theirs.tags));
    (frontmatter, conflicts)
}

// Every field but those `skip` names, merged three ways: base, ours, and theirs.
fn fields(
    maps: &[Mapping; 3],
    held: [&[FieldConflict]; 3],
    skip: &[&str],
    labels: &Labels,
) -> (Mapping, Vec<FieldConflict>) {
    let keys: BTreeSet<&str> = maps
        .iter()
        .flat_map(Mapping::keys)
        .filter_map(Value::as_str)
        .filter(|key| !skip.contains(key))
        .collect();
    let mut merged = Mapping::new();
    let mut conflicts = Vec::new();
    for key in keys {
        let [b, o, t] = [0, 1, 2].map(|side| {
            (
                maps[side].get(key).cloned(),
                held[side]
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
    (merged, conflicts)
}

fn mapping(frontmatter: &impl Serialize) -> Mapping {
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
