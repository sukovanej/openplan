use std::collections::BTreeMap;

use op_backend::{
    Actor, BackendError, MemorySnapshot, MergeInput, MergePolicy, Op, Overlay, PreferTheirs,
    Resolution, Revision, RevisionId, Snapshot, Tips, merge,
};

fn revision(id: &str, author: &str) -> Revision {
    Revision {
        id: RevisionId::new(id),
        parents: Vec::new(),
        author: Actor::new(author),
        at: op_backend::now(),
        message: String::new(),
    }
}

fn tips() -> (Revision, Revision) {
    (revision("0001", "Ann"), revision("0002", "Ben"))
}

fn merged(
    base: &MemorySnapshot,
    ours: &MemorySnapshot,
    theirs: &MemorySnapshot,
    policy: &dyn MergePolicy,
) -> Resolution {
    let (mine, other) = tips();
    merge(
        base,
        ours,
        theirs,
        Tips {
            ours: &mine,
            theirs: &other,
        },
        policy,
    )
    .expect("merge")
}

fn tree(files: &[(&str, &str)]) -> MemorySnapshot {
    MemorySnapshot::new(
        None,
        files
            .iter()
            .map(|(path, text)| ((*path).to_owned(), text.as_bytes().to_vec()))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn applied(theirs: &MemorySnapshot, ops: Vec<Op>) -> BTreeMap<String, String> {
    let mut overlay = Overlay::new(theirs);
    overlay.apply(ops);
    overlay
        .files()
        .expect("files")
        .into_iter()
        .map(|path| {
            let text = overlay.read_text(&path).expect("read").expect("present");
            (path, text)
        })
        .collect()
}

fn files(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

#[test]
fn changes_on_different_paths_combine() {
    let base = tree(&[("a", "1"), ("b", "1"), ("c", "1")]);
    let ours = tree(&[("a", "ours"), ("b", "1")]);
    let theirs = tree(&[("a", "1"), ("b", "theirs"), ("c", "1"), ("d", "new")]);
    let ops = merged(&base, &ours, &theirs, &PreferTheirs).ops;
    assert_eq!(
        applied(&theirs, ops),
        files(&[("a", "ours"), ("b", "theirs"), ("d", "new")])
    );
}

#[test]
fn the_same_change_on_both_sides_is_no_conflict() {
    let base = tree(&[("a", "1")]);
    let ours = tree(&[("a", "2")]);
    let theirs = tree(&[("a", "2")]);
    let seen = Recorder::default();
    merged(&base, &ours, &theirs, &seen);
    assert_eq!(seen.conflicts(), Vec::<String>::new());
}

#[test]
fn a_conflict_keeps_theirs_until_the_policy_answers() {
    let base = tree(&[("a", "1")]);
    let ours = tree(&[("a", "ours")]);
    let theirs = tree(&[("a", "theirs")]);
    let ops = merged(&base, &ours, &theirs, &PreferTheirs).ops;
    assert_eq!(applied(&theirs, ops), files(&[("a", "theirs")]));

    let seen = Recorder::default();
    merged(&base, &ours, &theirs, &seen);
    assert_eq!(seen.conflicts(), vec!["a".to_owned()]);
}

#[test]
fn a_removal_against_an_edit_is_a_conflict() {
    let base = tree(&[("a", "1")]);
    let ours = tree(&[]);
    let theirs = tree(&[("a", "edited")]);
    let seen = Recorder::default();
    merged(&base, &ours, &theirs, &seen);
    assert_eq!(seen.conflicts(), vec!["a".to_owned()]);
}

#[test]
fn the_policy_ops_land_on_the_merge() {
    struct Both;
    impl MergePolicy for Both {
        fn resolve(&self, input: &MergeInput<'_>) -> Result<Resolution, BackendError> {
            assert_eq!(input.merged.read_text("b")?.as_deref(), Some("ours"));
            Ok(Resolution {
                ops: input
                    .conflicts
                    .iter()
                    .map(|path| Op::put(path.clone(), "both"))
                    .collect(),
                notes: vec![format!(
                    "{} and {} both wrote",
                    input.tips.ours.author.name, input.tips.theirs.author.name
                )],
            })
        }
    }
    let base = tree(&[("a", "1"), ("b", "1")]);
    let ours = tree(&[("a", "ours"), ("b", "ours")]);
    let theirs = tree(&[("a", "theirs"), ("b", "1")]);
    let resolution = merged(&base, &ours, &theirs, &Both);
    assert_eq!(
        applied(&theirs, resolution.ops),
        files(&[("a", "both"), ("b", "ours")])
    );
    assert_eq!(resolution.notes, vec!["Ann and Ben both wrote"]);
}

#[derive(Default)]
struct Recorder(std::sync::Mutex<Vec<String>>);

impl Recorder {
    fn conflicts(&self) -> Vec<String> {
        self.0.lock().expect("lock").clone()
    }
}

impl MergePolicy for Recorder {
    fn resolve(&self, input: &MergeInput<'_>) -> Result<Resolution, BackendError> {
        self.0
            .lock()
            .expect("lock")
            .extend(input.conflicts.iter().cloned());
        Ok(Resolution::default())
    }
}
