use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DiagramSource {
    pub source: String,
}

// The SVG carries classes and no colors, so the page puts it inline and styles it in its theme. A
// diagram with nothing in it has a width and a height of zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Drawing {
    pub svg: String,
    pub width: f32,
    pub height: f32,
}
