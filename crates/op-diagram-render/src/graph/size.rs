use op_diagram::{Cluster, Node, Row, Shape};

use crate::scene::{Outline, Text};
use crate::text::{Block, CAPTION, CELL, EDGE_LABEL, HEADER, LABEL, TABLE_HEADER, block};

pub(super) const MAX_LABEL: f32 = 200.0;
const MAX_HEADER: f32 = 320.0;
const PAD_X: f32 = 16.0;
const PAD_Y: f32 = 10.0;
pub(super) const CYLINDER_CAP: f32 = 7.0;
const DOUBLE_RING: f32 = 5.0;
const SUBROUTINE_BAR: f32 = 8.0;
const TABLE_PAD_X: f32 = 12.0;
const TABLE_PAD_Y: f32 = 8.0;
const TABLE_COLUMN_GAP: f32 = 12.0;
const EDGE_LABEL_PAD_X: f32 = 4.0;
const EDGE_LABEL_PAD_Y: f32 = 2.0;

pub(super) struct Body {
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) outline: Outline,
    pub(super) texts: Vec<Text>,
}

pub(super) fn node(node: &Node) -> Body {
    if let Shape::Table { rows } = &node.shape {
        return table(&node.label, rows);
    }
    let caption = match &node.caption {
        Some(caption) => block(std::slice::from_ref(caption), CAPTION, MAX_LABEL),
        None => Block::empty(CAPTION),
    };
    let label = block(&node.label, LABEL, MAX_LABEL);
    let text_width = caption.width.max(label.width);
    let text_height = caption.height + label.height;
    let frame = frame(&node.shape, text_width, text_height);
    let mut texts = caption.centered(frame.text_center, frame.text_top);
    texts.extend(label.centered(frame.text_center, frame.text_top + caption.height));
    Body {
        width: frame.width,
        height: frame.height,
        outline: outline(&node.shape),
        texts,
    }
}

struct Frame {
    width: f32,
    height: f32,
    text_center: f32,
    text_top: f32,
}

fn frame(shape: &Shape, text_width: f32, text_height: f32) -> Frame {
    let padded_height = text_height + 2.0 * PAD_Y;
    let around = |width: f32, height: f32| Frame {
        width,
        height,
        text_center: width / 2.0,
        text_top: (height - text_height) / 2.0,
    };
    match shape {
        Shape::Rectangle | Shape::Rounded | Shape::Table { .. } => {
            around(text_width + 2.0 * PAD_X, padded_height)
        }
        Shape::Subroutine => around(text_width + 2.0 * (PAD_X + SUBROUTINE_BAR), padded_height),
        Shape::Stadium | Shape::Hexagon => around(
            text_width + 2.0 * PAD_X + padded_height / 2.0,
            padded_height,
        ),
        Shape::LeanRight | Shape::LeanLeft | Shape::Trapezoid | Shape::InvertedTrapezoid => {
            around(text_width + 2.0 * PAD_X + padded_height, padded_height)
        }
        Shape::Cylinder => {
            let height = padded_height + 3.0 * CYLINDER_CAP;
            Frame {
                width: text_width + 2.0 * PAD_X,
                height,
                text_center: (text_width + 2.0 * PAD_X) / 2.0,
                text_top: 2.0 * CYLINDER_CAP + PAD_Y,
            }
        }
        Shape::Circle => {
            let diameter = text_width.hypot(text_height) + 2.0 * PAD_Y;
            around(diameter, diameter)
        }
        Shape::DoubleCircle => {
            let diameter = text_width.hypot(text_height) + 2.0 * (PAD_Y + DOUBLE_RING);
            around(diameter, diameter)
        }
        Shape::Asymmetric => {
            let notch = padded_height / 2.0;
            let width = text_width + 2.0 * PAD_X + notch;
            Frame {
                width,
                height: padded_height,
                text_center: notch + (width - notch) / 2.0,
                text_top: PAD_Y,
            }
        }
        // The smallest rhombus around a box of w by h has diagonals of 2w and 2h.
        Shape::Diamond => around(2.0 * (text_width + PAD_X), 2.0 * (text_height + PAD_Y)),
    }
}

fn outline(shape: &Shape) -> Outline {
    match shape {
        Shape::Rectangle => Outline::Rectangle,
        Shape::Rounded => Outline::Rounded,
        Shape::Stadium => Outline::Stadium,
        Shape::Subroutine => Outline::Subroutine,
        Shape::Cylinder => Outline::Cylinder,
        Shape::Circle => Outline::Circle,
        Shape::DoubleCircle => Outline::DoubleCircle,
        Shape::Asymmetric => Outline::Asymmetric,
        Shape::Diamond => Outline::Diamond,
        Shape::Hexagon => Outline::Hexagon,
        Shape::LeanRight => Outline::LeanRight,
        Shape::LeanLeft => Outline::LeanLeft,
        Shape::Trapezoid => Outline::Trapezoid,
        Shape::InvertedTrapezoid => Outline::InvertedTrapezoid,
        Shape::Table { .. } => Outline::Table {
            dividers: Vec::new(),
        },
    }
}

fn table(label: &[String], rows: &[Row]) -> Body {
    let header = block(label, TABLE_HEADER, f32::INFINITY);
    let columns = rows.iter().map(|row| row.cells.len()).max().unwrap_or(0);
    let widths: Vec<f32> = (0..columns)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.cells.get(column))
                .map(|cell| CELL.width(cell))
                .fold(0.0, f32::max)
        })
        .collect();
    let content = widths.iter().sum::<f32>() + TABLE_COLUMN_GAP * columns.saturating_sub(1) as f32;
    let width = header.width.max(content) + 2.0 * TABLE_PAD_X;
    let header_height = header.height + 2.0 * TABLE_PAD_Y;
    let row_height = CELL.line_height() + 4.0;
    let rows_height = match rows.len() {
        0 => 0.0,
        count => count as f32 * row_height + TABLE_PAD_Y,
    };
    let mut texts = header.centered(width / 2.0, TABLE_PAD_Y);
    for (at, row) in rows.iter().enumerate() {
        let top = header_height + TABLE_PAD_Y / 2.0 + at as f32 * row_height + 2.0;
        let mut left = TABLE_PAD_X;
        for (cell, column_width) in row.cells.iter().zip(&widths) {
            let cell = block(std::slice::from_ref(cell), CELL, f32::INFINITY);
            texts.extend(cell.left_aligned(left, top));
            left += column_width + TABLE_COLUMN_GAP;
        }
    }
    Body {
        width,
        height: header_height + rows_height,
        outline: Outline::Table {
            dividers: if rows.is_empty() {
                Vec::new()
            } else {
                vec![header_height]
            },
        },
        texts,
    }
}

pub(super) struct Header {
    pub(super) caption: Block,
    pub(super) label: Block,
}

impl Header {
    pub(super) fn width(&self) -> f32 {
        self.caption.width.max(self.label.width)
    }

    pub(super) fn height(&self) -> f32 {
        self.caption.height + self.label.height
    }

    pub(super) fn texts(&self, left: f32, top: f32) -> Vec<Text> {
        let mut texts = self.caption.left_aligned(left, top);
        texts.extend(self.label.left_aligned(left, top + self.caption.height));
        texts
    }
}

pub(super) fn header(cluster: &Cluster) -> Header {
    Header {
        caption: match &cluster.caption {
            Some(caption) => block(std::slice::from_ref(caption), CAPTION, MAX_HEADER),
            None => Block::empty(CAPTION),
        },
        label: block(&cluster.label, HEADER, MAX_HEADER),
    }
}

pub(super) struct Label {
    pub(super) block: Block,
    pub(super) width: f32,
    pub(super) height: f32,
}

pub(super) fn edge_label(lines: &[String]) -> Label {
    let block = block(lines, EDGE_LABEL, MAX_LABEL);
    Label {
        width: block.width + 2.0 * EDGE_LABEL_PAD_X,
        height: block.height + 2.0 * EDGE_LABEL_PAD_Y,
        block,
    }
}

impl Label {
    pub(super) fn texts(&self, center_x: f32, top: f32) -> Vec<Text> {
        self.block.centered(center_x, top + EDGE_LABEL_PAD_Y)
    }
}
