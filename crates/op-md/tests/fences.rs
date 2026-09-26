use op_md::fences;

#[test]
fn finds_each_fence_with_its_language_and_where_its_text_starts() {
    let body =
        "# Task\n\n```mermaid\nflowchart LR\n  a --> b\n```\n\n~~~rust title\nfn main() {}\n~~~\n";
    let found: Vec<(String, String, &str)> = fences(body)
        .into_iter()
        .map(|fence| (fence.language, fence.text, &body[fence.text_start..]))
        .map(|(language, text, rest)| (language, text, rest.lines().next().unwrap_or("")))
        .collect();
    assert_eq!(
        found,
        vec![
            (
                "mermaid".to_owned(),
                "flowchart LR\n  a --> b\n".to_owned(),
                "flowchart LR"
            ),
            (
                "rust".to_owned(),
                "fn main() {}\n".to_owned(),
                "fn main() {}"
            ),
        ]
    );
}

#[test]
fn skips_an_indented_block() {
    assert!(fences("# Task\n\n    flowchart LR\n").is_empty());
}

#[test]
fn keeps_an_empty_fence() {
    let found = fences("```mermaid\n```\n");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].text, "");
}
