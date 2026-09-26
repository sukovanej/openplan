use std::collections::{BTreeSet, HashMap, HashSet};

use op_api::{
    DocChild, DocDetail, DocListItem, DocMetadata, DocRef, Problem, ProblemCode, TaskListItem,
    updated_field,
};
use op_backend::Timestamp;
use op_task::reference::{self, Target};
use op_task::{Abbreviation, FieldError, FieldResult, layout};
use op_tracker::{Plan, TrackerError};

use crate::Index;

#[derive(Debug, Clone)]
pub(crate) struct DocEntry {
    raw: String,
    title: String,
    created: FieldResult<Timestamp>,
    metadata: DocMetadata,
    conflicts: usize,
    diagram_problems: Vec<String>,
    // What the published text names, found or not.
    task_refs: Vec<u64>,
    doc_refs: Vec<String>,
    unpathed: Vec<(String, Target)>,
}

impl DocEntry {
    fn parse(raw: String, abbreviation: Option<Abbreviation>) -> Self {
        let partial = op_task::doc::parse_partial(&raw);
        let text = op_task::conflict::published(&partial.body);
        let targets: Vec<Target> = op_task::body_ref_spans(&text)
            .into_iter()
            .filter_map(|(_, inner)| reference::body_target(abbreviation, layout::DOCS, inner))
            .collect();
        Self {
            title: partial.title.clone().unwrap_or_default(),
            created: partial.created(),
            metadata: DocMetadata::from_partial(&partial.metadata, &partial.conflicts),
            conflicts: partial.conflict_count(),
            diagram_problems: crate::diagram_problems(&raw),
            task_refs: targets
                .iter()
                .filter_map(|target| match target {
                    Target::Task(number) => Some(*number),
                    Target::Doc(_) => None,
                })
                .collect(),
            doc_refs: targets
                .into_iter()
                .filter_map(|target| match target {
                    Target::Doc(name) => Some(name),
                    Target::Task(_) => None,
                })
                .collect(),
            unpathed: reference::unpathed(abbreviation, layout::DOCS, &text),
            raw,
        }
    }
}

impl Index {
    pub(crate) fn load_docs(&mut self, plan: &Plan) -> Result<(), TrackerError> {
        let raw = plan.raw_docs()?;
        self.docs.retain(|name, _| raw.contains_key(name));
        for (name, text) in raw {
            self.put_doc(name, text);
        }
        Ok(())
    }

    // A task that names a doc has a problem when the doc goes, so the problems cover all again.
    pub fn update_docs(
        &mut self,
        plan: &Plan,
        names: &BTreeSet<String>,
    ) -> Result<(), TrackerError> {
        if names.is_empty() {
            return Ok(());
        }
        for name in names {
            match plan.doc_names().contains(name) {
                true => self.put_doc(name.clone(), plan.raw_doc(name)?),
                false => {
                    self.docs.remove(name);
                }
            }
        }
        self.problems = self.find_problems(plan.tag_names(), plan.shadowed());
        Ok(())
    }

    fn put_doc(&mut self, name: String, text: String) {
        if self.docs.get(&name).is_none_or(|entry| entry.raw != text) {
            self.docs
                .insert(name, DocEntry::parse(text, self.abbreviation));
        }
    }

    pub fn doc_problems(&self, name: &str) -> Vec<Problem> {
        self.docs
            .get(name)
            .map(|entry| self.problems_of_doc(name, entry))
            .unwrap_or_default()
    }

    fn problems_of_doc(&self, name: &str, entry: &DocEntry) -> Vec<Problem> {
        let mut found = Vec::new();
        let mut push = |code, message| found.push(Problem { code, message });
        for problem in entry.metadata.problems() {
            push(ProblemCode::Field, problem);
        }
        for problem in &entry.diagram_problems {
            push(ProblemCode::Diagram, problem.clone());
        }
        if let Some(parent) = entry.metadata.parent() {
            if !self.docs.contains_key(parent) {
                push(
                    ProblemCode::Reference,
                    format!("the parent doc {parent} does not exist"),
                );
            } else if self.in_parent_cycle(name) {
                push(
                    ProblemCode::ParentCycle,
                    "the doc is its own ancestor through its parents".to_owned(),
                );
            }
        }
        let tasks: BTreeSet<u64> = entry.task_refs.iter().copied().collect();
        for number in tasks.into_iter().filter(|number| !self.contains(*number)) {
            push(
                ProblemCode::Reference,
                format!("the text names {}, which does not exist", self.key(number)),
            );
        }
        let docs: BTreeSet<&String> = entry.doc_refs.iter().collect();
        for doc in docs.into_iter().filter(|doc| !self.docs.contains_key(*doc)) {
            push(
                ProblemCode::Reference,
                format!("the text names the doc {doc}, which does not exist"),
            );
        }
        for (spelled, target) in &entry.unpathed {
            if let Some(message) = self.unpathed_message(spelled, target) {
                push(ProblemCode::ReferencePath, message);
            }
        }
        found.sort_by(|a, b| (a.code, &a.message).cmp(&(b.code, &b.message)));
        found
    }

    // The visited set stops a cycle above the doc from looping forever.
    fn in_parent_cycle(&self, name: &str) -> bool {
        let mut seen = HashSet::new();
        let mut cursor = self
            .docs
            .get(name)
            .and_then(|entry| entry.metadata.parent());
        while let Some(current) = cursor {
            if current == name {
                return true;
            }
            if !seen.insert(current) {
                return false;
            }
            cursor = self
                .docs
                .get(current)
                .and_then(|entry| entry.metadata.parent());
        }
        false
    }

    pub fn touch_doc(&mut self, name: &str, at: Timestamp) {
        self.doc_updated.insert(name.to_owned(), at);
    }

    pub fn list_docs(&self, project: &str) -> Vec<DocListItem> {
        self.docs
            .iter()
            .map(|(name, entry)| self.doc_row(project, name, entry))
            .collect()
    }

    pub fn doc_detail(&self, project: &str, name: &str) -> Option<DocDetail> {
        let entry = self.docs.get(name)?;
        let partial = op_task::doc::parse_partial(&entry.raw);
        let parent_title = partial
            .parent()
            .and_then(|parent| self.docs.get(parent))
            .map(|parent| parent.title.clone());
        let children = self
            .docs
            .iter()
            .filter(|(_, child)| child.metadata.parent() == Some(name))
            .map(|(child, entry)| DocChild {
                name: child.clone(),
                title: entry.title.clone(),
            })
            .collect();
        Some(DocDetail {
            project: project.to_owned(),
            name: name.to_owned(),
            title: entry.title.clone(),
            metadata: entry.metadata.clone(),
            updated: self.doc_updated_of(name, entry),
            refs: self.task_refs_in(project, &partial.body),
            doc_refs: self.doc_refs_in(layout::DOCS, &partial.body),
            parent_title,
            children,
            body: self.body_of(&op_task::doc::content(&partial.body)),
            conflicts: entry.conflicts,
            problems: self.problems_of_doc(name, entry),
            author: self.doc_authors.get(name).cloned(),
        })
    }

    // Without the store's abbreviation no reference can be spelled as a key, so the body keeps the
    // spelling of the file.
    fn body_of(&self, content: &str) -> String {
        match self.abbreviation {
            Some(abbreviation) => op_api::body_to_keys(abbreviation, layout::DOCS, content),
            None => content.to_owned(),
        }
    }

    // Every `[[…]]` naming a doc that exists, once each, in first-seen order. A dangling one is
    // left out: the client renders it as a dangling chip, which needs no title. `dir` is the
    // directory of the file the body comes from.
    pub fn doc_refs_in(&self, dir: &str, body: &str) -> Vec<DocRef> {
        let mut seen = HashSet::new();
        op_task::doc::body_doc_names(self.abbreviation, dir, body)
            .into_iter()
            .filter(|name| seen.insert(name.clone()))
            .filter_map(|name| {
                let entry = self.docs.get(&name)?;
                Some(DocRef {
                    title: entry.title.clone(),
                    name,
                })
            })
            .collect()
    }

    fn task_refs_in(&self, project: &str, body: &str) -> Vec<op_api::TaskRef> {
        let Some(abbreviation) = self.abbreviation else {
            return Vec::new();
        };
        let rows = self.list(project);
        let by_id: HashMap<&str, &TaskListItem> =
            rows.iter().map(|row| (row.id.as_str(), row)).collect();
        crate::body_refs(abbreviation, layout::DOCS, body, &by_id)
    }

    fn doc_row(&self, project: &str, name: &str, entry: &DocEntry) -> DocListItem {
        DocListItem {
            project: project.to_owned(),
            name: name.to_owned(),
            title: entry.title.clone(),
            metadata: entry.metadata.clone(),
            updated: self.doc_updated_of(name, entry),
            conflicts: entry.conflicts,
            problems: self.problems_of_doc(name, entry),
            author: self.doc_authors.get(name).cloned(),
        }
    }

    fn doc_updated_of(&self, name: &str, entry: &DocEntry) -> op_api::Field<op_api::Rfc3339> {
        updated_field(
            entry.created.clone().ok(),
            self.doc_updated
                .get(name)
                .copied()
                .ok_or(FieldError::Missing),
        )
    }
}
