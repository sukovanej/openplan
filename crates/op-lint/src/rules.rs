use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use op_task::{FieldError, PartialFrontmatter, PartialMetadata};

use crate::diagnostic::{Code, Diagnostic, Span};
use crate::snapshot::{Snapshot, TaskFile, github_slug};

pub type TaskRule = fn(&Snapshot, &TaskFile, &mut Sink);
pub type StoreRule = fn(&Snapshot, &mut Sink);

#[derive(Debug, Default)]
pub struct Sink {
    diagnostics: Vec<Diagnostic>,
}

impl Sink {
    pub fn new() -> Self {
        Sink::default()
    }

    pub fn emit(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

pub const TASK_RULES: &[TaskRule] = &[
    frontmatter_parses,
    status_in_enum,
    created_valid,
    parent_is_ref,
    dependencies_are_refs,
    rank_is_base36,
    references_resolve,
    body_refs_rewritable,
    single_title,
    comment_log,
];

pub const STORE_RULES: &[StoreRule] = &[parent_cycles, dependency_cycles, unique_numbers];

fn fields(file: &TaskFile) -> Option<(&PartialFrontmatter, &serde_yaml::Mapping)> {
    match (&file.task.metadata, &file.task.frontmatter) {
        (PartialMetadata::Fields(f), Some(map)) => Some((f, map)),
        _ => None,
    }
}

fn scalar_text(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn frontmatter_span(source: &str) -> Option<Range<usize>> {
    let rest = source
        .strip_prefix("---\n")
        .or_else(|| source.strip_prefix("---\r\n"))?;
    let start = source.len() - rest.len();
    let mut pos = start;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']) == "---" {
            return Some(start..pos);
        }
        pos += line.len();
    }
    None
}

fn frontmatter_lines(file: &TaskFile) -> Vec<(usize, &str)> {
    let Some(region) = frontmatter_span(&file.source) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    let mut offset = region.start;
    for line in file.source[region].split_inclusive('\n') {
        lines.push((offset, line.trim_end_matches(['\n', '\r'])));
        offset += line.len();
    }
    lines
}

fn key_of(line: &str) -> Option<&str> {
    let (key, _) = line.split_once(':')?;
    (!key.is_empty() && !key.contains(char::is_whitespace)).then_some(key)
}

// A value is searched for on its own key's line: `42` in `parent: 42` also reads as the minutes of
// a `created:` timestamp above it, and a span that lands on another field points the reader, the
// editor, and `--json` at the wrong text.
fn value_region(file: &TaskFile, key: &str) -> Option<Range<usize>> {
    let (offset, line) = frontmatter_lines(file)
        .into_iter()
        .find(|(_, line)| key_of(line) == Some(key))?;
    Some(offset + key.len() + 1..offset + line.len())
}

fn item_region(file: &TaskFile, key: &str, index: usize) -> Option<Range<usize>> {
    let lines = frontmatter_lines(file);
    let at = lines
        .iter()
        .position(|(_, line)| key_of(line) == Some(key))?;
    let (offset, line) = lines[at];
    if !line[key.len() + 1..].trim().is_empty() {
        return Some(offset + key.len() + 1..offset + line.len());
    }
    lines[at + 1..]
        .iter()
        .map_while(|&(offset, line)| {
            let item = line.trim_start().strip_prefix('-')?.trim_start();
            Some(offset + line.len() - item.len()..offset + line.len())
        })
        .nth(index)
}

fn span_in(source: &str, region: Range<usize>, text: &str) -> Option<Span> {
    let start = source[region.clone()].find(text)? + region.start;
    Some(Span::new(start, start + text.len()))
}

fn value_span(file: &TaskFile, map: &serde_yaml::Mapping, key: &str) -> Option<Span> {
    let text = scalar_text(map.get(key)?)?;
    span_in(&file.source, value_region(file, key)?, &text)
}

fn item_span(file: &TaskFile, key: &str, index: usize, text: &str) -> Option<Span> {
    span_in(&file.source, item_region(file, key, index)?, text)
}

fn frontmatter_parses(_snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink) {
    if let PartialMetadata::Error(message) = &file.task.metadata {
        sink.emit(Diagnostic::error(
            Code::Frontmatter,
            file.path.clone(),
            message.clone(),
        ));
    }
}

fn status_in_enum(_snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink) {
    let Some((f, map)) = fields(file) else {
        return;
    };
    let Err(err) = &f.status else {
        return;
    };
    let mut d = Diagnostic::error(
        Code::Status,
        file.path.clone(),
        "status must be one of backlog, todo, in_progress, in_review, done, cancelled",
    );
    if let (FieldError::Invalid(_), Some(span)) = (err, value_span(file, map, "status")) {
        d = d.at(span, &file.source);
    }
    sink.emit(d);
}

fn created_valid(_snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink) {
    let Some((f, map)) = fields(file) else {
        return;
    };
    let Err(err) = &f.created else {
        return;
    };
    let d = match err {
        FieldError::Missing => Diagnostic::error(
            Code::Created,
            file.path.clone(),
            "created is missing",
        )
        .with_help(format!(
            "`openplan lint --fix` backfills it from the first commit that added the file; by hand: git log --diff-filter=A --format=%aI -1 -- {}",
            file.path.display()
        ))
        .mark_fixable(),
        FieldError::Invalid(_) => {
            let d = Diagnostic::error(
                Code::Created,
                file.path.clone(),
                "created must be an RFC3339 instant",
            );
            match value_span(file, map, "created") {
                Some(span) => d.at(span, &file.source),
                None => d,
            }
        }
    };
    sink.emit(d);
}

fn parent_is_ref(_snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink) {
    let Some((f, map)) = fields(file) else {
        return;
    };
    let Err(err) = &f.parent else {
        return;
    };
    let mut d = Diagnostic::error(
        Code::Parent,
        file.path.clone(),
        "parent must be a task reference and not a section reference",
    );
    if let (FieldError::Invalid(_), Some(span)) = (err, value_span(file, map, "parent")) {
        d = d.at(span, &file.source);
    }
    sink.emit(d);
}

// Read from the raw mapping rather than the parsed field: one bad element collapses the parsed list
// to a single error, and the whole point of carrying the mapping is to name the entry that is bad.
fn dependencies_are_refs(_snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink) {
    let Some((_, map)) = fields(file) else {
        return;
    };
    let Some(value) = map.get("dependencies") else {
        return;
    };
    let serde_yaml::Value::Sequence(items) = value else {
        let d = Diagnostic::error(
            Code::Dependencies,
            file.path.clone(),
            "dependencies must be a sequence of task references",
        );
        sink.emit(match value_span(file, map, "dependencies") {
            Some(span) => d.at(span, &file.source),
            None => d,
        });
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let Some(defect) = dependency_defect(item) else {
            continue;
        };
        let message = format!("dependencies entry {} {defect}", index + 1);
        let d = Diagnostic::error(Code::Dependencies, file.path.clone(), message);
        let span = scalar_text(item)
            .and_then(|text| item_span(file, "dependencies", index, &text))
            .or_else(|| item_region(file, "dependencies", index).map(Span::from));
        sink.emit(match span {
            Some(span) => d.at(span, &file.source),
            None => d,
        });
    }
}

fn dependency_defect(item: &serde_yaml::Value) -> Option<&'static str> {
    let Some(text) = scalar_text(item).filter(|text| op_task::ref_id(text).is_some()) else {
        return Some("must be a task reference, like ./00042-write-the-parser.md");
    };
    // A dependency may name a section, but only in file form: a sectioned reference is the one
    // spelling `--fix` cannot canonicalize, so the file name has to be written already.
    let sectioned_bare = text.contains('#') && !op_task::ref_target(&text).ends_with(".md");
    sectioned_bare.then_some(
        "names a section, so it must name the target's file, like ./00042-write-the-parser.md#Design",
    )
}

fn rank_is_base36(_snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink) {
    let Some((f, map)) = fields(file) else {
        return;
    };
    let report = match &f.rank {
        Ok(Some(rank)) => !op_task::rank::is_valid(rank),
        Err(_) => true,
        Ok(None) => false,
    };
    if !report {
        return;
    }
    let mut d = Diagnostic::error(
        Code::Rank,
        file.path.clone(),
        "rank must be a base-36 fractional index",
    );
    if let Some(span) = value_span(file, map, "rank") {
        d = d.at(span, &file.source);
    }
    sink.emit(d);
}

fn anchor_slugs(body: &str) -> HashSet<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut slugs = HashSet::new();
    for heading in op_task::comment::addressable(body) {
        let base = github_slug(&heading.text);
        let seen = counts.entry(base.clone()).or_insert(0);
        let anchor = if *seen == 0 {
            base.clone()
        } else {
            format!("{base}-{seen}")
        };
        *seen += 1;
        slugs.insert(anchor);
    }
    slugs
}

fn emit_ref(sink: &mut Sink, file: &TaskFile, span: Option<Span>, message: &str, fixable: bool) {
    let mut d = Diagnostic::error(Code::Reference, file.path.clone(), message);
    if let Some(span) = span {
        d = d.at(span, &file.source);
    }
    if fixable {
        d = d.mark_fixable();
    }
    sink.emit(d);
}

pub(crate) fn canonical_ref(target: &TaskFile, section: Option<&str>) -> String {
    let filename = target
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let reference = op_task::task_ref(filename);
    match section {
        Some(section) => format!("{reference}#{section}"),
        None => reference,
    }
}

struct RefSite<'a> {
    number: u64,
    section: Option<&'a str>,
    raw: &'a str,
    span: Option<Span>,
    canonicalizable: bool,
}

fn check_target_ref(snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink, site: RefSite<'_>) {
    let Some(target) = snapshot.file(site.number) else {
        emit_ref(
            sink,
            file,
            site.span,
            "reference resolves to no task",
            false,
        );
        return;
    };
    if let Some(section) = site.section {
        if !anchor_slugs(&target.task.body).contains(&github_slug(section)) {
            emit_ref(
                sink,
                file,
                site.span,
                "the section anchor matches no heading in the target",
                false,
            );
            return;
        }
    }
    if site.raw != canonical_ref(target, site.section) {
        emit_ref(
            sink,
            file,
            site.span,
            "reference is not written in its canonical file form",
            site.canonicalizable,
        );
    }
}

fn is_external(dest: &str) -> bool {
    dest.contains("://") || dest.starts_with("mailto:") || dest.starts_with("tel:")
}

fn lexically_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn markdown_link_dests(body: &str) -> Vec<(Range<usize>, &str)> {
    op_md::opaque_ranges(body)
        .into_iter()
        // `opaque_ranges` does not label its ranges, and of the three it reports — inline code,
        // code blocks and links — only a link's markup opens with `[`.
        .filter(|range| body[range.clone()].starts_with('['))
        .filter_map(|range| link_dest(body, range))
        .collect()
}

fn link_dest(body: &str, link: Range<usize>) -> Option<(Range<usize>, &str)> {
    let markup = body[link.clone()].strip_suffix(')')?;
    let open = link.start + markup.rfind("](")? + 2;
    let raw = &body[open..link.start + markup.len()];
    let lead = raw.len() - raw.trim_start().len();
    let trimmed = raw.trim_start();
    let (url, offset) = match trimmed.strip_prefix('<') {
        Some(rest) => match rest.find('>') {
            Some(gt) => (&rest[..gt], lead + 1),
            None => (trimmed, lead),
        },
        None => {
            let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
            (&trimmed[..end], lead)
        }
    };
    let start = open + offset;
    (!url.is_empty()).then_some((start..start + url.len(), url))
}

fn frontmatter_references(snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink) {
    let Some((f, map)) = fields(file) else {
        return;
    };
    if f.parent.is_ok() {
        let raw = map.get("parent").and_then(scalar_text).unwrap_or_default();
        if let Some(number) = op_task::ref_id(&raw) {
            check_target_ref(
                snapshot,
                file,
                sink,
                RefSite {
                    number,
                    section: None,
                    raw: &raw,
                    span: value_span(file, map, "parent"),
                    canonicalizable: true,
                },
            );
        }
    }
    let Some(items) = map
        .get("dependencies")
        .and_then(|value| value.as_sequence())
    else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        if dependency_defect(item).is_some() {
            continue;
        }
        let Some(raw) = scalar_text(item) else {
            continue;
        };
        let Some(number) = op_task::ref_id(&raw) else {
            continue;
        };
        let section = raw.split_once('#').map(|(_, section)| section);
        check_target_ref(
            snapshot,
            file,
            sink,
            RefSite {
                number,
                section,
                raw: &raw,
                span: item_span(file, "dependencies", index, &raw),
                // `--fix` rewrites a reference to its target's file name, and a sectioned one is
                // the spelling it leaves alone.
                canonicalizable: section.is_none(),
            },
        );
    }
}

fn references_resolve(snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink) {
    frontmatter_references(snapshot, file, sink);

    let source = &file.source;
    let body = &file.task.body;
    let body_offset = source.len().saturating_sub(body.len());
    let abbreviation = snapshot.abbreviation();

    for (range, inner) in outside_the_log(body) {
        if let Some(number) = op_task::body_ref_id(abbreviation, inner) {
            let section = inner.split_once('#').map(|(_, section)| section);
            let span = Span::new(body_offset + range.start, body_offset + range.end);
            check_target_ref(
                snapshot,
                file,
                sink,
                RefSite {
                    number,
                    section,
                    raw: inner,
                    span: Some(span),
                    canonicalizable: true,
                },
            );
        }
    }

    let Some(dir) = file.path.parent() else {
        return;
    };
    for (range, dest) in markdown_link_dests(body) {
        let (path_part, section) = match dest.split_once('#') {
            Some((path_part, section)) => (path_part, Some(section)),
            None => (dest, None),
        };
        if path_part.is_empty() || is_external(path_part) {
            continue;
        }
        let span = Span::new(body_offset + range.start, body_offset + range.end);
        let resolved = lexically_normalize(&dir.join(path_part));
        match snapshot.files().iter().find(|tf| tf.path == resolved) {
            Some(target) => {
                if let Some(section) = section {
                    if !anchor_slugs(&target.task.body).contains(&github_slug(section)) {
                        emit_ref(
                            sink,
                            file,
                            Some(span),
                            "the section anchor matches no heading in the target",
                            false,
                        );
                    }
                }
            }
            None => {
                // The one filesystem read left in a rule: the snapshot indexes task files, and a
                // link into source (`../../crates/…`) has nothing there to resolve against until it
                // indexes the paths around them too.
                if !resolved.exists() {
                    emit_ref(
                        sink,
                        file,
                        Some(span),
                        "link destination resolves to no file",
                        false,
                    );
                }
            }
        }
    }
}

fn body_refs_rewritable(snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink) {
    let source = &file.source;
    let body = &file.task.body;
    let body_offset = source.len().saturating_sub(body.len());
    let abbreviation = snapshot.abbreviation();

    for (range, inner) in outside_the_log(body) {
        if op_task::body_ref_id(abbreviation, inner).is_some() {
            continue;
        }
        if op_task::parse_id(inner).is_some() || op_task::is_key_shaped(inner) {
            let message =
                "a body reference must name a task by its file, not a bare number or foreign key";
            let d = Diagnostic::error(Code::UnrewritableRef, file.path.clone(), message).at(
                Span::new(body_offset + range.start, body_offset + range.end),
                source,
            );
            sink.emit(d);
        }
    }
}

// A comment holds text a person wrote, quoted line by line. A reference inside it belongs to that
// text: the log is append-only, so no rule may ask for it to be rewritten and no `--fix` may rewrite
// it.
fn outside_the_log(body: &str) -> Vec<(Range<usize>, &str)> {
    let log = op_task::comment::section_span(body);
    op_task::body_ref_spans(body)
        .into_iter()
        .filter(|(span, _)| !log.as_ref().is_some_and(|log| log.contains(&span.start)))
        .collect()
}

fn comment_log(_snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink) {
    let body = &file.task.body;
    let offset = file.source.len().saturating_sub(body.len());
    let at = |range: &Range<usize>| Span::new(offset + range.start, offset + range.end);
    let mut report = |span: Span, message: &str| {
        sink.emit(
            Diagnostic::error(Code::Comment, file.path.clone(), message).at(span, &file.source),
        );
    };

    let sections = op_task::comment::sections(body);
    let Some((first, extra)) = sections.split_first() else {
        return;
    };
    for section in extra {
        report(
            at(&(section.start..section.end)),
            "a task holds one `## Comments` section",
        );
    }
    if op_md::section_end(body, first) < body.len() {
        report(
            at(&(first.start..first.end)),
            "`## Comments` must be the last section of a task",
        );
    }

    for entry in op_task::comment::entries(body) {
        match (&entry.heading, &entry.quote) {
            (Some(heading), None) => report(
                at(heading),
                "a comment entry needs a blockquote below its heading",
            ),
            (None, Some(quote)) => report(
                at(quote),
                "a blockquote in the comment log needs an entry heading above it",
            ),
            (Some(heading), Some(_)) => {
                for message in heading_problems(&entry.comment) {
                    report(at(heading), &message);
                }
            }
            (None, None) => {}
        }
    }
}

fn heading_problems(comment: &op_task::comment::Comment) -> Vec<String> {
    let mut problems = Vec::new();
    match &comment.at {
        Ok(_) => {}
        Err(FieldError::Missing) => problems.push(
            "an entry heading reads `### <timestamp> by <author>`, and this one names no timestamp"
                .to_owned(),
        ),
        Err(FieldError::Invalid(message)) => problems.push(message.clone()),
    }
    if comment.author.is_err() {
        problems.push(
            "an entry heading reads `### <timestamp> by <author>`, and this one names no author"
                .to_owned(),
        );
    }
    problems
}

fn single_title(_snapshot: &Snapshot, file: &TaskFile, sink: &mut Sink) {
    let titles: Vec<_> = op_md::headings(&file.task.body)
        .into_iter()
        .filter(|heading| heading.level == 1)
        .collect();
    let ok = titles.len() == 1 && !titles[0].text.trim().is_empty();
    if !ok {
        sink.emit(Diagnostic::error(
            Code::Title,
            file.path.clone(),
            "a task needs exactly one non-empty title",
        ));
    }
}

fn resolved_parent(snapshot: &Snapshot, file: &TaskFile) -> Option<u64> {
    let PartialMetadata::Fields(f) = &file.task.metadata else {
        return None;
    };
    let Ok(Some(parent)) = &f.parent else {
        return None;
    };
    let number = parent.split('#').next()?.parse::<u64>().ok()?;
    snapshot.file(number).is_some().then_some(number)
}

fn resolved_dependencies(snapshot: &Snapshot, file: &TaskFile) -> Vec<u64> {
    let PartialMetadata::Fields(f) = &file.task.metadata else {
        return Vec::new();
    };
    let Ok(deps) = &f.dependencies else {
        return Vec::new();
    };
    deps.iter()
        .filter_map(|dep| dep.split('#').next().and_then(|s| s.parse::<u64>().ok()))
        .filter(|number| snapshot.file(*number).is_some())
        .collect()
}

fn parent_cycles(snapshot: &Snapshot, sink: &mut Sink) {
    let edges: HashMap<u64, u64> = snapshot
        .files()
        .iter()
        .filter_map(|file| resolved_parent(snapshot, file).map(|parent| (file.number, parent)))
        .collect();
    for file in snapshot.files() {
        let mut current = edges.get(&file.number).copied();
        let mut steps = 0;
        while let Some(number) = current {
            if number == file.number {
                sink.emit(Diagnostic::error(
                    Code::ParentCycle,
                    file.path.clone(),
                    "parent links form a cycle",
                ));
                break;
            }
            current = edges.get(&number).copied();
            steps += 1;
            if steps > snapshot.files().len() {
                break;
            }
        }
    }
}

fn reaches_self(edges: &HashMap<u64, Vec<u64>>, start: u64) -> bool {
    let mut stack: Vec<u64> = edges.get(&start).cloned().unwrap_or_default();
    let mut seen = HashSet::new();
    while let Some(number) = stack.pop() {
        if number == start {
            return true;
        }
        if seen.insert(number) {
            if let Some(next) = edges.get(&number) {
                stack.extend(next.iter().copied());
            }
        }
    }
    false
}

fn dependency_cycles(snapshot: &Snapshot, sink: &mut Sink) {
    let edges: HashMap<u64, Vec<u64>> = snapshot
        .files()
        .iter()
        .map(|file| (file.number, resolved_dependencies(snapshot, file)))
        .collect();
    for file in snapshot.files() {
        if reaches_self(&edges, file.number) {
            sink.emit(Diagnostic::error(
                Code::DependencyCycle,
                file.path.clone(),
                "dependencies form a cycle",
            ));
        }
    }
}

fn unique_numbers(snapshot: &Snapshot, sink: &mut Sink) {
    let mut by_number: HashMap<u64, Vec<&TaskFile>> = HashMap::new();
    for file in snapshot.files() {
        by_number.entry(file.number).or_default().push(file);
    }
    for group in by_number.values() {
        if group.len() < 2 {
            continue;
        }
        for file in group {
            sink.emit(Diagnostic::error(
                Code::DuplicateNumber,
                file.path.clone(),
                "more than one file claims this task number",
            ));
        }
    }
}
