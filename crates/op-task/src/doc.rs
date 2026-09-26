use serde::{Deserialize, Serialize};

use crate::layout::{self, Document};
use crate::name::{h1, one_line};
use crate::{FieldConflict, FieldError, FieldResult, Timestamp, conflict, split_frontmatter};

pub const NAME_RULE: &str = crate::name::RULE;

#[derive(Debug, thiserror::Error)]
#[error("invalid doc name {got:?}; a doc name is {NAME_RULE}")]
pub struct ParseNameError {
    got: String,
}

pub fn normalize_name(name: &str) -> Result<String, ParseNameError> {
    crate::name::normalize(name).ok_or_else(|| ParseNameError {
        got: name.to_owned(),
    })
}

// `parent` is another doc's name in memory. The file names the parent by its path, as every file
// names another, so `to_file_string` and `from_file_string` translate it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocFrontmatter {
    pub created: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(flatten)]
    pub extra: serde_yaml::Mapping,
}

// The name is the file's identity — its stem — and is not part of its content, so the caller that
// read the directory supplies it.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    pub name: String,
    pub frontmatter: DocFrontmatter,
    // Fields that two sides of a sync changed differently, kept as a task keeps them: `frontmatter`
    // holds the published value until someone sets the field.
    pub conflicts: Vec<FieldConflict>,
    pub body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("invalid frontmatter: {0}")]
    Frontmatter(#[from] serde_yaml::Error),
    #[error("missing frontmatter fence")]
    MissingFrontmatter,
    #[error("no `created:` field")]
    MissingCreated,
    #[error("a conflict in the frontmatter does not parse: {0}")]
    Conflict(String),
    #[error("invalid `parent:` {0:?}; {PARENT_EXPECTED}")]
    Parent(String),
}

pub const PARENT_EXPECTED: &str = "expected a doc file, like ./architecture.md";

// The path a doc's frontmatter names its parent by.
pub fn parent_path(name: &str) -> String {
    crate::reference::relative(layout::DOCS, &layout::doc_path(name))
}

// The doc a `parent:` path names. A bare name is not a path, so it names no doc.
pub fn parent_name(path: &str) -> Option<String> {
    match Document::of(&crate::reference::resolve(layout::DOCS, path)?) {
        Document::Doc(name) if !path.contains('#') => Some(name),
        _ => None,
    }
}

fn parent_conflicts(
    conflicts: &[FieldConflict],
    translate: impl Fn(&str) -> Option<String>,
) -> Vec<FieldConflict> {
    conflicts
        .iter()
        .cloned()
        .map(|mut conflict| {
            if conflict.field == "parent"
                && let Some(serde_yaml::Value::String(value)) = &conflict.other
                && let Some(translated) = translate(value)
            {
                conflict.other = Some(translated.into());
            }
            conflict
        })
        .collect()
}

impl Doc {
    pub fn new(display_name: &str, created: Timestamp) -> Result<Self, ParseNameError> {
        Ok(Self {
            name: normalize_name(display_name)?,
            frontmatter: DocFrontmatter {
                created,
                parent: None,
                extra: serde_yaml::Mapping::new(),
            },
            conflicts: Vec::new(),
            body: format!("# {}\n", one_line(display_name)),
        })
    }

    // The H1 goes with the name: a doc renamed to `architecture` whose heading still reads
    // `# Overview` would render under the name it no longer has.
    pub fn rename(&mut self, display_name: &str) -> Result<(), ParseNameError> {
        self.name = normalize_name(display_name)?;
        let heading = format!("# {}\n", one_line(display_name));
        self.body = match title_heading(&self.body) {
            Some(h1) => format!(
                "{}{heading}{}",
                &self.body[..h1.start],
                &self.body[h1.end..]
            ),
            None => format!("{heading}{}", self.body),
        };
        Ok(())
    }

    pub fn set_parent(&mut self, parent: Option<&str>) -> Result<(), ParseNameError> {
        self.frontmatter.parent = parent.map(normalize_name).transpose()?;
        self.conflicts.retain(|conflict| conflict.field != "parent");
        Ok(())
    }

    pub fn title(&self) -> Option<String> {
        title_of(&self.body)
    }

    pub fn conflict_count(&self) -> usize {
        self.conflicts.len() + conflict::blocks(&self.body).len()
    }

    pub fn content(&self) -> String {
        content(&self.body)
    }

    pub fn set_content(&mut self, content: &str) {
        let head = match title_heading(&self.body) {
            Some(h1) => self.body[..h1.end].trim_end_matches('\n'),
            None => "",
        };
        let content = content.trim_matches('\n');
        self.body = match (head.is_empty(), content.is_empty()) {
            (true, true) => String::new(),
            (true, false) => format!("{content}\n"),
            (false, true) => format!("{head}\n"),
            (false, false) => format!("{head}\n\n{content}\n"),
        };
    }

    pub fn to_file_string(&self) -> Result<String, DocError> {
        let mut frontmatter = self.frontmatter.clone();
        frontmatter.parent = frontmatter.parent.as_deref().map(parent_path);
        let fm = serde_yaml::to_string(&frontmatter)?;
        let conflicts = parent_conflicts(&self.conflicts, |name| Some(parent_path(name)));
        let fm = crate::with_field_conflicts(&fm, &conflicts)?;
        Ok(format!("---\n{fm}---\n{}", self.body))
    }

    pub fn from_file_string(name: String, input: &str) -> Result<Self, DocError> {
        let (fm_src, body) = split_frontmatter(input).ok_or(DocError::MissingFrontmatter)?;
        let with_blocks = fm_src.replace('\r', "");
        let fm_src = conflict::published(&with_blocks);
        match serde_yaml::from_str::<DocFrontmatter>(&fm_src) {
            Ok(mut frontmatter) => {
                frontmatter.parent = frontmatter
                    .parent
                    .map(|path| parent_name(&path).ok_or(DocError::Parent(path)))
                    .transpose()?;
                let conflicts = crate::field_conflicts(&with_blocks).map_err(DocError::Conflict)?;
                Ok(Self {
                    name,
                    frontmatter,
                    conflicts: parent_conflicts(&conflicts, parent_name),
                    body: body.to_owned(),
                })
            }
            Err(err) => Err(match serde_yaml::from_str::<serde_yaml::Mapping>(&fm_src) {
                Ok(map) if !map.contains_key("created") => DocError::MissingCreated,
                _ => DocError::Frontmatter(err),
            }),
        }
    }
}

// The markdown below the title heading. The title is the `# ` heading that opens the body: one
// further down is a section, and the text above it belongs to the body. A conflict block that opens
// the body holds each version's title, so the body stays whole and a resolve still finds the block
// as the file has it.
pub fn content(body: &str) -> String {
    match title_heading(body) {
        Some(h1) => body[h1.end..].trim_start_matches('\n').to_owned(),
        None => body.to_owned(),
    }
}

// The body that `title` and `content` make, in the layout of `current`: an unchanged title keeps its
// heading line, and the content keeps the blank lines under the heading. So a text that changes
// nothing makes `current` again, and a merge sees only what the writer changed.
pub fn joined(current: &str, title: &str, content: &str) -> String {
    let heading = title_heading(current);
    let head = match &heading {
        Some(h1) if h1.text == title => current[..h1.end].to_owned(),
        Some(h1) => format!("{}# {}", &current[..h1.start], one_line(title)),
        None if title.trim().is_empty() => String::new(),
        None => format!("# {}", one_line(title)),
    };
    let head = head.trim_end_matches('\n');
    let content = content.trim_start_matches('\n');
    let gap = match &heading {
        Some(h1) if !self::content(current).is_empty() => {
            let rest = &current[h1.end..];
            let newlines = rest.len() - rest.trim_start_matches('\n').len();
            let trimmed = current[..h1.end].len() - current[..h1.end].trim_end_matches('\n').len();
            "\n".repeat(trimmed + newlines)
        }
        _ => "\n\n".to_owned(),
    };
    match (head.is_empty(), content.is_empty()) {
        (true, _) => content.to_owned(),
        (false, true) => format!("{head}\n"),
        (false, false) => format!("{head}{gap}{content}"),
    }
}

// Read from the published version of each block, which is in force until someone picks.
pub fn title_of(body: &str) -> Option<String> {
    title_heading(&conflict::published(body)).map(|heading| heading.text)
}

// `=======` under a line makes that line a heading, so a conflict block can look like a title.
fn title_heading(body: &str) -> Option<op_md::Heading> {
    let heading = h1(body).filter(|heading| body[..heading.start].trim().is_empty())?;
    let held = conflict::blocks(body)
        .iter()
        .any(|block| block.range.start < heading.end);
    (!held).then_some(heading)
}

// The lenient counterpart to `from_file_string`: a file the model would reject still yields its
// heading, its body, and every field that did parse. It reads the way a task's does, so a reader
// renders what is there and flags only what is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialDocFields {
    pub created: FieldResult<Timestamp>,
    pub parent: FieldResult<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartialDocMetadata {
    Error(String),
    Fields(PartialDocFields),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartialDoc {
    pub metadata: PartialDocMetadata,
    pub conflicts: Vec<FieldConflict>,
    pub title: Option<String>,
    pub body: String,
}

impl PartialDoc {
    pub fn conflict_count(&self) -> usize {
        self.conflicts.len() + conflict::blocks(&self.body).len()
    }

    pub fn created(&self) -> FieldResult<Timestamp> {
        match &self.metadata {
            PartialDocMetadata::Error(_) => Err(FieldError::Missing),
            PartialDocMetadata::Fields(fields) => fields.created.clone(),
        }
    }

    pub fn parent(&self) -> Option<&str> {
        match &self.metadata {
            PartialDocMetadata::Error(_) => None,
            PartialDocMetadata::Fields(fields) => fields.parent.as_ref().ok()?.as_deref(),
        }
    }
}

pub fn parse_partial(input: &str) -> PartialDoc {
    let Some((fm_src, body)) = split_frontmatter(input) else {
        return PartialDoc {
            metadata: PartialDocMetadata::Error("missing frontmatter fence".to_owned()),
            conflicts: Vec::new(),
            title: title_of(input),
            body: input.to_owned(),
        };
    };
    let with_blocks = fm_src.replace('\r', "");
    let metadata =
        match serde_yaml::from_str::<serde_yaml::Mapping>(&conflict::published(&with_blocks)) {
            Err(err) => PartialDocMetadata::Error(err.to_string()),
            Ok(map) => PartialDocMetadata::Fields(fields_of(&map)),
        };
    PartialDoc {
        metadata,
        conflicts: parent_conflicts(
            &crate::field_conflicts(&with_blocks).unwrap_or_default(),
            parent_name,
        ),
        title: title_of(body),
        body: body.to_owned(),
    }
}

// The other version a conflict keeps, read as a doc's fields. Only `field` is set, so every other
// field reads as missing. A conflict read from a file already names the parent by its name.
pub fn other_fields(conflict: &FieldConflict) -> PartialDocFields {
    let mut map = serde_yaml::Mapping::new();
    if let Some(value) = &conflict.other {
        let value = match (conflict.field.as_str(), value) {
            ("parent", serde_yaml::Value::String(name)) => parent_path(name).into(),
            _ => value.clone(),
        };
        map.insert(conflict.field.clone().into(), value);
    }
    fields_of(&map)
}

fn fields_of(map: &serde_yaml::Mapping) -> PartialDocFields {
    PartialDocFields {
        created: match map.get("created") {
            None => Err(FieldError::Missing),
            Some(value) => serde_yaml::from_value::<Timestamp>(value.clone())
                .map_err(|err| FieldError::Invalid(err.to_string())),
        },
        parent: parent_of(map),
    }
}

fn parent_of(map: &serde_yaml::Mapping) -> FieldResult<Option<String>> {
    match map.get("parent") {
        None | Some(serde_yaml::Value::Null) => Ok(None),
        Some(serde_yaml::Value::String(path)) => match parent_name(path) {
            Some(name) => Ok(Some(name)),
            None => Err(FieldError::Invalid(format!("{path:?} {PARENT_EXPECTED}"))),
        },
        Some(_) => Err(FieldError::Invalid(PARENT_EXPECTED.to_owned())),
    }
}

// A child of `from` moves to `to`, or to the top level when `to` is `None`: a rename moves the
// children with their parent, and a delete lifts them to the deleted doc's own parent. Both versions
// of a parent conflict move, so the conflict still offers the two parents it held, and a conflict
// whose versions then agree is settled. A child the model rejects — one with no `created:`, say —
// has its `parent:` swapped on the frontmatter mapping, so it moves all the same. `None` when the
// file names no parent of `from`, which includes every file whose frontmatter is not a mapping: such
// a file names no parent at all, so it is nobody's child.
pub fn rewrite_parent(input: &str, from: &str, to: Option<&str>) -> Option<String> {
    if let Ok(mut doc) = Doc::from_file_string(String::new(), input) {
        let mut moved = false;
        if doc.frontmatter.parent.as_deref() == Some(from) {
            doc.frontmatter.parent = to.map(str::to_owned);
            moved = true;
        }
        for conflict in doc.conflicts.iter_mut().filter(|c| c.field == "parent") {
            if conflict.other.as_ref().and_then(serde_yaml::Value::as_str) == Some(from) {
                conflict.other = to.map(Into::into);
                moved = true;
            }
        }
        let published = doc.frontmatter.parent.clone();
        doc.conflicts.retain(|conflict| {
            conflict.field != "parent"
                || conflict
                    .other
                    .as_ref()
                    .and_then(serde_yaml::Value::as_str)
                    .map(str::to_owned)
                    != published
        });
        return moved.then(|| doc.to_file_string().ok()).flatten();
    }
    let (fm_src, body) = split_frontmatter(input)?;
    let mut map = serde_yaml::from_str::<serde_yaml::Mapping>(&fm_src.replace('\r', "")).ok()?;
    if parent_name(map.get("parent")?.as_str()?).as_deref() != Some(from) {
        return None;
    }
    match to {
        Some(to) => map.insert("parent".into(), parent_path(to).into()),
        None => map.remove("parent"),
    };
    let fm = serde_yaml::to_string(&map).ok()?;
    Some(format!("---\n{fm}---\n{body}"))
}

// Every doc a `[[…]]` in a body under `dir` names, by a path or by a name a person typed.
pub fn body_doc_names(
    abbreviation: Option<crate::Abbreviation>,
    dir: &str,
    body: &str,
) -> Vec<String> {
    crate::body_ref_spans(body)
        .into_iter()
        .filter_map(
            |(_, inner)| match crate::reference::body_target(abbreviation, dir, inner)? {
                crate::reference::Target::Doc(name) => Some(name),
                crate::reference::Target::Task(_) => None,
            },
        )
        .collect()
}
