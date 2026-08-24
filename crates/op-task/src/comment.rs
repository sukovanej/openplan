use std::ops::Range;

use crate::{FieldError, FieldResult, Timestamp};

pub const HEADING: &str = "Comments";
const LEVEL: u8 = 2;
const ENTRY: &str = "###";
const BY: &str = "by ";
const VIA: &str = " via ";

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

// The log as the file holds it. `stray` is every line the format has no place for — a log holds
// entry headings and blockquotes and nothing else — so a reader can report what it cannot carry
// rather than drop it without a word.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Log {
    pub entries: Vec<Entry>,
    pub stray: Vec<Range<usize>>,
}

// One `## Comments` section: the heading line, and everything the section occupies. Nothing inside
// `span` is addressable — the log is append-only, so no section path and no `#anchor` may reach a
// heading here, and no rewrite may touch the text a person wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub heading: Range<usize>,
    pub span: Range<usize>,
}

// More than one section is a hand-made state the linter reports. Every reader answers from all of
// them, in file order, rather than lose the entries in the rest.
pub fn sections(body: &str) -> Vec<Section> {
    with_ends(&op_md::headings(body), body.len())
        .map(|(heading, end)| Section {
            heading: heading.start..heading.end,
            span: heading.start..end,
        })
        .collect()
}

pub fn inside_log(sections: &[Section], offset: usize) -> bool {
    sections
        .iter()
        .any(|section| section.span.contains(&offset))
}

// The headings a section path or an anchor may name.
pub fn addressable(body: &str) -> Vec<op_md::Heading> {
    let headings = op_md::headings(body);
    let sections: Vec<Section> = with_ends(&headings, body.len())
        .map(|(heading, end)| Section {
            heading: heading.start..heading.end,
            span: heading.start..end,
        })
        .collect();
    headings
        .into_iter()
        .filter(|heading| !inside_log(&sections, heading.start))
        .collect()
}

pub fn read(body: &str) -> Log {
    let mut log = Log::default();
    for (heading, end) in with_ends(&op_md::headings(body), body.len()) {
        scan(body, heading.end..end, &mut log);
    }
    log
}

pub fn parse(body: &str) -> Vec<Comment> {
    read(body)
        .entries
        .into_iter()
        .map(|entry| entry.comment)
        .collect()
}

// The body with its comment log removed, which is what every reader above the daemon renders: the
// thread has its own shape on the wire, and a client that also found it in the body would show it
// twice.
pub fn strip(body: &str) -> String {
    let sections = sections(body);
    if sections.is_empty() {
        return body.to_owned();
    }
    let mut kept = Vec::new();
    let mut pos = 0;
    for section in &sections {
        kept.push(&body[pos..section.span.start]);
        pos = section.span.end;
    }
    kept.push(&body[pos..]);
    op_md::paragraphs(kept)
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
// than guessed at, as the frontmatter is; what is left still parses back to the same fields.
pub fn with_comments(body: &str, comments: &[Comment]) -> String {
    if comments.is_empty() {
        return body.to_owned();
    }
    let log: Vec<String> = comments
        .iter()
        .map(|comment| {
            block(
                comment.at.as_ref().ok().copied(),
                comment.author.as_deref().ok(),
                comment.agent.as_deref(),
                &comment.text,
            )
        })
        .collect();
    op_md::append_under(body, LEVEL, HEADING, &log.join("\n\n"))
}

fn block(at: Option<Timestamp>, author: Option<&str>, agent: Option<&str>, text: &str) -> String {
    let mut heading = ENTRY.to_owned();
    if let Some(at) = at {
        heading.push_str(&format!(" {at}"));
    }
    if let Some(author) = author {
        heading.push_str(&format!(" {BY}{}", one_line(author)));
    }
    if let Some(agent) = agent {
        heading.push_str(&format!("{VIA}{}", one_line(agent)));
    }
    format!("{heading}\n\n{}", quote(text))
}

// An entry heading is one line. A field holding a newline would close it and let everything below
// stand as entries of its own, so the log would carry writing no one signed.
fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
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
        .lines()
        .map(|line| {
            let line = line.strip_prefix('>').unwrap_or(line);
            line.strip_prefix(' ').unwrap_or(line)
        })
        .collect::<Vec<&str>>()
        .join("\n")
}

// Each `## Comments` heading with the offset its section ends at, read from one pass over the
// outline: the section runs to the next heading that outranks or matches it.
fn with_ends(
    headings: &[op_md::Heading],
    len: usize,
) -> impl Iterator<Item = (&op_md::Heading, usize)> {
    headings
        .iter()
        .enumerate()
        .filter(|(_, heading)| heading.level == LEVEL && heading.text == HEADING)
        .map(move |(index, heading)| {
            let end = headings[index + 1..]
                .iter()
                .find(|next| next.level <= LEVEL)
                .map_or(len, |next| next.start);
            (heading, end)
        })
}

enum Line {
    Entry,
    Quoted,
    Blank,
    Stray,
}

// `###` and nothing more of the hash run, then a space or the end of the line — the same shape
// markdown itself reads as a heading, so nothing an entry heading does not name becomes one.
fn classify(text: &str) -> Line {
    match text.strip_prefix(ENTRY) {
        Some(rest) if rest.is_empty() || rest.starts_with(' ') => Line::Entry,
        _ if text.starts_with('>') => Line::Quoted,
        _ if text.trim().is_empty() => Line::Blank,
        _ => Line::Stray,
    }
}

// Line by line rather than through the markdown parser: the file order is the true order, and an
// entry is delimited by an unquoted `###` line. A blank line ends the quote it follows, so a person
// who writes one inside a comment splits it in two, and the reader shows what the file says.
fn scan(body: &str, region: Range<usize>, log: &mut Log) {
    let mut heading: Option<Range<usize>> = None;
    let mut quote: Option<Range<usize>> = None;
    let mut pos = region.start;
    for line in body[region].split_inclusive('\n') {
        let span = pos..pos + line.len();
        pos = span.end;
        match classify(line.trim_end_matches(['\n', '\r'])) {
            Line::Entry => {
                flush(log, body, heading.take(), quote.take());
                heading = Some(span);
            }
            Line::Quoted => match &mut quote {
                Some(open) => open.end = span.end,
                None => quote = Some(span),
            },
            Line::Blank | Line::Stray if quote.is_some() => {
                flush(log, body, heading.take(), quote.take());
                if let Line::Stray = classify(line.trim_end_matches(['\n', '\r'])) {
                    log.stray.push(span);
                }
            }
            Line::Stray => log.stray.push(span),
            Line::Blank => {}
        }
    }
    flush(log, body, heading, quote);
}

fn flush(log: &mut Log, body: &str, heading: Option<Range<usize>>, quote: Option<Range<usize>>) {
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
    log.entries.push(Entry {
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
// timestamp holding those three letters cannot be read as the tool that typed the entry. A heading
// that opens on `by ` names no timestamp, which is how a rendering of a damaged entry reads back as
// the same fields it was rendered from.
fn fields(line: &str) -> (FieldResult<Timestamp>, FieldResult<String>, Option<String>) {
    let rest = line.trim_start_matches('#').trim();
    let (head, agent) = match rest.rsplit_once(VIA) {
        Some((head, agent)) if !agent.trim().is_empty() => {
            (head.trim(), Some(agent.trim().to_owned()))
        }
        _ => (rest, None),
    };
    let (at, author) = match head.strip_prefix(BY) {
        Some(author) => ("", author.trim()),
        None => match head.split_once(&format!(" {BY}")) {
            Some((at, author)) => (at.trim(), author.trim()),
            None => (head, ""),
        },
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
