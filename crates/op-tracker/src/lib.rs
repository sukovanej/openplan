use std::sync::Arc;

use op_backend::{
    Actor, Backend, BackendError, BackendExt as _, ChangeKind, Committed, Edit, LogEntry, LogQuery,
    Op, RevisionId, Snapshot,
};
use op_task::comment::{self, NewComment};
use op_task::config::Config;
use op_task::conflict::{self, Labels};
use op_task::content::{self, Text};
use op_task::layout;
use op_task::tag::Tag;
use op_task::{Abbreviation, PartialMetadata, Task, parse_partial};

mod describe;
mod docs;
mod error;
mod files;
mod message;
mod plan;
mod policy;

pub use describe::{Described, DocChange, FieldChange, TagChange, TaskChange};
pub use docs::{DocMoves, doc_moves};
pub use error::TrackerError;
pub use plan::Plan;
pub use policy::TaskMergePolicy;

#[derive(Clone)]
pub struct Tracker {
    backend: Arc<dyn Backend>,
}

#[derive(Debug, Clone)]
pub struct Created {
    pub number: u64,
    pub committed: Option<Committed>,
}

#[derive(Debug, Clone)]
pub struct Updated<T> {
    pub value: T,
    pub committed: Option<Committed>,
}

// A side that does not hold the document is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versions {
    pub before: Option<Vec<u8>>,
    pub after: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryQuery {
    pub before: Option<RevisionId>,
    pub limit: Option<usize>,
}

impl Tracker {
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &Arc<dyn Backend> {
        &self.backend
    }

    pub fn plan(&self) -> Result<Plan, TrackerError> {
        Plan::read(self.backend.head()?)
    }

    pub fn plan_at(&self, revision: &RevisionId) -> Result<Plan, TrackerError> {
        Plan::read(self.backend.at(revision)?)
    }

    pub(crate) fn write<T>(
        &self,
        actor: &Actor,
        mut write: impl FnMut(&Plan) -> Result<(Vec<Op>, T), TrackerError>,
    ) -> Result<(Option<Committed>, T), TrackerError> {
        self.backend.transact(actor, |snapshot: Arc<dyn Snapshot>| {
            let plan = Plan::read(snapshot)?;
            let (ops, value) = write(&plan)?;
            let message = message::of(&plan, &ops)?;
            Ok((Edit::new(message, ops), value))
        })
    }

    // The registry of default tags is born with the project, so a project deleting one keeps it
    // deleted.
    pub fn init(
        &self,
        actor: &Actor,
        abbreviation: Abbreviation,
    ) -> Result<Option<Committed>, TrackerError> {
        let (committed, ()) = self.write(actor, |plan| {
            match plan.config() {
                Ok(config) if config.abbreviation == abbreviation => {
                    return Ok((Vec::new(), ()));
                }
                Ok(config) => {
                    return Err(TrackerError::AlreadyInitialized(
                        config.abbreviation.to_string(),
                    ));
                }
                Err(TrackerError::NotInitialized) => {}
                Err(err) => return Err(err),
            }
            let mut ops = vec![Op::put(
                layout::CONFIG,
                Config::new(abbreviation).to_file_string(),
            )];
            if plan.tag_names().is_empty() {
                for tag in op_task::tag::defaults() {
                    ops.push(Op::put(layout::tag_path(&tag.name), tag_text(&tag)?));
                }
            }
            Ok((ops, ()))
        })?;
        Ok(committed)
    }

    pub fn create_task(&self, actor: &Actor, task: &Task) -> Result<Created, TrackerError> {
        let title = files::single_title(&task.body)?;
        files::keeps_conflicts(None, task)?;
        let (committed, number) = self.write(actor, |plan| {
            plan.config()?;
            files::validate(plan, None, None, &task.frontmatter)?;
            let number = match plan.max_number() {
                None => 1,
                Some(max) => max.checked_add(1).ok_or_else(|| {
                    TrackerError::Invalid(format!(
                        "{} holds the highest task number there is; no number is left",
                        plan.key(max)
                    ))
                })?,
            };
            let text = files::in_file_form(plan, task).to_file_string()?;
            Ok((
                vec![Op::put(layout::task_path(number, &title), text)],
                number,
            ))
        })?;
        Ok(Created { number, committed })
    }

    pub fn update_task(
        &self,
        actor: &Actor,
        number: u64,
        mut mutate: impl FnMut(&mut Task) -> Result<(), TrackerError>,
    ) -> Result<Updated<Task>, TrackerError> {
        let (committed, task) = self.write(actor, |plan| {
            let path = plan
                .path_of(number)
                .ok_or_else(|| plan.not_found(number))?
                .to_owned();
            let old = plan.task(number)?;
            let mut task = old.clone();
            mutate(&mut task)?;
            if task.body != old.body {
                files::single_title(&task.body)?;
            }
            files::validate(
                plan,
                Some(number),
                Some(&old.frontmatter),
                &task.frontmatter,
            )?;
            let in_file_form = files::in_file_form(plan, &task);
            files::keeps_conflicts(Some(&old), &in_file_form)?;
            let text = in_file_form.to_file_string()?;
            Ok((vec![Op::put(path, text)], task))
        })?;
        Ok(Updated {
            value: task,
            committed,
        })
    }

    // `base` is the text as the caller last read it, and `text` is the text the caller wants, both in
    // the store spelling. Lines that another writer changed since `base` merge with `text`. Where both
    // changed the same lines, a conflict block keeps both versions, and the published one stays in
    // force. So the check on conflicts compares `text` with `base`: a block the merge makes is not
    // the caller's edit.
    pub fn edit_text(
        &self,
        actor: &Actor,
        number: u64,
        base: &Text,
        text: &Text,
    ) -> Result<Updated<Task>, TrackerError> {
        if !comment::sections(&text.description).is_empty() {
            return Err(TrackerError::Invalid(format!(
                "the description cannot hold a `## {}` section; the comment log is append-only, \
                 so add a comment with `openplan comment`",
                comment::HEADING
            )));
        }
        let labels = Labels {
            ours: format!("{} (editor)", actor.name),
            theirs: self.writer_since(number, base)?,
        };
        let (committed, task) = self.write(actor, |plan| {
            let path = plan
                .path_of(number)
                .ok_or_else(|| plan.not_found(number))?
                .to_owned();
            let old = plan.task(number)?;
            let current = current_body(plan, &old);
            let base = joined(plan, &current, base)?;
            let text = joined(plan, &current, text)?;
            files::keeps_blocks(&base, &text)?;
            let merged = match base == current {
                true => text,
                false => conflict::merge(&base, &text, &current, &labels),
            };
            if merged == current {
                return Ok((Vec::new(), old));
            }
            files::single_title(&merged)?;
            let log = comment::sections(&old.body)
                .into_iter()
                .map(|section| &old.body[section.span]);
            let mut task = old.clone();
            task.body = op_md::paragraphs(std::iter::once(merged.as_str()).chain(log));
            files::validate(
                plan,
                Some(number),
                Some(&old.frontmatter),
                &task.frontmatter,
            )?;
            let text = files::in_file_form(plan, &task).to_file_string()?;
            Ok((vec![Op::put(path, text)], task))
        })?;
        Ok(Updated {
            value: task,
            committed,
        })
    }

    // The name the merge gives the version another writer published since `base`. It is read before
    // the write, because the backend holds its lock while a write runs and a log read takes it too. A
    // body nobody moved needs no name, and most saves are that, so they read no log. A writer that
    // lands in between is named one revision late, or `published`.
    fn writer_since(&self, number: u64, base: &Text) -> Result<String, TrackerError> {
        let plan = self.plan()?;
        let current = current_body(&plan, &plan.task(number)?);
        if joined(&plan, &current, base)? == current {
            return Ok(PUBLISHED.to_owned());
        }
        let entries = self.task_history(
            number,
            &HistoryQuery {
                before: None,
                limit: Some(1),
            },
        )?;
        Ok(entries.first().map_or_else(
            || PUBLISHED.to_owned(),
            |entry| policy::label(&entry.revision),
        ))
    }

    pub fn add_comment(
        &self,
        actor: &Actor,
        number: u64,
        comment: &NewComment,
    ) -> Result<Option<Committed>, TrackerError> {
        let (committed, ()) = self.write(actor, |plan| {
            let path = plan
                .path_of(number)
                .ok_or_else(|| plan.not_found(number))?
                .to_owned();
            let mut task = plan.task(number)?;
            task.append_comment(comment);
            Ok((vec![Op::put(path, task.to_file_string()?)], ()))
        })?;
        Ok(committed)
    }

    pub fn delete_task(
        &self,
        actor: &Actor,
        number: u64,
    ) -> Result<Option<Committed>, TrackerError> {
        let (committed, ()) = self.write(actor, |plan| {
            let path = plan
                .path_of(number)
                .ok_or_else(|| plan.not_found(number))?
                .to_owned();
            Ok((vec![Op::remove(path)], ()))
        })?;
        Ok(committed)
    }

    pub fn create_tag(&self, actor: &Actor, tag: &Tag) -> Result<Tag, TrackerError> {
        let mut tag = tag.clone();
        tag.name = plan::normalized(&tag.name)?;
        let (_, ()) = self.write(actor, |plan| {
            plan.config()?;
            if plan.tag_names().contains(&tag.name) {
                return Err(TrackerError::TagExists {
                    name: tag.name.clone(),
                });
            }
            Ok((
                vec![Op::put(layout::tag_path(&tag.name), tag_text(&tag)?)],
                (),
            ))
        })?;
        Ok(tag)
    }

    pub fn update_tag(
        &self,
        actor: &Actor,
        name: &str,
        mut mutate: impl FnMut(&mut Tag) -> Result<(), TrackerError>,
    ) -> Result<Tag, TrackerError> {
        let name = plan::normalized(name)?;
        let (_, tag) = self.write(actor, |plan| {
            let mut tag = plan.tag(&name)?;
            mutate(&mut tag)?;
            if tag.name != name {
                return Err(TrackerError::Invalid(format!(
                    "an update cannot rename {name} to {}; a rename also moves the tasks that \
                     reference it",
                    tag.name
                )));
            }
            Ok((vec![Op::put(layout::tag_path(&name), tag_text(&tag)?)], tag))
        })?;
        Ok(tag)
    }

    // `new_display_name` gives the new identity and the new heading at once. Returns the tag and
    // the tasks whose `tags:` the rename rewrote, all in one revision.
    pub fn rename_tag(
        &self,
        actor: &Actor,
        name: &str,
        new_display_name: &str,
    ) -> Result<Updated<(Tag, Vec<u64>)>, TrackerError> {
        let name = plan::normalized(name)?;
        let (committed, renamed) = self.write(actor, |plan| {
            let mut renamed = plan.tag(&name)?;
            renamed
                .rename(new_display_name)
                .map_err(|err| TrackerError::Invalid(err.to_string()))?;
            if renamed.name == name {
                return Ok((
                    vec![Op::put(layout::tag_path(&name), tag_text(&renamed)?)],
                    (renamed, Vec::new()),
                ));
            }
            if plan.tag_names().contains(&renamed.name) {
                return Err(TrackerError::TagExists {
                    name: renamed.name.clone(),
                });
            }
            let mut ops = vec![
                Op::remove(layout::tag_path(&name)),
                Op::put(layout::tag_path(&renamed.name), tag_text(&renamed)?),
            ];
            let referencing = tagged(plan, &name)?;
            for number in &referencing {
                let mut task = plan.task(*number)?;
                let tags = task
                    .frontmatter
                    .tags
                    .iter()
                    .map(|tag| match tag == &name {
                        true => renamed.name.clone(),
                        false => tag.clone(),
                    })
                    .collect();
                task.set_tags(tags);
                let path = plan.path_of(*number).expect("a tagged task has a path");
                ops.push(Op::put(path, task.to_file_string()?));
            }
            Ok((ops, (renamed, referencing)))
        })?;
        Ok(Updated {
            value: renamed,
            committed,
        })
    }

    pub fn delete_tag(&self, actor: &Actor, name: &str, force: bool) -> Result<(), TrackerError> {
        let name = plan::normalized(name)?;
        self.write(actor, |plan| {
            if !plan.tag_names().contains(&name) {
                return Err(TrackerError::TagNotFound { name: name.clone() });
            }
            if !force {
                let count = tagged(plan, &name)?.len();
                if count > 0 {
                    return Err(TrackerError::TagReferenced {
                        name: name.clone(),
                        count,
                    });
                }
            }
            Ok((vec![Op::remove(layout::tag_path(&name))], ()))
        })?;
        Ok(())
    }

    pub fn history(&self, query: &HistoryQuery) -> Result<Vec<LogEntry>, TrackerError> {
        Ok(self.backend.log(&LogQuery {
            prefix: String::new(),
            before: query.before.clone(),
            limit: query.limit,
        })?)
    }

    pub fn doc_history(
        &self,
        name: &str,
        query: &HistoryQuery,
    ) -> Result<Vec<LogEntry>, TrackerError> {
        Ok(self.backend.log(&LogQuery {
            prefix: layout::doc_path(&plan::doc_normalized(name)?),
            before: query.before.clone(),
            limit: query.limit,
        })?)
    }

    pub fn task_history(
        &self,
        number: u64,
        query: &HistoryQuery,
    ) -> Result<Vec<LogEntry>, TrackerError> {
        Ok(self.backend.log(&LogQuery {
            prefix: layout::task_prefix(number),
            before: query.before.clone(),
            limit: query.limit,
        })?)
    }

    // Read from the documents themselves, not from the message: a revision that another tool wrote
    // says what it changes in its own words, or in none. A merge is described against its first
    // parent.
    pub fn describe(&self, entry: &LogEntry) -> Result<Described, TrackerError> {
        let revision = &entry.revision;
        let before = |path: &str| match revision.parents.first() {
            Some(parent) => absent_when_unknown(self.backend.read_at(parent, path)),
            None => Ok(None),
        };
        let after = |path: &str| self.backend.read_at(&revision.id, path);
        let adds_a_task = entry.changes.iter().any(|change| {
            change.kind == ChangeKind::Added && layout::task_number(&change.path).is_some()
        });
        let moved_from = match revision.parents.as_slice() {
            [first, _, ..] if adds_a_task => match self.backend.at(first) {
                Err(BackendError::UnknownRevision(_)) => None,
                snapshot => Some(snapshot?),
            },
            _ => None,
        };
        Ok(describe::describe(
            &entry.changes,
            &before,
            &after,
            moved_from.as_deref(),
        )?)
    }

    // `None` where the revision leaves the document as its first parent had it. `before` names the
    // document on the parent's side, where a new title moved a task to a file with a new name.
    pub fn versions(
        &self,
        revision: &RevisionId,
        before: &str,
        after: &str,
    ) -> Result<Option<Versions>, TrackerError> {
        let revision = self.backend.revision(revision)?;
        let old = match revision.parents.first() {
            Some(parent) => absent_when_unknown(self.backend.read_at(parent, before))?,
            None => None,
        };
        let new = self.backend.read_at(&revision.id, after)?;
        Ok((old != new).then_some(Versions {
            before: old,
            after: new,
        }))
    }

    // The task's file as it stood at `revision`; `None` where the task did not exist then.
    pub fn task_at(
        &self,
        number: u64,
        revision: &RevisionId,
    ) -> Result<Option<String>, TrackerError> {
        let plan = self.plan_at(revision)?;
        match plan.exists(number) {
            true => plan.raw(number).map(Some),
            false => Ok(None),
        }
    }
}

// A shallow clone holds its oldest revisions without their parents, and a missing parent reads as
// empty rather than fail the whole page.
fn absent_when_unknown(
    read: Result<Option<Vec<u8>>, BackendError>,
) -> Result<Option<Vec<u8>>, BackendError> {
    match read {
        Err(BackendError::UnknownRevision(_)) => Ok(None),
        read => read,
    }
}

pub(crate) const PUBLISHED: &str = "published";

fn current_body(plan: &Plan, task: &Task) -> String {
    files::line_ended(&files::body_in_file_form(
        plan,
        layout::TASKS,
        &comment::strip(&task.body),
    ))
}

// The body the text makes in the layout of the current one, so the parts the text does not carry
// (the title line's markup, the blank lines around it) are no change.
fn joined(plan: &Plan, current: &str, text: &Text) -> Result<String, TrackerError> {
    let description = files::body_in_file_form(plan, layout::TASKS, &text.description);
    content::join(current, &text.title, &description)
        .map(|body| files::line_ended(&body))
        .map_err(|err| TrackerError::Invalid(err.to_string()))
}

fn tag_text(tag: &Tag) -> Result<String, TrackerError> {
    tag.to_file_string()
        .map_err(|err| TrackerError::Unreadable {
            path: layout::tag_path(&tag.name),
            reason: err.to_string(),
        })
}

// Read leniently: a task file too broken to write back is still a reference a delete must count.
fn tagged(plan: &Plan, name: &str) -> Result<Vec<u64>, TrackerError> {
    let mut numbers = Vec::new();
    for (number, raw) in plan.raw_all()? {
        if let PartialMetadata::Fields(fields) = parse_partial(&raw).metadata
            && fields
                .tags
                .is_ok_and(|tags| tags.iter().any(|tag| tag == name))
        {
            numbers.push(number);
        }
    }
    Ok(numbers)
}
