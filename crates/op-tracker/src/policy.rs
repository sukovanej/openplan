use std::collections::{BTreeMap, BTreeSet};

use op_backend::{
    BackendError, ChangeKind, MergeInput, MergePolicy, Overlay, Resolution, Revision, Snapshot,
    Tips,
};
use op_task::config::Config;
use op_task::conflict::{self, Labels};
use op_task::layout::{self, Document};
use op_task::{Task, merge, parse_partial, three_way};

// Sync runs unattended, so every conflict gets an answer and nothing a person wrote is lost. A field
// or lines that both sides changed differently keep both versions in the task, with the published
// one in force until a person or an agent picks. A task whose number another task took first moves
// to a free number.
#[derive(Debug, Clone, Copy, Default)]
pub struct TaskMergePolicy;

// A task file: its path and its text.
type File = (String, Vec<u8>);

impl MergePolicy for TaskMergePolicy {
    fn resolve(&self, input: &MergeInput<'_>) -> Result<Resolution, BackendError> {
        let labels = labels(input.tips);
        let mut state = Overlay::new(input.merged);
        for path in input.conflicts {
            if matches!(Document::of(path), Document::Task(_)) {
                continue;
            }
            // An edit outlives a removal; otherwise the published version stays.
            if let (Some(ours), None) = (input.ours.read(path)?, input.theirs.read(path)?) {
                state.set(path.clone(), Some(ours));
            }
        }
        let touched: BTreeSet<u64> = input
            .ours_changed
            .iter()
            .filter_map(|change| layout::task_number(&change.path))
            .collect();
        let base_paths = TaskPaths::of(input.base)?;
        let ours_paths = TaskPaths::of(input.ours)?;
        let theirs_paths = TaskPaths::of(input.theirs)?;
        let mut held = TaskPaths::of(input.merged)?;
        let mut displaced = Vec::new();
        for number in touched {
            let base = task_file(input.base, &base_paths, number)?;
            let ours = task_file(input.ours, &ours_paths, number)?;
            let theirs = task_file(input.theirs, &theirs_paths, number)?;
            let settled = match (base, ours, theirs) {
                (_, ours, theirs) if ours == theirs => theirs,
                (None, Some(ours), Some(theirs)) => {
                    displaced.push(ours);
                    Some(theirs)
                }
                (None, ours, None) => ours,
                (None, None, theirs) => theirs,
                // An edit outlives a removal on the other side.
                (Some(base), None, theirs) => theirs.filter(|theirs| *theirs != base),
                (Some(base), Some(ours), None) => (ours != base).then_some(ours),
                (Some(base), Some(ours), Some(theirs)) => {
                    Some(merged_file(&base, &ours, &theirs, &labels))
                }
            };
            settle(&mut state, &mut held, number, settled);
        }
        let notes = renumber(input, &mut state, displaced)?;
        Ok(Resolution {
            ops: state.into_ops(),
            notes,
        })
    }
}

fn labels(tips: Tips<'_>) -> Labels {
    Labels {
        ours: label(tips.ours),
        theirs: label(tips.theirs),
    }
}

pub(crate) fn label(revision: &Revision) -> String {
    let id = revision.id.as_str();
    format!("{} ({})", revision.author.name, &id[..id.len().min(7)])
}

// A task that only one side renamed keeps the new name.
fn merged_file(base: &File, ours: &File, theirs: &File, labels: &Labels) -> File {
    let path = three_way(&base.0, &ours.0, &theirs.0).unwrap_or_else(|| theirs.0.clone());
    let [base, ours, theirs] = [base, ours, theirs].map(|file| String::from_utf8_lossy(&file.1));
    let parse = |text: &str| Task::from_file_string(text).ok();
    let text = match (parse(&base), parse(&ours), parse(&theirs)) {
        (Some(base), Some(ours), Some(theirs)) => merge::task(&base, &ours, &theirs, labels)
            .to_file_string()
            .ok(),
        _ => None,
    }
    // A version that does not parse as a task still merges line by line, frontmatter included.
    .unwrap_or_else(|| conflict::merge(&base, &ours, &theirs, labels));
    (path, text.into_bytes())
}

// The files of each task number, listed once for a whole merge rather than once for each task.
struct TaskPaths(BTreeMap<u64, BTreeSet<String>>);

impl TaskPaths {
    fn of(snapshot: &dyn Snapshot) -> Result<Self, BackendError> {
        let mut paths: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
        for path in snapshot.list(layout::TASKS)? {
            if let Some(number) = layout::task_number(&path) {
                paths.entry(number).or_default().insert(path);
            }
        }
        Ok(Self(paths))
    }
}

// Two files of one number resolve to the lowest path, as a plan reads them.
fn task_file(
    snapshot: &dyn Snapshot,
    paths: &TaskPaths,
    number: u64,
) -> Result<Option<File>, BackendError> {
    let Some(path) = paths.0.get(&number).and_then(BTreeSet::first) else {
        return Ok(None);
    };
    Ok(snapshot.read(path)?.map(|content| (path.clone(), content)))
}

// The number keeps exactly the file `settled` names, or none.
fn settle(state: &mut Overlay<'_>, held: &mut TaskPaths, number: u64, settled: Option<File>) {
    for path in held.0.remove(&number).unwrap_or_default() {
        if settled.as_ref().is_none_or(|(kept, _)| *kept != path) {
            state.set(path, None);
        }
    }
    if let Some((path, content)) = settled {
        held.0.entry(number).or_default().insert(path.clone());
        state.set(path, Some(content));
    }
}

fn renumber(
    input: &MergeInput<'_>,
    state: &mut Overlay<'_>,
    displaced: Vec<File>,
) -> Result<Vec<String>, BackendError> {
    if displaced.is_empty() {
        return Ok(Vec::new());
    }
    let key = keys(state)?;
    let mut next = task_numbers(state)?
        .keys()
        .next_back()
        .map_or(1, |max| max + 1);
    let mut renamed = BTreeMap::new();
    let mut notes = Vec::new();
    for (path, content) in displaced {
        let Some(old) = layout::task_number(&path) else {
            continue;
        };
        let number = next;
        next += 1;
        let title = parse_partial(&String::from_utf8_lossy(&content))
            .title
            .unwrap_or_else(|| "task".to_owned());
        let new_path = layout::task_path(number, &title);
        notes.push(format!(
            "{} \"{title}\" is now {}: another task took {} first.",
            key(old),
            key(number),
            key(old)
        ));
        state.set(new_path.clone(), Some(content));
        renamed.insert(path, new_path);
    }
    rewrite_references(input, state, &renamed)?;
    Ok(notes)
}

// Our own documents name a moved task by its old file; only ours can, because the other side
// never saw it.
fn rewrite_references(
    input: &MergeInput<'_>,
    state: &mut Overlay<'_>,
    renamed: &BTreeMap<String, String>,
) -> Result<(), BackendError> {
    let mut ours: BTreeSet<String> = input
        .ours_changed
        .iter()
        .filter(|change| change.kind != ChangeKind::Removed)
        .map(|change| renamed.get(&change.path).unwrap_or(&change.path).clone())
        .collect();
    ours.extend(renamed.values().cloned());
    for path in ours {
        if !matches!(Document::of(&path), Document::Task(_)) {
            continue;
        }
        let Some(text) = state.read_text(&path)? else {
            continue;
        };
        let mut rewritten = text.clone();
        for (old, new) in renamed {
            rewritten = rewritten.replace(
                &op_task::task_ref(layout::file_name(old)),
                &op_task::task_ref(layout::file_name(new)),
            );
        }
        if rewritten != text {
            state.set(path, Some(rewritten.into_bytes()));
        }
    }
    Ok(())
}

fn task_numbers(snapshot: &dyn Snapshot) -> Result<BTreeMap<u64, String>, BackendError> {
    Ok(snapshot
        .list(layout::TASKS)?
        .into_iter()
        .filter_map(|path| Some((layout::task_number(&path)?, path)))
        .collect())
}

fn keys(snapshot: &dyn Snapshot) -> Result<impl Fn(u64) -> String + use<>, BackendError> {
    let abbreviation = snapshot
        .read_text(layout::CONFIG)?
        .and_then(|text| Config::parse(&text).ok())
        .map(|config| config.abbreviation);
    Ok(move |number: u64| match abbreviation {
        Some(abbreviation) => abbreviation.format_key(number),
        None => number.to_string(),
    })
}
