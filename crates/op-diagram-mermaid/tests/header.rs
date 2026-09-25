mod common;

use common::{error, graph};

#[test]
fn an_empty_source_is_an_error() {
    let empty = error("");
    assert_eq!((empty.line, empty.column), (1, 1));
    assert_eq!(empty.message, "the diagram is empty");
    assert_eq!(
        error("%% only a comment\n\n").message,
        "the diagram is empty"
    );
}

#[test]
fn a_comment_may_come_before_the_header() {
    let graph = graph("%% the pipeline\nflowchart LR\n  a --> b");
    assert_eq!(graph.edges.len(), 1);
}

#[test]
fn names_a_diagram_type_it_does_not_draw() {
    let refused = error("\n  gantt\n  title A plan");
    assert_eq!((refused.line, refused.column), (2, 3));
    assert_eq!(
        refused.message,
        "`gantt` is not a supported diagram type; use `flowchart`, `sequenceDiagram`, or `erDiagram`"
    );
    assert!(
        error("classDiagram\n  A <|-- B")
            .message
            .starts_with("`classDiagram`")
    );
}

#[test]
fn refuses_a_directive() {
    let refused = error("flowchart\n  %%{init: {'theme': 'dark'}}%%\n  a --> b");
    assert_eq!((refused.line, refused.column), (2, 3));
    assert_eq!(
        refused.message,
        "directives such as `%%{init}%%` are not supported"
    );
}

#[test]
fn refuses_front_matter() {
    let refused = error("---\ntitle: Flow\n---\nflowchart\n  a --> b");
    assert_eq!(refused.line, 1);
    assert_eq!(refused.message, "front matter is not supported");
}

#[test]
fn the_error_reads_as_one_line() {
    assert_eq!(
        error("pie\n  \"a\" : 1").to_string(),
        "line 1, column 1: `pie` is not a supported diagram type; use `flowchart`, `sequenceDiagram`, or `erDiagram`"
    );
}
