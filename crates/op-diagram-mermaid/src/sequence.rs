use op_diagram::{
    Block, Head, Message, Note, NotePlacement, Operator, Participant, ParticipantKind, Section,
    Sequence, SequenceItem, Stroke,
};

use crate::ParseError;
use crate::source::{Cursor, Line};
use crate::text::label;

const UNSUPPORTED: [&str; 10] = [
    "box",
    "create",
    "destroy",
    "title",
    "link",
    "links",
    "properties",
    "details",
    "accTitle",
    "accDescr",
];

// Longest first, so `-->>` is not read as `-->` and a stray `>`.
const ARROWS: [(&str, Stroke, Head, Head); 10] = [
    ("<<-->>", Stroke::Dotted, Head::Arrow, Head::Arrow),
    ("<<->>", Stroke::Solid, Head::Arrow, Head::Arrow),
    ("-->>", Stroke::Dotted, Head::None, Head::Arrow),
    ("--x", Stroke::Dotted, Head::None, Head::Cross),
    ("--)", Stroke::Dotted, Head::None, Head::OpenArrow),
    ("-->", Stroke::Dotted, Head::None, Head::None),
    ("->>", Stroke::Solid, Head::None, Head::Arrow),
    ("-x", Stroke::Solid, Head::None, Head::Cross),
    ("-)", Stroke::Solid, Head::None, Head::OpenArrow),
    ("->", Stroke::Solid, Head::None, Head::None),
];

const FORBIDDEN_IN_ID: [char; 5] = ['-', '<', '>', ':', ','];

pub(crate) fn parse(mut header: Cursor<'_>, body: &[Line<'_>]) -> Result<Sequence, ParseError> {
    header.skip_spaces();
    if !header.at_end() {
        return Err(header.error("expected a new line after `sequenceDiagram`"));
    }
    let mut chart = Chart::default();
    for line in body {
        let mut cursor = Cursor::new(line);
        loop {
            let end = cursor.offset() + cursor.rest().find(';').unwrap_or(cursor.rest().len());
            chart.statement(&mut cursor.until(end))?;
            cursor.advance(end - cursor.offset() + 1);
            if cursor.at_end() {
                break;
            }
        }
    }
    chart.finish()
}

#[derive(Default)]
struct Chart {
    sequence: Sequence,
    open: Vec<Frame>,
}

// A `rect` only colors the messages it holds. The color is not drawn, so its messages join the
// block around it when it closes.
struct Frame {
    operator: Option<Operator>,
    sections: Vec<Section>,
    error: ParseError,
}

impl Chart {
    fn statement(&mut self, cursor: &mut Cursor<'_>) -> Result<(), ParseError> {
        cursor.skip_spaces();
        if cursor.at_end() {
            return Ok(());
        }
        let start = cursor.offset();
        let word = cursor
            .rest()
            .split(|c: char| c.is_whitespace() || c == ':')
            .next()
            .unwrap_or_default();
        if word.eq_ignore_ascii_case("note") {
            cursor.advance(word.len());
            return self.note(cursor);
        }
        if UNSUPPORTED.contains(&word) {
            return Err(cursor.error(format!("`{word}` is not supported")));
        }
        let operator = match word {
            "loop" => Some(Operator::Loop),
            "alt" => Some(Operator::Alt),
            "opt" => Some(Operator::Opt),
            "par" => Some(Operator::Par),
            "critical" => Some(Operator::Critical),
            "break" => Some(Operator::Break),
            _ => None,
        };
        let keyword = operator.is_some()
            || matches!(
                word,
                "participant"
                    | "actor"
                    | "autonumber"
                    | "activate"
                    | "deactivate"
                    | "else"
                    | "and"
                    | "option"
                    | "rect"
                    | "end"
            );
        if !keyword {
            return self.message(cursor);
        }
        cursor.advance(word.len());
        cursor.skip_spaces();
        let rest = cursor.rest().trim_end();
        match word {
            "participant" => self.declare(cursor, ParticipantKind::Box),
            "actor" => self.declare(cursor, ParticipantKind::Actor),
            "autonumber" if rest.is_empty() => {
                self.sequence.autonumber = true;
                Ok(())
            }
            "autonumber" => Err(cursor.error("only a bare `autonumber` is supported")),
            "activate" | "deactivate" => {
                let participant = self.participant(cursor, rest)?;
                self.items().push(match word {
                    "activate" => SequenceItem::Activate { participant },
                    _ => SequenceItem::Deactivate { participant },
                });
                Ok(())
            }
            "else" => self.section(cursor, start, Operator::Alt, "else", rest),
            "and" => self.section(cursor, start, Operator::Par, "and", rest),
            "option" => self.section(cursor, start, Operator::Critical, "option", rest),
            "rect" => {
                self.open_block(
                    None,
                    Vec::new(),
                    cursor.error_at(start, "this `rect` has no `end`"),
                );
                Ok(())
            }
            "end" => self.close(cursor, start),
            _ => {
                self.open_block(
                    operator,
                    label_or_none(rest),
                    cursor.error_at(start, format!("this `{word}` has no `end`")),
                );
                Ok(())
            }
        }
    }

    fn open_block(&mut self, operator: Option<Operator>, label: Vec<String>, error: ParseError) {
        self.open.push(Frame {
            operator,
            sections: vec![Section {
                label,
                items: Vec::new(),
            }],
            error,
        });
    }

    fn section(
        &mut self,
        cursor: &Cursor<'_>,
        start: usize,
        operator: Operator,
        word: &str,
        rest: &str,
    ) -> Result<(), ParseError> {
        match self.open.last_mut() {
            Some(frame) if frame.operator == Some(operator) => {
                frame.sections.push(Section {
                    label: label_or_none(rest),
                    items: Vec::new(),
                });
                Ok(())
            }
            _ => Err(cursor.error_at(
                start,
                format!(
                    "`{word}` belongs inside `{}`",
                    match operator {
                        Operator::Alt => "alt",
                        Operator::Par => "par",
                        _ => "critical",
                    }
                ),
            )),
        }
    }

    fn close(&mut self, cursor: &Cursor<'_>, start: usize) -> Result<(), ParseError> {
        let Some(frame) = self.open.pop() else {
            return Err(cursor.error_at(start, "this `end` closes no block"));
        };
        match frame.operator {
            Some(operator) => self.items().push(SequenceItem::Block(Block {
                operator,
                sections: frame.sections,
            })),
            None => {
                let held = frame.sections.into_iter().flat_map(|section| section.items);
                self.items().extend(held);
            }
        }
        Ok(())
    }

    fn items(&mut self) -> &mut Vec<SequenceItem> {
        match self.open.last_mut() {
            Some(frame) => {
                &mut frame
                    .sections
                    .last_mut()
                    .expect("a block opens with one section")
                    .items
            }
            None => &mut self.sequence.items,
        }
    }

    // `participant a as Alice` gives the id `a` the label `Alice`. A name may hold spaces, as in
    // Mermaid, so `as` is found as a word of its own.
    fn declare(&mut self, cursor: &Cursor<'_>, kind: ParticipantKind) -> Result<(), ParseError> {
        let rest = cursor.rest().trim_end();
        let words: Vec<&str> = rest.split_whitespace().collect();
        let (id, text) = match words.iter().skip(1).position(|word| *word == "as") {
            Some(at) => {
                let alias = rest
                    .split_whitespace()
                    .skip(at + 2)
                    .collect::<Vec<_>>()
                    .join(" ");
                (words[..=at].join(" "), alias)
            }
            None => (words.join(" "), String::new()),
        };
        let id = self.participant(cursor, &id)?;
        let at = self
            .sequence
            .participants
            .iter()
            .position(|participant| participant.id == id)
            .expect("`participant` adds the id");
        let declared = &mut self.sequence.participants[at];
        declared.kind = kind;
        if !text.is_empty() {
            declared.label = label(&text);
        }
        Ok(())
    }

    fn participant(&mut self, cursor: &Cursor<'_>, raw: &str) -> Result<String, ParseError> {
        let id = raw.trim();
        if id.is_empty() {
            return Err(cursor.error("expected a participant"));
        }
        if let Some(bad) = id.chars().find(|c| FORBIDDEN_IN_ID.contains(c)) {
            return Err(cursor.error(format!("the participant `{id}` cannot hold `{bad}`")));
        }
        if !self
            .sequence
            .participants
            .iter()
            .any(|known| known.id == id)
        {
            self.sequence.participants.push(Participant {
                id: id.to_owned(),
                label: vec![id.to_owned()],
                kind: ParticipantKind::Box,
            });
        }
        Ok(id.to_owned())
    }

    fn note(&mut self, cursor: &mut Cursor<'_>) -> Result<(), ParseError> {
        cursor.skip_spaces();
        let side_at = cursor.offset();
        let side = cursor
            .take_while(|c| c.is_alphabetic())
            .to_ascii_lowercase();
        cursor.skip_spaces();
        let needs_of = side == "left" || side == "right";
        if needs_of && !cursor.keyword("of") {
            return Err(cursor.error(format!("expected `of` after `{side}`")));
        }
        if !needs_of && side != "over" {
            return Err(cursor.error_at(side_at, "expected `left of`, `right of`, or `over`"));
        }
        let rest = cursor.rest();
        let Some(colon) = rest.find(':') else {
            return Err(cursor.error("expected `:` and the text of the note"));
        };
        let names: Vec<&str> = rest[..colon].split(',').collect();
        let text = label(&rest[colon + 1..]);
        let placement = match (side.as_str(), names.as_slice()) {
            ("left", [one]) => NotePlacement::LeftOf(self.participant(cursor, one)?),
            ("right", [one]) => NotePlacement::RightOf(self.participant(cursor, one)?),
            ("over", [one]) => NotePlacement::Over(self.participant(cursor, one)?),
            ("over", [first, last]) => NotePlacement::Spanning(
                self.participant(cursor, first)?,
                self.participant(cursor, last)?,
            ),
            _ => {
                return Err(cursor
                    .error("a note names one participant, or two after `over`, as in `over A,B`"));
            }
        };
        self.items()
            .push(SequenceItem::Note(Note { placement, text }));
        Ok(())
    }

    // `+` after the arrow starts the activation of the receiver; `-` ends the one of the sender.
    fn message(&mut self, cursor: &mut Cursor<'_>) -> Result<(), ParseError> {
        let start = cursor.offset();
        let rest = cursor.rest();
        let expected = "expected a message such as `A->>B: text`";
        let Some(arrow_at) = rest.find(['-', '<']) else {
            return Err(cursor.error_at(start, expected));
        };
        let from = rest[..arrow_at].trim();
        if from.is_empty() {
            return Err(cursor.error_at(start, expected));
        }
        cursor.advance(arrow_at);
        let Some((arrow, stroke, tail, head)) = ARROWS
            .iter()
            .find(|(arrow, ..)| cursor.starts_with(arrow))
            .copied()
        else {
            return Err(cursor.error("this is not a message arrow; use one such as `->>`"));
        };
        cursor.advance(arrow.len());
        let activates = cursor.eat("+");
        let deactivates = !activates && cursor.eat("-");
        let rest = cursor.rest();
        let (to, text) = match rest.find(':') {
            Some(colon) => (&rest[..colon], label(&rest[colon + 1..])),
            None => (rest, Vec::new()),
        };
        let from = self.participant(cursor, from)?;
        let to = self.participant(cursor, to)?;
        let items = self.items();
        items.push(SequenceItem::Message(Message {
            from: from.clone(),
            to: to.clone(),
            text,
            stroke,
            tail,
            head,
        }));
        if activates {
            items.push(SequenceItem::Activate { participant: to });
        }
        if deactivates {
            items.push(SequenceItem::Deactivate { participant: from });
        }
        Ok(())
    }

    fn finish(self) -> Result<Sequence, ParseError> {
        match self.open.last() {
            Some(frame) => Err(frame.error.clone()),
            None => Ok(self.sequence),
        }
    }
}

fn label_or_none(rest: &str) -> Vec<String> {
    match rest.trim() {
        "" => Vec::new(),
        text => label(text),
    }
}
