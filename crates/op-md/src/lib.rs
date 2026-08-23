use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};

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

pub fn headings(body: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut pending: Option<(u8, usize)> = None;
    let mut text = String::new();

    for (event, range) in Parser::new(body).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                pending = Some((heading_level(level), range.start));
                text.clear();
            }
            Event::Text(t) | Event::Code(t) if pending.is_some() => text.push_str(&t),
            Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
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
