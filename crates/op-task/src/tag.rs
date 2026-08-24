use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{split_frontmatter, with_paragraph};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    Slate,
    Red,
    Orange,
    Amber,
    Yellow,
    Green,
    Teal,
    Cyan,
    Blue,
    Indigo,
    Violet,
    Pink,
}

impl Color {
    pub const ALL: [Color; 12] = [
        Color::Slate,
        Color::Red,
        Color::Orange,
        Color::Amber,
        Color::Yellow,
        Color::Green,
        Color::Teal,
        Color::Cyan,
        Color::Blue,
        Color::Indigo,
        Color::Violet,
        Color::Pink,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Color::Slate => "slate",
            Color::Red => "red",
            Color::Orange => "orange",
            Color::Amber => "amber",
            Color::Yellow => "yellow",
            Color::Green => "green",
            Color::Teal => "teal",
            Color::Cyan => "cyan",
            Color::Blue => "blue",
            Color::Indigo => "indigo",
            Color::Violet => "violet",
            Color::Pink => "pink",
        }
    }

    pub fn for_name(name: &str) -> Color {
        Color::ALL[fnv1a(name) as usize % Color::ALL.len()]
    }
}

// A tag file written today must keep its color when read by a binary built years from now, so the
// derivation spells its own hash out. `DefaultHasher` is explicitly free to change between Rust
// releases, which would recolor every tag that omits `color:`.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a(text: &str) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[derive(Debug, thiserror::Error)]
#[error("invalid color {got:?}; expected one of {}", Color::ALL.map(|c| c.as_str()).join(", "))]
pub struct ParseColorError {
    got: String,
}

impl std::str::FromStr for Color {
    type Err = ParseColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Color::ALL
            .into_iter()
            .find(|color| color.as_str() == s)
            .ok_or_else(|| ParseColorError { got: s.to_owned() })
    }
}

pub const NAME_RULE: &str = "a tag name is lowercase letters, digits, and hyphens, and starts with a letter or a digit; spaces and underscores become hyphens";

#[derive(Debug, thiserror::Error)]
#[error("invalid tag name {got:?}; {NAME_RULE}")]
pub struct ParseNameError {
    got: String,
}

// Unlike `slug`, which drops whatever it cannot spell, this refuses it: a tag name is an identity a
// human typed, so `C++` must come back as an error rather than as a silently different tag.
pub fn normalize_name(name: &str) -> Result<String, ParseNameError> {
    let mut normalized = String::with_capacity(name.len());
    for ch in name.trim().chars() {
        let ch = if ch.is_whitespace() || ch == '_' {
            '-'
        } else {
            ch.to_ascii_lowercase()
        };
        if ch == '-' && normalized.ends_with('-') {
            continue;
        }
        normalized.push(ch);
    }
    is_normalized(&normalized)
        .then_some(normalized)
        .ok_or_else(|| ParseNameError {
            got: name.to_owned(),
        })
}

fn is_normalized(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagFrontmatter {
    // `None` is a file that omits `color:`. `Tag::color` derives one from the name so a hand-written
    // tag still reads, and a rewrite leaves the omission alone rather than materializing it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(flatten)]
    pub extra: serde_yaml::Mapping,
}

// The name is the file's identity — its filename — and is not part of its content, so the caller
// that read the directory supplies it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    pub name: String,
    pub frontmatter: TagFrontmatter,
    pub body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("invalid frontmatter: {0}")]
    Frontmatter(#[from] serde_yaml::Error),
    #[error("missing frontmatter fence")]
    MissingFrontmatter,
}

impl Tag {
    pub fn new(display_name: &str, color: Option<Color>) -> Result<Self, ParseNameError> {
        let name = normalize_name(display_name)?;
        let color = color.unwrap_or_else(|| Color::for_name(&name));
        Ok(Self {
            name,
            frontmatter: TagFrontmatter {
                color: Some(color),
                extra: serde_yaml::Mapping::new(),
            },
            body: format!("# {}\n", one_line(display_name)),
        })
    }

    // The H1 goes with the name: a tag renamed to `infra` whose heading still reads `# Backend`
    // would render under the name it no longer has.
    pub fn rename(&mut self, display_name: &str) -> Result<(), ParseNameError> {
        self.name = normalize_name(display_name)?;
        self.body = retitle(&self.body, &one_line(display_name));
        Ok(())
    }

    pub fn color(&self) -> Color {
        self.frontmatter
            .color
            .unwrap_or_else(|| Color::for_name(&self.name))
    }

    pub fn set_color(&mut self, color: Color) {
        self.frontmatter.color = Some(color);
    }

    pub fn display_name(&self) -> Option<String> {
        op_md::title(&self.body)
    }

    pub fn description(&self) -> String {
        match h1(&self.body) {
            Some(h1) => self.body[h1.end..].trim().to_owned(),
            None => self.body.trim().to_owned(),
        }
    }

    pub fn set_description(&mut self, description: &str) {
        let head = match h1(&self.body) {
            Some(h1) => &self.body[..h1.end],
            None => "",
        };
        self.body = with_paragraph(head, description);
    }

    pub fn append_body(&mut self, content: &str) {
        self.body = with_paragraph(&self.body, content);
    }

    pub fn to_file_string(&self) -> Result<String, TagError> {
        let fm = serde_yaml::to_string(&self.frontmatter)?;
        Ok(format!("---\n{fm}---\n{}", self.body))
    }

    pub fn from_file_string(name: String, input: &str) -> Result<Self, TagError> {
        let (fm_src, body) = split_frontmatter(input).ok_or(TagError::MissingFrontmatter)?;
        Ok(Self {
            name,
            frontmatter: serde_yaml::from_str(&fm_src.replace('\r', ""))?,
            body: body.to_owned(),
        })
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn retitle(body: &str, display_name: &str) -> String {
    let heading = format!("# {display_name}\n");
    match h1(body) {
        Some(h1) => format!("{}{heading}{}", &body[..h1.start], &body[h1.end..]),
        None => format!("{heading}{body}"),
    }
}

fn h1(body: &str) -> Option<op_md::Heading> {
    op_md::headings(body).into_iter().find(|h| h.level == 1)
}
