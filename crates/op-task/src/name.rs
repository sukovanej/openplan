// The spelling a file-named record is identified by: its file stem. Unlike `slug`, which drops
// whatever it cannot spell, this refuses it — the name is an identity a human typed, so `C++` must
// come back as `None` rather than as a silently different record.
pub const RULE: &str = "lowercase letters, digits, and hyphens, and starts with a letter or a digit; spaces and underscores become hyphens";

pub fn normalize(name: &str) -> Option<String> {
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
    is_normalized(&normalized).then_some(normalized)
}

fn is_normalized(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn h1(body: &str) -> Option<op_md::Heading> {
    op_md::headings(body).into_iter().find(|h| h.level == 1)
}

pub fn retitle(body: &str, display_name: &str) -> String {
    let heading = format!("# {display_name}\n");
    match h1(body) {
        Some(h1) => format!("{}{heading}{}", &body[..h1.start], &body[h1.end..]),
        None => format!("{heading}{body}"),
    }
}
