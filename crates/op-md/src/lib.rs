use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub segments: Vec<String>,
}

impl Target {
    pub fn parse(path: &str) -> Self {
        Self {
            segments: path.split('.').map(str::to_owned).collect(),
        }
    }
}

// A heading inside a blockquote belongs to the quoted text, not to the document that quotes it, so
// it names no section and heads none: quoting a document must not splice its outline into this one.
pub fn headings(body: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut pending: Option<(u8, usize)> = None;
    let mut text = String::new();
    let mut quoted = 0usize;

    for (event, range) in Parser::new(body).into_offset_iter() {
        match event {
            Event::Start(Tag::BlockQuote(_)) => quoted += 1,
            Event::End(TagEnd::BlockQuote(_)) => quoted = quoted.saturating_sub(1),
            Event::Start(Tag::Heading { level, .. }) if quoted == 0 => {
                pending = Some((heading_level(level), range.start));
                text.clear();
            }
            Event::Text(t) | Event::Code(t) if pending.is_some() => text.push_str(&t),
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, start)) = pending.take() {
                    out.push(Heading {
                        level,
                        text: text.trim().to_owned(),
                        start,
                        end: range.end,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

pub fn heading(body: &str, level: u8, title: &str) -> Option<Heading> {
    headings(body)
        .into_iter()
        .find(|h| h.level == level && h.text == title)
}

// Where the section a heading owns ends: at the next heading that outranks or matches it, else at
// the end of the body.
pub fn section_end(body: &str, heading: &Heading) -> usize {
    headings(body)
        .into_iter()
        .find(|h| h.start > heading.start && h.level <= heading.level)
        .map_or(body.len(), |next| next.start)
}

// `block` placed at the end of the named section, which is created at the end of the body when it
// is absent. One blank line separates the block from what it follows and from what follows it.
pub fn append_under(body: &str, level: u8, title: &str, block: &str) -> String {
    let block = block.trim_matches('\n');
    let Some(found) = heading(body, level, title) else {
        let heading = format!("{} {title}", "#".repeat(level as usize));
        return paragraphs([body, &heading, block]);
    };
    let end = section_end(body, &found);
    paragraphs([&body[..end], block, &body[end..]])
}

fn paragraphs<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut out = String::new();
    for part in parts {
        let part = part.trim_matches('\n');
        if part.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(part);
        out.push('\n');
    }
    out
}

// The byte ranges where markdown reads text as something other than prose: inline code, fenced and
// indented blocks, and existing link syntax. A `[[…]]` inside one is quoted source or already a
// link, so a scanner looking for references must skip it rather than rewrite what it finds.
pub fn opaque_ranges(body: &str) -> Vec<std::ops::Range<usize>> {
    Parser::new(body)
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Code(_) | Event::Start(Tag::CodeBlock(_) | Tag::Link { .. }) => Some(range),
            _ => None,
        })
        .collect()
}

pub fn title(body: &str) -> Option<String> {
    headings(body)
        .into_iter()
        .find(|h| h.level == 1)
        .map(|h| h.text)
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
