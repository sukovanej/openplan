use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use op_claude::{Record, parse_jsonl};

// The committed fixtures are a curated slice; this walks whatever transcripts the developer has on
// disk, which is the only way to catch a schema change shipped in a CLI release we have no sample
// of. Opt in with OP_CLAUDE_CORPUS=~/.claude/projects.
fn corpus_root() -> Option<PathBuf> {
    let raw = std::env::var("OP_CLAUDE_CORPUS").ok()?;
    let expanded = match raw.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").expect("HOME is set")).join(rest),
        None => PathBuf::from(&raw),
    };

    // Opting in and mistyping the path must not look the same as not opting in.
    assert!(
        expanded.is_dir(),
        "OP_CLAUDE_CORPUS={raw} does not resolve to a directory ({})",
        expanded.display()
    );
    Some(expanded)
}

fn jsonl_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            jsonl_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            found.push(path);
        }
    }
}

#[test]
fn local_transcript_corpus_parses_and_round_trips() {
    let Some(root) = corpus_root() else {
        eprintln!("skipped: set OP_CLAUDE_CORPUS to a transcript directory to run this");
        return;
    };

    let mut files = Vec::new();
    jsonl_files(&root, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "no .jsonl files under {}",
        root.display()
    );

    let mut records = 0usize;
    let mut unknown: BTreeMap<String, usize> = BTreeMap::new();
    let mut unknown_examples: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    let mut lossy: Vec<String> = Vec::new();

    for path in &files {
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let name = path.display().to_string();

        for (line_number, line) in source
            .lines()
            .enumerate()
            .map(|(index, line)| (index + 1, line))
            .filter(|(_, line)| !line.trim().is_empty())
        {
            records += 1;
            match serde_json::from_str::<Record>(line) {
                Err(error) => failures.push(format!("{name}:{line_number}: {error}")),
                Ok(record) => {
                    if record.known().is_none() {
                        let kind = serde_json::from_str::<serde_json::Value>(line)
                            .ok()
                            .and_then(|value| value["type"].as_str().map(str::to_owned))
                            .unwrap_or_else(|| "<untyped>".to_owned());
                        if unknown_examples.len() < 5 {
                            unknown_examples.push(format!("{name}:{line_number} ({kind})"));
                        }
                        *unknown.entry(kind).or_default() += 1;
                    }
                    let original: serde_json::Value = serde_json::from_str(line).unwrap();
                    if serde_json::to_value(&record).unwrap() != original {
                        lossy.push(format!("{name}:{line_number}"));
                    }
                }
            }
        }
    }

    eprintln!(
        "parsed {records} records from {} files under {}",
        files.len(),
        root.display()
    );

    assert!(
        failures.is_empty(),
        "{} hard parse failures, first 5: {:#?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
    assert!(
        lossy.is_empty(),
        "{} records lost data on round trip, first 5: {:#?}",
        lossy.len(),
        &lossy[..lossy.len().min(5)]
    );
    // A record that degrades to the raw-value fallback still round-trips byte-identically, so
    // without this the whole test stays green through exactly the drift it exists to detect.
    assert!(
        unknown.is_empty(),
        "{} records did not match any known shape — the CLI schema has moved. Counts: {unknown:?}. First: {unknown_examples:#?}",
        unknown.values().sum::<usize>(),
    );
}

// A sanity check that the crate is exercised even without the opt-in corpus.
#[test]
fn parse_jsonl_reports_the_offending_line() {
    let source = "{\"type\":\"ai-title\",\"sessionId\":\"s\",\"aiTitle\":\"t\"}\n\nnot json\n";
    let results: Vec<_> = parse_jsonl::<Record>("sample.jsonl", source).collect();

    assert_eq!(results.len(), 2);
    assert!(results[0].is_ok());
    let error = results[1].as_ref().unwrap_err();
    assert_eq!(error.line, 3);
    assert!(error.to_string().starts_with("sample.jsonl:3:"));
}
