use crate::{BackendError, Change, Op, Overlay, Revision, Snapshot, diff};

pub struct MergeInput<'a> {
    pub base: &'a dyn Snapshot,
    pub ours: &'a dyn Snapshot,
    pub theirs: &'a dyn Snapshot,
    // Theirs with every change of ours that theirs left alone. A conflicting path still holds theirs.
    pub merged: &'a dyn Snapshot,
    pub conflicts: &'a [String],
    pub ours_changed: &'a [Change],
    pub tips: Tips<'a>,
}

// The newest revision on each side, which names that side to a reader.
#[derive(Debug, Clone, Copy)]
pub struct Tips<'a> {
    pub ours: &'a Revision,
    pub theirs: &'a Revision,
}

// `notes` tell a reader what the merge did beyond joining the two sides; they go into the message
// of the merge revision.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolution {
    pub ops: Vec<Op>,
    pub notes: Vec<String>,
}

pub trait MergePolicy: Send + Sync {
    // `ops` apply on top of `merged`. Sync runs unattended, so every conflict needs an answer.
    fn resolve(&self, input: &MergeInput<'_>) -> Result<Resolution, BackendError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PreferTheirs;

impl MergePolicy for PreferTheirs {
    fn resolve(&self, _: &MergeInput<'_>) -> Result<Resolution, BackendError> {
        Ok(Resolution::default())
    }
}

// The ops that turn `theirs` into the merge of both sides.
pub fn merge(
    base: &dyn Snapshot,
    ours: &dyn Snapshot,
    theirs: &dyn Snapshot,
    tips: Tips<'_>,
    policy: &dyn MergePolicy,
) -> Result<Resolution, BackendError> {
    let ours_changed = diff(base, ours)?;
    let mut merged = Overlay::new(theirs);
    let mut conflicts = Vec::new();
    for change in &ours_changed {
        let path = &change.path;
        let (old, mine, other) = (base.read(path)?, ours.read(path)?, theirs.read(path)?);
        if other == old {
            merged.set(path.clone(), mine);
        } else if other != mine {
            conflicts.push(path.clone());
        }
    }
    let resolution = policy.resolve(&MergeInput {
        base,
        ours,
        theirs,
        merged: &merged,
        conflicts: &conflicts,
        ours_changed: &ours_changed,
        tips,
    })?;
    merged.apply(resolution.ops);
    Ok(Resolution {
        ops: merged.into_ops(),
        notes: resolution.notes,
    })
}
