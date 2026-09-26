use std::fmt::{self, Write as _};

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, header};
use axum::response::IntoResponse;
use gix_imara_diff::{
    Algorithm, Diff, InternedInput, Interner, Token, UnifiedDiffConfig, UnifiedDiffPrinter,
};
use op_api::{ApiErrorBody, DocumentDiff};
use op_backend::RevisionId;
use op_tracker::Versions;
use serde::Deserialize;

use crate::{ApiError, AppState, blocking, project_of};

// A popover shows the diff, and a file that the revision rewrote whole would fill it many times over.
const MAX_LINES: usize = 400;

// A revision never changes, so neither does the diff of one of its documents.
const IMMUTABLE_PRIVATE: HeaderValue =
    HeaderValue::from_static("private, max-age=31536000, immutable");

#[derive(Deserialize, utoipa::IntoParams)]
pub(crate) struct DiffQuery {
    // The document's path, as the history names it.
    path: String,
    // The document's path on the parent's side, where a new title moved a task to a new file.
    from: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/revisions/{revision}/diff",
    params(
        ("project" = String, Path, description = "Project name"),
        ("revision" = String, Path, description = "Revision id, as the history names it"),
        DiffQuery
    ),
    responses(
        (status = 200, description = "The document across the revision, against its first parent", body = DocumentDiff),
        (status = 404, description = "No such project or revision, or the revision does not change the document", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn revision_diff(
    State(state): State<AppState>,
    Path((project, revision)): Path<(String, String)>,
    Query(query): Query<DiffQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let project = project_of(&state, &project)?;
    let diff = blocking(move || {
        let before = query.from.as_deref().unwrap_or(&query.path);
        let versions = project
            .tracker()
            .versions(&RevisionId::new(revision), before, &query.path)?
            .ok_or_else(|| {
                ApiError::not_found(format!("the revision does not change {}", query.path))
            })?;
        Ok(document_diff(versions, before, &query.path))
    })
    .await?;
    Ok(([(header::CACHE_CONTROL, IMMUTABLE_PRIVATE)], Json(diff)))
}

fn document_diff(versions: Versions, before: &str, after: &str) -> DocumentDiff {
    let (Some(old), Some(new)) = (text(&versions.before), text(&versions.after)) else {
        return DocumentDiff::Binary;
    };
    let mut out = Capped::default();
    let side = |held: &Option<Vec<u8>>, prefix: &str, path: &str| match held {
        Some(_) => format!("{prefix}{path}"),
        None => "/dev/null".to_owned(),
    };
    let input = InternedInput::new(old, new);
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    // `Capped` refuses what comes after its last line, and that refusal is the only error here.
    let written = write!(
        out,
        "--- {}\n+++ {}\n{}",
        side(&versions.before, "a/", before),
        side(&versions.after, "b/", after),
        diff.unified_diff(
            &Lines(&input.interner),
            UnifiedDiffConfig::default(),
            &input
        )
    );
    DocumentDiff::Text {
        diff: out.text,
        truncated: written.is_err(),
    }
}

// A missing side reads as an empty file. A NUL byte is how git tells a binary file too.
fn text(side: &Option<Vec<u8>>) -> Option<&str> {
    match side {
        None => Some(""),
        Some(bytes) => std::str::from_utf8(bytes)
            .ok()
            .filter(|text| !text.contains('\0')),
    }
}

#[derive(Default)]
struct Capped {
    text: String,
    lines: usize,
}

impl fmt::Write for Capped {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for line in s.split_inclusive('\n') {
            if self.lines == MAX_LINES {
                return Err(fmt::Error);
            }
            self.text.push_str(line);
            self.lines += usize::from(line.ends_with('\n'));
        }
        Ok(())
    }
}

struct Lines<'a>(&'a Interner<&'a str>);

impl UnifiedDiffPrinter for Lines<'_> {
    fn display_header(
        &self,
        mut f: impl fmt::Write,
        start_before: u32,
        start_after: u32,
        len_before: u32,
        len_after: u32,
    ) -> fmt::Result {
        // `diff -u` numbers an empty side from the line before it: `-0,0` for a new file.
        let start = |start: u32, len: u32| if len == 0 { start } else { start + 1 };
        writeln!(
            f,
            "@@ -{},{len_before} +{},{len_after} @@",
            start(start_before, len_before),
            start(start_after, len_after),
        )
    }

    fn display_context_token(&self, f: impl fmt::Write, token: Token) -> fmt::Result {
        line(f, ' ', self.0[token])
    }

    fn display_hunk(
        &self,
        mut f: impl fmt::Write,
        before: &[Token],
        after: &[Token],
    ) -> fmt::Result {
        for &token in before {
            line(&mut f, '-', self.0[token])?;
        }
        for &token in after {
            line(&mut f, '+', self.0[token])?;
        }
        Ok(())
    }
}

fn line(mut f: impl fmt::Write, sign: char, text: &str) -> fmt::Result {
    match text.strip_suffix('\n') {
        Some(text) => writeln!(f, "{sign}{text}"),
        None => writeln!(f, "{sign}{text}\n\\ No newline at end of file"),
    }
}
