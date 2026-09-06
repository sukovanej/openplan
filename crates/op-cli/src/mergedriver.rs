use std::collections::HashMap;
use std::process::ExitCode;

use op_task::{Frontmatter, Task, three_way};

pub struct Args<'a> {
    pub ancestor: &'a str,
    pub current: &'a str,
    pub other: &'a str,
    pub marker_size: usize,
    pub path: Option<&'a str>,
    pub label_ours: Option<&'a str>,
    pub label_theirs: Option<&'a str>,
}

pub fn run(args: &Args<'_>) -> ExitCode {
    let read = |path: &str| std::fs::read_to_string(path);
    let (base, ours, theirs) = match (read(args.ancestor), read(args.current), read(args.other)) {
        (Ok(base), Ok(ours), Ok(theirs)) => (base, ours, theirs),
        _ => {
            eprintln!("openplan merge-driver: cannot read the three sides");
            return ExitCode::FAILURE;
        }
    };

    let labels = Labels {
        size: args.marker_size.max(1),
        ours: args.label_ours.unwrap_or("ours"),
        theirs: args.label_theirs.unwrap_or("theirs"),
    };
    let merged = merge(&base, &ours, &theirs, &labels);
    if std::fs::write(args.current, merged.text()).is_err() {
        eprintln!("openplan merge-driver: cannot write {}", args.current);
        return ExitCode::FAILURE;
    }
    match merged {
        Merged::Clean(_) => ExitCode::SUCCESS,
        Merged::Conflicted { what, .. } => {
            let path = args.path.unwrap_or(args.current);
            eprintln!("openplan merge-driver: {path}: {what}");
            ExitCode::FAILURE
        }
    }
}

pub struct Labels<'a> {
    pub size: usize,
    pub ours: &'a str,
    pub theirs: &'a str,
}

impl Labels<'_> {
    fn wrap(&self, ours: &str, theirs: &str) -> String {
        let open = "<".repeat(self.size);
        let split = "=".repeat(self.size);
        let close = ">".repeat(self.size);
        format!(
            "{open} {}\n{}\n{split}\n{}\n{close} {}",
            self.ours,
            ours.trim_end(),
            theirs.trim_end(),
            self.theirs,
        )
    }
}

pub enum Merged {
    Clean(String),
    Conflicted { text: String, what: String },
}

impl Merged {
    fn text(&self) -> &str {
        match self {
            Merged::Clean(text) | Merged::Conflicted { text, .. } => text,
        }
    }
}

pub fn merge(base: &str, ours: &str, theirs: &str, labels: &Labels<'_>) -> Merged {
    if ours == theirs {
        return Merged::Clean(ours.to_owned());
    }
    if base == ours {
        return Merged::Clean(theirs.to_owned());
    }
    if base == theirs {
        return Merged::Clean(ours.to_owned());
    }

    let parsed = (
        Task::from_file_string(base),
        Task::from_file_string(ours),
        Task::from_file_string(theirs),
    );
    let (Ok(base_task), Ok(our_task), Ok(their_task)) = parsed else {
        return whole_file(ours, theirs, labels, "the file does not parse as a task");
    };

    // A frontmatter field carries no place to put markers that still leaves YAML a reader can
    // parse, and a same-field divergence is a choice only a person can make. Fall back to the
    // whole-file form rather than emit frontmatter no parser accepts.
    let Some(frontmatter) = Frontmatter::merged(
        &base_task.frontmatter,
        &our_task.frontmatter,
        &their_task.frontmatter,
    ) else {
        return whole_file(
            ours,
            theirs,
            labels,
            "the same frontmatter field changed on both sides",
        );
    };

    let (body, conflicts) = merge_body(&base_task.body, &our_task.body, &their_task.body, labels);
    let task = Task { frontmatter, body };
    let Ok(text) = task.to_file_string() else {
        return whole_file(ours, theirs, labels, "the merged task does not serialize");
    };
    if conflicts.is_empty() {
        Merged::Clean(text)
    } else {
        Merged::Conflicted {
            text,
            what: format!("both sides changed {}", conflicts.join(", ")),
        }
    }
}

fn whole_file(ours: &str, theirs: &str, labels: &Labels<'_>, what: &str) -> Merged {
    Merged::Conflicted {
        text: format!("{}\n", labels.wrap(ours, theirs)),
        what: what.to_owned(),
    }
}

fn merge_body(base: &str, ours: &str, theirs: &str, labels: &Labels<'_>) -> (String, Vec<String>) {
    let (base, ours, theirs) = (sections(base), sections(ours), sections(theirs));
    let find = |list: &[(Section, String)], key: &Section| {
        list.iter()
            .find(|(section, _)| section == key)
            .map(|(_, text)| text.clone())
    };

    let mut order: Vec<Section> = ours.iter().map(|(key, _)| key.clone()).collect();
    for (key, _) in &theirs {
        if !order.contains(key) {
            order.push(key.clone());
        }
    }

    let mut blocks: Vec<String> = Vec::new();
    let mut conflicts = Vec::new();
    for key in order {
        let (b, o, t) = (find(&base, &key), find(&ours, &key), find(&theirs, &key));
        match three_way(&b, &o, &t) {
            Some(Some(text)) => blocks.push(text),
            Some(None) => {}
            None => {
                blocks.push(labels.wrap(
                    o.as_deref().unwrap_or_default(),
                    t.as_deref().unwrap_or_default(),
                ));
                conflicts.push(key.name());
            }
        }
    }
    (format!("{}\n", blocks.join("\n\n")), conflicts)
}

// The body split into the blocks a person edits: the text before the first heading, then every
// heading of level one or two with what it heads. A deeper heading belongs to the block it sits in,
// so an edit inside it counts as an edit to that block.
fn sections(body: &str) -> Vec<(Section, String)> {
    let heads: Vec<op_md::Heading> = op_md::headings(body)
        .into_iter()
        .filter(|heading| heading.level <= 2)
        .collect();
    let mut out = Vec::new();
    let first = heads.first().map_or(body.len(), |heading| heading.start);
    if !body[..first].trim().is_empty() {
        out.push((Section::preamble(), body[..first].trim_end().to_owned()));
    }
    let mut seen: HashMap<(u8, String), usize> = HashMap::new();
    for (index, heading) in heads.iter().enumerate() {
        let end = heads.get(index + 1).map_or(body.len(), |next| next.start);
        let repeats = seen
            .entry((heading.level, heading.text.clone()))
            .or_insert(0);
        out.push((
            Section {
                level: heading.level,
                heading: heading.text.clone(),
                repeat: *repeats,
            },
            body[heading.start..end].trim_end().to_owned(),
        ));
        *repeats += 1;
    }
    out
}

#[derive(Clone, PartialEq)]
struct Section {
    level: u8,
    heading: String,
    repeat: usize,
}

impl Section {
    fn preamble() -> Self {
        Section {
            level: 0,
            heading: String::new(),
            repeat: 0,
        }
    }

    fn name(&self) -> String {
        if self.level == 0 {
            return "the text above the first heading".to_owned();
        }
        format!("\"{}\"", self.heading)
    }
}
