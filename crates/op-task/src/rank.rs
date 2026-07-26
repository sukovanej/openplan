// Sibling order (§3.2) is a fractional index: a short base-36 string key that sorts
// lexicographically. `between` returns a key strictly between two neighbours, so a task can be
// reordered by writing one key without renumbering its siblings. Base-36 digits (`0-9a-z`) are in
// ASCII order, so byte comparison of keys matches their fractional value.

const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const BASE: usize = 36;

pub fn is_valid(key: &str) -> bool {
    !key.is_empty() && key.bytes().all(|b| DIGITS.contains(&b))
}

// Trailing smallest digits leave a key's value unchanged, so `a` and `a0` name the same point and
// must not be treated as a gap. Comparing the trimmed keys orders them by value, not by spelling.
fn value_of(key: &str) -> &str {
    key.trim_end_matches('0')
}

fn digit_value(byte: u8) -> usize {
    DIGITS
        .iter()
        .position(|&d| d == byte)
        .expect("callers only pass validated base-36 keys")
}

// The digit index at `pos`, treating a key as an infinite fraction padded with the smallest digit:
// trailing smallest digits leave the value unchanged, so a missing position reads as 0.
fn digit_at(key: &str, pos: usize) -> usize {
    key.as_bytes()
        .get(pos)
        .map(|&b| digit_value(b))
        .unwrap_or(0)
}

// The shortest key strictly between `lower` and `upper`, where `None` means the open end (before the
// first sibling / after the last). `None` when no such key exists or can be trusted: a malformed key
// (hand-edited frontmatter, an unvalidated PATCH) or bounds that do not straddle a gap. Callers
// rebalance the whole sibling group instead — searching for a gap that cannot exist would descend
// forever.
pub fn between(lower: Option<&str>, upper: Option<&str>) -> Option<String> {
    if [lower, upper].iter().flatten().any(|key| !is_valid(key)) {
        return None;
    }
    if let (Some(lower), Some(upper)) = (lower, upper) {
        if value_of(lower) >= value_of(upper) {
            return None;
        }
    }
    let mut out = Vec::new();
    let mut open_upper = upper.is_none();
    let mut pos = 0;
    loop {
        let lo = lower.map(|k| digit_at(k, pos)).unwrap_or(0);
        let hi = if open_upper {
            BASE
        } else {
            digit_at(upper.unwrap(), pos)
        };
        if lo == hi {
            out.push(DIGITS[lo]);
            pos += 1;
            continue;
        }
        let mid = (lo + hi) / 2;
        if mid > lo {
            out.push(DIGITS[mid]);
            return Some(String::from_utf8(out).expect("base-36 digits are ASCII"));
        }
        // No gap at this position (hi == lo + 1); commit the lower digit and descend into the space
        // below `upper`, where any deeper digit keeps the key under it.
        out.push(DIGITS[lo]);
        open_upper = true;
        pos += 1;
    }
}

// Whether a sibling group's ranks admit a single-key insert anywhere: every key valid and strictly
// increasing by value. Missing, malformed, colliding, or same-point (`a` and `a0`) ranks leave some
// neighbour pair with no gap to split, and the group has to be rebalanced instead.
pub fn is_ordered(keys: &[&str]) -> bool {
    keys.iter().all(|key| is_valid(key))
        && keys
            .windows(2)
            .all(|pair| value_of(pair[0]) < value_of(pair[1]))
}

// `n` keys spread evenly across the whole range, leaving as much room before the first and after the
// last as between any two neighbours. Rebalancing a sibling group onto these keys is what heals a
// group whose ranks are missing, colliding, or malformed.
pub fn spaced(n: usize) -> Vec<String> {
    let mut width = 1;
    let mut slots = BASE;
    while slots <= n {
        width += 1;
        slots *= BASE;
    }
    (1..=n)
        .map(|i| encode(i * slots / (n + 1), width))
        .collect()
}

fn encode(mut value: usize, width: usize) -> String {
    let mut out = vec![b'0'; width];
    for digit in out.iter_mut().rev() {
        *digit = DIGITS[value % BASE];
        value /= BASE;
    }
    String::from_utf8(out).expect("base-36 digits are ASCII")
}
