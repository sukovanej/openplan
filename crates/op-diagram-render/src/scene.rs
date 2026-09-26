use op_diagram::{Head, Stroke};

use crate::measure::Weight;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn center(&self) -> Point {
        Point {
            x: self.x + self.width / 2.0,
            y: self.y + self.height / 2.0,
        }
    }

    pub fn contains(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    pub fn inset(&self, by: f32) -> Rect {
        Rect {
            x: self.x + by,
            y: self.y + by,
            width: (self.width - 2.0 * by).max(0.0),
            height: (self.height - 2.0 * by).max(0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    Label,
    Caption,
    Header,
    Cell,
    EdgeLabel,
    Number,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub content: String,
    pub x: f32,
    pub baseline: f32,
    pub size: f32,
    pub weight: Weight,
    pub anchor: Anchor,
    pub role: TextRole,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outline {
    Rectangle,
    Rounded,
    Stadium,
    Subroutine,
    Cylinder,
    Circle,
    DoubleCircle,
    Asymmetric,
    Diamond,
    Hexagon,
    LeanRight,
    LeanLeft,
    Trapezoid,
    InvertedTrapezoid,
    Table { dividers: Vec<f32> },
    Actor,
    Note,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeBox {
    pub id: String,
    pub parent: Option<String>,
    pub outline: Outline,
    pub rect: Rect,
    pub texts: Vec<Text>,
    pub classes: Vec<String>,
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClusterBox {
    pub id: String,
    pub parent: Option<String>,
    pub depth: usize,
    pub rect: Rect,
    pub texts: Vec<Text>,
    pub classes: Vec<String>,
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLabel {
    pub rect: Rect,
    pub texts: Vec<Text>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgePath {
    pub from: String,
    pub to: String,
    pub points: Vec<Point>,
    pub stroke: Stroke,
    pub tail: Head,
    pub head: Head,
    pub label: Option<EdgeLabel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideKind {
    Lifeline,
    Divider,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Guide {
    pub from: Point,
    pub to: Point,
    pub kind: GuideKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width: f32,
    pub height: f32,
    pub clusters: Vec<ClusterBox>,
    pub nodes: Vec<NodeBox>,
    pub edges: Vec<EdgePath>,
    pub guides: Vec<Guide>,
}

impl Point {
    fn moved(self, dx: f32, dy: f32) -> Point {
        Point {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

impl Rect {
    fn moved(self, dx: f32, dy: f32) -> Rect {
        Rect {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }
}

fn move_texts(texts: &mut [Text], dx: f32, dy: f32) {
    for text in texts {
        text.x += dx;
        text.baseline += dy;
    }
}

impl Scene {
    pub(crate) fn translate(&mut self, dx: f32, dy: f32) {
        for cluster in &mut self.clusters {
            cluster.rect = cluster.rect.moved(dx, dy);
            move_texts(&mut cluster.texts, dx, dy);
        }
        for node in &mut self.nodes {
            node.rect = node.rect.moved(dx, dy);
            move_texts(&mut node.texts, dx, dy);
        }
        for edge in &mut self.edges {
            for point in &mut edge.points {
                *point = point.moved(dx, dy);
            }
            if let Some(label) = &mut edge.label {
                label.rect = label.rect.moved(dx, dy);
                move_texts(&mut label.texts, dx, dy);
            }
        }
        for guide in &mut self.guides {
            guide.from = guide.from.moved(dx, dy);
            guide.to = guide.to.moved(dx, dy);
        }
    }
}
