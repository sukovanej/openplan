mod graph;
mod icon;
mod measure;
mod scene;
mod sequence;
mod svg;
mod text;

use op_diagram::Diagram;

pub use measure::{LineMetrics, Weight, line_metrics, text_width};
pub use scene::{
    Anchor, ClusterBox, EdgeLabel, EdgePath, Guide, GuideKind, IconBox, NodeBox, Outline, Point,
    Rect, Scene, Text, TextRole,
};
pub use svg::svg;

pub fn layout(diagram: &Diagram) -> Scene {
    match diagram {
        Diagram::Graph(graph) => graph::layout(graph),
        Diagram::Sequence(sequence) => sequence::layout(sequence),
    }
}
