use crate::measure::{Weight, line_metrics, text_width};
use crate::scene::{Anchor, Text, TextRole};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Style {
    pub(crate) size: f32,
    pub(crate) weight: Weight,
    pub(crate) role: TextRole,
}

pub(crate) const LABEL: Style = Style {
    size: 14.0,
    weight: Weight::Regular,
    role: TextRole::Label,
};
pub(crate) const CAPTION: Style = Style {
    size: 12.0,
    weight: Weight::Regular,
    role: TextRole::Caption,
};
pub(crate) const HEADER: Style = Style {
    size: 13.0,
    weight: Weight::SemiBold,
    role: TextRole::Header,
};
pub(crate) const TABLE_HEADER: Style = Style {
    size: 14.0,
    weight: Weight::SemiBold,
    role: TextRole::Header,
};
pub(crate) const CELL: Style = Style {
    size: 13.0,
    weight: Weight::Regular,
    role: TextRole::Cell,
};
pub(crate) const EDGE_LABEL: Style = Style {
    size: 12.0,
    weight: Weight::Regular,
    role: TextRole::EdgeLabel,
};

impl Style {
    pub(crate) fn line_height(self) -> f32 {
        (self.size * 1.4).round()
    }

    pub(crate) fn width(self, text: &str) -> f32 {
        text_width(text, self.size, self.weight)
    }

    fn baseline(self, top: f32) -> f32 {
        let metrics = line_metrics(self.size);
        top + (self.line_height() - metrics.ascent - metrics.descent) / 2.0 + metrics.ascent
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Block {
    pub(crate) style: Style,
    pub(crate) lines: Vec<String>,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

// A label keeps the lines its author broke with `<br>` and wraps each one at the spaces that fit
// `max_width`. A word wider than that stays whole on a line of its own.
pub(crate) fn block(lines: &[String], style: Style, max_width: f32) -> Block {
    let lines: Vec<String> = lines
        .iter()
        .flat_map(|line| wrap(line, style, max_width))
        .collect();
    let width = lines
        .iter()
        .map(|line| style.width(line))
        .fold(0.0, f32::max);
    let height = lines.len() as f32 * style.line_height();
    Block {
        style,
        lines,
        width,
        height,
    }
}

fn wrap(line: &str, style: Style, max_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        let joined = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if !current.is_empty() && style.width(&joined) > max_width {
            lines.push(std::mem::replace(&mut current, word.to_owned()));
        } else {
            current = joined;
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

impl Block {
    pub(crate) fn empty(style: Style) -> Block {
        Block {
            style,
            lines: Vec::new(),
            width: 0.0,
            height: 0.0,
        }
    }

    pub(crate) fn centered(&self, center_x: f32, top: f32) -> Vec<Text> {
        self.texts(center_x, top, Anchor::Middle)
    }

    pub(crate) fn left_aligned(&self, left: f32, top: f32) -> Vec<Text> {
        self.texts(left, top, Anchor::Start)
    }

    fn texts(&self, x: f32, top: f32, anchor: Anchor) -> Vec<Text> {
        self.lines
            .iter()
            .enumerate()
            .map(|(at, line)| Text {
                content: line.clone(),
                x,
                baseline: self
                    .style
                    .baseline(top + at as f32 * self.style.line_height()),
                size: self.style.size,
                weight: self.style.weight,
                anchor,
                role: self.style.role,
            })
            .collect()
    }
}
