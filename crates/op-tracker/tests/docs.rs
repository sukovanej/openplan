use std::sync::Arc;

use op_backend::{Actor, Edit, Op};
use op_backend_local::{LocalBackend, Options};
use op_task::Timestamp;
use op_task::doc::Doc;
use op_task::layout;
use op_tracker::{HistoryQuery, Tracker, TrackerError};

struct Fixture {
    _dir: tempfile::TempDir,
    tracker: Tracker,
}

fn actor() -> Actor {
    Actor::new("Ada").with_email("ada@example.com")
}

fn stamp() -> Timestamp {
    "2026-01-01T00:00:00Z".parse().expect("time")
}

fn started() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let backend = LocalBackend::open(dir.path(), Options::default()).expect("open");
    let tracker = Tracker::new(Arc::new(backend));
    tracker
        .init(&actor(), "OPP".parse().expect("abbreviation"))
        .expect("init");
    Fixture { _dir: dir, tracker }
}

impl Fixture {
    fn planted(&self, display_name: &str, content: &str) -> String {
        let mut doc = Doc::new(display_name, stamp()).expect("name");
        doc.set_content(content);
        self.tracker
            .create_doc(&actor(), &doc)
            .expect("create")
            .value
            .name
    }

    fn put(&self, path: &str, text: &str) {
        self.tracker
            .backend()
            .commit(&actor(), &mut |_| {
                Ok(Edit::new("Plant", vec![Op::put(path, text)]))
            })
            .expect("commit");
    }

    fn doc(&self, name: &str) -> Doc {
        self.tracker.plan().expect("plan").doc(name).expect("doc")
    }

    fn nest(&self, name: &str, parent: Option<&str>) -> Result<(), TrackerError> {
        self.tracker.update_doc(&actor(), name, |doc| {
            doc.set_parent(parent)
                .map_err(|err| TrackerError::Invalid(err.to_string()))
        })?;
        Ok(())
    }
}

#[test]
fn a_doc_is_stored_under_its_name() {
    let fixture = started();
    let name = fixture.planted("Architecture", "The tasks live in a git ref.");
    assert_eq!(name, "architecture");
    let plan = fixture.tracker.plan().expect("plan");
    let text = plan.raw_doc("architecture").expect("raw");
    assert!(text.contains("# Architecture"), "{text}");
    assert_eq!(
        plan.doc_names().iter().collect::<Vec<_>>(),
        vec!["architecture"]
    );
}

#[test]
fn a_doc_needs_a_started_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let backend = LocalBackend::open(dir.path(), Options::default()).expect("open");
    let tracker = Tracker::new(Arc::new(backend));
    let doc = Doc::new("Architecture", stamp()).expect("name");
    assert!(matches!(
        tracker.create_doc(&actor(), &doc),
        Err(TrackerError::NotInitialized)
    ));
}

#[test]
fn a_second_doc_of_one_name_is_refused() {
    let fixture = started();
    fixture.planted("Architecture", "first");
    let again = Doc::new("architecture", stamp()).expect("name");
    assert!(matches!(
        fixture.tracker.create_doc(&actor(), &again),
        Err(TrackerError::DocExists { .. })
    ));
    assert_eq!(fixture.doc("architecture").content(), "first\n");
}

#[test]
fn an_update_replaces_the_content_below_the_title() {
    let fixture = started();
    fixture.planted("Architecture", "old");
    fixture
        .tracker
        .update_doc(&actor(), "architecture", |doc| {
            doc.set_content("new");
            Ok(())
        })
        .expect("update");
    let doc = fixture.doc("architecture");
    assert_eq!(doc.content(), "new\n");
    assert_eq!(doc.title().as_deref(), Some("Architecture"));
}

#[test]
fn a_rename_moves_the_doc_and_leaves_no_old_one() {
    let fixture = started();
    fixture.planted("Architecture", "body");
    let renamed = fixture
        .tracker
        .rename_doc(&actor(), "architecture", "Storage Layout")
        .expect("rename");
    assert_eq!(renamed.value.name, "storage-layout");
    let plan = fixture.tracker.plan().expect("plan");
    assert!(!plan.doc_names().contains("architecture"));
    let doc = plan.doc("storage-layout").expect("doc");
    assert_eq!(doc.title().as_deref(), Some("Storage Layout"));
    assert_eq!(doc.content(), "body\n");
}

#[test]
fn a_rename_moves_every_link_to_the_doc_in_the_same_revision() {
    let fixture = started();
    fixture.planted("Architecture", "See [[architecture#Layers]].");
    fixture.planted("Storage", "Part of [[architecture]], next to [[other]].");
    fixture.put(
        "tasks/00001-parser.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Parser\n\nRead [[../docs/architecture.md#Layers]].\n",
    );
    let before = fixture
        .tracker
        .history(&HistoryQuery::default())
        .expect("history")
        .len();

    fixture
        .tracker
        .rename_doc(&actor(), "architecture", "Design")
        .expect("rename");

    let plan = fixture.tracker.plan().expect("plan");
    assert_eq!(
        plan.doc("design").expect("doc").content(),
        "See [[./design.md#Layers]].\n"
    );
    assert_eq!(
        plan.doc("storage").expect("doc").content(),
        "Part of [[./design.md]], next to [[./other.md]].\n"
    );
    assert!(
        plan.raw(1)
            .expect("task")
            .contains("Read [[../docs/design.md#Layers]].")
    );
    let after = fixture
        .tracker
        .history(&HistoryQuery::default())
        .expect("history")
        .len();
    assert_eq!(after, before + 1);
}

#[test]
fn a_rename_onto_a_taken_name_is_refused_and_leaves_both_docs() {
    let fixture = started();
    fixture.planted("Architecture", "one");
    fixture.planted("Storage", "two");
    assert!(matches!(
        fixture
            .tracker
            .rename_doc(&actor(), "architecture", "Storage"),
        Err(TrackerError::DocExists { .. })
    ));
    assert_eq!(fixture.doc("architecture").content(), "one\n");
    assert_eq!(fixture.doc("storage").content(), "two\n");
}

#[test]
fn a_path_whose_stem_is_not_a_doc_name_is_no_doc() {
    let fixture = started();
    fixture.planted("Architecture", "body");
    fixture.put("docs/Not A Doc.md", "# x\n");
    let plan = fixture.tracker.plan().expect("plan");
    assert_eq!(
        plan.doc_names().iter().collect::<Vec<_>>(),
        vec!["architecture"]
    );
}

#[test]
fn a_missing_doc_is_named_in_every_refusal() {
    let fixture = started();
    assert!(matches!(
        fixture.tracker.plan().expect("plan").doc("architecture"),
        Err(TrackerError::DocNotFound { .. })
    ));
    assert!(matches!(
        fixture.tracker.delete_doc(&actor(), "architecture"),
        Err(TrackerError::DocNotFound { .. })
    ));
}

#[test]
fn a_delete_removes_the_doc() {
    let fixture = started();
    fixture.planted("Architecture", "body");
    fixture
        .tracker
        .delete_doc(&actor(), "architecture")
        .expect("delete");
    assert!(fixture.tracker.plan().expect("plan").doc_names().is_empty());
}

#[test]
fn a_doc_nests_under_another_one() {
    let fixture = started();
    fixture.planted("Architecture", "the whole");
    fixture.planted("Storage", "one part");
    fixture.nest("storage", Some("architecture")).expect("nest");
    assert_eq!(
        fixture.doc("storage").frontmatter.parent.as_deref(),
        Some("architecture")
    );
    fixture.nest("storage", None).expect("lift");
    assert_eq!(fixture.doc("storage").frontmatter.parent, None);
}

#[test]
fn a_parent_that_does_not_exist_is_refused() {
    let fixture = started();
    fixture.planted("Storage", "one part");
    assert!(matches!(
        fixture.nest("storage", Some("architecture")),
        Err(TrackerError::Invalid(_))
    ));
    let mut orphan = Doc::new("Runtime", stamp()).expect("name");
    orphan.set_parent(Some("architecture")).expect("parent");
    assert!(matches!(
        fixture.tracker.create_doc(&actor(), &orphan),
        Err(TrackerError::Invalid(_))
    ));
}

#[test]
fn a_doc_cannot_be_its_own_parent_or_close_a_cycle() {
    let fixture = started();
    fixture.planted("Architecture", "the whole");
    fixture.planted("Storage", "one part");
    fixture.planted("Blobs", "a smaller part");
    assert!(matches!(
        fixture.nest("storage", Some("storage")),
        Err(TrackerError::Invalid(_))
    ));
    fixture.nest("storage", Some("architecture")).expect("nest");
    fixture.nest("blobs", Some("storage")).expect("nest");
    assert!(matches!(
        fixture.nest("architecture", Some("blobs")),
        Err(TrackerError::Invalid(_))
    ));
}

#[test]
fn a_rename_carries_the_children_in_the_same_revision() {
    let fixture = started();
    fixture.planted("Architecture", "the whole");
    fixture.planted("Storage", "one part");
    fixture.nest("storage", Some("architecture")).expect("nest");
    let renamed = fixture
        .tracker
        .rename_doc(&actor(), "architecture", "The Design")
        .expect("rename");
    assert_eq!(
        fixture.doc("storage").frontmatter.parent.as_deref(),
        Some("the-design")
    );
    let paths: Vec<String> = renamed
        .committed
        .expect("committed")
        .changes
        .into_iter()
        .map(|change| change.path)
        .collect();
    assert!(paths.contains(&layout::doc_path("storage")), "{paths:?}");
}

#[test]
fn an_edit_does_not_have_to_repair_a_parent_its_doc_lost() {
    let fixture = started();
    fixture.put(
        &layout::doc_path("storage"),
        "---\ncreated: 2026-01-01T00:00:00Z\nparent: ./architecture.md\n---\n# Storage\n\none part\n",
    );
    fixture
        .tracker
        .update_doc(&actor(), "storage", |doc| {
            doc.set_content("still one part");
            Ok(())
        })
        .expect("update");
    let doc = fixture.doc("storage");
    assert_eq!(doc.content(), "still one part\n");
    assert_eq!(doc.frontmatter.parent.as_deref(), Some("architecture"));
}

#[test]
fn a_child_the_parser_rejects_still_follows_its_parent() {
    let fixture = started();
    fixture.planted("Architecture", "the whole");
    fixture.put(
        &layout::doc_path("storage"),
        "---\nparent: ./architecture.md\n---\n# Storage\n",
    );
    fixture.put(&layout::doc_path("broken"), "no frontmatter\n");
    fixture
        .tracker
        .rename_doc(&actor(), "architecture", "The Design")
        .expect("rename");
    let text = fixture
        .tracker
        .plan()
        .expect("plan")
        .raw_doc("storage")
        .expect("raw");
    assert!(text.contains("parent: ./the-design.md"), "{text}");
    assert!(text.contains("# Storage"));
}

#[test]
fn a_doc_the_parser_rejects_does_not_block_a_nest() {
    let fixture = started();
    fixture.planted("Architecture", "the whole");
    fixture.planted("Storage", "one part");
    fixture.put(&layout::doc_path("broken"), "no frontmatter\n");
    fixture.nest("storage", Some("architecture")).expect("nest");
    assert_eq!(
        fixture.doc("storage").frontmatter.parent.as_deref(),
        Some("architecture")
    );
}

#[test]
fn a_doc_keeps_its_own_history() {
    let fixture = started();
    fixture.planted("Architecture", "first");
    fixture.planted("Storage", "other");
    fixture
        .tracker
        .update_doc(&actor(), "architecture", |doc| {
            doc.set_content("second");
            Ok(())
        })
        .expect("update");
    let messages: Vec<String> = fixture
        .tracker
        .doc_history("architecture", &HistoryQuery::default())
        .expect("history")
        .into_iter()
        .map(|entry| entry.revision.message)
        .collect();
    assert_eq!(
        messages,
        vec!["doc architecture: edit", "doc architecture: create"]
    );
}

#[test]
fn a_refused_rename_leaves_the_rest_of_the_update_unwritten() {
    let fixture = started();
    fixture.planted("Architecture", "The old text.");
    fixture.planted("Storage", "");
    let revisions = |name: &str| {
        fixture
            .tracker
            .doc_history(name, &HistoryQuery::default())
            .expect("history")
            .len()
    };
    let before = revisions("architecture");

    let refused = fixture.tracker.update_doc(&actor(), "architecture", |doc| {
        doc.set_content("The new text.");
        doc.rename("Storage")
            .map_err(|err| TrackerError::Invalid(err.to_string()))
    });

    assert!(matches!(refused, Err(TrackerError::DocExists { .. })));
    assert_eq!(fixture.doc("architecture").content(), "The old text.\n");
    assert_eq!(revisions("architecture"), before);
}

#[test]
fn a_rename_cannot_hide_a_nest_under_the_doc_s_own_child() {
    let fixture = started();
    fixture.planted("Architecture", "");
    fixture.planted("Storage", "");
    fixture.nest("storage", Some("architecture")).expect("nest");

    let refused = fixture.tracker.update_doc(&actor(), "architecture", |doc| {
        doc.rename("System design")
            .map_err(|err| TrackerError::Invalid(err.to_string()))?;
        doc.set_parent(Some("storage"))
            .map_err(|err| TrackerError::Invalid(err.to_string()))
    });

    assert!(
        matches!(refused, Err(TrackerError::Invalid(_))),
        "{refused:?}"
    );
    assert!(fixture.doc("architecture").frontmatter.parent.is_none());
}

#[test]
fn a_deleted_doc_lifts_its_nested_docs_to_its_own_parent() {
    let fixture = started();
    for name in ["Guides", "Storage", "Indexes"] {
        fixture.planted(name, "");
    }
    fixture.nest("storage", Some("guides")).expect("nest");
    fixture.nest("indexes", Some("storage")).expect("nest");

    fixture
        .tracker
        .delete_doc(&actor(), "storage")
        .expect("delete");
    assert_eq!(
        fixture.doc("indexes").frontmatter.parent.as_deref(),
        Some("guides")
    );

    fixture
        .tracker
        .delete_doc(&actor(), "guides")
        .expect("delete");
    assert_eq!(fixture.doc("indexes").frontmatter.parent, None);
}
