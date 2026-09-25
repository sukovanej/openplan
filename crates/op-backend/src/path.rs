use crate::BackendError;

// A leading dot is reserved: the local backend keeps its history and its temp files beside the
// documents, and git gives `.git*` names a meaning of its own.
pub fn check_path(path: &str) -> Result<(), BackendError> {
    let usable = !path.is_empty()
        && !path.contains(['\\', '\0'])
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && !segment.starts_with('.'));
    match usable {
        true => Ok(()),
        false => Err(BackendError::InvalidPath(path.to_owned())),
    }
}
