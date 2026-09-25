use std::ops::Range;

// Git's conflict markers. Sync writes them where two people changed the same lines differently,
// and they stay in the text until a person or an agent keeps a version.
const START: &str = "<<<<<<<";
const BASE: &str = "|||||||";
const SPLIT: &str = "=======";
const END: &str = ">>>>>>>";

// The largest diff table a merge fills; past it, the changed middle becomes one block.
const TABLE_CELLS: usize = 4_000_000;

// `ours` comes first, as git writes it. `theirs` is the version the remote already published, so
// it is the one every reader shows until someone picks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub range: Range<usize>,
    pub ours: Side,
    pub theirs: Side,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Side {
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    pub ours: String,
    pub theirs: String,
}

enum Part {
    Ours,
    Base,
    Theirs,
}

struct Open {
    start: usize,
    part: Part,
    ours: Side,
    theirs: String,
}

// Marker lines inside a fenced code block are text: a task may show a conflict as an example.
pub fn blocks(text: &str) -> Vec<Block> {
    let mut found = Vec::new();
    let mut fence = Fence::default();
    let mut open: Option<Open> = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let bare = line.trim_end_matches(['\n', '\r']);
        if fence.is_open() {
            fence.see(bare);
            push_line(&mut open, line);
            continue;
        }
        match (&mut open, marker(bare)) {
            (None, Some((Marker::Start, label))) => {
                open = Some(Open {
                    start,
                    part: Part::Ours,
                    ours: Side {
                        label,
                        text: String::new(),
                    },
                    theirs: String::new(),
                });
            }
            (Some(block), Some((Marker::Base, _))) if matches!(block.part, Part::Ours) => {
                block.part = Part::Base;
            }
            (Some(block), Some((Marker::Split, _))) if !matches!(block.part, Part::Theirs) => {
                block.part = Part::Theirs;
            }
            (
                Some(Open {
                    part: Part::Theirs, ..
                }),
                Some((Marker::End, label)),
            ) => {
                let block = open.take().expect("a block is open");
                found.push(Block {
                    range: block.start..offset,
                    ours: block.ours,
                    theirs: Side {
                        label,
                        text: block.theirs,
                    },
                });
            }
            _ => {
                fence.see(bare);
                push_line(&mut open, line);
            }
        }
    }
    found
}

fn push_line(open: &mut Option<Open>, line: &str) {
    if let Some(block) = open {
        match block.part {
            Part::Ours => block.ours.text.push_str(line),
            Part::Base => {}
            Part::Theirs => block.theirs.push_str(line),
        }
    }
}

enum Marker {
    Start,
    Base,
    Split,
    End,
}

fn marker(line: &str) -> Option<(Marker, String)> {
    let labelled = |sign: &str| {
        let rest = line.strip_prefix(sign)?;
        match rest.is_empty() || rest.starts_with(' ') {
            true => Some(rest.trim().to_owned()),
            false => None,
        }
    };
    if line == SPLIT {
        return Some((Marker::Split, String::new()));
    }
    if let Some(label) = labelled(START) {
        return Some((Marker::Start, label));
    }
    if let Some(label) = labelled(BASE) {
        return Some((Marker::Base, label));
    }
    labelled(END).map(|label| (Marker::End, label))
}

#[derive(Default)]
struct Fence {
    open: Option<String>,
}

impl Fence {
    fn is_open(&self) -> bool {
        self.open.is_some()
    }

    fn see(&mut self, line: &str) {
        let trimmed = line.trim_start();
        if line.len() - trimmed.len() > 3 {
            return;
        }
        let sign: String = trimmed
            .chars()
            .take_while(|c| *c == '`' || *c == '~')
            .collect();
        let uniform = sign.chars().all(|c| sign.starts_with(c));
        if sign.len() < 3 || !uniform {
            return;
        }
        match &self.open {
            None => self.open = Some(sign),
            Some(opened)
                if sign.starts_with(opened.as_str()) && trimmed[sign.len()..].trim().is_empty() =>
            {
                self.open = None;
            }
            Some(_) => {}
        }
    }
}

// The comment log is append-only text a person wrote; a marker line quoted there is no conflict.
pub fn in_body(body: &str) -> Vec<Block> {
    let log = crate::comment::sections(body);
    blocks(body)
        .into_iter()
        .filter(|block| !crate::comment::inside_log(&log, block.range.start))
        .collect()
}

// The text as the remote published it: every block keeps its second version.
pub fn published(text: &str) -> String {
    replaced(text, |block| block.theirs.text.clone())
}

pub fn replaced(text: &str, mut keep: impl FnMut(&Block) -> String) -> String {
    let mut out = String::new();
    let mut last = 0;
    for block in blocks(text) {
        out.push_str(&text[last..block.range.start]);
        out.push_str(&keep(&block));
        last = block.range.end;
    }
    out.push_str(&text[last..]);
    out
}

pub fn render(ours: &Side, theirs: &Side) -> String {
    format!(
        "{START} {}\n{}{SPLIT}\n{}{END} {}\n",
        ours.label,
        with_newline(&ours.text),
        with_newline(&theirs.text),
        theirs.label
    )
}

fn with_newline(text: &str) -> String {
    match text.is_empty() || text.ends_with('\n') {
        true => text.to_owned(),
        false => format!("{text}\n"),
    }
}

// A three-way merge of lines. A fenced code block and an open conflict block each count as one
// line, so a new block never cuts into either. Where both sides changed the same lines
// differently, the result holds a block with both versions.
pub fn merge(base: &str, ours: &str, theirs: &str, labels: &Labels) -> String {
    if ours == theirs || base == theirs {
        return ours.to_owned();
    }
    if base == ours {
        return theirs.to_owned();
    }
    let (base, ours, theirs) = (units(base), units(ours), units(theirs));
    let to_ours = matches(&base, &ours);
    let to_theirs = matches(&base, &theirs);
    let (mut b, mut o, mut t) = (0, 0, 0);
    let mut out = String::new();
    loop {
        while b < base.len() && to_ours[b] == Some(o) && to_theirs[b] == Some(t) {
            out.push_str(base[b]);
            (b, o, t) = (b + 1, o + 1, t + 1);
        }
        if b == base.len() && o == ours.len() && t == theirs.len() {
            return out;
        }
        let next = (b..base.len()).find(|&at| to_ours[at].is_some() && to_theirs[at].is_some());
        let (nb, no, nt) = match next {
            Some(at) => (
                at,
                to_ours[at].expect("matched"),
                to_theirs[at].expect("matched"),
            ),
            None => (base.len(), ours.len(), theirs.len()),
        };
        let (was, mine, other) = (
            base[b..nb].concat(),
            ours[o..no].concat(),
            theirs[t..nt].concat(),
        );
        if mine == was || mine == other {
            out.push_str(&other);
        } else if other == was {
            out.push_str(&mine);
        } else {
            // A block the other side resolved stays out of the new one, so blocks never nest.
            let side = |label: &str, text: &str| Side {
                label: label.to_owned(),
                text: published(text),
            };
            out.push_str(&render(
                &side(&labels.ours, &mine),
                &side(&labels.theirs, &other),
            ));
        }
        (b, o, t) = (nb, no, nt);
    }
}

fn units(text: &str) -> Vec<&str> {
    let blocks = blocks(text);
    let mut out = Vec::new();
    let mut fence = Fence::default();
    let mut unit_start: Option<usize> = None;
    let mut offset = 0;
    let mut lines = text.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        let start = offset;
        offset += line.len();
        if unit_start.is_none()
            && let Some(block) = blocks.iter().find(|block| block.range.start == start)
        {
            while offset < block.range.end {
                offset += lines.next().map_or(0, str::len);
            }
            out.push(&text[start..offset]);
            continue;
        }
        let was_open = fence.is_open();
        fence.see(line.trim_end_matches(['\n', '\r']));
        match (was_open, fence.is_open()) {
            (false, true) => unit_start = Some(start),
            (true, false) => {
                out.push(&text[unit_start.take().expect("a fence is open")..offset]);
            }
            (true, true) => {}
            (false, false) => out.push(line),
        }
    }
    if let Some(start) = unit_start {
        out.push(&text[start..]);
    }
    out
}

// For each unit of `from`, the unit of `to` it matches in a longest common subsequence.
fn matches(from: &[&str], to: &[&str]) -> Vec<Option<usize>> {
    let mut out = vec![None; from.len()];
    let head = from.iter().zip(to).take_while(|(a, b)| a == b).count();
    let tail = from[head..]
        .iter()
        .rev()
        .zip(to[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    for (at, slot) in out.iter_mut().enumerate().take(head) {
        *slot = Some(at);
    }
    for back in 0..tail {
        out[from.len() - 1 - back] = Some(to.len() - 1 - back);
    }
    let (a, b) = (&from[head..from.len() - tail], &to[head..to.len() - tail]);
    if a.is_empty() || b.is_empty() || (a.len() + 1) * (b.len() + 1) > TABLE_CELLS {
        return out;
    }
    let width = b.len() + 1;
    let mut table = vec![0u32; (a.len() + 1) * width];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            table[i * width + j] = match a[i] == b[j] {
                true => table[(i + 1) * width + j + 1] + 1,
                false => table[(i + 1) * width + j].max(table[i * width + j + 1]),
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out[head + i] = Some(head + j);
            (i, j) = (i + 1, j + 1);
        } else if table[(i + 1) * width + j] >= table[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}
