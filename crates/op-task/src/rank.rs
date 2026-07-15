// Sibling order (§3.2) is a fractional index: a short base-36 string key that sorts
// lexicographically. `between` returns a key strictly between two neighbours, so a task can be
// reordered by writing one key without renumbering its siblings. Base-36 digits (`0-9a-z`) are in
// ASCII order, so byte comparison of keys matches their fractional value.

const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const BASE: usize = 36;

pub fn is_valid(key: &str) -> bool {
    !key.is_empty() && key.bytes().all(|b| DIGITS.contains(&b))
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

// A key strictly between `lower` and `upper`, where `None` means the open end (before the first
// sibling / after the last). `lower` must sort before `upper`; the result is the shortest key that
// lands between them.
pub fn between(lower: Option<&str>, upper: Option<&str>) -> String {
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
            return String::from_utf8(out).expect("base-36 digits are ASCII");
        }
        // No gap at this position (hi == lo + 1); commit the lower digit and descend into the space
        // below `upper`, where any deeper digit keeps the key under it.
        out.push(DIGITS[lo]);
        open_upper = true;
        pos += 1;
    }
}
