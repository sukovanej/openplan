use crate::layout::{self, Document};
use crate::{Abbreviation, ref_target, with_section};

// What a `[[…]]` in a body names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Task(u64),
    Doc(String),
}

// A file names another by the target's path relative to its own directory, so a plain markdown
// reader can follow the link and a `grep` can see it. `dir` is the directory of the file that holds
// the reference, and `to` is the target's path in the store.
pub fn relative(dir: &str, to: &str) -> String {
    let from = components(dir);
    let to = components(to);
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let rest = to[shared..].join("/");
    match from.len() - shared {
        0 => format!("./{rest}"),
        ups => format!("{}{rest}", "../".repeat(ups)),
    }
}

// The store path a reference names, read against `dir` as a markdown link is. A target that is not a
// markdown file is a key, a name, or prose, and names no path.
pub fn resolve(dir: &str, reference: &str) -> Option<String> {
    let target = ref_target(reference);
    if !target.ends_with(".md") || target.starts_with('/') {
        return None;
    }
    let mut parts = components(dir);
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

fn components(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect()
}

// A path is read against `dir`, so `./00042-x.md` names a task in `tasks/` and a doc in `docs/`. This
// store's key names a task and a doc name names a doc: a person types those, and a write turns each
// into a path. Bare digits name nothing, because the file layer spells a task that way in memory and
// a body must not.
pub fn body_target(abbreviation: Option<Abbreviation>, dir: &str, inner: &str) -> Option<Target> {
    if let Some(path) = resolve(dir, inner) {
        return match Document::of(&path) {
            Document::Task(number) => Some(Target::Task(number)),
            Document::Doc(name) => Some(Target::Doc(name)),
            _ => None,
        };
    }
    let target = ref_target(inner);
    if let Some(number) = abbreviation.and_then(|abbreviation| abbreviation.parse_key(target)) {
        return Some(Target::Task(number));
    }
    doc_name(target).map(|name| Target::Doc(name.to_owned()))
}

// A spelling that is not a path, which a file must not keep.
pub fn is_path(dir: &str, inner: &str) -> bool {
    resolve(dir, inner).is_some()
}

fn doc_name(target: &str) -> Option<&str> {
    let named = crate::name::normalize(target).as_deref() == Some(target)
        && !target.bytes().all(|b| b.is_ascii_digit());
    named.then_some(target)
}

// The path a file under `dir` names a doc by, with the section the reference aims at.
pub fn doc_ref(dir: &str, name: &str, reference: &str) -> String {
    with_section(&relative(dir, &layout::doc_path(name)), reference)
}

// The path a file under `dir` names a task file by, with the section the reference aims at.
pub fn task_file_ref(dir: &str, path: &str, reference: &str) -> String {
    with_section(&relative(dir, path), reference)
}

// Every reference in `body` that names a task or a doc but is not a path, as the file spells it.
// A write turns each into a path, so a file that still holds one was written by hand.
pub fn unpathed(
    abbreviation: Option<Abbreviation>,
    dir: &str,
    body: &str,
) -> Vec<(String, Target)> {
    crate::body_ref_spans(body)
        .into_iter()
        .filter(|(_, inner)| !is_path(dir, inner))
        .filter_map(|(_, inner)| Some((inner.to_owned(), body_target(abbreviation, dir, inner)?)))
        .collect()
}

// The `parent:` and `dependencies:` of a task file as the file spells them, published version.
pub fn frontmatter_spellings(input: &str) -> Vec<String> {
    let Some((frontmatter, _)) = crate::split_frontmatter(input) else {
        return Vec::new();
    };
    let published = crate::conflict::published(&frontmatter.replace('\r', ""));
    let Ok(map) = serde_yaml::from_str::<serde_yaml::Mapping>(&published) else {
        return Vec::new();
    };
    let spelled = |value: &serde_yaml::Value| match value {
        serde_yaml::Value::String(text) => Some(text.clone()),
        serde_yaml::Value::Number(number) => Some(number.to_string()),
        _ => None,
    };
    let mut out: Vec<String> = map.get("parent").and_then(spelled).into_iter().collect();
    if let Some(serde_yaml::Value::Sequence(items)) = map.get("dependencies") {
        out.extend(items.iter().filter_map(spelled));
    }
    out
}
