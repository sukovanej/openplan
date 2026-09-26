use std::io::Write as _;
use std::path::Path;

use anyhow::Result;
use op_api::{CreateDoc, DocDetail, DocListItem, DocPatch, FieldUpdate};
use op_task::doc::normalize_name;

use crate::DocCommand;
use crate::plan::Plan;

pub fn run(command: DocCommand, root: &Path, daemon_url: Option<&str>) -> Result<()> {
    let plan = Plan::resolve(root, daemon_url)?;
    match command {
        DocCommand::Create { name, body, parent } => {
            let parent = parent.as_deref().map(identity).transpose()?;
            let doc = plan.create_doc(&CreateDoc { name, body, parent })?;
            println!("{}", doc.name);
            Ok(())
        }
        DocCommand::List { json } => list(&plan, json),
        DocCommand::Get { name, json } => get(&plan, &identity(&name)?, json),
        DocCommand::Set { name, body } => {
            plan.patch_doc(
                &identity(&name)?,
                &DocPatch {
                    body: Some(body),
                    ..DocPatch::default()
                },
            )?;
            Ok(())
        }
        DocCommand::Nest { name, parent } => {
            // "" and "-" lift the doc to the top level, the way `move --parent` lifts a task.
            let parent = match parent.as_str() {
                "" | "-" => FieldUpdate::Clear,
                named => FieldUpdate::Set(identity(named)?),
            };
            plan.patch_doc(
                &identity(&name)?,
                &DocPatch {
                    parent,
                    ..DocPatch::default()
                },
            )?;
            Ok(())
        }
        DocCommand::Rename { from, to } => {
            let doc = plan.patch_doc(
                &identity(&from)?,
                &DocPatch {
                    name: Some(to),
                    ..DocPatch::default()
                },
            )?;
            println!("{}", doc.name);
            Ok(())
        }
        DocCommand::Delete { name, yes } => delete(&plan, &identity(&name)?, yes),
    }
}

// A doc is identified by its file stem, and a name a caller typed is a spelling of one. Settling
// that spelling here answers a name like `C++` with the rule it breaks rather than "no such doc".
pub fn identity(name: &str) -> Result<String> {
    Ok(normalize_name(name)?)
}

fn list(plan: &Plan, json: bool) -> Result<()> {
    let docs = plan.docs()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&docs)?);
        return Ok(());
    }
    if docs.is_empty() {
        println!("no docs yet");
        return Ok(());
    }
    let nested = nested(&docs);
    let width = nested
        .iter()
        .map(|(depth, doc)| doc.name.len() + 2 * depth)
        .max()
        .unwrap_or(0);
    for (depth, doc) in nested {
        let name = format!("{}{}", "  ".repeat(depth), doc.name);
        let conflicted = match doc.conflicts {
            0 => "",
            _ => "  [conflict]",
        };
        println!("{name:<width$}  {}{conflicted}", doc.title);
    }
    Ok(())
}

// The docs in tree order, each with its depth. A doc whose parent was deleted stands at the top
// level, because hiding it would lose it. A cycle among the docs has no root, so nothing above
// reaches it; those docs are still docs, and the list is where a reader sees them to break the cycle.
fn nested(docs: &[DocListItem]) -> Vec<(usize, &DocListItem)> {
    let held = |name: &str| docs.iter().any(|doc| doc.name == name);
    let mut out = Vec::new();
    for doc in docs
        .iter()
        .filter(|doc| !doc.metadata.parent().is_some_and(&held))
    {
        push_subtree(docs, doc, 0, &mut out);
    }
    for doc in docs {
        push_subtree(docs, doc, 0, &mut out);
    }
    out
}

fn push_subtree<'a>(
    docs: &'a [DocListItem],
    doc: &'a DocListItem,
    depth: usize,
    out: &mut Vec<(usize, &'a DocListItem)>,
) {
    // A cycle the files carry would otherwise walk forever, and it can only reach a doc already out.
    if out.iter().any(|(_, seen)| seen.name == doc.name) {
        return;
    }
    out.push((depth, doc));
    for child in docs
        .iter()
        .filter(|c| c.metadata.parent() == Some(doc.name.as_str()))
    {
        push_subtree(docs, child, depth + 1, out);
    }
}

fn get(plan: &Plan, name: &str, json: bool) -> Result<()> {
    let doc = plan.doc(name)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&doc)?);
    } else {
        if doc.conflicts > 0 {
            eprintln!("{}", conflict_notice(&doc));
        }
        print!("{}", markdown(&doc));
    }
    Ok(())
}

// Sync leaves a conflict where two people changed one thing differently; the agent or person who
// reads the doc settles it.
fn conflict_notice(doc: &DocDetail) -> String {
    format!(
        "{} has {} unresolved conflict{} from a sync. Each block between `<<<<<<<` and \
         `>>>>>>>` holds two versions, and the one after `=======` is in force. Keep the right \
         version of each block, remove the markers, and write the body back with `openplan doc \
         set`, or settle the parent with `openplan doc nest`.",
        doc.name,
        doc.conflicts,
        if doc.conflicts == 1 { "" } else { "s" }
    )
}

fn markdown(doc: &DocDetail) -> String {
    match doc.body.trim().is_empty() {
        true => format!("# {}\n", doc.title),
        false => format!("# {}\n\n{}\n", doc.title, doc.body.trim_end()),
    }
}

fn delete(plan: &Plan, name: &str, yes: bool) -> Result<()> {
    let doc = plan.doc(name)?;
    if !yes && !confirm(&doc)? {
        println!("aborted");
        return Ok(());
    }
    plan.delete_doc(&doc.name)?;
    println!("deleted {}", doc.name);
    Ok(())
}

// The delete moves the docs nested under this one, so the question says where they go.
fn nested_note(doc: &DocDetail) -> String {
    let place = match (doc.metadata.parent(), &doc.parent_title) {
        (Some(parent), Some(_)) => format!("up under {parent}"),
        _ => "to the top level".to_owned(),
    };
    match doc.children.len() {
        0 => String::new(),
        1 => format!(" Its nested doc moves {place}."),
        count => format!(" Its {count} nested docs move {place}."),
    }
}

fn confirm(doc: &DocDetail) -> Result<bool> {
    print!(
        "delete doc {} ({})?{} [y/N] ",
        doc.name,
        doc.title,
        nested_note(doc)
    );
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
