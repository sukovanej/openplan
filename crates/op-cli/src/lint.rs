use std::collections::HashSet;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Result, bail};
use op_api::{DocListItem, ProblemCode, TaskListItem};
use op_index::Index;
use op_server::Location;
use op_tracker::Tracker;
use serde::Serialize;

#[derive(Serialize)]
struct Finding {
    #[serde(skip_serializing_if = "Option::is_none")]
    task: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    doc: Option<String>,
    code: &'static str,
    message: String,
    help: &'static str,
}

// The tasks are read where they live, without the daemon, as CI and a fresh clone need them. The
// daemon finds the same problems after every change; this prints them for one run.
pub fn run(root: &Path, keys: &[String], json: bool) -> Result<ExitCode> {
    let mut findings = Vec::new();
    let location = Location::find_or_join(root)?;
    let signer = op_backend_git::signer(&location.root);
    let backend = op_server::open_backend(&location, &signer, false)?;
    // A hand edit that no one can sign stays out of the history, and a check without it would
    // pass the files as they were.
    backend.refresh()?;
    let tracker = Tracker::new(backend);
    let mut index = Index::new();
    index.load(&tracker.plan()?)?;
    let wanted = wanted(&index, keys)?;
    let mut tasks = 0;
    for row in index
        .list("")
        .iter()
        .filter(|row| wanted.as_ref().is_none_or(|keys| keys.contains(&row.id)))
    {
        tasks += 1;
        findings.extend(task_findings(row));
    }
    let mut docs = 0;
    if wanted.is_none() {
        let rows = index.list_docs("");
        docs = rows.len();
        findings.extend(rows.iter().flat_map(doc_findings));
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&findings)?);
    } else {
        for finding in &findings {
            let place = match (&finding.task, &finding.doc) {
                (Some(task), _) => task.clone(),
                (None, Some(doc)) => format!("doc {doc}"),
                (None, None) => String::new(),
            };
            println!("{place}: error[{}]: {}", finding.code, finding.message);
            println!("  help: {}", finding.help);
        }
        println!(
            "checked {tasks} task{} and {docs} doc{}, found {} problem{}",
            plural(tasks),
            plural(docs),
            findings.len(),
            plural(findings.len())
        );
    }
    Ok(match findings.is_empty() {
        true => ExitCode::SUCCESS,
        false => ExitCode::FAILURE,
    })
}

// A key that names no task would filter every finding away and pass, so it stops the run instead.
fn wanted(index: &Index, keys: &[String]) -> Result<Option<HashSet<String>>> {
    if keys.is_empty() {
        return Ok(None);
    }
    for key in keys {
        if !index
            .number(key)
            .is_some_and(|number| index.contains(number))
        {
            bail!("no task matches {key}");
        }
    }
    Ok(Some(keys.iter().cloned().collect()))
}

fn task_findings(row: &TaskListItem) -> Vec<Finding> {
    let mut findings: Vec<Finding> = row
        .problems
        .iter()
        .map(|problem| Finding {
            task: Some(row.id.clone()),
            doc: None,
            code: problem.code.as_str(),
            message: problem.message.clone(),
            help: help(problem.code),
        })
        .collect();
    if row.conflicts > 0 {
        findings.push(Finding {
            task: Some(row.id.clone()),
            doc: None,
            code: "conflict",
            message: format!(
                "{} unresolved conflict{} from a sync",
                row.conflicts,
                plural(row.conflicts)
            ),
            help: "keep one version of each block and write the task back with \
                   `openplan tasks write`, or settle a field with `openplan tasks set`",
        });
    }
    findings
}

fn doc_findings(doc: &DocListItem) -> Vec<Finding> {
    let mut findings: Vec<Finding> = doc
        .problems
        .iter()
        .map(|problem| Finding {
            task: None,
            doc: Some(doc.name.clone()),
            code: problem.code.as_str(),
            message: problem.message.clone(),
            help: doc_help(problem.code),
        })
        .collect();
    findings.extend(conflict_finding(doc));
    findings
}

fn conflict_finding(doc: &DocListItem) -> Option<Finding> {
    (doc.conflicts > 0).then(|| Finding {
        task: None,
        doc: Some(doc.name.clone()),
        code: "conflict",
        message: format!(
            "{} unresolved conflict{} from a sync",
            doc.conflicts,
            plural(doc.conflicts)
        ),
        help: "keep one version of each block and write the body back with `openplan doc set`, \
               or settle the parent with `openplan doc nest`",
    })
}

fn doc_help(code: ProblemCode) -> &'static str {
    match code {
        ProblemCode::Field => "set the parent with `openplan doc nest`, or repair the frontmatter",
        ProblemCode::ReferencePath => {
            "write the body back with `openplan doc set`; a write spells each reference as a path"
        }
        ProblemCode::ParentCycle => "nest one doc of the cycle elsewhere with `openplan doc nest`",
        other => help(other),
    }
}

fn help(code: ProblemCode) -> &'static str {
    match code {
        ProblemCode::Field => {
            "set the field with `openplan tasks set`, or write the task back with \
             `openplan tasks write`"
        }
        ProblemCode::Title => "write the task back with `openplan tasks write` and one `# ` title",
        ProblemCode::Comment => {
            "repair the comment log with `openplan tasks write`; add entries with \
             `openplan tasks comment`"
        }
        ProblemCode::Diagram => {
            "repair the `mermaid` fence; the openplan skill lists the Mermaid it accepts"
        }
        ProblemCode::Reference => "name a task or a doc that exists, or remove the reference",
        ProblemCode::ReferencePath => {
            "write the task back with `openplan tasks write`; a write spells each reference as a \
             path"
        }
        ProblemCode::Tag => {
            "register the tag with `openplan tag`, or remove it with \
             `openplan tasks set <key> tags`"
        }
        ProblemCode::ParentCycle | ProblemCode::DependencyCycle => {
            "change one parent or dependency in the cycle with `openplan tasks set`"
        }
        ProblemCode::DuplicateNumber => {
            "delete the task file that is not read, or give it a free number"
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}
