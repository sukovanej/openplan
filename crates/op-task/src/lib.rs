use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub use jiff::Timestamp;

pub mod comment;
pub mod rank;
pub mod tag;

const SLUG_MAX: usize = 32;
const ID_DIGITS: usize = 5;
const ABBREVIATION_LEN: usize = 3;

// Task files are hand-written and diffed, so a stored timestamp carries whole seconds — the clock's
// sub-second tail is noise no reader of a task file wants.
pub fn now() -> Timestamp {
    Timestamp::from_second(Timestamp::now().as_second())
        .expect("a whole second of the current time is in range")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Backlog,
    Todo,
    InProgress,
    InReview,
    Done,
    Cancelled,
}

impl Status {
    pub const ALL: [Status; 6] = [
        Status::Backlog,
        Status::Todo,
        Status::InProgress,
        Status::InReview,
        Status::Done,
        Status::Cancelled,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Backlog => "backlog",
            Status::Todo => "todo",
            Status::InProgress => "in_progress",
            Status::InReview => "in_review",
            Status::Done => "done",
            Status::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid status {got:?}; expected one of {}", Status::ALL.map(|s| s.as_str()).join(", "))]
pub struct ParseStatusError {
    got: String,
}

impl std::str::FromStr for Status {
    type Err = ParseStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Status::ALL
            .into_iter()
            .find(|status| status.as_str() == s)
            .ok_or_else(|| ParseStatusError { got: s.to_owned() })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    pub status: Status,
    pub created: Timestamp,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_parent"
    )]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "deserialize_dependencies"
    )]
    pub dependencies: Vec<String>,
    // An unordered set, unlike `dependencies`: two branches that each add a tag must merge to the
    // union, so the field carries no order a merge could disagree about.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "serialize_tags",
        deserialize_with = "deserialize_tags"
    )]
    pub tags: Vec<String>,
    // Fields the model does not name are preserved verbatim across a read-modify-write
    // so a `set` never silently drops them.
    #[serde(flatten)]
    pub extra: serde_yaml::Mapping,
}

// The number the daemon allocated, in the one spelling that names a file: decimal, no sign,
// no padding. `042`, `+7`, and a slug from before the scheme name no task.
pub fn parse_id(id: &str) -> Option<u64> {
    let number: u64 = id.parse().ok()?;
    (number.to_string() == id).then_some(number)
}

// The store's key prefix. The number allocates a task and names its file; the key —
// `<ABBR>-<number>` — is the id everywhere above the store, so one number reads as one key across
// the API, the CLI, and the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Abbreviation([u8; ABBREVIATION_LEN]);

#[derive(Debug, thiserror::Error)]
#[error("must be exactly three uppercase letters")]
pub struct ParseAbbreviationError;

impl std::str::FromStr for Abbreviation {
    type Err = ParseAbbreviationError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let letters: [u8; ABBREVIATION_LEN] = text
            .as_bytes()
            .try_into()
            .map_err(|_| ParseAbbreviationError)?;
        letters
            .iter()
            .all(u8::is_ascii_uppercase)
            .then_some(Self(letters))
            .ok_or(ParseAbbreviationError)
    }
}

impl std::fmt::Display for Abbreviation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Abbreviation {
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("uppercase ASCII letters are UTF-8")
    }

    pub fn format_key(&self, number: u64) -> String {
        format!("{}-{number}", self.as_str())
    }

    // One spelling, no leniency: this store's prefix, then the number as `parse_id` spells it. A
    // lowercased prefix, a padded number, and another store's prefix all name no task.
    pub fn parse_key(&self, key: &str) -> Option<u64> {
        parse_id(key.strip_prefix(self.as_str())?.strip_prefix('-')?)
    }

    // A reference may aim at a section (`OPP-42#Design`); these carry it across the two spellings.
    pub fn format_ref(&self, reference: &str) -> Option<String> {
        let number = parse_id(ref_target(reference))?;
        Some(with_section(&self.format_key(number), reference))
    }

    pub fn parse_ref(&self, reference: &str) -> Option<String> {
        let number = self.parse_key(ref_target(reference))?;
        Some(with_section(&number.to_string(), reference))
    }
}

fn with_section(target: &str, reference: &str) -> String {
    match section_of(reference) {
        Some(section) => format!("{target}#{section}"),
        None => target.to_owned(),
    }
}

// Text spelled like some store's key (`WEB-7`), whichever store that is — enough to tell a
// reference written for another project from ordinary bracketed prose.
pub fn is_key_shaped(text: &str) -> bool {
    match text.split_once('-') {
        Some((prefix, number)) => {
            prefix.len() == ABBREVIATION_LEN
                && prefix.bytes().all(|b| b.is_ascii_uppercase())
                && parse_id(number).is_some()
        }
        None => false,
    }
}

// Zero-padded so a directory listing reads in task order, then the title so a human browsing the
// files can tell them apart. Neither is part of the id: the padding is a naming convention
// and the slug is a snapshot of the title, free to be re-slugged by hand as long as the digits stay.
pub fn task_filename(number: u64, title: &str) -> String {
    format!("{number:0ID_DIGITS$}-{}.md", slug(title, "task"))
}

pub fn file_id(stem: &str) -> Option<u64> {
    let digits = stem.bytes().take_while(u8::is_ascii_digit).count();
    let tail = &stem[digits..];
    if digits == 0 || !(tail.is_empty() || tail.starts_with('-')) {
        return None;
    }
    stem[..digits].parse().ok()
}

pub fn slug(text: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch.to_ascii_lowercase());
            if out.len() >= SLUG_MAX {
                break;
            }
        } else {
            pending_dash = true;
        }
    }
    if out.is_empty() {
        out.push_str(fallback);
    }
    out
}

// How one task file names another: the path to its file, so the reference is one a plain markdown
// reader can follow and a `grep` can see. Written next to the target, which is where every
// task file lives.
pub fn task_ref(filename: &str) -> String {
    format!("./{filename}")
}

// A reference may aim at a section of its target (`./00042-design.md#Design`); the file is the part
// before the `#`.
pub fn ref_target(reference: &str) -> &str {
    reference
        .split_once('#')
        .map_or(reference, |(target, _)| target)
}

fn file_stem(target: &str) -> Option<&str> {
    target
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".md"))
}

// The task a frontmatter reference names, by the digits its file name starts with — the slug in
// between is a snapshot of the title and carries no identity, so a retitled target still resolves.
// The bare number is the spelling the model carries in memory, so it resolves too.
pub fn ref_id(reference: &str) -> Option<u64> {
    let target = ref_target(reference);
    match file_stem(target) {
        Some(stem) => file_id(stem),
        None => parse_id(target),
    }
}

// The task a `[[…]]` in a body names, in the spellings a body may carry: the target's file, or this
// store's key. A bare number is not one of them — above the store the key is the whole id.
pub fn body_ref_id(abbreviation: Abbreviation, reference: &str) -> Option<u64> {
    let target = ref_target(reference);
    match file_stem(target) {
        Some(stem) => file_id(stem),
        None => abbreviation.parse_key(target),
    }
}

// Every `[[…]]` in a body that a renderer would treat as a reference, as the range it occupies and
// its trimmed inner text. Inner text carrying a bracket or a newline is bracketed prose, and a span
// inside code or an existing link is quoted source — a body documenting `[[42]]` in a code span says
// what the spelling looks like rather than naming task 42, so a rewrite must leave it alone.
pub fn body_ref_spans(body: &str) -> Vec<(std::ops::Range<usize>, &str)> {
    let opaque = op_md::opaque_ranges(body);
    let quoted = |span: &std::ops::Range<usize>| {
        opaque
            .iter()
            .any(|r| r.start < span.end && span.start < r.end)
    };
    let mut spans = Vec::new();
    let mut pos = 0;
    while let Some(open) = body[pos..].find("[[") {
        let start = pos + open;
        let inner_at = start + 2;
        let Some(close) = body[inner_at..].find("]]") else {
            break;
        };
        let inner = &body[inner_at..inner_at + close];
        pos = inner_at + close + 2;
        let span = start..pos;
        if !inner.contains(['[', ']', '\n']) && !quoted(&span) {
            spans.push((span, inner.trim()));
        }
    }
    spans
}

fn section_of(reference: &str) -> Option<&str> {
    reference.split_once('#').map(|(_, section)| section)
}

// The in-memory spelling of a reference is the id, so nothing above the store has to know
// what a task's file is called; only the store, which can see the directory, writes the file form.
fn ref_of(value: &serde_yaml::Value) -> Option<String> {
    let reference = match value {
        serde_yaml::Value::String(reference) => reference.clone(),
        serde_yaml::Value::Number(number) => number.as_u64()?.to_string(),
        _ => return None,
    };
    let number = ref_id(&reference)?;
    Some(match section_of(&reference) {
        Some(section) => format!("{number}#{section}"),
        None => number.to_string(),
    })
}

fn parent_of(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::String(reference) if section_of(reference).is_some() => None,
        other => ref_of(other),
    }
}

pub const REFERENCE_EXPECTED: &str = "expected a task file, like ./00042-write-the-parser.md";

fn deserialize_parent<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    match Option::<serde_yaml::Value>::deserialize(deserializer)? {
        None | Some(serde_yaml::Value::Null) => Ok(None),
        Some(value) => parent_of(&value)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(REFERENCE_EXPECTED)),
    }
}

fn deserialize_dependencies<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    Vec::<serde_yaml::Value>::deserialize(deserializer)?
        .iter()
        .map(|value| {
            ref_of(value).ok_or_else(|| {
                serde::de::Error::custom(format!("{REFERENCE_EXPECTED}, one per entry"))
            })
        })
        .collect()
}

pub const TAGS_EXPECTED: &str = "expected a list of tag names, like [backend, wip]";

pub fn sorted_set(mut names: Vec<String>) -> Vec<String> {
    names.sort_unstable();
    names.dedup();
    names
}

fn serialize_tags<S: serde::Serializer>(tags: &[String], serializer: S) -> Result<S::Ok, S::Error> {
    sorted_set(tags.to_vec()).serialize(serializer)
}

// A hand-written `tags: [7, on-hold]` means two names, not a type error: the entry is whatever the
// human typed, and only a nested collection has no name to read.
fn tag_name_of(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::String(name) => Some(name.clone()),
        serde_yaml::Value::Number(number) => Some(number.to_string()),
        serde_yaml::Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn deserialize_tags<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    Vec::<serde_yaml::Value>::deserialize(deserializer)
        .map_err(|_| serde::de::Error::custom(TAGS_EXPECTED))?
        .iter()
        .map(|value| tag_name_of(value).ok_or_else(|| serde::de::Error::custom(TAGS_EXPECTED)))
        .collect::<Result<Vec<String>, D::Error>>()
        .map(sorted_set)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub frontmatter: Frontmatter,
    pub body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("invalid frontmatter: {0}")]
    Frontmatter(#[from] serde_yaml::Error),
    #[error("missing frontmatter fence")]
    MissingFrontmatter,
    // Its own variant, not a plain YAML error, because it is the one parse failure a caller can
    // explain in terms of what the reader must do — see `StoreError::MissingCreated`.
    #[error("no `created:` field")]
    MissingCreated,
}

impl Task {
    pub fn new(title: &str, status: Status, created: Timestamp) -> Self {
        Self {
            frontmatter: Frontmatter {
                status,
                created,
                parent: None,
                rank: None,
                dependencies: Vec::new(),
                tags: Vec::new(),
                extra: serde_yaml::Mapping::new(),
            },
            body: format!("# {title}\n"),
        }
    }

    pub fn append_body(&mut self, content: &str) {
        self.body = with_paragraph(&self.body, content);
    }

    pub fn append_comment(&mut self, comment: &comment::NewComment) {
        self.body =
            op_md::append_under(&self.body, 2, comment::HEADING, &comment::markdown(comment));
    }

    pub fn set_status(&mut self, status: Status) {
        self.frontmatter.status = status;
    }

    pub fn set_parent(&mut self, parent: Option<String>) {
        self.frontmatter.parent = parent;
    }

    pub fn set_rank(&mut self, rank: Option<String>) {
        self.frontmatter.rank = rank;
    }

    pub fn set_dependencies(&mut self, dependencies: Vec<String>) {
        self.frontmatter.dependencies = dependencies;
    }

    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.frontmatter.tags = sorted_set(tags);
    }

    pub fn title(&self) -> Option<String> {
        op_md::title(&self.body)
    }

    pub fn to_file_string(&self) -> Result<String, TaskError> {
        let fm = serde_yaml::to_string(&self.frontmatter)?;
        Ok(format!("---\n{fm}---\n{}", self.body))
    }

    pub fn from_file_string(input: &str) -> Result<Self, TaskError> {
        let (fm_src, body) = split_frontmatter(input).ok_or(TaskError::MissingFrontmatter)?;
        let fm_src = fm_src.replace('\r', "");
        match serde_yaml::from_str::<Frontmatter>(&fm_src) {
            Ok(frontmatter) => Ok(Self {
                frontmatter,
                body: body.to_owned(),
            }),
            Err(err) => Err(match serde_yaml::from_str::<serde_yaml::Mapping>(&fm_src) {
                Ok(map) if !map.contains_key("created") => TaskError::MissingCreated,
                _ => TaskError::Frontmatter(err),
            }),
        }
    }
}

// A per-field parse outcome for the read/display path, where one bad field must not sink the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldError {
    Missing,
    Invalid(String),
}

pub type FieldResult<T> = Result<T, FieldError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialFrontmatter {
    pub status: FieldResult<Status>,
    pub created: FieldResult<Timestamp>,
    pub parent: FieldResult<Option<String>>,
    pub rank: FieldResult<Option<String>>,
    pub dependencies: FieldResult<Vec<String>>,
    pub tags: FieldResult<Vec<String>>,
}

// The frontmatter parsed as far as it can be: `Fields` when the YAML is a mapping (each field then
// succeeds or fails on its own), `Error` when the fence or the YAML itself is unparseable and no
// field can be recovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartialMetadata {
    Error(String),
    Fields(PartialFrontmatter),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartialTask {
    pub metadata: PartialMetadata,
    // The frontmatter as raw YAML, present whenever it parsed as a mapping — so a reader can report
    // `path:line` and name which `dependencies` entry is bad rather than lose the whole list to one.
    pub frontmatter: Option<serde_yaml::Mapping>,
    pub title: Option<String>,
    pub body: String,
}

// The lenient counterpart to `from_file_string`: never fails, so a task with one bad field still
// yields its status, title, body, and every other readable field for the UI to render.
pub fn parse_partial(input: &str) -> PartialTask {
    match split_frontmatter(input) {
        None => PartialTask {
            metadata: PartialMetadata::Error("missing frontmatter fence".to_owned()),
            frontmatter: None,
            title: op_md::title(input),
            body: input.to_owned(),
        },
        Some((fm_src, body)) => {
            let (metadata, frontmatter) =
                match serde_yaml::from_str::<serde_yaml::Mapping>(&fm_src.replace('\r', "")) {
                    Ok(map) => (PartialMetadata::Fields(extract_fields(&map)), Some(map)),
                    Err(err) => (PartialMetadata::Error(err.to_string()), None),
                };
            PartialTask {
                metadata,
                frontmatter,
                title: op_md::title(body),
                body: body.to_owned(),
            }
        }
    }
}

fn extract_fields(map: &serde_yaml::Mapping) -> PartialFrontmatter {
    PartialFrontmatter {
        status: required(map, "status"),
        created: required(map, "created"),
        parent: optional_parent(map),
        rank: optional_string(map, "rank"),
        dependencies: match map.get("dependencies") {
            None => Ok(Vec::new()),
            Some(serde_yaml::Value::Sequence(items)) => items
                .iter()
                .map(|item| {
                    ref_of(item).ok_or_else(|| {
                        FieldError::Invalid(format!("{REFERENCE_EXPECTED}, one per entry"))
                    })
                })
                .collect(),
            Some(_) => Err(FieldError::Invalid(format!(
                "{REFERENCE_EXPECTED}, one per entry"
            ))),
        },
        tags: extract_tags(map),
    }
}

fn extract_tags(map: &serde_yaml::Mapping) -> FieldResult<Vec<String>> {
    let Some(value) = map.get("tags") else {
        return Ok(Vec::new());
    };
    let serde_yaml::Value::Sequence(items) = value else {
        return Err(FieldError::Invalid(TAGS_EXPECTED.to_owned()));
    };
    items
        .iter()
        .map(|item| tag_name_of(item).ok_or_else(|| FieldError::Invalid(TAGS_EXPECTED.to_owned())))
        .collect::<FieldResult<Vec<String>>>()
        .map(sorted_set)
}

fn required<T: serde::de::DeserializeOwned>(
    map: &serde_yaml::Mapping,
    field: &str,
) -> FieldResult<T> {
    match map.get(field) {
        None => Err(FieldError::Missing),
        Some(value) => serde_yaml::from_value::<T>(value.clone())
            .map_err(|err| FieldError::Invalid(err.to_string())),
    }
}

fn optional_parent(map: &serde_yaml::Mapping) -> FieldResult<Option<String>> {
    match map.get("parent") {
        None | Some(serde_yaml::Value::Null) => Ok(None),
        Some(value) => parent_of(value)
            .map(Some)
            .ok_or_else(|| FieldError::Invalid(REFERENCE_EXPECTED.to_owned())),
    }
}

fn optional_string(map: &serde_yaml::Mapping, field: &str) -> FieldResult<Option<String>> {
    match map.get(field) {
        None | Some(serde_yaml::Value::Null) => Ok(None),
        Some(serde_yaml::Value::String(id)) => Ok(Some(id.clone())),
        Some(_) => Err(FieldError::Invalid("expected a string".to_owned())),
    }
}

fn is_fence(line: &str) -> bool {
    line.trim_end_matches('\n').trim_end_matches('\r') == "---"
}

pub(crate) fn with_paragraph(body: &str, content: &str) -> String {
    let content = content.trim_end_matches('\n');
    if content.is_empty() {
        return body.to_owned();
    }
    let head = body.trim_end_matches('\n');
    format!("{head}\n\n{content}\n")
}

pub(crate) fn split_frontmatter(input: &str) -> Option<(&str, &str)> {
    let rest = input
        .strip_prefix("---\n")
        .or_else(|| input.strip_prefix("---\r\n"))?;
    let fm_start = input.len() - rest.len();
    let mut pos = fm_start;
    for line in rest.split_inclusive('\n') {
        if is_fence(line) {
            return Some((&input[fm_start..pos], &input[pos + line.len()..]));
        }
        pos += line.len();
    }
    None
}
