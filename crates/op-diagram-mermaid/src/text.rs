const BREAKS: [&str; 3] = ["<br>", "<br/>", "<br />"];

pub(crate) fn label(raw: &str) -> Vec<String> {
    split_breaks(raw)
        .into_iter()
        .map(|line| decode(line.trim()))
        .collect()
}

fn split_breaks(raw: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while let Some(found) = raw[at..].find('<') {
        at += found;
        match BREAKS.iter().find(|tag| {
            raw.get(at..at + tag.len())
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(tag))
        }) {
            Some(tag) => {
                lines.push(&raw[start..at]);
                at += tag.len();
                start = at;
            }
            None => at += 1,
        }
    }
    lines.push(&raw[start..]);
    lines
}

// Mermaid spells a character that its syntax reserves as `#quot;` or `#35;`, the HTML entity with
// `#` for `&`. An unknown name stays as it was written.
fn decode(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(hash) = rest.find('#') {
        out.push_str(&rest[..hash]);
        rest = &rest[hash..];
        let decoded = rest[1..]
            .find(';')
            .filter(|end| *end <= 8)
            .and_then(|end| entity(&rest[1..1 + end]).map(|c| (c, end + 2)));
        match decoded {
            Some((c, length)) => {
                out.push(c);
                rest = &rest[length..];
            }
            None => {
                out.push('#');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    if !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()) {
        return name.parse().ok().and_then(char::from_u32);
    }
    match name {
        "quot" => Some('"'),
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        _ => None,
    }
}
