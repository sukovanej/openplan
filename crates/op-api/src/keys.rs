use op_task::Abbreviation;

// A spelling of an id the store has no id for. One key spelling is accepted and nothing else is, so
// a refusal names the form that would have worked rather than guessing at what was meant.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a task key: {got:?}; expected {expected}")]
pub struct KeyError {
    pub got: String,
    pub expected: String,
}

impl KeyError {
    pub fn new(abbreviation: Abbreviation, got: &str) -> Self {
        Self {
            got: got.to_owned(),
            expected: abbreviation.format_key(42),
        }
    }
}

pub(crate) fn reference_of(abbreviation: Abbreviation, key: &str) -> Result<String, KeyError> {
    abbreviation
        .parse_ref(key)
        .ok_or_else(|| KeyError::new(abbreviation, key))
}

// In-memory references are the numbers the file layer allocates — `op_task` normalizes every stored
// spelling to one — and above the store each is rendered as this store's key.
pub(crate) fn key_of(abbreviation: Abbreviation, reference: &str) -> String {
    abbreviation
        .format_ref(reference)
        .unwrap_or_else(|| reference.to_owned())
}

// A body as the store carries it in memory: a `[[…]]` in the key spelling becomes the number the file
// layer names. Any other spelling of a reference is refused rather than written as prose — a bare
// number and another store's key both name no task here, and a file that already holds one shows it
// as the plain text it is.
pub fn body_from_keys(abbreviation: Abbreviation, body: &str) -> Result<String, KeyError> {
    let mut out = String::new();
    let mut last = 0;
    for (span, inner) in op_task::body_ref_spans(body) {
        let target = op_task::ref_target(inner);
        if let Some(reference) = abbreviation.parse_ref(inner) {
            out.push_str(&body[last..span.start]);
            out.push_str(&format!("[[{reference}]]"));
            last = span.end;
        } else if op_task::parse_id(target).is_some() || op_task::is_key_shaped(target) {
            return Err(KeyError::new(abbreviation, target));
        }
    }
    if last == 0 {
        return Ok(body.to_owned());
    }
    out.push_str(&body[last..]);
    Ok(out)
}

pub(crate) fn key_number(key: &str) -> Option<u64> {
    op_task::parse_id(key.rsplit_once('-')?.1)
}
