use std::io::Write as _;
use std::path::Path;

use anyhow::{Result, bail};
use op_api::{CreateTag, FieldUpdate, TagPatch, TagView};
use op_task::tag::{Color, normalize_name};

use crate::TagCommand;
use crate::plan::Plan;

pub fn run(command: TagCommand, root: &Path, daemon_url: Option<&str>) -> Result<()> {
    match command {
        // The palette is a closed set this binary carries, so it answers without a daemon and
        // without a repository — the one tag command that needs neither.
        TagCommand::Colors => {
            colors();
            Ok(())
        }
        TagCommand::Create {
            name,
            color,
            description,
        } => create(&Plan::resolve(root, daemon_url)?, name, color, description),
        TagCommand::List { json } => list(&Plan::resolve(root, daemon_url)?, json),
        TagCommand::Show { name, json } => {
            show(&Plan::resolve(root, daemon_url)?, &identity(&name)?, json)
        }
        TagCommand::Set { name, field, value } => set(
            &Plan::resolve(root, daemon_url)?,
            &identity(&name)?,
            &field,
            &value,
        ),
        TagCommand::Rename { from, to } => {
            rename(&Plan::resolve(root, daemon_url)?, &identity(&from)?, to)
        }
        TagCommand::Delete { name, force, yes } => delete(
            &Plan::resolve(root, daemon_url)?,
            &identity(&name)?,
            force,
            yes,
        ),
    }
}

// A tag is identified by its name, and a name a caller typed is a spelling of one. Settling that
// spelling here is what keeps `--tag "Front End"` from asking the daemon about a tag no name can
// have, and what answers a name like `C++` with the rule it breaks rather than with "no such tag".
pub fn identity(name: &str) -> Result<String> {
    Ok(normalize_name(name)?)
}

pub fn identities(names: Vec<String>) -> Result<Vec<String>> {
    names.iter().map(|name| identity(name)).collect()
}

fn colors() {
    for color in Color::ALL {
        println!("{}", color.as_str());
    }
}

fn create(
    plan: &Plan,
    name: String,
    color: Option<Color>,
    description: Option<String>,
) -> Result<()> {
    let tag = plan.create_tag(&CreateTag {
        name,
        color,
        description,
    })?;
    println!("{}", tag.name);
    Ok(())
}

fn list(plan: &Plan, json: bool) -> Result<()> {
    let tags = plan.tags()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&tags)?);
    } else if tags.is_empty() {
        println!("no tags yet");
    } else {
        let width = tags.iter().map(|tag| tag.name.len()).max().unwrap_or(0);
        for tag in &tags {
            println!(
                "{:<width$}  {:<7} {}",
                tag.name,
                tag.color.as_str(),
                tag.description.as_deref().unwrap_or_default(),
            );
        }
    }
    Ok(())
}

fn show(plan: &Plan, name: &str, json: bool) -> Result<()> {
    let tag = plan.tag(name)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&tag)?);
    } else {
        println!("name:    {}", tag.name);
        println!("display: {}", tag.display);
        println!("color:   {}", tag.color.as_str());
        println!("desc:    {}", tag.description.as_deref().unwrap_or("-"));
    }
    Ok(())
}

fn set(plan: &Plan, name: &str, field: &str, value: &str) -> Result<()> {
    // Parse before reaching the daemon so a typo fails without a round trip.
    let patch = parse_field(field, value)?;
    plan.patch_tag(name, &patch)?;
    Ok(())
}

fn parse_field(field: &str, value: &str) -> Result<TagPatch> {
    Ok(match field {
        "color" => TagPatch {
            color: Some(value.parse()?),
            ..TagPatch::default()
        },
        // "" clears the description, the way `dependencies ""` clears a task's dependencies.
        "desc" => TagPatch {
            description: match value.is_empty() {
                true => FieldUpdate::Clear,
                false => FieldUpdate::Set(value.to_owned()),
            },
            ..TagPatch::default()
        },
        other => bail!("unknown field {other:?}; expected color | desc"),
    })
}

fn rename(plan: &Plan, from: &str, to: String) -> Result<()> {
    let tag = plan.patch_tag(
        from,
        &TagPatch {
            name: Some(to),
            ..TagPatch::default()
        },
    )?;
    println!("{}", tag.name);
    Ok(())
}

fn delete(plan: &Plan, name: &str, force: bool, yes: bool) -> Result<()> {
    // The delete targets the caller's branch, so the prompt has to be about a tag that branch
    // registers — a name it does not know must refuse before it asks the reader to confirm one.
    let tag = plan.tag(name)?;
    let carried = tasks_carrying(plan, &tag.name)?;
    if !yes && !confirm(&tag, carried.len())? {
        println!("aborted");
        return Ok(());
    }
    plan.delete_tag(&tag.name, force)?;
    println!("deleted {}", tag.name);
    // A forced delete leaves those tasks holding a name this branch no longer registers, and every
    // write validates the whole set, so each of them refuses even a status change until the name
    // goes. Saying so is what keeps --force from costing the reader a debugging session.
    if !carried.is_empty() {
        eprintln!(
            "warning: {} still carr{} {}; each one refuses a write until you drop the name with `openplan set <id> tags \"…\"`",
            plural_tasks(carried.len()),
            if carried.len() == 1 { "ies" } else { "y" },
            tag.name,
        );
    }
    Ok(())
}

fn tasks_carrying(plan: &Plan, name: &str) -> Result<Vec<String>> {
    Ok(plan
        .list(plan.branch())?
        .into_iter()
        .filter(|task| task.metadata.tags().iter().any(|tag| tag == name))
        .map(|task| task.id)
        .collect())
}

fn plural_tasks(count: usize) -> String {
    format!("{count} task{}", if count == 1 { "" } else { "s" })
}

fn confirm(tag: &TagView, carried: usize) -> Result<bool> {
    match carried {
        0 => print!("delete tag {}? [y/N] ", tag.name),
        carried => print!(
            "delete tag {}? {} carr{} it [y/N] ",
            tag.name,
            plural_tasks(carried),
            if carried == 1 { "ies" } else { "y" }
        ),
    }
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
