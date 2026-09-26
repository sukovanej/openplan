use std::collections::BTreeSet;

use op_backend::{Actor, Change, ChangeKind, Committed, LogEntry, LogQuery, Op, RevisionId};
use op_task::conflict::{self, Labels};
use op_task::content::Text;
use op_task::doc::Doc;
use op_task::layout;

use crate::plan::doc_normalized;
use crate::{HistoryQuery, Plan, Tracker, TrackerError, Updated, files, policy};

impl Tracker {
    pub fn create_doc(&self, actor: &Actor, doc: &Doc) -> Result<Updated<Doc>, TrackerError> {
        let mut doc = doc.clone();
        doc.name = doc_normalized(&doc.name)?;
        let (committed, ()) = self.write(actor, |plan| {
            plan.config()?;
            if plan.doc_names().contains(&doc.name) {
                return Err(TrackerError::DocExists {
                    name: doc.name.clone(),
                });
            }
            files::doc_keeps_conflicts(None, &doc)?;
            validate(plan, &doc.name, &doc, None)?;
            Ok((
                vec![Op::put(layout::doc_path(&doc.name), doc_text(plan, &doc)?)],
                (),
            ))
        })?;
        Ok(Updated {
            value: doc,
            committed,
        })
    }

    // One revision holds every change `mutate` makes, a rename included, so a change the tracker
    // refuses leaves the doc as it was.
    pub fn update_doc(
        &self,
        actor: &Actor,
        name: &str,
        mut mutate: impl FnMut(&mut Doc) -> Result<(), TrackerError>,
    ) -> Result<Updated<Doc>, TrackerError> {
        let name = doc_normalized(name)?;
        let (committed, doc) = self.write(actor, |plan| {
            let old = plan.doc(&name)?;
            let mut doc = old.clone();
            mutate(&mut doc)?;
            files::doc_keeps_conflicts(Some(&old), &doc)?;
            let ops = written(plan, &name, &doc)?;
            validate(plan, &name, &doc, old.frontmatter.parent.as_deref())?;
            Ok((ops, doc))
        })?;
        Ok(Updated {
            value: doc,
            committed,
        })
    }

    // As `edit_text` does for a task: lines that another writer changed since `base` merge with
    // `text`, and where both changed the same lines a conflict block keeps both versions. A new title
    // renames the doc, because a doc's name is its title.
    pub fn edit_doc_text(
        &self,
        actor: &Actor,
        name: &str,
        base: &Text,
        text: &Text,
    ) -> Result<Updated<Doc>, TrackerError> {
        let name = doc_normalized(name)?;
        let labels = Labels {
            ours: format!("{} (editor)", actor.name),
            theirs: self.doc_writer_since(&name, base)?,
        };
        let (committed, doc) = self.write(actor, |plan| {
            let old = plan.doc(&name)?;
            let current = current_body(plan, &old);
            let base = joined(plan, &current, base);
            let text = joined(plan, &current, text);
            files::doc_keeps_blocks(&base, &text)?;
            let merged = match base == current {
                true => text,
                false => conflict::merge(&base, &text, &current, &labels),
            };
            if merged == current {
                return Ok((Vec::new(), old));
            }
            let mut doc = old.clone();
            doc.body = merged;
            let title = doc.title().unwrap_or_default();
            if title != old.title().unwrap_or_default() {
                if title.trim().is_empty() {
                    return Err(TrackerError::Invalid(
                        "a doc must have a non-empty title; its name comes from it".to_owned(),
                    ));
                }
                doc.name = doc_normalized(&title)?;
            }
            let ops = written(plan, &name, &doc)?;
            Ok((ops, doc))
        })?;
        Ok(Updated {
            value: doc,
            committed,
        })
    }

    // Read before the write, for the reason `writer_since` gives.
    fn doc_writer_since(&self, name: &str, base: &Text) -> Result<String, TrackerError> {
        let plan = self.plan()?;
        let current = current_body(&plan, &plan.doc(name)?);
        if joined(&plan, &current, base) == current {
            return Ok(crate::PUBLISHED.to_owned());
        }
        let entries = self.doc_history(
            name,
            &HistoryQuery {
                before: None,
                limit: Some(1),
            },
        )?;
        Ok(entries.first().map_or_else(
            || crate::PUBLISHED.to_owned(),
            |entry| policy::label(&entry.revision),
        ))
    }

    // Each revision that changed the doc, newest first, with the name the doc had after it. A rename
    // moves the walk on to the old name, so the history runs back past it, as a task's history runs
    // back past a new title.
    pub fn doc_revisions(
        &self,
        name: &str,
        query: &HistoryQuery,
    ) -> Result<Vec<(LogEntry, String)>, TrackerError> {
        let walk = self.doc_walk(name)?;
        let start = match &query.before {
            None => 0,
            Some(before) => walk
                .iter()
                .position(|(entry, _)| entry.revision.id == *before)
                .map_or(walk.len(), |at| at + 1),
        };
        Ok(walk
            .into_iter()
            .skip(start)
            .take(query.limit.unwrap_or(usize::MAX))
            .collect())
    }

    fn doc_walk(&self, name: &str) -> Result<Vec<(LogEntry, String)>, TrackerError> {
        let mut tracked = doc_normalized(name)?;
        let log = self.backend.log(&LogQuery {
            prefix: format!("{}/", layout::DOCS),
            before: None,
            limit: None,
        })?;
        let mut walk = Vec::new();
        for entry in log {
            let path = layout::doc_path(&tracked);
            if !entry.changes.iter().any(|change| change.path == path) {
                continue;
            }
            let renamed = doc_moves(&entry.changes)
                .renamed
                .into_iter()
                .find(|(_, new)| *new == tracked);
            walk.push((entry, tracked.clone()));
            if let Some((old, _)) = renamed {
                tracked = old;
            }
        }
        Ok(walk)
    }

    // The doc's file as it stood at `revision`, under the name it had then; `None` where the doc did
    // not exist then.
    pub fn doc_at(
        &self,
        name: &str,
        revision: &RevisionId,
    ) -> Result<Option<String>, TrackerError> {
        let plan = self.plan_at(revision)?;
        let mut names = vec![doc_normalized(name)?];
        for (_, name) in self.doc_walk(name)? {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        match names.iter().find(|name| plan.doc_names().contains(*name)) {
            Some(name) => plan.raw_doc(name).map(Some),
            None => Ok(None),
        }
    }

    // `new_display_name` gives the new identity and the new H1 at once.
    pub fn rename_doc(
        &self,
        actor: &Actor,
        name: &str,
        new_display_name: &str,
    ) -> Result<Updated<Doc>, TrackerError> {
        self.update_doc(actor, name, |doc| {
            doc.rename(new_display_name)
                .map_err(|err| TrackerError::Invalid(err.to_string()))
        })
    }

    // The docs nested under the doc move up to its own parent in the same revision, so the subtree
    // keeps its place in the tree rather than naming a doc that is gone. A parent that is gone too
    // lifts them to the top level.
    pub fn delete_doc(&self, actor: &Actor, name: &str) -> Result<Option<Committed>, TrackerError> {
        let name = doc_normalized(name)?;
        let (committed, ()) = self.write(actor, |plan| {
            let parent = op_task::doc::parse_partial(&plan.raw_doc(&name)?)
                .parent()
                .filter(|parent| plan.doc_names().contains(*parent))
                .map(str::to_owned);
            let mut ops = vec![Op::remove(layout::doc_path(&name))];
            for (child, text) in plan.raw_docs()? {
                // A child that is also the parent closes a cycle, and lifting it under itself would
                // close another one.
                let to = parent.as_deref().filter(|parent| *parent != child);
                if child != name
                    && let Some(rewritten) = op_task::doc::rewrite_parent(&text, &name, to)
                {
                    ops.push(Op::put(layout::doc_path(&child), rewritten));
                }
            }
            Ok((ops, ()))
        })?;
        Ok(committed)
    }
}

// `doc` written back over the doc the plan holds as `name`. A new name moves the file, and in the same
// revision every child's `parent:` and every link to the doc move with it, else the subtree would
// drop to the top level and the links would dangle.
fn written(plan: &Plan, name: &str, doc: &Doc) -> Result<Vec<Op>, TrackerError> {
    if doc.name == name {
        return Ok(vec![Op::put(layout::doc_path(name), doc_text(plan, doc)?)]);
    }
    if plan.doc_names().contains(&doc.name) {
        return Err(TrackerError::DocExists {
            name: doc.name.clone(),
        });
    }
    let abbreviation = plan.abbreviation().ok();
    let relinked =
        |dir: &str, text: &str| files::relinked_doc(abbreviation, dir, text, name, &doc.name);
    let mut renamed = doc.clone();
    renamed.body = relinked(layout::DOCS, &doc.body);
    let mut ops = vec![
        Op::remove(layout::doc_path(name)),
        Op::put(layout::doc_path(&doc.name), doc_text(plan, &renamed)?),
    ];
    // The swap goes through the frontmatter mapping, so a child the model rejects still follows its
    // parent. The doc's own old file is gone, and a rewrite of it would bring it back.
    for (other, text) in plan.raw_docs()? {
        if other == name {
            continue;
        }
        let moved = op_task::doc::rewrite_parent(&text, name, Some(&doc.name));
        let rewritten = relinked(layout::DOCS, moved.as_deref().unwrap_or(&text));
        if rewritten != text {
            ops.push(Op::put(layout::doc_path(&other), rewritten));
        }
    }
    for (number, text) in plan.raw_all()? {
        let rewritten = relinked(layout::TASKS, &text);
        if rewritten != text
            && let Some(path) = plan.path_of(number)
        {
            ops.push(Op::put(path, rewritten));
        }
    }
    Ok(ops)
}

fn doc_text(plan: &Plan, doc: &Doc) -> Result<String, TrackerError> {
    files::doc_in_file_form(plan, doc)
        .to_file_string()
        .map_err(|err| TrackerError::Invalid(err.to_string()))
}

fn current_body(plan: &Plan, doc: &Doc) -> String {
    files::line_ended(&files::body_in_file_form(plan, layout::DOCS, &doc.body))
}

fn joined(plan: &Plan, current: &str, text: &Text) -> String {
    let content = files::body_in_file_form(plan, layout::DOCS, &text.description);
    files::line_ended(&op_task::doc::joined(current, &text.title, &content))
}

// Only a parent this write sets is checked: one the doc already names may dangle because its doc
// was deleted, and that must not block an edit to the body. `name` is the doc's name in the plan,
// which a rename in the same write leaves behind.
fn validate(plan: &Plan, name: &str, doc: &Doc, was: Option<&str>) -> Result<(), TrackerError> {
    let Some(parent) = doc.frontmatter.parent.as_deref() else {
        return Ok(());
    };
    if was == Some(parent) {
        return Ok(());
    }
    if parent == name || parent == doc.name {
        return Err(TrackerError::Invalid(format!(
            "doc {} cannot be its own parent",
            doc.name
        )));
    }
    if !plan.doc_names().contains(parent) {
        return Err(TrackerError::Invalid(format!(
            "parent doc {parent} does not exist"
        )));
    }
    refuse_cycle(plan, name, parent)
}

// Walks up from the new parent; reaching the doc itself would close a cycle. The plan names the doc
// by `name`, so a rename in the same write does not hide it. The visited set stops a cycle the docs
// already carry from looping forever. An ancestor is read leniently, so one the model rejects still
// names its parent.
fn refuse_cycle(plan: &Plan, name: &str, parent: &str) -> Result<(), TrackerError> {
    let mut seen = BTreeSet::new();
    let mut cursor = Some(parent.to_owned());
    while let Some(current) = cursor {
        if current == name {
            return Err(TrackerError::Invalid(format!(
                "cannot nest {name} under its own descendant {parent}"
            )));
        }
        if !seen.insert(current.clone()) {
            break;
        }
        cursor = match plan.raw_doc(&current) {
            Ok(text) => op_task::doc::parse_partial(&text)
                .parent()
                .map(str::to_owned),
            Err(TrackerError::DocNotFound { .. }) => None,
            Err(err) => return Err(err),
        };
    }
    Ok(())
}

pub struct DocMoves {
    pub created: BTreeSet<String>,
    pub renamed: Vec<(String, String)>,
}

// A change that removes one doc and adds one other renames it; any other added doc is new.
pub fn doc_moves(changes: &[Change]) -> DocMoves {
    let names = |kind: ChangeKind| -> BTreeSet<String> {
        changes
            .iter()
            .filter(|change| change.kind == kind)
            .filter_map(|change| layout::doc_name(&change.path).map(str::to_owned))
            .collect()
    };
    let added = names(ChangeKind::Added);
    let removed = names(ChangeKind::Removed);
    match (removed.len(), added.len()) {
        (1, 1) if removed != added => DocMoves {
            created: BTreeSet::new(),
            renamed: removed.into_iter().zip(added).collect(),
        },
        _ => DocMoves {
            created: &added - &removed,
            renamed: Vec::new(),
        },
    }
}
