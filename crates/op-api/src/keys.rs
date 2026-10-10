use op_task::reference::Target;
use op_task::{ProjectCode, TaskLink};

// A spelling of an id the store has no id for. One key spelling is accepted and nothing else is, so
// a refusal names the form that would have worked rather than guessing at what was meant.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error("{got} is in another project; this project's keys start with {project_code}-")]
    OtherProject {
        got: String,
        project_code: ProjectCode,
    },
    #[error("not a task key: {got:?}; expected {expected}")]
    NotAKey { got: String, expected: String },
}

impl KeyError {
    pub fn new(project_code: ProjectCode, got: &str) -> Self {
        let got = got.to_owned();
        if op_task::is_key_shaped(op_task::ref_target(&got)) {
            Self::OtherProject { got, project_code }
        } else {
            Self::NotAKey {
                got,
                expected: project_code.format_key(42),
            }
        }
    }

    pub fn got(&self) -> &str {
        match self {
            Self::OtherProject { got, .. } | Self::NotAKey { got, .. } => got,
        }
    }
}

pub(crate) fn link_of(project_code: ProjectCode, key: &str) -> Result<TaskLink, KeyError> {
    project_code
        .parse_ref(key)
        .as_deref()
        .and_then(TaskLink::from_id)
        .ok_or_else(|| KeyError::new(project_code, key))
}

// In-memory references are the numbers the file layer allocates — `op_task` normalizes every stored
// spelling to one — and above the store each is rendered as this store's key.
pub(crate) fn key_of(project_code: ProjectCode, reference: &str) -> String {
    project_code
        .format_ref(reference)
        .unwrap_or_else(|| reference.to_owned())
}

// A body as the store carries it in memory: a `[[…]]` in the key spelling becomes the number the file
// layer names. Any other spelling of a reference is refused rather than written as prose — a bare
// number and another store's key both name no task here, and a file that already holds one shows it
// as the plain text it is.
pub fn body_from_keys(project_code: ProjectCode, body: &str) -> Result<String, KeyError> {
    body_from_keys_keeping(project_code, body, |_| false)
}

pub(crate) fn body_from_keys_keeping(
    project_code: ProjectCode,
    body: &str,
    left_as_prose: impl Fn(&str) -> bool,
) -> Result<String, KeyError> {
    let mut out = String::new();
    let mut last = 0;
    for (span, inner) in op_task::body_ref_spans(body) {
        let target = op_task::ref_target(inner);
        if let Some(reference) = project_code.parse_ref(inner) {
            out.push_str(&body[last..span.start]);
            out.push_str(&format!("[[{reference}]]"));
            last = span.end;
        } else if (op_task::parse_id(target).is_some() || op_task::is_key_shaped(target))
            && !left_as_prose(target)
        {
            return Err(KeyError::new(project_code, target));
        }
    }
    if last == 0 {
        return Ok(body.to_owned());
    }
    out.push_str(&body[last..]);
    Ok(out)
}

// A body as the API carries it: a task is named by this store's key and a doc by its name. `dir` is
// the directory of the file the body comes from, which every path in it is relative to.
pub fn body_to_keys(project_code: ProjectCode, dir: &str, body: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for (span, inner) in op_task::body_ref_spans(body) {
        let target = match op_task::reference::body_target(Some(project_code), dir, inner) {
            Some(Target::Task(number)) => project_code.format_key(number),
            Some(Target::Doc(name)) => name,
            None => continue,
        };
        out.push_str(&body[last..span.start]);
        out.push_str("[[");
        out.push_str(&target);
        if let Some((_, section)) = inner.split_once('#') {
            out.push('#');
            out.push_str(section);
        }
        out.push_str("]]");
        last = span.end;
    }
    out.push_str(&body[last..]);
    out
}

pub(crate) fn key_number(key: &str) -> Option<u64> {
    op_task::parse_id(key.rsplit_once('-')?.1)
}
