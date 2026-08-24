use std::io::Write as _;
use std::path::Path;

use anyhow::{Result, bail};
use op_api::{CreateTag, FieldUpdate, TagPatch, TagView};
use op_task::tag::Color;

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
        TagCommand::Show { name, json } => show(&Plan::resolve(root, daemon_url)?, &name, json),
        TagCommand::Set { name, field, value } => {
            set(&Plan::resolve(root, daemon_url)?, &name, &field, &value)
        }
        TagCommand::Rename { from, to } => rename(&Plan::resolve(root, daemon_url)?, &from, to),
        TagCommand::Delete { name, force, yes } => {
            delete(&Plan::resolve(root, daemon_url)?, &name, force, yes)
        }
    }
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
    if !yes && !confirm(&tag)? {
        println!("aborted");
        return Ok(());
    }
    plan.delete_tag(&tag.name, force)?;
    println!("deleted {}", tag.name);
    Ok(())
}

fn confirm(tag: &TagView) -> Result<bool> {
    print!("delete tag {}? [y/N] ", tag.name);
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
