use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use op_task::tag::{Color, ParseNameError, Tag};

use crate::field::FieldUpdate;

// One registered tag. `name` is the identity a task's `tags:` holds; `display` is the heading a
// human reads, which differs from the name only in case and separators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TagView {
    pub name: String,
    pub display: String,
    pub color: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub description: Option<String>,
}

impl From<&Tag> for TagView {
    fn from(tag: &Tag) -> Self {
        let description = tag.description();
        Self {
            name: tag.name.clone(),
            display: tag.display_name().unwrap_or_else(|| tag.name.clone()),
            color: tag.color(),
            description: (!description.is_empty()).then_some(description),
        }
    }
}

// `name` is the name a human typed, so `Front End` registers `front-end` and reads back as
// `# Front End`. An omitted color is derived from the name rather than left unset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateTag {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub description: Option<String>,
}

impl CreateTag {
    pub fn into_tag(self) -> Result<Tag, ParseNameError> {
        let mut tag = Tag::new(&self.name, self.color)?;
        if let Some(description) = &self.description {
            tag.set_description(description);
        }
        Ok(tag)
    }
}

// `name` renames: it carries the new display name, and its normalization is the tag's new identity,
// so a change of case alone moves the heading and leaves the file where it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TagPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "FieldUpdate::is_keep")]
    #[schema(value_type = Option<String>)]
    pub description: FieldUpdate<String>,
}

impl TagPatch {
    pub fn changes_content(&self) -> bool {
        self.color.is_some() || !self.description.is_keep()
    }

    // The rename is the store's to carry out — it moves the file and rewrites every task that
    // references the tag — so this covers only what lives inside the tag file.
    pub fn apply(self, tag: &mut Tag) {
        if let Some(color) = self.color {
            tag.set_color(color);
        }
        match self.description {
            FieldUpdate::Keep => {}
            FieldUpdate::Clear => tag.set_description(""),
            FieldUpdate::Set(description) => tag.set_description(&description),
        }
    }
}
