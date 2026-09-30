use std::io::{Read as _, Write as _};
use std::path::Path;

use anyhow::{Result, bail};
use op_api::{
    Comment, CreateComment, CreateTask, Field, FieldError, FieldUpdate, Metadata, SearchHit,
    TaskListItem, TaskPatch, TaskTree, list_item_cmp,
};
use op_task::{Status, rank};

use crate::plan::Plan;
use crate::{PullRequestCommand, TaskCommand};
use crate::{author, tag};

pub fn run(command: TaskCommand, root: &Path, daemon_url: Option<&str>) -> Result<()> {
    match command {
        TaskCommand::Create {
            title,
            parent,
            status,
            dependencies,
            tags,
            pull_requests,
            body,
            body_file,
        } => {
            let body = resolve_body(body, body_file)?;
            let tags = tag::identities(tags)?;
            create(
                root,
                daemon_url,
                &CreateTask {
                    title,
                    status,
                    parent,
                    dependencies,
                    tags,
                    pull_requests,
                    body,
                },
            )
        }
        TaskCommand::List {
            status,
            parent,
            json,
        } => list(root, daemon_url, status, parent.as_deref(), json),
        TaskCommand::Search { query, json } => search(root, daemon_url, &query, json),
        TaskCommand::Get { id, json, revision } => {
            get(root, daemon_url, &id, json, revision.as_deref())
        }
        TaskCommand::Write { id, file } => {
            let text = resolve_body(None, Some(file))?.unwrap_or_default();
            write(root, daemon_url, &id, &text)
        }
        TaskCommand::Comment {
            id,
            text,
            body_file,
        } => {
            let text = resolve_body(text, body_file)?.unwrap_or_default();
            comment(root, daemon_url, &id, &text)
        }
        TaskCommand::Comments { id, json } => comments(root, daemon_url, &id, json),
        TaskCommand::Show { id } => show(root, daemon_url, &id),
        TaskCommand::Pr { command } => {
            let (id, patch) = match command {
                PullRequestCommand::Add { id, pull_request } => (
                    id,
                    TaskPatch {
                        add_pull_requests: vec![pull_request],
                        ..TaskPatch::default()
                    },
                ),
                PullRequestCommand::Remove { id, pull_request } => (
                    id,
                    TaskPatch {
                        remove_pull_requests: vec![pull_request],
                        ..TaskPatch::default()
                    },
                ),
            };
            Plan::resolve(root, daemon_url)?.patch(&id, &patch)?;
            Ok(())
        }
        TaskCommand::Tree { id, depth, json } => tree(root, daemon_url, &id, depth, json),
        TaskCommand::Move {
            id,
            parent,
            before,
            after,
        } => move_task(root, daemon_url, &id, parent, before, after),
        TaskCommand::Set { id, field, value } => set(root, daemon_url, &id, &field, &value),
        TaskCommand::Delete { id, yes } => delete(root, daemon_url, &id, yes),
    }
}

fn resolve_body(body: Option<String>, body_file: Option<String>) -> Result<Option<String>> {
    match body_file {
        Some(path) if path == "-" => {
            let mut content = String::new();
            std::io::stdin().read_to_string(&mut content)?;
            Ok(Some(content))
        }
        Some(path) => Ok(Some(std::fs::read_to_string(&path)?)),
        None => Ok(body),
    }
}

fn create(root: &Path, daemon_url: Option<&str>, task: &CreateTask) -> Result<()> {
    let id = Plan::resolve(root, daemon_url)?.create(task)?;
    println!("{id}");
    Ok(())
}

fn list(
    root: &Path,
    daemon_url: Option<&str>,
    status: Option<Status>,
    parent: Option<&str>,
    json: bool,
) -> Result<()> {
    let held = Plan::resolve(root, daemon_url)?.list()?;
    let matching: Vec<&TaskListItem> = held
        .iter()
        .filter(|task| status.is_none_or(|s| task.metadata.status() == Some(s)))
        .filter(|task| parent.is_none_or(|p| task.metadata.parent() == Some(p)))
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&matching)?);
    } else if !matching.is_empty() {
        print_tasks(&matching);
    } else if held.is_empty() {
        println!("no tasks yet");
    } else {
        println!("no matching tasks");
    }
    Ok(())
}

fn search(root: &Path, daemon_url: Option<&str>, query: &str, json: bool) -> Result<()> {
    let hits = Plan::resolve(root, daemon_url)?.search(query)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&hits)?);
    } else if hits.is_empty() {
        println!("no matching tasks");
    } else {
        print_hits(&hits);
    }
    Ok(())
}

fn get(
    root: &Path,
    daemon_url: Option<&str>,
    id: &str,
    json: bool,
    revision: Option<&str>,
) -> Result<()> {
    let plan = Plan::resolve(root, daemon_url)?;
    if let Some(revision) = revision {
        let then = plan.revision(id, revision)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&then)?);
            return Ok(());
        }
        match then.task {
            Some(task) => print!("{}", task.raw),
            None => bail!("{id} did not exist at revision {revision}"),
        }
        return Ok(());
    }
    let detail = plan.get(id)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&detail)?);
    } else {
        // The daemon holds parsed state, so this is a canonical rendering of the task and not a copy
        // of its file. A field it could not parse has no canonical form, so it is reported instead.
        for problem in &detail.problems {
            eprintln!("{id}: {}", problem.message);
        }
        if detail.conflicts > 0 {
            eprintln!("{}", conflict_notice(id, detail.conflicts));
        }
        print!(
            "{}",
            op_api::render_task_file(
                &detail.metadata,
                &detail.title,
                &detail.description,
                &detail.comments
            )?
        );
    }
    Ok(())
}

fn write(root: &Path, daemon_url: Option<&str>, id: &str, text: &str) -> Result<()> {
    let detail = Plan::resolve(root, daemon_url)?.write(id, text)?;
    println!("{}: {}", detail.id, detail.title);
    Ok(())
}

fn comment(root: &Path, daemon_url: Option<&str>, id: &str, text: &str) -> Result<()> {
    if text.trim().is_empty() {
        bail!("a comment needs text");
    }
    let tasks = Plan::resolve(root, daemon_url)?;
    let entry = CreateComment {
        text: text.trim_end_matches('\n').to_owned(),
        author: author::author(root)?,
        agent: author::agent(),
    };
    let written = tasks.comment(id, &entry)?;
    println!("{id}: {}", heading_of(&written));
    Ok(())
}

fn comments(root: &Path, daemon_url: Option<&str>, id: &str, json: bool) -> Result<()> {
    let comments = Plan::resolve(root, daemon_url)?.comments(id)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&comments)?);
        return Ok(());
    }
    for comment in &comments {
        print_comment(comment);
    }
    Ok(())
}

fn print_comment(comment: &Comment) {
    println!("{}", heading_of(comment));
    for line in comment.text.lines() {
        match line.is_empty() {
            true => println!(),
            false => println!("    {line}"),
        }
    }
    println!();
}

fn heading_of(comment: &Comment) -> String {
    let agent = match &comment.agent {
        Some(agent) => format!(" via {agent}"),
        None => String::new(),
    };
    format!(
        "{} by {}{agent}",
        shown(&comment.at),
        shown(&comment.author)
    )
}

// A field the daemon could not parse reads as the reason it could not, where its value would have
// been: the entry is still worth showing, and hiding why it is broken sends the reader to the file
// with nothing to look for.
fn shown<T: std::fmt::Display>(field: &Field<T>) -> String {
    match field {
        Field::Value(value) => value.to_string(),
        Field::Conflict(conflict) => conflict.value.to_string(),
        Field::Error(FieldError::Missing) => "(missing)".to_owned(),
        Field::Error(FieldError::Invalid { message }) => format!("({message})"),
    }
}

fn show(root: &Path, daemon_url: Option<&str>, id: &str) -> Result<()> {
    let detail = Plan::resolve(root, daemon_url)?.get(id)?;
    let metadata = &detail.metadata;
    println!("id:     {id}");
    println!("title:  {}", detail.title);
    println!("status: {}", status_label(metadata));
    println!("parent: {}", metadata.parent().unwrap_or("-"));
    let dependencies = metadata.dependencies();
    println!(
        "dependencies: {}",
        if dependencies.is_empty() {
            "-".to_owned()
        } else {
            dependencies.join(", ")
        }
    );
    let tags = metadata.tags();
    println!(
        "tags: {}",
        if tags.is_empty() {
            "-".to_owned()
        } else {
            tags.join(", ")
        }
    );
    for address in metadata.pull_requests() {
        println!("pull request: {address}");
    }
    for problem in &detail.problems {
        println!("!       {}", problem.message);
    }
    if detail.conflicts > 0 {
        let fields = metadata.conflicted_fields();
        match fields.is_empty() {
            true => println!("conflicts: {}", detail.conflicts),
            false => println!(
                "conflicts: {} (fields: {})",
                detail.conflicts,
                fields.join(", ")
            ),
        }
        eprintln!("{}", conflict_notice(id, detail.conflicts));
    }
    Ok(())
}

// Sync leaves a conflict where two people changed one thing differently; the agent or person who
// reads the task settles it.
fn conflict_notice(id: &str, count: usize) -> String {
    format!(
        "{id} has {count} unresolved conflict{} from a sync. Each block between `<<<<<<<` and \
         `>>>>>>>` holds two versions, and the one after `=======` is in force. Keep the right \
         version of each block, remove the markers, and write the task back with \
         `openplan tasks write`, or settle one field with `openplan tasks set`.",
        if count == 1 { "" } else { "s" }
    )
}

fn status_label(metadata: &Metadata) -> String {
    match metadata.status() {
        Some(status) => status.as_str().to_owned(),
        None => "unreadable".to_owned(),
    }
}

fn print_tasks(tasks: &[&TaskListItem]) {
    for task in tasks {
        let status = status_label(&task.metadata);
        let conflicted = match task.conflicts {
            0 => "",
            _ => "  [conflict]",
        };
        let problems = match task.problems.len() {
            0 => String::new(),
            1 => "  [1 problem]".to_owned(),
            count => format!("  [{count} problems]"),
        };
        println!(
            "{:<10} {status:<11} {}{conflicted}{problems}",
            task.id, task.title
        );
    }
}

fn print_hits(hits: &[SearchHit]) {
    for hit in hits {
        let status = status_label(&hit.task.metadata);
        println!("{:<10} {status:<11} {}", hit.task.id, hit.task.title);
    }
}

fn set(root: &Path, daemon_url: Option<&str>, id: &str, field: &str, value: &str) -> Result<()> {
    // Parse before reaching for the daemon so a typo fails without starting one.
    let patch = parse_field(field, value)?;
    Plan::resolve(root, daemon_url)?.patch(id, &patch)?;
    Ok(())
}

fn parse_field(field: &str, value: &str) -> Result<TaskPatch> {
    Ok(match field {
        "status" => TaskPatch {
            status: Some(value.parse()?),
            ..TaskPatch::default()
        },
        // "" or "-" clears the parent (top level), mirroring how `dependencies ""` clears them.
        "parent" => TaskPatch {
            parent: parent_update(parse_parent(value)),
            ..TaskPatch::default()
        },
        "dependencies" => TaskPatch {
            dependencies: Some(comma_separated(value)),
            ..TaskPatch::default()
        },
        // The whole set, like `dependencies`: what the caller names is what the task ends up with,
        // and "" clears it.
        "tags" => TaskPatch {
            tags: Some(tag::identities(comma_separated(value))?),
            ..TaskPatch::default()
        },
        "pull_requests" => TaskPatch {
            pull_requests: Some(comma_separated(value)),
            ..TaskPatch::default()
        },
        other => bail!(
            "unknown field {other:?}; expected status | parent | dependencies | tags | \
             pull_requests"
        ),
    })
}

fn comma_separated(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parent_update(parent: Option<String>) -> FieldUpdate<String> {
    match parent {
        Some(id) => FieldUpdate::Set(id),
        None => FieldUpdate::Clear,
    }
}

fn delete(root: &Path, daemon_url: Option<&str>, id: &str, yes: bool) -> Result<()> {
    let plan = Plan::resolve(root, daemon_url)?;
    // A typo must refuse before it asks the reader to confirm a delete.
    plan.get(id)?;
    if !yes && !confirm(id)? {
        println!("aborted");
        return Ok(());
    }
    plan.delete(id)?;
    println!("deleted {id}");
    Ok(())
}

fn confirm(id: &str) -> Result<bool> {
    print!("delete {id}? [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

// "" or the "-" sentinel clears the parent (top level); any other value sets it.
fn parse_parent(value: &str) -> Option<String> {
    if value.is_empty() || value == "-" {
        None
    } else {
        Some(value.to_owned())
    }
}

fn tree(
    root: &Path,
    daemon_url: Option<&str>,
    id: &str,
    depth: Option<usize>,
    json: bool,
) -> Result<()> {
    let view = Plan::resolve(root, daemon_url)?.tree(id, depth)?;
    for cycle in &view.cycles {
        eprintln!("warning: parent cycle at {cycle}; its subtree is truncated");
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&view.tree)?);
    } else {
        print_tree(&view.tree, 0);
    }
    Ok(())
}

fn print_tree(node: &TaskTree, depth: usize) {
    println!(
        "{}{:<10} {:<11} {}",
        "  ".repeat(depth),
        node.id,
        status_label(&node.metadata),
        node.title,
    );
    for child in &node.children {
        print_tree(child, depth + 1);
    }
}

fn move_task(
    root: &Path,
    daemon_url: Option<&str>,
    id: &str,
    parent: Option<String>,
    before: Option<String>,
    after: Option<String>,
) -> Result<()> {
    // The ranks are computed from the daemon's view of the tasks, the state the write lands on.
    let plan = Plan::resolve(root, daemon_url)?;
    let group = plan.list()?;
    // The task's own row, from the same read the siblings come from: asking for it separately would
    // walk the repository a second time to learn what this list already says.
    let current_parent = group
        .iter()
        .find(|task| task.id == id)
        .ok_or_else(|| anyhow::anyhow!("no such task: {id}"))?
        .metadata
        .parent()
        .map(str::to_owned);
    let new_parent = match parent {
        None => current_parent,
        Some(value) => parse_parent(&value),
    };
    let mut siblings: Vec<&TaskListItem> = group
        .iter()
        .filter(|s| s.metadata.parent() == new_parent.as_deref() && s.id != id)
        .collect();
    siblings.sort_by(|a, b| list_item_cmp(a, b));
    let insert = insert_index(&siblings, before.as_deref(), after.as_deref())?;

    match rank_plan(&siblings, insert) {
        RankPlan::Single(new_rank) => {
            plan.patch(
                id,
                &TaskPatch {
                    parent: parent_update(new_parent),
                    rank: Some(new_rank),
                    ..TaskPatch::default()
                },
            )?;
        }
        // A sibling group with missing, colliding, or malformed ranks can't be split by a single
        // fractional key, so materialize a fresh, evenly-spaced order for the whole group.
        RankPlan::Rebalance {
            siblings: assigned,
            x_rank,
        } => {
            // The moved task goes first: its write is the one the daemon validates (parent exists,
            // no cycle), so a refused move leaves the siblings' ranks untouched.
            plan.patch(
                id,
                &TaskPatch {
                    parent: parent_update(new_parent),
                    rank: Some(x_rank),
                    ..TaskPatch::default()
                },
            )?;
            for (sibling_id, sibling_rank) in assigned {
                plan.patch(
                    &sibling_id,
                    &TaskPatch {
                        rank: Some(sibling_rank),
                        ..TaskPatch::default()
                    },
                )?;
            }
        }
    }
    Ok(())
}

fn insert_index(
    siblings: &[&TaskListItem],
    before: Option<&str>,
    after: Option<&str>,
) -> Result<usize> {
    if let Some(before) = before {
        let pos = sibling_pos(siblings, before)?;
        Ok(pos)
    } else if let Some(after) = after {
        let pos = sibling_pos(siblings, after)?;
        Ok(pos + 1)
    } else {
        Ok(siblings.len())
    }
}

fn sibling_pos(siblings: &[&TaskListItem], target: &str) -> Result<usize> {
    siblings
        .iter()
        .position(|s| s.id == target)
        .ok_or_else(|| anyhow::anyhow!("{target} is not a sibling under the target parent"))
}

enum RankPlan {
    Single(String),
    Rebalance {
        siblings: Vec<(String, String)>,
        x_rank: String,
    },
}

fn rank_plan(siblings: &[&TaskListItem], insert: usize) -> RankPlan {
    let ranks: Vec<&str> = siblings.iter().filter_map(|s| s.metadata.rank()).collect();
    if ranks.len() == siblings.len() && rank::is_ordered(&ranks) {
        let neighbour = |i: usize| ranks.get(i).copied();
        let between = rank::between(insert.checked_sub(1).and_then(neighbour), neighbour(insert));
        if let Some(new_rank) = between {
            return RankPlan::Single(new_rank);
        }
    }
    let keys = rank::spaced(siblings.len() + 1);
    let assigned = siblings
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let slot = if i < insert { i } else { i + 1 };
            (s.id.clone(), keys[slot].clone())
        })
        .collect();
    RankPlan::Rebalance {
        siblings: assigned,
        x_rank: keys[insert].clone(),
    }
}
