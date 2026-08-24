use std::ops::Range;

use crate::{FieldError, FieldResult, Timestamp};

pub const HEADING: &str = "Comments";
const LEVEL: u8 = 2;
const ENTRY: &str = "###";

// One entry of a task's comment log, as lenient as the frontmatter read path: a hand-damaged
// heading keeps the text it introduces and reports which field it lost. `agent` is an `Option`
// rather than a field that can fail, because no agent at all is the ordinary case — a person typed
// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub at: FieldResult<Timestamp>,
    pub author: FieldResult<String>,
    pub agent: Option<String>,
    pub text: String,
}

// A comment the writer knows every field of, which is every comment openplan writes: the daemon
// stamps the time, the CLI resolves the identity, and only a hand edit can produce anything less.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewComment {
    pub at: Timestamp,
    pub author: String,
    pub agent: Option<String>,
    pub text: String,
}

// A parsed entry with the byte ranges it occupies in the body, for the linter: an entry with no
// heading and one with no quote are both readable and both worth reporting, so each half is
// separately absent rather than the pair being all-or-nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub comment: Comment,
    pub heading: Option<Range<usize>>,
    pub quote: Option<Range<usize>>,
}

// Everything the log occupies, its own heading included. Nothing inside it is addressable: the log
// is append-only, so no section path and no `#anchor` may reach a heading here, and no rewrite may
// touch the text a person wrote.
pub fn section_span(body: &str) -> Option<Range<usize>> {
    let (heading, end) = section(body)?;
    Some(heading.start..end)
}

// A second `## Comments` section is a hand-made state the linter reports; every reader answers from
// the first.
fn section(body: &str) -> Option<(op_md::Heading, usize)> {
    let heading = sections(body).into_iter().next()?;
    let end = op_md::section_end(body, &heading);
    Some((heading, end))
}

// The headings a section path or an anchor may name.
pub fn addressable(body: &str) -> Vec<op_md::Heading> {
    let log = section_span(body);
    op_md::headings(body)
        .into_iter()
        .filter(|heading| !log.as_ref().is_some_and(|log| log.contains(&heading.start)))
        .collect()
}

pub fn sections(body: &str) -> Vec<op_md::Heading> {
    op_md::headings(body)
        .into_iter()
        .filter(|h| h.level == LEVEL && h.text == HEADING)
        .collect()
}

pub fn parse(body: &str) -> Vec<Comment> {
    entries(body)
        .into_iter()
        .map(|entry| entry.comment)
        .collect()
}

pub fn entries(body: &str) -> Vec<Entry> {
    let Some(region) = content(body) else {
        return Vec::new();
    };
    scan(body, region)
}

// The body with its comment log removed, which is what every reader above the daemon renders: the
// thread has its own shape on the wire, and a client that also found it in the body would show it
// twice.
pub fn strip(body: &str) -> String {
    let Some(section) = section_span(body) else {
        return body.to_owned();
    };
    let head = body[..section.start].trim_end_matches('\n');
    let tail = body[section.end..].trim_start_matches('\n');
    match (head.is_empty(), tail.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("{tail}\n"),
        (false, true) => format!("{head}\n"),
        (false, false) => format!("{head}\n\n{tail}\n"),
    }
}

pub fn markdown(comment: &NewComment) -> String {
    block(
        Some(comment.at),
        Some(&comment.author),
        comment.agent.as_deref(),
        &comment.text,
    )
}

// A log rebuilt from parsed entries, for a caller that renders the whole task file from what the
// daemon holds rather than from the file's bytes. A field that did not parse is left out rather
// than guessed at, as the frontmatter is.
pub fn with_comments(body: &str, comments: &[Comment]) -> String {
    comments.iter().fold(body.to_owned(), |body, comment| {
        op_md::append_under(
            &body,
            LEVEL,
            HEADING,
            &block(
                comment.at.as_ref().ok().copied(),
                comment.author.as_deref().ok(),
                comment.agent.as_deref(),
                &comment.text,
            ),
        )
    })
}

fn block(at: Option<Timestamp>, author: Option<&str>, agent: Option<&str>, text: &str) -> String {
    let mut heading = ENTRY.to_owned();
    if let Some(at) = at {
        heading.push_str(&format!(" {at}"));
    }
    if let Some(author) = author {
        heading.push_str(&format!(" by {author}"));
    }
    if let Some(agent) = agent {
        heading.push_str(&format!(" via {agent}"));
    }
    format!("{heading}\n\n{}", quote(text))
}

// Every content line carries the prefix, so nothing a comment holds — a heading of any level, a
// fence, a nested quote — can end the entry or reach the document outline.
fn quote(text: &str) -> String {
    text.trim_end_matches('\n')
        .split('\n')
        .map(|line| match line.is_empty() {
            true => ">".to_owned(),
            false => format!("> {line}"),
        })
        .collect::<Vec<String>>()
        .join("\n")
}

fn unquote(quoted: &str) -> String {
    quoted
        .trim_end_matches('\n')
        .split('\n')
        .map(|line| {
            let line = line.strip_prefix('>').unwrap_or(line);
            line.strip_prefix(' ').unwrap_or(line)
        })
        .collect::<Vec<&str>>()
        .join("\n")
}

fn content(body: &str) -> Option<Range<usize>> {
    let (heading, end) = section(body)?;
    Some(heading.end..end)
}

// Line by line rather than through the markdown parser: the file order is the true order, and an
// entry is delimited by an unquoted `###` line. A blank line ends the quote it follows, so a person
// who writes one inside a comment splits it in two, and the reader shows what the file says.
fn scan(body: &str, region: Range<usize>) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut heading: Option<Range<usize>> = None;
    let mut quote: Option<Range<usize>> = None;
    let mut pos = region.start;
    for line in body[region].split_inclusive('\n') {
        let span = pos..pos + line.len();
        pos = span.end;
        let text = line.trim_end_matches(['\n', '\r']);
        if is_entry(text) {
            flush(&mut out, body, heading.take(), quote.take());
            heading = Some(span);
        } else if text.starts_with('>') {
            match &mut quote {
                Some(open) => open.end = span.end,
                None => quote = Some(span),
            }
        } else if quote.is_some() {
            flush(&mut out, body, heading.take(), quote.take());
        }
    }
    flush(&mut out, body, heading, quote);
    out
}

// `###` and nothing more of the hash run, then a space or the end of the line — the same shape
// markdown itself reads as a heading, so nothing an entry heading does not name becomes one.
fn is_entry(line: &str) -> bool {
    match line.strip_prefix(ENTRY) {
        Some(rest) => rest.is_empty() || rest.starts_with(' '),
        None => false,
    }
}

fn flush(
    out: &mut Vec<Entry>,
    body: &str,
    heading: Option<Range<usize>>,
    quote: Option<Range<usize>>,
) {
    if heading.is_none() && quote.is_none() {
        return;
    }
    let (at, author, agent) = match &heading {
        Some(span) => fields(body[span.clone()].trim_end_matches(['\n', '\r'])),
        None => (Err(FieldError::Missing), Err(FieldError::Missing), None),
    };
    let text = match &quote {
        Some(span) => unquote(&body[span.clone()]),
        None => String::new(),
    };
    out.push(Entry {
        comment: Comment {
            at,
            author,
            agent,
            text,
        },
        heading,
        quote,
    });
}

// `### <timestamp> by <author>[ via <agent>]`, split on the last ` via ` so an author or a
// timestamp holding those three letters cannot be read as the tool that typed the entry.
fn fields(line: &str) -> (FieldResult<Timestamp>, FieldResult<String>, Option<String>) {
    let rest = line.trim_start_matches('#').trim();
    let (head, agent) = match rest.rsplit_once(" via ") {
        Some((head, agent)) if !agent.trim().is_empty() => (head, Some(agent.trim().to_owned())),
        _ => (rest, None),
    };
    let (at, author) = match head.split_once(" by ") {
        Some((at, author)) => (at.trim(), author.trim()),
        None => (head.trim(), ""),
    };
    (parse_at(at), text_field(author), agent)
}

// One spelling, the one `Timestamp`'s own rendering produces: whole seconds in UTC. A form that
// names the same instant differently — a sub-second tail, a numeric offset — is reported rather
// than accepted, so every heading in a log reads the same way.
fn parse_at(text: &str) -> FieldResult<Timestamp> {
    if text.is_empty() {
        return Err(FieldError::Missing);
    }
    match text.parse::<Timestamp>() {
        Ok(at) if at.subsec_nanosecond() == 0 && at.to_string() == text => Ok(at),
        _ => Err(FieldError::Invalid(format!(
            "not an RFC3339 UTC timestamp: {text:?}"
        ))),
    }
}

fn text_field(text: &str) -> FieldResult<String> {
    match text.is_empty() {
        true => Err(FieldError::Missing),
        false => Ok(text.to_owned()),
    }
}
