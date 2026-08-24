use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use op_lint::{Code, Diagnostic, Position, Snapshot, Span, github_slug, lint};
use op_task::Abbreviation;

const ROOT: &str = "/repo";

fn abbr() -> Abbreviation {
    "OPP".parse().unwrap()
}

fn tpath(name: &str) -> PathBuf {
    Path::new(ROOT).join(".plan/tasks").join(name)
}

fn gpath(name: &str) -> PathBuf {
    Path::new(ROOT).join(".plan/tags").join(name)
}

fn file(name: &str, source: &str) -> (PathBuf, String) {
    (tpath(name), source.to_owned())
}

fn tag(name: &str, source: &str) -> (PathBuf, String) {
    (gpath(name), source.to_owned())
}

fn lint_files(files: &[(PathBuf, String)]) -> Vec<Diagnostic> {
    lint_all(files, &[])
}

fn lint_all(files: &[(PathBuf, String)], tags: &[(PathBuf, String)]) -> Vec<Diagnostic> {
    lint(
        &Snapshot::from_files(PathBuf::from(ROOT), abbr(), files.to_vec()).with_tags(tags.to_vec()),
    )
}

// Spans are byte offsets into the file source; a fixture computes the expected span from the offending
// substring rather than hard-coding an offset, so a reworded fixture cannot silently drift.
fn span_of(source: &str, needle: &str) -> Span {
    let start = source
        .find(needle)
        .unwrap_or_else(|| panic!("`{needle}` is not in the fixture"));
    Span {
        start,
        end: start + needle.len(),
    }
}

fn only(diags: Vec<Diagnostic>) -> Diagnostic {
    assert_eq!(
        diags.len(),
        1,
        "expected exactly one diagnostic, got {diags:#?}"
    );
    diags.into_iter().next().unwrap()
}

fn assert_single(
    name: &str,
    source: &str,
    others: &[(PathBuf, String)],
    code: Code,
    span: Option<Span>,
    fixable: bool,
) {
    let mut files = vec![file(name, source)];
    files.extend_from_slice(others);
    let d = only(lint_files(&files));
    assert_eq!(d.code, code, "code for {name}");
    assert_eq!(d.path, tpath(name), "path for {name}");
    assert_eq!(d.span, span, "span for {name}");
    assert_eq!(d.fixable, fixable, "fixable for {name}");
    assert_eq!(
        d.position,
        span.map(|span| Position::in_source(source, span.start)),
        "a span carries the position it resolves to in the snapshot's own source, not the disk's"
    );
}

fn assert_single_tag(name: &str, source: &str, code: Code, span: Option<Span>, fixable: bool) {
    let d = only(lint_all(&[], &[tag(name, source)]));
    assert_eq!(d.code, code, "code for {name}");
    assert_eq!(d.path, gpath(name), "path for {name}");
    assert_eq!(d.span, span, "span for {name}");
    assert_eq!(d.fixable, fixable, "fixable for {name}");
}

fn assert_graph(diags: &[Diagnostic], code: Code, names: &[&str]) {
    assert!(!diags.is_empty(), "expected {code:?} diagnostics, got none");
    let expected: BTreeSet<PathBuf> = names.iter().map(|name| tpath(name)).collect();
    for d in diags {
        assert_eq!(d.code, code, "only {code:?} expected: {d:#?}");
        assert!(expected.contains(&d.path), "unexpected path: {d:#?}");
        assert!(
            d.span.is_none(),
            "a graph diagnostic carries no span: {d:#?}"
        );
        assert!(!d.fixable, "a graph diagnostic is report-only: {d:#?}");
    }
    let reported: BTreeSet<PathBuf> = diags.iter().map(|d| d.path.clone()).collect();
    assert_eq!(
        reported, expected,
        "every participant named in a multi-file defect must be reported, not just one of them"
    );
}

#[test]
fn a_well_formed_store_lints_clean() {
    let files = vec![
        file(
            "00001-one.md",
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\nbody\n",
        ),
        file(
            "00002-two.md",
            "---\nstatus: done\ncreated: 2026-02-01T00:00:00Z\n---\n# Two\n\nbody\n",
        ),
    ];
    let diags = lint_files(&files);
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn resolvable_references_lint_clean() {
    let target = file(
        "00002-two.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Two\n\nDetails.\n",
    );
    let referrer = file(
        "00001-one.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: ./00002-two.md\ndependencies:\n  - ./00002-two.md\n---\n# One\n\nSee [[./00002-two.md]] and [[./00002-two.md#Two]].\n",
    );
    let diags = lint_files(&[target, referrer]);
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn a_missing_frontmatter_fence_is_reported() {
    let source = "# Title\n\nbody with no frontmatter\n";
    assert_single(
        "00001-no-fence.md",
        source,
        &[],
        Code::Frontmatter,
        None,
        false,
    );
}

#[test]
fn unparseable_frontmatter_yaml_is_reported() {
    let source = "---\n- not\n- a\n- map\n---\n# Title\n\nbody\n";
    assert_single(
        "00001-bad-yaml.md",
        source,
        &[],
        Code::Frontmatter,
        None,
        false,
    );
}

#[test]
fn a_status_outside_the_enum_is_reported() {
    let source = "---\nstatus: nope\ncreated: 2026-01-01T00:00:00Z\n---\n# Title\n\nbody\n";
    assert_single(
        "00001-status.md",
        source,
        &[],
        Code::Status,
        Some(span_of(source, "nope")),
        false,
    );
}

#[test]
fn a_created_that_is_not_rfc3339_is_reported() {
    let source = "---\nstatus: todo\ncreated: not-a-timestamp\n---\n# Title\n\nbody\n";
    assert_single(
        "00001-created.md",
        source,
        &[],
        Code::Created,
        Some(span_of(source, "not-a-timestamp")),
        false,
    );
}

#[test]
fn a_parent_that_is_a_section_ref_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: \"42#Design\"\n---\n# Title\n\nbody\n";
    assert_single(
        "00001-parent-section.md",
        source,
        &[],
        Code::Parent,
        Some(span_of(source, "42#Design")),
        false,
    );
}

#[test]
fn dependencies_that_are_not_a_sequence_are_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ndependencies: nope\n---\n# Title\n\nbody\n";
    assert_single(
        "00001-deps.md",
        source,
        &[],
        Code::Dependencies,
        Some(span_of(source, "nope")),
        false,
    );
}

#[test]
fn a_dependency_sequence_entry_that_is_a_section_ref_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ndependencies:\n  - ./00002-two.md\n  - \"42#Design\"\n---\n# Title\n\nbody\n";
    let target = file(
        "00002-two.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Two\n",
    );
    assert_single(
        "00001-dep-section.md",
        source,
        &[target],
        Code::Dependencies,
        Some(span_of(source, "42#Design")),
        false,
    );
}

// The snapshot is the only source a rule may read, so a diagnostic locates itself without touching
// the disk — which is what lets a caller lint bytes that were never written to one.
#[test]
fn a_bad_dependency_entry_is_located_by_line_and_named_by_index() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ndependencies:\n  - ./00002-two.md\n  - nonsense\n---\n# Title\n\nbody\n";
    let target = file(
        "00002-two.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Two\n",
    );
    let d = only(lint_files(&[file("00001-deps.md", source), target]));
    assert_eq!(d.code, Code::Dependencies);
    assert_eq!(d.position, Some(Position { line: 6, column: 5 }));
    assert!(
        d.message.contains("entry 2"),
        "the message names which entry is bad, got {:?}",
        d.message
    );
    assert_eq!(
        format!("{d}").lines().next().unwrap_or_default(),
        format!(
            "{}:6:5: error[dependencies]: dependencies entry 2 must be a task reference, like ./00042-write-the-parser.md",
            tpath("00001-deps.md").display()
        )
    );
}

#[test]
fn a_rank_that_is_not_base36_is_reported() {
    let source =
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nrank: A_B\n---\n# Title\n\nbody\n";
    assert_single(
        "00001-rank.md",
        source,
        &[],
        Code::Rank,
        Some(span_of(source, "A_B")),
        false,
    );
}

#[test]
fn an_unresolved_parent_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: ./00099-missing.md\n---\n# Title\n\nbody\n";
    assert_single(
        "00001-dangling-parent.md",
        source,
        &[],
        Code::Reference,
        Some(span_of(source, "./00099-missing.md")),
        false,
    );
}

#[test]
fn an_unresolved_dependency_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ndependencies:\n  - ./00099-missing.md\n---\n# Title\n\nbody\n";
    assert_single(
        "00001-dangling-dep.md",
        source,
        &[],
        Code::Reference,
        Some(span_of(source, "./00099-missing.md")),
        false,
    );
}

#[test]
fn an_unresolved_body_reference_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Title\n\nSee [[./00099-missing.md]] please.\n";
    assert_single(
        "00001-dangling-body.md",
        source,
        &[],
        Code::Reference,
        Some(span_of(source, "[[./00099-missing.md]]")),
        false,
    );
}

#[test]
fn an_unresolved_markdown_link_into_source_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tasks = root.join(".plan/tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    let path = tasks.join("00001-source-link.md");
    let dest = "../../crates/op-lint/src/nope.rs";
    let source = format!(
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Title\n\nSee [the crate]({dest}).\n"
    );
    let snap = Snapshot::from_files(root.clone(), abbr(), vec![(path.clone(), source.clone())]);
    let d = only(lint(&snap));
    assert_eq!(d.code, Code::Reference);
    assert_eq!(d.path, path);
    assert_eq!(d.span, Some(span_of(&source, dest)));
    assert!(!d.fixable);
}

#[test]
fn an_anchor_matching_no_heading_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\nSee [[./00002-two.md#Nowhere]].\n";
    let target = file(
        "00002-two.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Two\n",
    );
    assert_single(
        "00001-anchor.md",
        source,
        &[target],
        Code::Reference,
        Some(span_of(source, "[[./00002-two.md#Nowhere]]")),
        false,
    );
}

#[test]
fn github_slug_follows_the_forge_scheme() {
    assert_eq!(github_slug("Two"), "two");
    assert_eq!(github_slug("Design and Plan"), "design-and-plan");
    assert_eq!(github_slug("Design & Plan"), "design--plan");
    assert_eq!(github_slug("Plan (v2)"), "plan-v2");
}

#[test]
fn multi_word_and_duplicate_heading_anchors_lint_clean() {
    let target = file(
        "00002-two.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Two\n\n## Design & Plan\n\n## Notes\n\n## Notes\n",
    );
    let referrer = file(
        "00001-one.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\nSee [[./00002-two.md#Design & Plan]], [[./00002-two.md#Notes]], and [[./00002-two.md#Notes-1]].\n",
    );
    let diags = lint_files(&[target, referrer]);
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn a_bare_number_body_reference_is_reported_unrewritable() {
    let source =
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\nSee [[42]] here.\n";
    assert_single(
        "00001-bare-number.md",
        source,
        &[],
        Code::UnrewritableRef,
        Some(span_of(source, "[[42]]")),
        false,
    );
}

#[test]
fn a_foreign_key_body_reference_is_reported_unrewritable() {
    let source =
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\nBlocked by [[WEB-7]].\n";
    assert_single(
        "00001-foreign-key.md",
        source,
        &[],
        Code::UnrewritableRef,
        Some(span_of(source, "[[WEB-7]]")),
        false,
    );
}

#[test]
fn a_body_with_no_title_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\nno heading at all\n";
    assert_single("00001-no-title.md", source, &[], Code::Title, None, false);
}

#[test]
fn a_body_with_an_empty_title_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# \n\nbody\n";
    assert_single(
        "00001-empty-title.md",
        source,
        &[],
        Code::Title,
        None,
        false,
    );
}

#[test]
fn a_body_with_multiple_titles_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\n# Two\n";
    assert_single("00001-two-titles.md", source, &[], Code::Title, None, false);
}

#[test]
fn a_parent_cycle_is_reported() {
    let files = vec![
        file(
            "00001-one.md",
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: ./00002-two.md\n---\n# One\n",
        ),
        file(
            "00002-two.md",
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\nparent: ./00001-one.md\n---\n# Two\n",
        ),
    ];
    assert_graph(
        &lint_files(&files),
        Code::ParentCycle,
        &["00001-one.md", "00002-two.md"],
    );
}

#[test]
fn a_dependency_cycle_is_reported() {
    let files = vec![
        file(
            "00001-one.md",
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ndependencies:\n  - ./00002-two.md\n---\n# One\n",
        ),
        file(
            "00002-two.md",
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ndependencies:\n  - ./00001-one.md\n---\n# Two\n",
        ),
    ];
    assert_graph(
        &lint_files(&files),
        Code::DependencyCycle,
        &["00001-one.md", "00002-two.md"],
    );
}

#[test]
fn two_files_claiming_one_number_are_reported() {
    let files = vec![
        file(
            "00001-alpha.md",
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Alpha\n",
        ),
        file(
            "00001-beta.md",
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Beta\n",
        ),
    ];
    assert_graph(
        &lint_files(&files),
        Code::DuplicateNumber,
        &["00001-alpha.md", "00001-beta.md"],
    );
}

#[test]
fn a_broken_frontmatter_file_does_not_hide_the_rest() {
    let files = vec![
        file(
            "00001-broken.md",
            "---\n- not\n- a\n- map\n---\n# Broken\n\nbody\n",
        ),
        file(
            "00002-bad-status.md",
            "---\nstatus: nope\ncreated: 2026-01-01T00:00:00Z\n---\n# Bad\n\nbody\n",
        ),
        file(
            "00003-no-title.md",
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\nno heading here\n",
        ),
    ];
    let diags = lint_files(&files);
    assert!(
        diags
            .iter()
            .any(|d| d.code == Code::Frontmatter && d.path == tpath("00001-broken.md")),
        "the broken file's own diagnostic is missing: {diags:#?}"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == Code::Status && d.path == tpath("00002-bad-status.md")),
        "a broken sibling hid the bad-status file: {diags:#?}"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code == Code::Title && d.path == tpath("00003-no-title.md")),
        "a broken sibling hid the no-title file: {diags:#?}"
    );
    assert_eq!(
        diags.len(),
        3,
        "each file contributes exactly its own diagnostic: {diags:#?}"
    );
}

#[test]
fn a_reference_that_names_an_entry_heading_does_not_resolve() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\nSee [[./00002-two.md#2026-01-01T00:00:00Z by Test]].\n";
    let target = file(
        "00002-two.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Two\n\n## Comments\n\n### 2026-01-01T00:00:00Z by Test\n\n> a\n",
    );
    assert_single(
        "00001-anchor.md",
        source,
        &[target],
        Code::Reference,
        Some(span_of(
            source,
            "[[./00002-two.md#2026-01-01T00:00:00Z by Test]]",
        )),
        false,
    );
}

#[test]
fn the_comments_section_itself_is_no_anchor() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\nSee [[./00002-two.md#Comments]].\n";
    let target = file(
        "00002-two.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Two\n\n## Comments\n\n### 2026-01-01T00:00:00Z by Test\n\n> a\n",
    );
    assert_single(
        "00001-anchor.md",
        source,
        &[target],
        Code::Reference,
        Some(span_of(source, "[[./00002-two.md#Comments]]")),
        false,
    );
}

#[test]
fn a_reference_inside_a_comment_is_left_alone() {
    let target = file(
        "00002-two.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# Two\n",
    );
    let referrer = file(
        "00001-one.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\n## Comments\n\n### 2026-01-01T00:00:00Z by Test\n\n> See [[42]] and [[./00099-gone.md]].\n",
    );
    let diags = lint_files(&[target, referrer]);
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn a_well_formed_comment_log_lints_clean() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\n## Comments\n\n### 2026-01-01T00:00:00Z by Test via claude-code\n\n> # a quoted heading\n>\n> and prose\n\n### 2026-01-02T00:00:00Z by Test\n\n> more\n";
    let diags = lint_files(&[file("00001-one.md", source)]);
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn a_comments_section_that_is_not_last_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\n## Comments\n\n### 2026-01-01T00:00:00Z by Test\n\n> a\n\n## Plan\n\nsteps\n";
    assert_single(
        "00001-one.md",
        source,
        &[],
        Code::Comment,
        Some(span_of(source, "## Comments\n")),
        false,
    );
}

#[test]
fn prose_the_comment_log_has_no_place_for_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\n---\n# One\n\n## Comments\n\nplain prose a human typed\n\n### 2026-01-01T00:00:00Z by Test\n\n> a\n";
    assert_single(
        "00001-one.md",
        source,
        &[],
        Code::Comment,
        Some(span_of(source, "plain prose a human typed\n")),
        false,
    );
}

const BACKEND_TAG: &str = "---\ncolor: teal\n---\n# Backend\n\nWork below the API.\n";

#[test]
fn a_well_formed_tag_registry_lints_clean() {
    let tags = vec![
        tag("backend", BACKEND_TAG),
        tag("wip", "---\ncolor: amber\n---\n# WIP\n"),
    ];
    let files = vec![file(
        "00001-one.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- backend\n- wip\n---\n# One\n",
    )];
    let diags = lint_all(&files, &tags);
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn a_tag_filename_that_is_not_normalized_is_reported() {
    let d = only(lint_all(&[], &[tag("Back End", BACKEND_TAG)]));
    assert_eq!(d.code, Code::TagName);
    assert_eq!(d.path, gpath("Back End"));
    assert!(!d.fixable, "a rename moves a file, which fix cannot do");
    assert_eq!(
        d.help.as_deref(),
        Some("rename the file to back-end.md"),
        "the reader is told the one name the file may have"
    );
}

#[test]
fn a_tag_filename_that_names_no_tag_is_reported() {
    assert_single_tag("c++", BACKEND_TAG, Code::TagName, None, false);
}

#[test]
fn a_missing_tag_color_is_reported_and_fixable() {
    assert_single_tag(
        "backend",
        "---\nrank: a0\n---\n# Backend\n",
        Code::TagColor,
        None,
        true,
    );
}

#[test]
fn a_tag_color_outside_the_palette_is_reported() {
    let source = "---\ncolor: notacolor\n---\n# Backend\n";
    assert_single_tag(
        "backend",
        source,
        Code::TagColor,
        Some(span_of(source, "notacolor")),
        false,
    );
}

#[test]
fn a_tag_without_one_title_is_reported() {
    assert_single_tag(
        "backend",
        "---\ncolor: teal\n---\n\nWork below the API.\n",
        Code::Title,
        None,
        false,
    );
    assert_single_tag(
        "backend",
        "---\ncolor: teal\n---\n# Backend\n\n# Also Backend\n",
        Code::Title,
        None,
        false,
    );
}

#[test]
fn tag_frontmatter_that_does_not_parse_is_reported() {
    assert_single_tag("backend", "# Backend\n", Code::Frontmatter, None, false);
}

#[test]
fn a_tags_field_that_is_not_a_sequence_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags: backend\n---\n# Title\n";
    assert_single(
        "00001-tags.md",
        source,
        &[],
        Code::Tags,
        Some(span_of(source, "backend")),
        false,
    );
}

#[test]
fn a_tags_entry_that_is_not_normalized_is_reported_and_fixable() {
    let source =
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- Back End\n---\n# Title\n";
    assert_single(
        "00001-tags.md",
        source,
        &[],
        Code::Tags,
        Some(span_of(source, "Back End")),
        true,
    );
}

#[test]
fn a_tags_entry_that_names_no_tag_is_reported() {
    let source = "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- c++\n---\n# Title\n";
    assert_single(
        "00001-tags.md",
        source,
        &[],
        Code::Tags,
        Some(span_of(source, "c++")),
        false,
    );
}

// An entry no rewrite can repair takes the whole field with it: the fix replaces the block as one,
// so no other entry in it may be reported as fixable either.
#[test]
fn one_unrepairable_tags_entry_makes_the_whole_field_unfixable() {
    let source =
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- Wip\n- c++\n---\n# Title\n";
    let diags = lint_files(&[file("00001-tags.md", source)]);
    assert_eq!(diags.len(), 2, "{diags:#?}");
    assert!(
        diags.iter().all(|d| d.code == Code::Tags && !d.fixable),
        "{diags:#?}"
    );
}

// A whole-field defect spans the whole field, down to the last item and no further: the reader is
// pointed at the set, not at one entry that is no worse than its neighbours.
#[test]
fn tags_that_are_unsorted_or_repeated_are_reported_and_fixable() {
    for (source, field) in [
        (
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- wip\n- backend\n---\n# Title\n",
            "tags:\n- wip\n- backend",
        ),
        (
            "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- backend\n- backend\n---\n# Title\n",
            "tags:\n- backend\n- backend",
        ),
    ] {
        assert_single(
            "00001-tags.md",
            source,
            &[],
            Code::Tags,
            Some(span_of(source, field)),
            true,
        );
    }
}

// A task may name a tag this branch does not hold: the registry is read globally and written
// locally, so a name created on another branch is legal rather than dangling.
#[test]
fn a_tags_entry_that_no_registered_tag_matches_lints_clean() {
    let files = vec![file(
        "00001-one.md",
        "---\nstatus: todo\ncreated: 2026-01-01T00:00:00Z\ntags:\n- elsewhere\n---\n# One\n",
    )];
    let diags = lint_all(&files, &[tag("backend", BACKEND_TAG)]);
    assert!(diags.is_empty(), "{diags:#?}");
}
