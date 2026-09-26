use std::time::{Duration, Instant};

use op_backend::{Actor, Backend, BackendEvent, Edit, LogQuery, Op, Origin};
use op_backend_local::{EXTERNAL_MESSAGE, HISTORY_FILE, LocalBackend, Options};

fn open(dir: &std::path::Path) -> LocalBackend {
    LocalBackend::open(dir, Options::default()).expect("open")
}

fn put(backend: &dyn Backend, path: &str, text: &str) {
    let (path, text) = (path.to_owned(), text.to_owned());
    backend
        .commit(&Actor::new("Ada"), &mut |_| {
            Ok(Edit::new("Write", vec![Op::put(&path, text.as_bytes())]))
        })
        .expect("commit")
        .expect("a revision");
}

#[test]
fn documents_are_plain_files_beside_a_hidden_history() {
    let dir = tempfile::tempdir().expect("tempdir");
    let backend = open(dir.path());
    put(&backend, "tasks/00001-a.md", "a");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("tasks/00001-a.md")).expect("read"),
        "a"
    );
    assert!(dir.path().join(HISTORY_FILE).is_file());
    assert_eq!(
        backend.head().expect("head").files().expect("files"),
        vec!["tasks/00001-a.md"]
    );
}

#[test]
fn refresh_records_a_hand_edit_as_an_external_revision() {
    let dir = tempfile::tempdir().expect("tempdir");
    let author = Actor::new("Milan").with_email("milan@example.com");
    let backend = LocalBackend::open(
        dir.path(),
        Options {
            watch: false,
            external_author: author.clone(),
        },
    )
    .expect("open");
    put(&backend, "a.md", "a");
    std::fs::write(dir.path().join("a.md"), "edited").expect("write");
    std::fs::write(dir.path().join("b.md"), "new").expect("write");

    let moved = backend.refresh().expect("refresh").expect("a move");
    assert_eq!(moved.origin, Origin::External);
    assert_eq!(moved.to.author, author);
    assert_eq!(moved.to.message, EXTERNAL_MESSAGE);
    assert_eq!(moved.changes.len(), 2);
    assert_eq!(backend.refresh().expect("refresh"), None);
    assert_eq!(
        backend
            .head()
            .expect("head")
            .read_text("a.md")
            .expect("read")
            .as_deref(),
        Some("edited")
    );
}

#[test]
fn an_edit_made_while_closed_is_recorded_on_open() {
    let dir = tempfile::tempdir().expect("tempdir");
    put(&open(dir.path()), "a.md", "a");
    std::fs::remove_file(dir.path().join("a.md")).expect("remove");
    let backend = open(dir.path());
    let log = backend.log(&LogQuery::default()).expect("log");
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].revision.message, EXTERNAL_MESSAGE);
    assert_eq!(
        backend.head().expect("head").files().expect("files"),
        Vec::<String>::new()
    );
}

#[test]
fn temp_and_hidden_files_are_not_documents() {
    let dir = tempfile::tempdir().expect("tempdir");
    let backend = open(dir.path());
    std::fs::write(dir.path().join(".swap"), "x").expect("write");
    std::fs::create_dir_all(dir.path().join(".cache")).expect("mkdir");
    std::fs::write(dir.path().join(".cache/x"), "x").expect("write");
    assert_eq!(backend.refresh().expect("refresh"), None);
}

#[test]
fn the_watch_records_a_hand_edit_by_itself() {
    let dir = tempfile::tempdir().expect("tempdir");
    let backend = LocalBackend::open(
        dir.path(),
        Options {
            watch: true,
            ..Options::default()
        },
    )
    .expect("open");
    let mut events = backend.subscribe();
    std::fs::write(dir.path().join("a.md"), "by hand").expect("write");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(BackendEvent::HeadMoved(moved)) = events.try_recv() {
            assert_eq!(moved.origin, Origin::External);
            break;
        }
        assert!(Instant::now() < deadline, "no event");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn two_handles_on_one_directory_see_each_other() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first = open(dir.path());
    let second = open(dir.path());
    put(&first, "a.md", "1");
    put(&second, "a.md", "2");
    put(&first, "b.md", "b");
    let head = first.head().expect("head");
    assert_eq!(head.read_text("a.md").expect("read").as_deref(), Some("2"));
    assert_eq!(first.log(&LogQuery::default()).expect("log").len(), 3);
}

#[test]
fn a_write_from_another_process_is_announced() {
    let dir = tempfile::tempdir().expect("tempdir");
    let watching = open(dir.path());
    let mut events = watching.subscribe();
    put(&open(dir.path()), "a.md", "a");
    let moved = watching.refresh().expect("refresh").expect("a move");
    assert_eq!(moved.origin, Origin::External);
    assert_eq!(moved.to.author.name, "Ada");
    assert!(matches!(events.try_recv(), Ok(BackendEvent::HeadMoved(_))));
    assert_eq!(watching.refresh().expect("refresh"), None);
}

fn head_text(backend: &dyn Backend, path: &str) -> Option<String> {
    backend.head().expect("head").read_text(path).expect("read")
}

#[test]
fn a_hand_edit_of_the_same_size_is_recorded_after_the_scan_trusts_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let backend = open(dir.path());
    put(&backend, "a.md", "aaaa");
    // A scan trusts a file's stamp only when the file is two seconds old.
    std::thread::sleep(Duration::from_millis(2_200));
    assert_eq!(backend.refresh().expect("refresh"), None);

    std::fs::write(dir.path().join("a.md"), "bbbb").expect("write");
    let moved = backend.refresh().expect("refresh").expect("a move");
    assert_eq!(moved.changes.len(), 1);
    assert_eq!(head_text(&backend, "a.md").as_deref(), Some("bbbb"));
}

#[test]
fn a_large_asset_edited_by_hand_is_recorded_and_read_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let backend = open(dir.path());
    let first = "first version of the asset\n".repeat(10_000);
    let second = "second version of the asset\n".repeat(10_000);
    std::fs::create_dir_all(dir.path().join("assets")).expect("mkdir");
    std::fs::write(dir.path().join("assets/big.txt"), &first).expect("write");
    assert!(backend.refresh().expect("refresh").is_some());
    assert_eq!(backend.refresh().expect("refresh"), None);

    std::fs::write(dir.path().join("assets/big.txt"), &second).expect("write");
    assert!(backend.refresh().expect("refresh").is_some());
    drop(backend);
    let reopened = open(dir.path());
    assert_eq!(
        head_text(&reopened, "assets/big.txt").as_deref(),
        Some(second.as_str())
    );
}

// The daemon opens a new project once for each first write that races to register it.
#[test]
fn a_new_store_that_several_callers_open_at_the_same_time_keeps_its_files() {
    for _ in 0..50 {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("config.toml"), "abbreviation = \"OPP\"\n").expect("write");
        let start = std::sync::Barrier::new(4);
        std::thread::scope(|scope| {
            let opens: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        start.wait();
                        LocalBackend::open(dir.path(), Options::default()).map(drop)
                    })
                })
                .collect();
            for open in opens {
                open.join().expect("an open").expect("the store opens");
            }
        });
        assert!(dir.path().join("config.toml").is_file());
        assert!(head_text(&open(dir.path()), "config.toml").is_some());
    }
}
