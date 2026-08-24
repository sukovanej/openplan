use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use op_task::{
    Abbreviation, FieldError, PartialMetadata, Timestamp, body_ref_id, ref_id, task_ref,
};

use crate::snapshot::{Snapshot, TaskFile};

// Where `created:` backfill draws its answer: the author time of the first commit that added a path.
// An uncommitted file has no answer and stays unfixable.
pub trait CreatedSource {
    fn created(&self, path: &Path) -> Option<Timestamp>;
}

pub struct Uncommitted;

impl CreatedSource for Uncommitted {
    fn created(&self, _path: &Path) -> Option<Timestamp> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    pub range: Range<usize>,
    pub replacement: String,
}

pub fn file_fixes(snapshot: &Snapshot, file: &TaskFile, created: &dyn CreatedSource) -> Vec<Fix> {
    let body_offset = file.source.len() - file.task.body.len();
    let mut fixes = Vec::new();
    if let PartialMetadata::Fields(fields) = &file.task.metadata {
        if fields.created == Err(FieldError::Missing) {
            if let Some(fix) = created_backfill(file, body_offset, created) {
                fixes.push(fix);
            }
        }
        fixes.extend(frontmatter_ref_fixes(snapshot, &file.source[..body_offset]));
    }
    fixes.extend(body_ref_fixes(snapshot, &file.task.body, body_offset));
    fixes
}

pub fn apply(source: &str, fixes: &[Fix]) -> String {
    let mut sorted: Vec<&Fix> = fixes.iter().collect();
    sorted.sort_by_key(|fix| fix.range.start);
    let mut out = String::with_capacity(source.len());
    let mut pos = 0;
    for fix in sorted {
        if fix.range.start < pos {
            continue;
        }
        out.push_str(&source[pos..fix.range.start]);
        out.push_str(&fix.replacement);
        pos = fix.range.end;
    }
    out.push_str(&source[pos..]);
    out
}

// Every file's content after fixes are spliced to a fixpoint — unchanged files carry their original
// bytes, so a caller diffs against the store to decide what to write.
pub fn fix(snapshot: &Snapshot, created: &dyn CreatedSource) -> BTreeMap<PathBuf, String> {
    let root = snapshot.root().to_path_buf();
    let abbreviation = snapshot.abbreviation();
    let mut current: Vec<(PathBuf, String)> = snapshot
        .files()
        .iter()
        .map(|file| (file.path.clone(), file.source.clone()))
        .collect();
    loop {
        let snap = Snapshot::from_files(root.clone(), abbreviation, current.clone());
        let mut next = Vec::with_capacity(snap.files().len());
        let mut changed = false;
        for file in snap.files() {
            let after = apply(&file.source, &file_fixes(&snap, file, created));
            if after != file.source {
                changed = true;
            }
            next.push((file.path.clone(), after));
        }
        current = next;
        if !changed {
            break;
        }
    }
    current.into_iter().collect()
}

fn created_backfill(
    file: &TaskFile,
    body_offset: usize,
    created: &dyn CreatedSource,
) -> Option<Fix> {
    let timestamp = created.created(&file.path)?;
    let frontmatter = &file.source[..body_offset];
    let mut insert_at = if frontmatter.starts_with("---\r\n") {
        5
    } else if frontmatter.starts_with("---\n") {
        4
    } else {
        0
    };
    let mut offset = 0;
    for line in frontmatter.split_inclusive('\n') {
        if line_content(line).strip_prefix("status:").is_some() {
            insert_at = offset + line.len();
            break;
        }
        offset += line.len();
    }
    Some(Fix {
        range: insert_at..insert_at,
        replacement: format!("created: {timestamp}\n"),
    })
}

fn frontmatter_ref_fixes(snapshot: &Snapshot, frontmatter: &str) -> Vec<Fix> {
    let mut lines = Vec::new();
    let mut offset = 0;
    for line in frontmatter.split_inclusive('\n') {
        lines.push((offset, line));
        offset += line.len();
    }

    let mut fixes = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let (start, line) = lines[i];
        let content = line_content(line);
        if let Some(after) = content.strip_prefix("parent:") {
            fixes.extend(scalar_ref_fix(snapshot, start + "parent:".len(), after));
        } else if let Some(rest) = content.strip_prefix("dependencies:") {
            let base = start + "dependencies:".len();
            if rest.trim_start().starts_with('[') {
                fixes.extend(flow_sequence_fixes(snapshot, base, rest));
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < lines.len() {
                let (item_start, item_line) = lines[j];
                let Some((dash, after_dash)) = sequence_item(line_content(item_line)) else {
                    break;
                };
                fixes.extend(scalar_ref_fix(snapshot, item_start + dash + 1, after_dash));
                j += 1;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    fixes
}

// serde_yaml writes block sequence items at the key's own indentation, so a zero-indent `- ` is the
// spelling the store itself produces; the space after the dash is what keeps `---` from parsing as
// one.
fn sequence_item(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let after_dash = trimmed.strip_prefix('-')?;
    if !after_dash.is_empty() && !after_dash.starts_with([' ', '\t']) {
        return None;
    }
    Some((line.len() - trimmed.len(), after_dash))
}

fn flow_sequence_fixes(snapshot: &Snapshot, base: usize, rest: &str) -> Vec<Fix> {
    let (Some(open), Some(close)) = (rest.find('['), rest.rfind(']')) else {
        return Vec::new();
    };
    let Some(inner) = rest.get(open + 1..close) else {
        return Vec::new();
    };
    let mut fixes = Vec::new();
    let mut offset = open + 1;
    for item in inner.split(',') {
        fixes.extend(scalar_ref_fix(snapshot, base + offset, item));
        offset += item.len() + 1;
    }
    fixes
}

fn scalar_ref_fix(snapshot: &Snapshot, base: usize, after: &str) -> Option<Fix> {
    let raw = after.trim();
    if raw.is_empty() {
        return None;
    }
    let written = unquote(raw);
    if written.contains('#') {
        return None;
    }
    let number = ref_id(written)?;
    let target = snapshot.file(number)?;
    let canonical = task_ref(&file_name(target));
    if raw == canonical {
        return None;
    }
    let start = base + (after.len() - after.trim_start().len());
    Some(Fix {
        range: start..start + raw.len(),
        replacement: canonical,
    })
}

fn body_ref_fixes(snapshot: &Snapshot, body: &str, body_offset: usize) -> Vec<Fix> {
    let abbreviation = snapshot.abbreviation();
    let mut fixes = Vec::new();
    for (span, inner) in crate::rules::outside_the_log(body) {
        let Some(canonical) = canonical_body_ref(snapshot, abbreviation, inner) else {
            continue;
        };
        if canonical != inner {
            fixes.push(Fix {
                range: body_offset + span.start..body_offset + span.end,
                replacement: format!("[[{canonical}]]"),
            });
        }
    }
    fixes
}

fn canonical_body_ref(
    snapshot: &Snapshot,
    abbreviation: Abbreviation,
    inner: &str,
) -> Option<String> {
    let number = body_ref_id(abbreviation, inner)?;
    let target = snapshot.file(number)?;
    let canonical = task_ref(&file_name(target));
    Some(match inner.split_once('#') {
        Some((_, section)) => format!("{canonical}#{section}"),
        None => canonical,
    })
}

fn file_name(file: &TaskFile) -> String {
    file.path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn line_content(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let quote = bytes[0];
        if (quote == b'"' || quote == b'\'') && bytes[bytes.len() - 1] == quote {
            return &value[1..value.len() - 1];
        }
    }
    value
}
