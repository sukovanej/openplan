use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::field::{Field, Rfc3339};

// One entry of a task's comment log. `at` and `author` are fields on the read path for the same
// reason the frontmatter's are: a hand-damaged heading must still deliver the text it introduces.
// `agent` is an `Option` rather than a field, because a comment a person typed has no agent and
// that is not a failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Comment {
    pub at: Field<Rfc3339>,
    pub author: Field<String>,
    // Always on the wire, `null` for a comment a person typed: a reader tells "no agent" from "the
    // daemon did not say" without knowing which keys this shape may drop.
    #[serde(default)]
    pub agent: Option<String>,
    pub text: String,
}

impl From<&op_task::comment::Comment> for Comment {
    fn from(comment: &op_task::comment::Comment) -> Self {
        Self {
            at: Field::from(comment.at.clone()).map(Rfc3339),
            author: Field::from(comment.author.clone()),
            agent: comment.agent.clone(),
            text: comment.text.clone(),
        }
    }
}

impl From<&op_task::comment::NewComment> for Comment {
    fn from(comment: &op_task::comment::NewComment) -> Self {
        Self {
            at: Field::Value(Rfc3339(comment.at)),
            author: Field::Value(comment.author.clone()),
            agent: comment.agent.clone(),
            text: comment.text.clone(),
        }
    }
}

impl From<&Comment> for op_task::comment::Comment {
    fn from(comment: &Comment) -> Self {
        Self {
            at: comment.at.clone().into_result().map(|at| at.0),
            author: comment.author.clone().into_result(),
            agent: comment.agent.clone(),
            text: comment.text.clone(),
        }
    }
}

// One branch's whole log, for the read that spans every branch. Grouped rather than flat so no
// entry repeats the branch it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BranchComments {
    pub branch: String,
    pub comments: Vec<Comment>,
}

// A comment to append. The daemon stamps the time, because it is the single in-band writer and one
// clock must order every write; the caller carries the identity, because only the CLI process sees
// the environment that names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateComment {
    pub text: String,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub agent: Option<String>,
}
