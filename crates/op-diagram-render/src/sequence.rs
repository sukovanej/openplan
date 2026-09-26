use std::collections::HashMap;

use op_diagram::{
    Block, Message, Note, NotePlacement, Operator, Participant, ParticipantKind, Sequence,
    SequenceItem,
};

use crate::scene::{
    ClusterBox, EdgeLabel, EdgePath, Guide, GuideKind, NodeBox, Outline, Point, Rect, Scene,
};
use crate::text::{BLOCK_LABEL, Block as Lines, HEADER, LABEL, MESSAGE, NOTE, NUMBER, block};

const MARGIN: f32 = 8.0;
const HEAD_PAD_X: f32 = 16.0;
const HEAD_PAD_Y: f32 = 10.0;
const HEAD_MIN_WIDTH: f32 = 72.0;
const HEAD_MAX_TEXT: f32 = 160.0;
const ACTOR_FIGURE: f32 = 36.0;
const COLUMN_GAP: f32 = 40.0;
const MAX_TEXT: f32 = 240.0;
const LABEL_ROOM: f32 = 16.0;
const LABEL_GAP: f32 = 4.0;
const LABEL_PAD_X: f32 = 4.0;
const LABEL_PAD_Y: f32 = 1.0;
const HEAD_GAP: f32 = 20.0;
const ROW_GAP: f32 = 14.0;
const SELF_REACH: f32 = 32.0;
const SELF_DROP: f32 = 20.0;
const NOTE_PAD_X: f32 = 10.0;
const NOTE_PAD_Y: f32 = 6.0;
const NOTE_OFFSET: f32 = 10.0;
const NOTE_OVERHANG: f32 = 16.0;
const BLOCK_PAD: f32 = 12.0;
const BLOCK_HEADER_GAP: f32 = 6.0;
const ACTIVATION_HALF: f32 = 5.0;
const ACTIVATION_STEP: f32 = 4.0;
const ACTIVATION_LEAST: f32 = 8.0;
const NUMBER_RADIUS: f32 = 9.0;

struct Head {
    width: f32,
    height: f32,
    outline: Outline,
    label: Lines,
}

fn head(participant: &Participant) -> Head {
    let label = block(&participant.label, LABEL, HEAD_MAX_TEXT);
    match participant.kind {
        ParticipantKind::Box => Head {
            width: (label.width + 2.0 * HEAD_PAD_X).max(HEAD_MIN_WIDTH),
            height: label.height + 2.0 * HEAD_PAD_Y,
            outline: Outline::Rectangle,
            label,
        },
        ParticipantKind::Actor => Head {
            width: label.width.max(ACTOR_FIGURE),
            height: ACTOR_FIGURE + label.height,
            outline: Outline::Actor,
            label,
        },
    }
}

impl Head {
    fn node(&self, participant: &Participant, center: f32, top: f32) -> NodeBox {
        let rect = Rect {
            x: center - self.width / 2.0,
            y: top,
            width: self.width,
            height: self.height,
        };
        let text_top = match self.outline {
            Outline::Actor => top + ACTOR_FIGURE,
            _ => top + HEAD_PAD_Y,
        };
        NodeBox {
            id: participant.id.clone(),
            parent: None,
            outline: self.outline.clone(),
            rect,
            texts: self.label.centered(center, text_top),
            icon: None,
            classes: vec!["participant".to_owned()],
            link: None,
        }
    }
}

fn message_text(message: &Message) -> Lines {
    block(&message.text, MESSAGE, MAX_TEXT)
}

fn note_text(note: &Note) -> Lines {
    block(&note.text, NOTE, MAX_TEXT)
}

fn note_width(text: &Lines) -> f32 {
    text.width + 2.0 * NOTE_PAD_X
}

// How far apart two columns must stand, and how much room the first and the last column need
// outside them. A message label must fit between the lifelines it joins, and a note beside a
// lifeline must fit before the next one.
struct Spacing {
    pairs: Vec<(usize, usize, f32)>,
    before: f32,
    after: f32,
    last: usize,
    number_room: f32,
}

impl Spacing {
    fn need(&mut self, first: usize, second: usize, distance: f32) {
        let (low, high) = (first.min(second), first.max(second));
        if low != high {
            self.pairs.push((low, high, distance));
        }
    }

    fn beside(&mut self, column: usize, right: bool, room: f32) {
        match (right, column) {
            (true, column) if column == self.last => self.after = self.after.max(room),
            (true, column) => self.need(column, column + 1, room),
            (false, 0) => self.before = self.before.max(room),
            (false, column) => self.need(column - 1, column, room),
        }
    }

    fn collect(&mut self, items: &[SequenceItem], column: &HashMap<&str, usize>) {
        for item in items {
            match item {
                SequenceItem::Message(message) => {
                    let (from, to) = (column[message.from.as_str()], column[message.to.as_str()]);
                    let width = message_text(message).width;
                    if from == to {
                        self.beside(
                            from,
                            true,
                            SELF_REACH
                                + 2.0 * LABEL_GAP
                                + width
                                + ACTIVATION_HALF
                                + self.number_room,
                        );
                    } else {
                        self.need(from, to, width + 2.0 * (LABEL_ROOM + ACTIVATION_HALF));
                    }
                }
                SequenceItem::Note(note) => {
                    let width = note_width(&note_text(note));
                    match &note.placement {
                        NotePlacement::LeftOf(at) => {
                            self.beside(column[at.as_str()], false, width + 2.0 * NOTE_OFFSET)
                        }
                        NotePlacement::RightOf(at) => {
                            self.beside(column[at.as_str()], true, width + 2.0 * NOTE_OFFSET)
                        }
                        NotePlacement::Over(at) => {
                            let at = column[at.as_str()];
                            self.beside(at, false, width / 2.0 + NOTE_OFFSET);
                            self.beside(at, true, width / 2.0 + NOTE_OFFSET);
                        }
                        NotePlacement::Spanning(first, last) => {
                            let (first, last) = (column[first.as_str()], column[last.as_str()]);
                            self.need(first, last, width - 2.0 * NOTE_OVERHANG);
                        }
                    }
                }
                SequenceItem::Block(block) => {
                    for section in &block.sections {
                        self.collect(&section.items, column);
                    }
                }
                SequenceItem::Activate { .. } | SequenceItem::Deactivate { .. } => {}
            }
        }
    }

    // The narrowest requirement first: widening one pair of columns may already satisfy a wider
    // one that contains it.
    fn centers(mut self, heads: &[Head]) -> Vec<f32> {
        let mut centers = Vec::with_capacity(heads.len());
        let mut at = 0.0;
        for (index, head) in heads.iter().enumerate() {
            if index > 0 {
                at += heads[index - 1].width / 2.0 + COLUMN_GAP + head.width / 2.0;
            }
            centers.push(at);
        }
        self.pairs.sort_by_key(|(low, high, _)| high - low);
        for (low, high, distance) in self.pairs {
            let short = distance - (centers[high] - centers[low]);
            if short > 0.0 {
                for center in &mut centers[high..] {
                    *center += short;
                }
            }
        }
        centers
    }
}

#[derive(Clone, Copy)]
struct Extent {
    left: f32,
    right: f32,
}

impl Extent {
    const EMPTY: Extent = Extent {
        left: f32::INFINITY,
        right: f32::NEG_INFINITY,
    };

    fn include(&mut self, left: f32, right: f32) {
        self.left = self.left.min(left);
        self.right = self.right.max(right);
    }
}

struct Activation {
    start: f32,
    level: usize,
}

struct Builder<'a> {
    participants: &'a [Participant],
    column: HashMap<&'a str, usize>,
    centers: Vec<f32>,
    heads: Vec<Head>,
    autonumber: bool,
    cursor: f32,
    last_message: Option<f32>,
    active: Vec<Vec<Activation>>,
    open: Vec<Extent>,
    scene: Scene,
    numbers: usize,
    notes: usize,
    blocks: usize,
    block_ids: Vec<String>,
}

pub(crate) fn layout(sequence: &Sequence) -> Scene {
    if sequence.participants.is_empty() {
        return Scene::default();
    }
    let participants = &sequence.participants;
    let column: HashMap<&str, usize> = participants
        .iter()
        .enumerate()
        .map(|(at, participant)| (participant.id.as_str(), at))
        .collect();
    let heads: Vec<Head> = participants.iter().map(head).collect();
    let mut spacing = Spacing {
        pairs: Vec::new(),
        before: 0.0,
        after: 0.0,
        last: participants.len().saturating_sub(1),
        number_room: number_room(sequence.autonumber),
    };
    spacing.collect(&sequence.items, &column);
    let centers = spacing.centers(&heads);
    let head_height = heads.iter().map(|head| head.height).fold(0.0, f32::max);
    let mut builder = Builder {
        participants,
        column,
        centers,
        heads,
        autonumber: sequence.autonumber,
        cursor: head_height + HEAD_GAP,
        last_message: None,
        active: (0..participants.len()).map(|_| Vec::new()).collect(),
        open: Vec::new(),
        scene: Scene {
            width: 0.0,
            height: 0.0,
            clusters: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            guides: Vec::new(),
        },
        numbers: 0,
        notes: 0,
        blocks: 0,
        block_ids: Vec::new(),
    };
    builder.items(&sequence.items);
    builder.finish(head_height)
}

impl Builder<'_> {
    fn items(&mut self, items: &[SequenceItem]) {
        for (at, item) in items.iter().enumerate() {
            match item {
                SequenceItem::Message(message) => {
                    let opens_target = matches!(
                        items.get(at + 1),
                        Some(SequenceItem::Activate { participant }) if *participant == message.to
                    );
                    self.message(message, opens_target);
                }
                SequenceItem::Note(note) => self.note(note),
                SequenceItem::Activate { participant } => {
                    let column = self.column[participant.as_str()];
                    let level = self.active[column].len();
                    let start = self.last_message.unwrap_or(self.cursor);
                    self.active[column].push(Activation { start, level });
                }
                SequenceItem::Deactivate { participant } => {
                    let column = self.column[participant.as_str()];
                    if let Some(activation) = self.active[column].pop() {
                        let end = self.last_message.unwrap_or(self.cursor);
                        self.bar(column, activation, end);
                    }
                }
                SequenceItem::Block(block) => self.block(block),
            }
        }
    }

    // The side of the topmost activation bar that faces `toward`, or the lifeline itself.
    fn edge_of(&self, column: usize, toward: f32, extra_level: bool) -> f32 {
        let levels = self.active[column].len() + usize::from(extra_level);
        let center = self.centers[column];
        match levels {
            0 => center,
            count => {
                center + (count - 1) as f32 * ACTIVATION_STEP + toward.signum() * ACTIVATION_HALF
            }
        }
    }

    fn include(&mut self, left: f32, right: f32) {
        for extent in &mut self.open {
            extent.include(left, right);
        }
    }

    fn message(&mut self, message: &Message, opens_target: bool) {
        let (from, to) = (
            self.column[message.from.as_str()],
            self.column[message.to.as_str()],
        );
        let text = message_text(message);
        let label_height = if text.lines.is_empty() {
            0.0
        } else {
            text.height + 2.0 * LABEL_PAD_Y
        };
        let top = self.cursor;
        let line = top + label_height + LABEL_GAP;
        let (points, label_left, end) = if from == to {
            let x = self.edge_of(from, 1.0, false);
            let bottom = line + SELF_DROP;
            let points = vec![
                Point { x, y: line },
                Point {
                    x: x + SELF_REACH,
                    y: line,
                },
                Point {
                    x: x + SELF_REACH,
                    y: bottom,
                },
                Point { x, y: bottom },
            ];
            (points, x + LABEL_GAP + number_room(self.autonumber), bottom)
        } else {
            let toward = self.centers[to] - self.centers[from];
            let start = self.edge_of(from, toward, false);
            let finish = self.edge_of(to, -toward, opens_target);
            let points = vec![Point { x: start, y: line }, Point { x: finish, y: line }];
            (
                points,
                (start + finish) / 2.0 - text.width / 2.0 - LABEL_PAD_X,
                line,
            )
        };
        let label = (!text.lines.is_empty()).then(|| {
            let rect = Rect {
                x: label_left,
                y: top,
                width: text.width + 2.0 * LABEL_PAD_X,
                height: label_height,
            };
            EdgeLabel {
                texts: text.centered(rect.x + rect.width / 2.0, rect.y + LABEL_PAD_Y),
                rect,
            }
        });
        let left = points
            .iter()
            .map(|point| point.x)
            .fold(f32::INFINITY, f32::min);
        let right = points
            .iter()
            .map(|point| point.x)
            .fold(f32::NEG_INFINITY, f32::max);
        match &label {
            Some(label) => self.include(left.min(label.rect.x), right.max(label.rect.right())),
            None => self.include(left, right),
        }
        if self.autonumber {
            self.numbers += 1;
            let center = points[0];
            self.scene.nodes.push(NodeBox {
                id: format!("number {}", self.numbers),
                parent: None,
                outline: Outline::Circle,
                rect: Rect {
                    x: center.x - NUMBER_RADIUS,
                    y: center.y - NUMBER_RADIUS,
                    width: 2.0 * NUMBER_RADIUS,
                    height: 2.0 * NUMBER_RADIUS,
                },
                texts: block(&[self.numbers.to_string()], NUMBER, f32::INFINITY)
                    .centered(center.x, center.y - NUMBER.line_height() / 2.0),
                icon: None,
                classes: vec!["sequence-number".to_owned()],
                link: None,
            });
        }
        self.scene.edges.push(EdgePath {
            from: message.from.clone(),
            to: message.to.clone(),
            points,
            stroke: message.stroke,
            tail: message.tail,
            head: message.head,
            label,
        });
        self.last_message = Some(end);
        self.cursor = end + ROW_GAP;
    }

    fn note(&mut self, note: &Note) {
        let text = note_text(note);
        let width = note_width(&text);
        let height = text.height + 2.0 * NOTE_PAD_Y;
        let (left, width) = match &note.placement {
            NotePlacement::LeftOf(at) => {
                let column = self.column[at.as_str()];
                (
                    self.edge_of(column, -1.0, false) - NOTE_OFFSET - width,
                    width,
                )
            }
            NotePlacement::RightOf(at) => {
                let column = self.column[at.as_str()];
                (self.edge_of(column, 1.0, false) + NOTE_OFFSET, width)
            }
            NotePlacement::Over(at) => {
                (self.centers[self.column[at.as_str()]] - width / 2.0, width)
            }
            NotePlacement::Spanning(first, last) => {
                let a = self.centers[self.column[first.as_str()]];
                let b = self.centers[self.column[last.as_str()]];
                let (low, high) = (a.min(b) - NOTE_OVERHANG, a.max(b) + NOTE_OVERHANG);
                let span = width.max(high - low);
                ((low + high - span) / 2.0, span)
            }
        };
        let rect = Rect {
            x: left,
            y: self.cursor,
            width,
            height,
        };
        self.include(rect.x, rect.right());
        self.notes += 1;
        self.scene.nodes.push(NodeBox {
            id: format!("note {}", self.notes),
            parent: None,
            outline: Outline::Note,
            rect,
            texts: text.centered(rect.x + rect.width / 2.0, rect.y + NOTE_PAD_Y),
            icon: None,
            classes: vec!["note".to_owned()],
            link: None,
        });
        self.last_message = None;
        self.cursor = rect.bottom() + ROW_GAP;
    }

    fn block(&mut self, block: &Block) {
        self.blocks += 1;
        let id = format!("block {}", self.blocks);
        let parent = self.block_ids.last().cloned();
        let depth = self.block_ids.len();
        self.block_ids.push(id.clone());
        let top = self.cursor;
        let operator = operator_name(block.operator);
        let tag = block_text(&[operator.to_owned()], true);
        let first_label = block
            .sections
            .first()
            .map(|section| bracketed(&section.label))
            .unwrap_or_default();
        let first = block_text(&first_label, false);
        let header = tag.height.max(first.height) + 2.0 * BLOCK_HEADER_GAP;
        self.open.push(Extent::EMPTY);
        self.cursor = top + header;
        self.last_message = None;
        let mut dividers = Vec::new();
        for (at, section) in block.sections.iter().enumerate() {
            if at > 0 {
                let label = block_text(&bracketed(&section.label), false);
                dividers.push((self.cursor, label));
                self.cursor +=
                    dividers.last().map_or(0.0, |(_, label)| label.height) + 2.0 * BLOCK_HEADER_GAP;
                self.last_message = None;
            }
            self.items(&section.items);
        }
        let extent = self.open.pop().unwrap_or(Extent::EMPTY);
        let extent = if extent.left.is_finite() {
            extent
        } else {
            self.all_columns()
        };
        let least = tag.width + first.width + 3.0 * BLOCK_PAD;
        let left = extent.left - BLOCK_PAD;
        let right = (extent.right + BLOCK_PAD).max(left + least);
        let bottom = self.cursor - ROW_GAP + BLOCK_PAD;
        let rect = Rect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        };
        let mut texts = tag.left_aligned(left + BLOCK_PAD, top + BLOCK_HEADER_GAP);
        texts
            .extend(first.left_aligned(left + 2.0 * BLOCK_PAD + tag.width, top + BLOCK_HEADER_GAP));
        for (y, label) in dividers {
            self.scene.guides.push(Guide {
                from: Point { x: left, y },
                to: Point { x: right, y },
                kind: GuideKind::Divider,
            });
            texts.extend(label.left_aligned(left + BLOCK_PAD, y + BLOCK_HEADER_GAP));
        }
        self.block_ids.pop();
        self.include(left, right);
        self.scene.clusters.push(ClusterBox {
            id,
            parent,
            depth,
            rect,
            texts,
            icon: None,
            classes: vec!["block".to_owned(), format!("block-{operator}")],
            link: None,
        });
        self.last_message = None;
        self.cursor = bottom + ROW_GAP;
    }

    fn all_columns(&self) -> Extent {
        let mut extent = Extent::EMPTY;
        for (center, head) in self.centers.iter().zip(&self.heads) {
            extent.include(center - head.width / 2.0, center + head.width / 2.0);
        }
        extent
    }

    fn bar(&mut self, column: usize, activation: Activation, end: f32) {
        let end = end.max(activation.start + ACTIVATION_LEAST);
        let center = self.centers[column] + activation.level as f32 * ACTIVATION_STEP;
        self.scene.nodes.push(NodeBox {
            id: self.participants[column].id.clone(),
            parent: None,
            outline: Outline::Rectangle,
            rect: Rect {
                x: center - ACTIVATION_HALF,
                y: activation.start,
                width: 2.0 * ACTIVATION_HALF,
                height: end - activation.start,
            },
            texts: Vec::new(),
            icon: None,
            classes: vec!["activation".to_owned()],
            link: None,
        });
    }

    fn finish(mut self, head_height: f32) -> Scene {
        let last_row = self.cursor - ROW_GAP;
        for column in 0..self.active.len() {
            while let Some(activation) = self.active[column].pop() {
                self.bar(column, activation, last_row);
            }
        }
        let bottom_top = last_row + HEAD_GAP;
        for (column, participant) in self.participants.iter().enumerate() {
            let head = &self.heads[column];
            let center = self.centers[column];
            self.scene.guides.push(Guide {
                from: Point {
                    x: center,
                    y: head.height,
                },
                to: Point {
                    x: center,
                    y: bottom_top,
                },
                kind: GuideKind::Lifeline,
            });
            self.scene.nodes.push(head.node(participant, center, 0.0));
            self.scene
                .nodes
                .push(head.node(participant, center, bottom_top));
        }
        let mut extent = self.all_columns();
        for node in &self.scene.nodes {
            extent.include(node.rect.x, node.rect.right());
        }
        for cluster in &self.scene.clusters {
            extent.include(cluster.rect.x, cluster.rect.right());
        }
        for edge in &self.scene.edges {
            for point in &edge.points {
                extent.include(point.x, point.x);
            }
            if let Some(label) = &edge.label {
                extent.include(label.rect.x, label.rect.right());
            }
        }
        let (left, right) = if extent.left.is_finite() {
            (extent.left, extent.right)
        } else {
            (0.0, 0.0)
        };
        let mut scene = self.scene;
        scene.translate(MARGIN - left, MARGIN);
        scene.width = right - left + 2.0 * MARGIN;
        scene.height = bottom_top + head_height + 2.0 * MARGIN;
        scene
    }
}

// A numbered message carries its number on its first point, so the label of a message to itself
// starts after it.
fn number_room(autonumber: bool) -> f32 {
    if autonumber { NUMBER_RADIUS } else { 0.0 }
}

fn operator_name(operator: Operator) -> &'static str {
    match operator {
        Operator::Loop => "loop",
        Operator::Alt => "alt",
        Operator::Opt => "opt",
        Operator::Par => "par",
        Operator::Critical => "critical",
        Operator::Break => "break",
    }
}

fn bracketed(label: &[String]) -> Vec<String> {
    match label {
        [] => Vec::new(),
        [only] => vec![format!("[{only}]")],
        [first, middle @ .., last] => std::iter::once(format!("[{first}"))
            .chain(middle.iter().cloned())
            .chain(std::iter::once(format!("{last}]")))
            .collect(),
    }
}

fn block_text(lines: &[String], operator: bool) -> Lines {
    block(lines, if operator { HEADER } else { BLOCK_LABEL }, MAX_TEXT)
}
