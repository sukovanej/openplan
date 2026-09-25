mod common;

use common::{error, sequence};
use op_diagram::{
    Block, Head, Message, Note, NotePlacement, Operator, ParticipantKind, Section, SequenceItem,
    Stroke,
};

fn message(from: &str, to: &str, text: &str) -> SequenceItem {
    SequenceItem::Message(Message {
        from: from.into(),
        to: to.into(),
        text: if text.is_empty() {
            vec![]
        } else {
            vec![text.into()]
        },
        stroke: Stroke::Solid,
        tail: Head::None,
        head: Head::Arrow,
    })
}

fn participants(source: &str) -> Vec<(String, Vec<String>, ParticipantKind)> {
    sequence(source)
        .participants
        .into_iter()
        .map(|participant| (participant.id, participant.label, participant.kind))
        .collect()
}

#[test]
fn lists_the_participants_in_the_order_they_first_appear() {
    assert_eq!(
        participants(
            "sequenceDiagram
  participant b as Bob
  a->>b: hi
  actor c
  b->>c: hello"
        ),
        [
            ("b".into(), vec!["Bob".into()], ParticipantKind::Box),
            ("a".into(), vec!["a".into()], ParticipantKind::Box),
            ("c".into(), vec!["c".into()], ParticipantKind::Actor),
        ]
    );
}

#[test]
fn a_late_declaration_keeps_the_place_and_sets_the_label_and_the_kind() {
    assert_eq!(
        participants(
            "sequenceDiagram
  a->>b: hi
  actor a as Alice"
        ),
        [
            ("a".into(), vec!["Alice".into()], ParticipantKind::Actor),
            ("b".into(), vec!["b".into()], ParticipantKind::Box),
        ]
    );
}

#[test]
fn a_participant_name_may_hold_spaces() {
    let sequence = sequence(
        "sequenceDiagram
  participant Game server as The server
  Game server->>db: store",
    );
    assert_eq!(sequence.participants[0].id, "Game server");
    assert_eq!(sequence.participants[0].label, ["The server"]);
    assert_eq!(sequence.items, [message("Game server", "db", "store")]);
}

#[test]
fn reads_every_arrow() {
    for (arrow, stroke, tail, head) in [
        ("->>", Stroke::Solid, Head::None, Head::Arrow),
        ("-->>", Stroke::Dotted, Head::None, Head::Arrow),
        ("->", Stroke::Solid, Head::None, Head::None),
        ("-->", Stroke::Dotted, Head::None, Head::None),
        ("-x", Stroke::Solid, Head::None, Head::Cross),
        ("--x", Stroke::Dotted, Head::None, Head::Cross),
        ("-)", Stroke::Solid, Head::None, Head::OpenArrow),
        ("--)", Stroke::Dotted, Head::None, Head::OpenArrow),
        ("<<->>", Stroke::Solid, Head::Arrow, Head::Arrow),
        ("<<-->>", Stroke::Dotted, Head::Arrow, Head::Arrow),
    ] {
        let sequence = sequence(&format!("sequenceDiagram\n  a{arrow}b: text"));
        assert_eq!(
            sequence.items,
            [SequenceItem::Message(Message {
                from: "a".into(),
                to: "b".into(),
                text: vec!["text".into()],
                stroke,
                tail,
                head,
            })],
            "{arrow}"
        );
    }
}

#[test]
fn a_message_may_go_to_its_sender_and_may_have_no_text() {
    let sequence = sequence("sequenceDiagram\n  a->>a: think\n  a->>b");
    assert_eq!(
        sequence.items,
        [message("a", "a", "think"), message("a", "b", "")]
    );
}

#[test]
fn the_text_keeps_arrows_colons_and_unicode() {
    let sequence = sequence("sequenceDiagram\n  d->>m: „Cože? uranu-238 -> 4,5 : miliardy“");
    assert_eq!(
        sequence.items,
        [message("d", "m", "„Cože? uranu-238 -> 4,5 : miliardy“")]
    );
}

#[test]
fn a_plus_activates_the_receiver_and_a_minus_deactivates_the_sender() {
    let sequence = sequence("sequenceDiagram\n  a->>+b: ask\n  b-->>-a: answer");
    let SequenceItem::Message(answer) = &sequence.items[2] else {
        panic!("expected the answer, got {:?}", sequence.items[2]);
    };
    assert_eq!((answer.from.as_str(), answer.to.as_str()), ("b", "a"));
    assert_eq!(
        sequence.items[1],
        SequenceItem::Activate {
            participant: "b".into()
        }
    );
    assert_eq!(
        sequence.items[3],
        SequenceItem::Deactivate {
            participant: "b".into()
        }
    );
}

#[test]
fn reads_activate_and_deactivate_statements() {
    let sequence = sequence("sequenceDiagram\n  activate a\n  deactivate a");
    assert_eq!(
        sequence.items,
        [
            SequenceItem::Activate {
                participant: "a".into()
            },
            SequenceItem::Deactivate {
                participant: "a".into()
            },
        ]
    );
}

#[test]
fn reads_each_placement_of_a_note() {
    let sequence = sequence(
        "sequenceDiagram
  Note left of a: one
  note right of b: two
  Note over a: three
  Note over a, b: four<br>lines",
    );
    let note = |placement, text: &[&str]| {
        SequenceItem::Note(Note {
            placement,
            text: text.iter().map(|line| (*line).to_owned()).collect(),
        })
    };
    assert_eq!(
        sequence.items,
        [
            note(NotePlacement::LeftOf("a".into()), &["one"]),
            note(NotePlacement::RightOf("b".into()), &["two"]),
            note(NotePlacement::Over("a".into()), &["three"]),
            note(
                NotePlacement::Spanning("a".into(), "b".into()),
                &["four", "lines"]
            ),
        ]
    );
}

#[test]
fn an_alt_holds_a_section_for_each_else() {
    let sequence = sequence(
        "sequenceDiagram
  alt is sick
    b->>a: not so good
  else is well
    b->>a: fine
  else
    b->>a: hm
  end",
    );
    assert_eq!(
        sequence.items,
        [SequenceItem::Block(Block {
            operator: Operator::Alt,
            sections: vec![
                Section {
                    label: vec!["is sick".into()],
                    items: vec![message("b", "a", "not so good")],
                },
                Section {
                    label: vec!["is well".into()],
                    items: vec![message("b", "a", "fine")],
                },
                Section {
                    label: vec![],
                    items: vec![message("b", "a", "hm")],
                },
            ],
        })]
    );
}

#[test]
fn reads_each_block_and_its_sections() {
    for (source, operator, sections) in [
        ("loop every minute\n a->>b\nend", Operator::Loop, 1),
        ("opt maybe\n a->>b\nend", Operator::Opt, 1),
        ("par one\n a->>b\nand two\n a->>c\nend", Operator::Par, 2),
        (
            "critical connect\n a->>b\noption timeout\n a->>c\nend",
            Operator::Critical,
            2,
        ),
        ("break on error\n a->>b\nend", Operator::Break, 1),
    ] {
        let sequence = sequence(&format!("sequenceDiagram\n{source}"));
        let [SequenceItem::Block(block)] = sequence.items.as_slice() else {
            panic!(
                "expected one block from {source:?}, got {:?}",
                sequence.items
            );
        };
        assert_eq!(
            (block.operator, block.sections.len()),
            (operator, sections),
            "{source}"
        );
    }
}

#[test]
fn blocks_nest() {
    let sequence = sequence(
        "sequenceDiagram
  loop daily
    opt if new
      a->>b: sync
    end
  end",
    );
    let [SequenceItem::Block(outer)] = sequence.items.as_slice() else {
        panic!("expected one block, got {:?}", sequence.items);
    };
    let [SequenceItem::Block(inner)] = outer.sections[0].items.as_slice() else {
        panic!("expected a nested block, got {:?}", outer.sections[0].items);
    };
    assert_eq!(inner.operator, Operator::Opt);
    assert_eq!(inner.sections[0].items, [message("a", "b", "sync")]);
}

#[test]
fn a_rect_passes_its_messages_to_the_block_around_it() {
    let sequence = sequence(
        "sequenceDiagram
  loop daily
    rect rgb(200, 150, 255)
      a->>b: one
    end
    a->>b: two
  end",
    );
    let [SequenceItem::Block(block)] = sequence.items.as_slice() else {
        panic!("expected one block, got {:?}", sequence.items);
    };
    assert_eq!(
        block.sections[0].items,
        [message("a", "b", "one"), message("a", "b", "two")]
    );
}

#[test]
fn reads_autonumber() {
    assert!(sequence("sequenceDiagram\n  autonumber\n  a->>b").autonumber);
    assert!(!sequence("sequenceDiagram\n  a->>b").autonumber);
}

#[test]
fn a_semicolon_splits_the_statements_of_one_line() {
    let sequence = sequence("sequenceDiagram\n  a->>b: one; b->>a: two");
    assert_eq!(
        sequence.items,
        [message("a", "b", "one"), message("b", "a", "two")]
    );
}

#[test]
fn refuses_a_section_outside_its_block() {
    assert_eq!(
        error("sequenceDiagram\n  else x").message,
        "`else` belongs inside `alt`"
    );
    assert_eq!(
        error("sequenceDiagram\n  loop x\n  and y\n  end").message,
        "`and` belongs inside `par`"
    );
    assert_eq!(
        error("sequenceDiagram\n  option y").message,
        "`option` belongs inside `critical`"
    );
}

#[test]
fn refuses_an_end_with_no_block() {
    let refused = error("sequenceDiagram\n  a->>b\n  end");
    assert_eq!((refused.line, refused.column), (3, 3));
    assert_eq!(refused.message, "this `end` closes no block");
}

#[test]
fn refuses_a_block_with_no_end_at_the_block() {
    let refused = error("sequenceDiagram\n  a->>b\n  loop forever\n    a->>b");
    assert_eq!((refused.line, refused.column), (3, 3));
    assert_eq!(refused.message, "this `loop` has no `end`");
}

#[test]
fn names_what_it_does_not_support() {
    for word in ["box", "create", "destroy", "title", "accTitle"] {
        assert_eq!(
            error(&format!("sequenceDiagram\n  {word} something")).message,
            format!("`{word}` is not supported")
        );
    }
    assert_eq!(
        error("sequenceDiagram\n  autonumber 10 5").message,
        "only a bare `autonumber` is supported"
    );
}

#[test]
fn refuses_what_is_not_a_message() {
    assert_eq!(
        error("sequenceDiagram\n  a=>b: text").message,
        "expected a message such as `A->>B: text`"
    );
    let arrow = error("sequenceDiagram\n  a-~b: text");
    assert_eq!((arrow.line, arrow.column), (2, 4));
    assert_eq!(
        arrow.message,
        "this is not a message arrow; use one such as `->>`"
    );
}

#[test]
fn refuses_a_participant_that_mermaid_cannot_name() {
    assert_eq!(
        error("sequenceDiagram\n  participant web-app").message,
        "the participant `web-app` cannot hold `-`"
    );
    assert_eq!(
        error("sequenceDiagram\n  a->>b-c: text").message,
        "the participant `b-c` cannot hold `-`"
    );
}

#[test]
fn refuses_a_note_it_cannot_place() {
    assert_eq!(
        error("sequenceDiagram\n  Note beside a: x").message,
        "expected `left of`, `right of`, or `over`"
    );
    assert_eq!(
        error("sequenceDiagram\n  Note left a: x").message,
        "expected `of` after `left`"
    );
    assert_eq!(
        error("sequenceDiagram\n  Note over a").message,
        "expected `:` and the text of the note"
    );
    assert_eq!(
        error("sequenceDiagram\n  Note left of a, b: x").message,
        "a note names one participant, or two after `over`, as in `over A,B`"
    );
}

#[test]
fn refuses_text_after_the_header() {
    assert_eq!(
        error("sequenceDiagram a->>b").message,
        "expected a new line after `sequenceDiagram`"
    );
}
