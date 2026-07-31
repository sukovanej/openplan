use std::collections::BTreeSet;

use op_claude::{ContentBlock, Record, StreamInput, StreamOutput};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

const TRANSCRIPTS: &[&str] = &[
    "session_basic.jsonl",
    "workflow_journal.jsonl",
    "record_types.jsonl",
    "tool_results.jsonl",
    "content_blocks.jsonl",
];

const STREAM_OUTPUTS: &[&str] = &["stream_output_basic.jsonl", "stream_output_partial.jsonl"];

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("reading {path}: {error}"))
}

fn numbered_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line))
        .filter(|(_, line)| !line.trim().is_empty())
}

fn tag(line: &str) -> String {
    serde_json::from_str::<Value>(line).expect("valid json")["type"]
        .as_str()
        .expect("record carries a type tag")
        .to_owned()
}

fn tags(name: &str) -> BTreeSet<String> {
    numbered_lines(&fixture(name))
        .map(|(_, line)| tag(line))
        .collect()
}

fn assert_round_trips<T: DeserializeOwned + Serialize>(name: &str) {
    let source = fixture(name);
    for (line_number, line) in numbered_lines(&source) {
        let original: Value = serde_json::from_str(line).expect("fixture line is valid json");
        let parsed: T = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("{name}:{line_number}: {error}"));
        let reserialised = serde_json::to_value(&parsed).expect("model serialises");

        assert_eq!(
            original, reserialised,
            "{name}:{line_number} lost or altered data on round trip"
        );
    }
}

#[test]
fn transcript_fixtures_round_trip_without_loss() {
    for name in TRANSCRIPTS {
        assert_round_trips::<Record>(name);
    }
}

#[test]
fn stream_output_fixtures_round_trip_without_loss() {
    for name in STREAM_OUTPUTS {
        assert_round_trips::<StreamOutput>(name);
    }
}

#[test]
fn stream_input_fixtures_round_trip_without_loss() {
    assert_round_trips::<StreamInput>("stream_input.jsonl");
}

// A round trip alone would still pass if every record degraded to the raw-value fallback, so pin
// the recognised shapes down separately.
#[test]
fn every_transcript_record_is_recognised() {
    for name in TRANSCRIPTS {
        for (line_number, line) in numbered_lines(&fixture(name)) {
            let record: Record = serde_json::from_str(line).unwrap();
            assert!(
                record.known().is_some(),
                "{name}:{line_number} fell back to Record::Unknown ({})",
                tag(line)
            );
        }
    }
}

#[test]
fn every_stream_output_is_recognised() {
    for name in STREAM_OUTPUTS {
        for (line_number, line) in numbered_lines(&fixture(name)) {
            let output: StreamOutput = serde_json::from_str(line).unwrap();
            assert!(
                output.known().is_some(),
                "{name}:{line_number} fell back to StreamOutput::Unknown ({})",
                tag(line)
            );
        }
    }
}

#[test]
fn every_stream_input_is_recognised() {
    for (line_number, line) in numbered_lines(&fixture("stream_input.jsonl")) {
        let input: StreamInput = serde_json::from_str(line).unwrap();
        assert!(
            input.known().is_some(),
            "stream_input.jsonl:{line_number} fell back to StreamInput::Unknown ({})",
            tag(line)
        );
    }
}

#[test]
fn record_types_fixture_covers_every_kind_seen_locally() {
    let kinds = tags("record_types.jsonl");

    let expected: BTreeSet<_> = [
        "ai-title",
        "assistant",
        "attachment",
        "custom-title",
        "file-history-delta",
        "file-history-snapshot",
        "frame-link",
        "last-prompt",
        "mode",
        "permission-mode",
        "pr-link",
        "queue-operation",
        "relocated",
        "result",
        "started",
        "system",
        "user",
        "worktree-state",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    assert_eq!(kinds, expected);
}

#[test]
fn stream_fixtures_cover_the_protocol_records() {
    let kinds: BTreeSet<String> = STREAM_OUTPUTS.iter().flat_map(|name| tags(name)).collect();

    for expected in [
        "system",
        "assistant",
        "user",
        "stream_event",
        "rate_limit_event",
        "result",
    ] {
        assert!(kinds.contains(expected), "missing {expected} in {kinds:?}");
    }
}

#[test]
fn content_block_kinds_are_recognised() {
    let mut kinds = BTreeSet::new();
    for name in ["content_blocks.jsonl", "record_types.jsonl"] {
        for (line_number, line) in numbered_lines(&fixture(name)) {
            let record: Record = serde_json::from_str(line).unwrap();
            for block in record
                .message()
                .map(|message| message.content.blocks())
                .unwrap_or_default()
            {
                assert!(
                    matches!(block, ContentBlock::Known(_)),
                    "{name}:{line_number} unrecognised content block: {block:?}"
                );
            }
            let blocks = serde_json::from_str::<Value>(line).unwrap()["message"]["content"].clone();
            for block in blocks.as_array().into_iter().flatten() {
                if let Some(kind) = block["type"].as_str() {
                    kinds.insert(kind.to_owned());
                }
            }
        }
    }

    for expected in ["text", "thinking", "tool_use", "tool_result", "image"] {
        assert!(kinds.contains(expected), "missing {expected} in {kinds:?}");
    }
}

// The transcript log and the CLI stream both use `type: "result"`, for unrelated shapes.
#[test]
fn a_journal_result_does_not_decode_as_a_stream_result() {
    let journal = fixture("workflow_journal.jsonl");
    let journal_result = journal
        .lines()
        .find(|line| line.contains(r#""type":"result""#))
        .expect("journal has a result record");

    let as_transcript: Record = serde_json::from_str(journal_result).expect("parses as transcript");
    assert!(as_transcript.known().is_some());

    let as_stream: StreamOutput = serde_json::from_str(journal_result).expect("never hard-fails");
    assert!(
        as_stream.known().is_none(),
        "a journal result must not decode as a stream result"
    );
}

// `result` is the collision that separates cleanly; `user` is the one that does not. Transcript turn
// records carry a snake_case `session_id` alongside the camelCase `sessionId`, which is enough to
// satisfy the stream model — so feeding a transcript to the stream parser yields plausible, wrong,
// *typed* records rather than the Unknown fallback. Pinned here so the hazard is recorded rather
// than rediscovered: pick the parser from the source of the bytes, never from a probe.
#[test]
fn transcript_user_records_alias_into_the_stream_model() {
    let source = fixture("tool_results.jsonl");
    let aliased = numbered_lines(&source)
        .filter(|(_, line)| {
            serde_json::from_str::<StreamOutput>(line)
                .ok()
                .and_then(|output| output.known().map(|_| ()))
                .is_some()
        })
        .count();

    assert!(
        aliased > 0,
        "expected transcript user records to alias into KnownStreamOutput::User"
    );
}

#[test]
fn unknown_records_survive_a_round_trip() {
    let line = r#"{"type":"some-future-record","sessionId":"s1","payload":{"nested":[1,2]}}"#;
    let record: Record = serde_json::from_str(line).expect("unknown types never fail to parse");

    let Record::Unknown(raw) = &record else {
        panic!("an unmodelled type must not decode as a known record");
    };
    assert_eq!(raw["type"], "some-future-record");
    assert_eq!(
        serde_json::to_value(&record).unwrap(),
        serde_json::from_str::<Value>(line).unwrap()
    );
}

#[test]
fn unknown_stream_input_survives_a_round_trip() {
    let line = r#"{"type":"control_cancel_request","request_id":"r1"}"#;
    let input: StreamInput = serde_json::from_str(line).expect("unknown input types never fail");

    let StreamInput::Unknown(raw) = &input else {
        panic!("an unmodelled type must not decode as a known input");
    };
    assert_eq!(raw["type"], "control_cancel_request");
    assert_eq!(
        serde_json::to_value(&input).unwrap(),
        serde_json::from_str::<Value>(line).unwrap()
    );
}

#[test]
fn unknown_fields_on_known_records_survive_a_round_trip() {
    let line = r#"{"type":"ai-title","sessionId":"s1","aiTitle":"t","futureField":{"a":1}}"#;
    let record: Record = serde_json::from_str(line).unwrap();

    assert!(record.known().is_some());
    assert_eq!(
        serde_json::to_value(&record).unwrap(),
        serde_json::from_str::<Value>(line).unwrap()
    );
}

// Absent and explicitly-null are different bytes on the wire, and the same field arrives both ways
// across a single session; every modelled optional has to keep them apart in both directions.
#[test]
fn explicit_nulls_are_preserved_on_modelled_fields() {
    let cases = [
        r#"{"type":"assistant","uuid":"u","sessionId":"s","timestamp":"t","parentUuid":null,"isSidechain":null,"cwd":null,"message":{"role":"assistant","content":[],"stop_reason":null,"stop_sequence":null,"usage":null},"requestId":null}"#,
        r#"{"type":"system","uuid":"u","sessionId":"s","timestamp":"t","subtype":"x","content":null}"#,
        r#"{"type":"last-prompt","sessionId":"s","leafUuid":"l","lastPrompt":null}"#,
        r#"{"type":"user","uuid":"u","sessionId":"s","timestamp":"t","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":null,"is_error":null}]},"toolUseResult":null}"#,
    ];

    for line in cases {
        let record: Record = serde_json::from_str(line).unwrap();
        assert!(record.known().is_some(), "should be a known record: {line}");
        assert_eq!(
            serde_json::to_value(&record).unwrap(),
            serde_json::from_str::<Value>(line).unwrap(),
            "explicit nulls were dropped: {line}"
        );
    }
}

#[test]
fn absent_fields_stay_absent_on_round_trip() {
    let line = r#"{"type":"assistant","uuid":"u","sessionId":"s","timestamp":"t","message":{"role":"assistant","content":[]}}"#;
    let record: Record = serde_json::from_str(line).unwrap();

    assert_eq!(
        serde_json::to_value(&record).unwrap(),
        serde_json::from_str::<Value>(line).unwrap(),
        "a missing field must not be materialised as null"
    );
}

#[test]
fn explicit_nulls_are_preserved_on_stream_records() {
    let line = r#"{"type":"result","subtype":"success","session_id":"s","uuid":"u","is_error":false,"num_turns":1,"duration_ms":10,"result":null,"total_cost_usd":null,"usage":null}"#;
    let output: StreamOutput = serde_json::from_str(line).unwrap();

    assert!(output.known().is_some());
    assert_eq!(
        serde_json::to_value(&output).unwrap(),
        serde_json::from_str::<Value>(line).unwrap()
    );
}

#[test]
fn user_text_builds_the_shape_the_cli_accepts() {
    let built = serde_json::to_value(StreamInput::user_text("hello")).unwrap();
    let expected: Value = serde_json::from_str(
        r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"hello"}]}}"#,
    )
    .unwrap();

    assert_eq!(built, expected);
}
