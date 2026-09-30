use op_diagram::{Attribute, Cluster, Icon, Node, Shape};

use crate::scene::{Anchor, IconBox, Outline, Rect, Text, move_texts};
use crate::text::{
    Block, CAPTION, CELL, CODE, COMMENT, EDGE_LABEL, HEADER, KEY, LABEL, Style, TABLE_HEADER, block,
};

pub(super) const MAX_LABEL: f32 = 200.0;
const MAX_HEADER: f32 = 320.0;
const PAD_X: f32 = 16.0;
const PAD_Y: f32 = 10.0;
const DOUBLE_RING: f32 = 5.0;
const MAX_DIAMOND_RATIO: f32 = 2.0;
const SUBROUTINE_BAR: f32 = 8.0;
const TABLE_PAD_X: f32 = 12.0;
const TABLE_PAD_Y: f32 = 8.0;
const TABLE_COLUMN_GAP: f32 = 12.0;
const TABLE_ROW_GAP: f32 = 6.0;
const CODE_PAD_X: f32 = 4.0;
const CODE_PAD_Y: f32 = 1.0;
const EDGE_LABEL_PAD_X: f32 = 4.0;
const EDGE_LABEL_PAD_Y: f32 = 2.0;
const ICON: f32 = 16.0;
const ICON_GAP: f32 = 10.0;
const HEADER_ICON: f32 = 14.0;
const HEADER_ICON_GAP: f32 = 8.0;
const CAPTION_GAP: f32 = 8.0;

pub(super) struct Body {
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) outline: Outline,
    pub(super) texts: Vec<Text>,
    pub(super) icon: Option<IconBox>,
}

pub(super) fn node(node: &Node) -> Body {
    if let Shape::Table { attributes } = &node.shape {
        return table(&node.label, attributes);
    }
    if let Some(icon) = node.icon {
        return card(node, icon);
    }
    let caption = match &node.caption {
        Some(caption) => block(std::slice::from_ref(caption), CAPTION, MAX_LABEL),
        None => Block::empty(CAPTION),
    };
    let label = match node.shape {
        Shape::Diamond => diamond_label(&node.label, &caption),
        _ => block(&node.label, LABEL, MAX_LABEL),
    };
    let frame = frame(
        &node.shape,
        caption.width.max(label.width),
        caption.height + label.height,
    );
    let mut texts = caption.centered(frame.text_center, frame.text_top);
    texts.extend(label.centered(frame.text_center, frame.text_top + caption.height));
    Body {
        width: frame.width,
        height: frame.height,
        outline: outline(&node.shape, frame.width),
        texts,
        icon: None,
    }
}

// All cards take the width of the longest line a label can have beside the icon, so the cards of a
// flow line up in columns.
fn card(node: &Node, icon: Icon) -> Body {
    let width = ICON + ICON_GAP + MAX_LABEL;
    let title = title(
        Some(Mark {
            icon,
            size: ICON,
            gap: ICON_GAP,
        }),
        node.caption.as_deref(),
        &node.label,
        LABEL,
        width,
    );
    let content_width = width.max(title.width);
    let frame = frame(&node.shape, content_width, title.height);
    let left = frame.text_center - content_width / 2.0;
    Body {
        width: frame.width,
        height: frame.height,
        outline: outline(&node.shape, frame.width),
        texts: title.texts(left, frame.text_top),
        icon: title.icon(left, frame.text_top),
    }
}

// A label on one long line makes a flat diamond, so the label wraps narrower until the diamond is no
// more than twice as wide as it is high, or until its longest word stops it.
fn diamond_label(label: &[String], caption: &Block) -> Block {
    let ratio = |label: &Block| {
        (caption.width.max(label.width) + PAD_X) / (caption.height + label.height + PAD_Y)
    };
    let mut wrapped = block(label, LABEL, MAX_LABEL);
    while ratio(&wrapped) > MAX_DIAMOND_RATIO {
        let narrower = block(label, LABEL, wrapped.width - 1.0);
        if narrower.width >= wrapped.width {
            break;
        }
        wrapped = narrower;
    }
    wrapped
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
            let width = text_width + 2.0 * PAD_X;
            let cap = cylinder_cap(width);
            Frame {
                width,
                height: padded_height + 3.0 * cap,
                text_center: width / 2.0,
                text_top: 2.0 * cap + PAD_Y,
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

// A fixed cap flattens a wide cylinder into a tray.
fn cylinder_cap(width: f32) -> f32 {
    (width * 0.07).clamp(6.0, 12.0)
}

fn outline(shape: &Shape, width: f32) -> Outline {
    match shape {
        Shape::Rectangle => Outline::Rectangle,
        Shape::Rounded => Outline::Rounded,
        Shape::Stadium => Outline::Stadium,
        Shape::Subroutine => Outline::Subroutine,
        Shape::Cylinder => Outline::Cylinder {
            cap: cylinder_cap(width),
        },
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
            header: 0.0,
            codes: Vec::new(),
        },
    }
}

fn table(label: &[String], attributes: &[Attribute]) -> Body {
    let header = block(label, TABLE_HEADER, f32::INFINITY);
    let widest = |width: fn(&Attribute) -> f32| attributes.iter().map(width).fold(0.0, f32::max);
    let widths = [
        widest(|attribute| code_width(&attribute.data_type)),
        widest(|attribute| CELL.width(&attribute.name)),
        widest(|attribute| KEY.width(&key_list(attribute))),
        widest(|attribute| {
            attribute
                .comment
                .as_deref()
                .map_or(0.0, |comment| COMMENT.width(comment))
        }),
    ];
    let mut lefts = [TABLE_PAD_X; 4];
    let mut left = TABLE_PAD_X;
    for (column, width) in widths.iter().enumerate() {
        lefts[column] = left;
        if *width > 0.0 {
            left += width + TABLE_COLUMN_GAP;
        }
    }
    let content = (left - TABLE_COLUMN_GAP - TABLE_PAD_X).max(0.0);
    let width = header.width.max(content) + 2.0 * TABLE_PAD_X;
    let header_height = header.height + 2.0 * TABLE_PAD_Y;
    let row_height = CELL.line_height() + TABLE_ROW_GAP;
    let rows_height = match attributes.len() {
        0 => 0.0,
        count => count as f32 * row_height + TABLE_PAD_Y,
    };
    let code_height = CODE.line_height() + 2.0 * CODE_PAD_Y;
    let mut texts = header.centered(width / 2.0, TABLE_PAD_Y);
    let mut codes = Vec::new();
    for (at, attribute) in attributes.iter().enumerate() {
        let top = header_height + TABLE_PAD_Y / 2.0 + at as f32 * row_height;
        let baseline = CELL.baseline(top + TABLE_ROW_GAP / 2.0);
        codes.push(Rect {
            x: lefts[0],
            y: top + (row_height - code_height) / 2.0,
            width: code_width(&attribute.data_type),
            height: code_height,
        });
        texts.push(CODE.text(
            &attribute.data_type,
            lefts[0] + CODE_PAD_X,
            baseline,
            Anchor::Start,
        ));
        texts.push(CELL.text(&attribute.name, lefts[1], baseline, Anchor::Start));
        let keys = key_list(attribute);
        if !keys.is_empty() {
            texts.push(KEY.text(&keys, lefts[2], baseline, Anchor::Start));
        }
        if let Some(comment) = &attribute.comment {
            texts.push(COMMENT.text(comment, lefts[3], baseline, Anchor::Start));
        }
    }
    Body {
        width,
        height: header_height + rows_height,
        outline: Outline::Table {
            header: header_height,
            codes,
        },
        texts,
        icon: None,
    }
}

fn code_width(data_type: &str) -> f32 {
    CODE.width(data_type) + 2.0 * CODE_PAD_X
}

fn key_list(attribute: &Attribute) -> String {
    attribute.keys.join(", ")
}

struct Mark {
    icon: Icon,
    size: f32,
    gap: f32,
}

// The line of the key takes the line height of the label too, so the height stays a whole number.
pub(super) struct Title {
    pub(super) width: f32,
    pub(super) height: f32,
    texts: Vec<Text>,
    icon: Option<IconBox>,
}

fn title(
    mark: Option<Mark>,
    caption: Option<&str>,
    label: &[String],
    style: Style,
    width: f32,
) -> Title {
    let line = style.line_height();
    let caption_left = mark.as_ref().map_or(0.0, |mark| mark.size + mark.gap);
    let (lead_end, label_left) = match (&mark, caption) {
        (_, Some(caption)) => {
            let end = caption_left + CAPTION.width(caption);
            (end, end + CAPTION_GAP)
        }
        (Some(mark), None) => (mark.size, caption_left),
        (None, None) => {
            let label = block(label, style, width);
            return Title {
                width: label.width,
                height: label.height,
                texts: label.left_aligned(0.0, 0.0),
                icon: None,
            };
        }
    };
    let mut texts: Vec<Text> = caption
        .map(|caption| CAPTION.text(caption, caption_left, style.baseline(0.0), Anchor::Start))
        .into_iter()
        .collect();
    let icon = mark.map(|mark| IconBox {
        icon: mark.icon,
        rect: Rect {
            x: 0.0,
            y: (line - mark.size) / 2.0,
            width: mark.size,
            height: mark.size,
        },
    });
    let beside = block(label, style, width - label_left);
    let (width, height) = if beside.lines.len() <= 1 && label_left + beside.width <= width {
        texts.extend(beside.left_aligned(label_left, 0.0));
        (label_left + beside.width, line)
    } else {
        let below = block(label, style, width);
        texts.extend(below.left_aligned(0.0, line));
        (lead_end.max(below.width), line + below.height)
    };
    Title {
        width,
        height,
        texts,
        icon,
    }
}

impl Title {
    pub(super) fn texts(&self, left: f32, top: f32) -> Vec<Text> {
        let mut texts = self.texts.clone();
        move_texts(&mut texts, left, top);
        texts
    }

    pub(super) fn icon(&self, left: f32, top: f32) -> Option<IconBox> {
        self.icon.map(|icon| icon.moved(left, top))
    }
}

pub(super) fn header(cluster: &Cluster) -> Title {
    let mark = cluster.icon.map(|icon| Mark {
        icon,
        size: HEADER_ICON,
        gap: HEADER_ICON_GAP,
    });
    let width = mark.as_ref().map_or(0.0, |mark| mark.size + mark.gap) + MAX_HEADER;
    title(
        mark,
        cluster.caption.as_deref(),
        &cluster.label,
        HEADER,
        width,
    )
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
