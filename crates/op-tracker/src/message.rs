use std::collections::BTreeSet;

use op_backend::{Change, ChangeKind, Op, Overlay, Snapshot as _};
use op_task::config::Config;
use op_task::layout;

use crate::describe::describe;
use crate::{Plan, TrackerError};

// A write says what it changes in the words the history uses for every revision, so `git log` and
// `openplan history` agree.
pub(crate) fn of(plan: &Plan, ops: &[Op]) -> Result<String, TrackerError> {
    let before = &**plan.snapshot();
    let mut after = Overlay::new(before);
    after.apply(ops.iter().cloned());
    let mut changes = Vec::new();
    for path in ops.iter().map(Op::path).collect::<BTreeSet<_>>() {
        let kind = match (before.read(path)?, after.read(path)?) {
            (None, Some(_)) => ChangeKind::Added,
            (Some(_), None) => ChangeKind::Removed,
            (Some(old), Some(new)) if old != new => ChangeKind::Modified,
            _ => continue,
        };
        changes.push(Change::new(path, kind));
    }
    let described = describe(
        &changes,
        &|path| before.read(path),
        &|path| after.read(path),
        None,
    )?;
    let abbreviation = after
        .read(layout::CONFIG)?
        .and_then(|bytes| Config::parse(&String::from_utf8_lossy(&bytes)).ok())
        .map(|config| config.abbreviation);
    let mut lines = described.lines(abbreviation).into_iter();
    let subject = lines.next().unwrap_or_default();
    let rest: Vec<String> = lines.collect();
    Ok(match rest.is_empty() {
        true => subject,
        false => format!("{subject}\n\n{}", rest.join("\n")),
    })
}
