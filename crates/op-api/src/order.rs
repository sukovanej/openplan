use crate::field::Rfc3339;
use crate::keys::key_number;
use crate::task::{TaskListItem, TaskSummary};

// A `rank` is an order somebody set on purpose, so ranked tasks hold the top of the list
// and `unranked` decides among the rest.
pub(crate) fn rank_cmp(
    ra: Option<&str>,
    rb: Option<&str>,
    unranked: impl Fn() -> std::cmp::Ordering,
) -> std::cmp::Ordering {
    match (ra, rb) {
        (Some(x), Some(y)) => x.cmp(y).then_with(unranked),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => unranked(),
    }
}

// A key is a number behind a prefix, so it orders as one — text order would file `OPP-10`
// between `OPP-1` and `OPP-2`. The prefix is constant within a store, so dropping it changes nothing.
pub fn id_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    match (key_number(a), key_number(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        _ => a.cmp(b),
    }
}

// Rank ties break by id so the order is stable across rebuilds.
pub fn sibling_cmp(a: &TaskSummary, b: &TaskSummary) -> std::cmp::Ordering {
    rank_cmp(a.metadata.rank(), b.metadata.rank(), || {
        id_cmp(&a.id, &b.id)
    })
}

pub fn list_item_cmp(a: &TaskListItem, b: &TaskListItem) -> std::cmp::Ordering {
    rank_cmp(a.metadata.rank(), b.metadata.rank(), || {
        id_cmp(&a.id, &b.id)
    })
}

// The board leads with the work touched most recently, so what someone just changed is at hand
// without scrolling for it. A merged board holds tasks from several projects, so the project name
// breaks a tie before the id does: two stores can issue the same key, and the order must not depend
// on which project was read first. It only breaks a tie. Each store ranks its own tasks, so ranked
// tasks from two projects interleave, and every ranked task still leads every unranked one — a
// merged board is ordered, not grouped by project.
pub fn board_cmp(a: &TaskListItem, b: &TaskListItem) -> std::cmp::Ordering {
    rank_cmp(a.metadata.rank(), b.metadata.rank(), || {
        newest_first(a.updated.as_value(), b.updated.as_value())
            .then_with(|| a.project.cmp(&b.project))
            .then_with(|| id_cmp(&a.id, &b.id))
    })
}

// An `updated` that could not be derived places the task nowhere on the scale, so it sorts after
// every dated one rather than reading as the oldest.
fn newest_first(a: Option<&Rfc3339>, b: Option<&Rfc3339>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(x), Some(y)) => y.0.cmp(&x.0),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}
