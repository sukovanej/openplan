mod common;

use common::{ends, error, graph, ids, node, parent};
use op_diagram::{Direction, Head, Shape, Stroke};

#[test]
fn reads_the_direction_of_the_header() {
    for (header, direction) in [
        ("flowchart", Direction::Down),
        ("flowchart TD", Direction::Down),
        ("flowchart TB", Direction::Down),
        ("graph BT", Direction::Up),
        ("flowchart LR", Direction::Right),
        ("graph RL", Direction::Left),
    ] {
        assert_eq!(
            graph(&format!("{header}\n  a --> b")).direction,
            direction,
            "{header}"
        );
    }
}

#[test]
fn takes_statements_after_the_header_on_the_same_line() {
    let graph = graph("graph TD; a-->b; b-->c;");
    assert_eq!(ends(&graph), [("a", "b"), ("b", "c")]);
}

#[test]
fn a_bare_id_is_a_rectangle_with_the_id_as_its_label() {
    let graph = graph("flowchart\n  first");
    assert_eq!(node(&graph, "first").shape, Shape::Rectangle);
    assert_eq!(node(&graph, "first").label, ["first"]);
}

#[test]
fn reads_every_classic_shape() {
    for (source, shape) in [
        ("a[x]", Shape::Rectangle),
        ("a(x)", Shape::Rounded),
        ("a([x])", Shape::Stadium),
        ("a[[x]]", Shape::Subroutine),
        ("a[(x)]", Shape::Cylinder),
        ("a((x))", Shape::Circle),
        ("a(((x)))", Shape::DoubleCircle),
        ("a>x]", Shape::Asymmetric),
        ("a{x}", Shape::Diamond),
        ("a{{x}}", Shape::Hexagon),
        ("a[/x/]", Shape::LeanRight),
        ("a[\\x\\]", Shape::LeanLeft),
        ("a[/x\\]", Shape::Trapezoid),
        ("a[\\x/]", Shape::InvertedTrapezoid),
    ] {
        let graph = graph(&format!("flowchart\n  {source}"));
        assert_eq!(graph.nodes[0].shape, shape, "{source}");
        assert_eq!(graph.nodes[0].label, ["x"], "{source}");
    }
}

#[test]
fn quotes_protect_what_would_close_the_shape() {
    let graph = graph("flowchart\n  a[\"b ] c; d --> e\"]");
    assert_eq!(ids(&graph), ["a"]);
    assert_eq!(node(&graph, "a").label, ["b ] c; d --> e"]);
}

#[test]
fn a_break_starts_a_new_line_of_the_label() {
    let graph = graph("flowchart\n  a[one<br>two<BR/>three<br />four]");
    assert_eq!(node(&graph, "a").label, ["one", "two", "three", "four"]);
}

#[test]
fn decodes_the_entity_codes_of_mermaid() {
    let graph = graph("flowchart\n  a[\"#quot;hi#quot; #35;1 #lt;tag#gt; #unknown; #\"]");
    assert_eq!(node(&graph, "a").label, ["\"hi\" #1 <tag> #unknown; #"]);
}

#[test]
fn a_later_shape_replaces_an_earlier_one_in_the_same_place() {
    let graph = graph("flowchart\n  a --> b\n  a{Choice}");
    assert_eq!(ids(&graph), ["a", "b"]);
    assert_eq!(node(&graph, "a").shape, Shape::Diamond);
    assert_eq!(node(&graph, "a").label, ["Choice"]);
}

#[test]
fn a_dash_inside_an_id_belongs_to_the_id() {
    let graph = graph("flowchart\n  my-node-->b-2");
    assert_eq!(ends(&graph), [("my-node", "b-2")]);
}

#[test]
fn keeps_unicode_in_ids_and_labels() {
    let graph = graph("flowchart\n  dívka[Dívka u baru] --> mladík");
    assert_eq!(ends(&graph), [("dívka", "mladík")]);
    assert_eq!(node(&graph, "dívka").label, ["Dívka u baru"]);
}

#[test]
fn reads_the_stroke_and_the_heads_of_each_link() {
    for (link, stroke, tail, head) in [
        ("-->", Stroke::Solid, Head::None, Head::Arrow),
        ("---", Stroke::Solid, Head::None, Head::None),
        ("-.->", Stroke::Dotted, Head::None, Head::Arrow),
        ("-.-", Stroke::Dotted, Head::None, Head::None),
        ("==>", Stroke::Thick, Head::None, Head::Arrow),
        ("===", Stroke::Thick, Head::None, Head::None),
        ("~~~", Stroke::Invisible, Head::None, Head::None),
        ("--o", Stroke::Solid, Head::None, Head::Circle),
        ("--x", Stroke::Solid, Head::None, Head::Cross),
        ("<-->", Stroke::Solid, Head::Arrow, Head::Arrow),
        ("o--o", Stroke::Solid, Head::Circle, Head::Circle),
        ("x--x", Stroke::Solid, Head::Cross, Head::Cross),
        ("<-.->", Stroke::Dotted, Head::Arrow, Head::Arrow),
        ("<==>", Stroke::Thick, Head::Arrow, Head::Arrow),
    ] {
        let graph = graph(&format!("flowchart\n  a {link} b"));
        let edge = &graph.edges[0];
        assert_eq!(
            (edge.stroke, edge.tail, edge.head, edge.min_length),
            (stroke, tail, head, 1),
            "{link}"
        );
    }
}

#[test]
fn each_extra_character_makes_a_link_one_rank_longer() {
    for (link, length) in [
        ("--->", 2),
        ("---->", 3),
        ("----", 2),
        ("-..->", 2),
        ("-...-", 3),
        ("===>", 2),
        ("~~~~", 2),
    ] {
        let graph = graph(&format!("flowchart\n  a {link} b"));
        assert_eq!(graph.edges[0].min_length, length, "{link}");
    }
}

#[test]
fn reads_a_label_between_pipes() {
    let graph = graph("flowchart\n  a -->|yes| b\n  b --> |\"a | b\"| c");
    assert_eq!(graph.edges[0].label, ["yes"]);
    assert_eq!(graph.edges[1].label, ["a | b"]);
}

#[test]
fn reads_a_label_inside_the_link() {
    for (source, stroke, tail, head, length, label) in [
        (
            "a -- yes --> b",
            Stroke::Solid,
            Head::None,
            Head::Arrow,
            1,
            "yes",
        ),
        (
            "a -- no --- b",
            Stroke::Solid,
            Head::None,
            Head::None,
            1,
            "no",
        ),
        (
            "a -- far ---> b",
            Stroke::Solid,
            Head::None,
            Head::Arrow,
            2,
            "far",
        ),
        (
            "a == go ==> b",
            Stroke::Thick,
            Head::None,
            Head::Arrow,
            1,
            "go",
        ),
        (
            "a -. maybe .-> b",
            Stroke::Dotted,
            Head::None,
            Head::Arrow,
            1,
            "maybe",
        ),
        (
            "a -. v1.2 ..-> b",
            Stroke::Dotted,
            Head::None,
            Head::Arrow,
            2,
            "v1.2",
        ),
        (
            "a <-- both --> b",
            Stroke::Solid,
            Head::Arrow,
            Head::Arrow,
            1,
            "both",
        ),
        (
            "a -- \"x --> y\" --> b",
            Stroke::Solid,
            Head::None,
            Head::Arrow,
            1,
            "x --> y",
        ),
    ] {
        let graph = graph(&format!("flowchart\n  {source}"));
        assert_eq!(ends(&graph), [("a", "b")], "{source}");
        let edge = &graph.edges[0];
        assert_eq!(
            (edge.stroke, edge.tail, edge.head, edge.min_length),
            (stroke, tail, head, length),
            "{source}"
        );
        assert_eq!(edge.label, [label], "{source}");
    }
}

#[test]
fn a_letter_right_after_the_dashes_is_a_head_as_in_mermaid() {
    let graph = graph("flowchart\n  a --oak");
    assert_eq!(ends(&graph), [("a", "ak")]);
    assert_eq!(graph.edges[0].head, Head::Circle);
}

#[test]
fn a_chain_links_each_neighbour() {
    let graph = graph("flowchart\n  a --> b -->|then| c");
    assert_eq!(ends(&graph), [("a", "b"), ("b", "c")]);
    assert_eq!(graph.edges[1].label, ["then"]);
}

#[test]
fn an_ampersand_links_every_pair() {
    let graph = graph("flowchart\n  a & b --> c & d");
    assert_eq!(
        ends(&graph),
        [("a", "c"), ("a", "d"), ("b", "c"), ("b", "d")]
    );
}

#[test]
fn a_subgraph_holds_the_nodes_it_mentions() {
    let graph = graph(
        "flowchart
  subgraph box [The box]
    a --> b
  end
  c --> a",
    );
    assert_eq!(graph.clusters.len(), 1);
    assert_eq!(graph.clusters[0].id, "box");
    assert_eq!(graph.clusters[0].label, ["The box"]);
    assert_eq!(graph.clusters[0].parent, None);
    assert_eq!(parent(&graph, "a"), Some("box"));
    assert_eq!(parent(&graph, "b"), Some("box"));
    assert_eq!(parent(&graph, "c"), None);
}

#[test]
fn the_innermost_subgraph_that_mentions_a_node_holds_it() {
    let graph = graph(
        "flowchart
  subgraph outer
    a
    subgraph inner
      a --> b
    end
    c
  end",
    );
    assert_eq!(parent(&graph, "a"), Some("inner"));
    assert_eq!(parent(&graph, "b"), Some("inner"));
    assert_eq!(parent(&graph, "c"), Some("outer"));
    assert_eq!(graph.clusters[1].id, "inner");
    assert_eq!(graph.clusters[1].parent.as_deref(), Some("outer"));
}

#[test]
fn a_node_first_mentioned_at_the_top_moves_into_the_subgraph_that_mentions_it() {
    let graph = graph("flowchart\n  a --> b\n  subgraph s\n    b\n  end");
    assert_eq!(parent(&graph, "b"), Some("s"));
    assert_eq!(parent(&graph, "a"), None);
}

#[test]
fn the_first_subgraph_to_close_keeps_a_node_that_two_mention() {
    let graph = graph(
        "flowchart
  subgraph one
    a
  end
  subgraph two
    a --> b
  end",
    );
    assert_eq!(parent(&graph, "a"), Some("one"));
    assert_eq!(parent(&graph, "b"), Some("two"));
}

#[test]
fn a_subgraph_may_have_a_title_and_no_id() {
    let graph = graph(
        "flowchart
  subgraph \"Quoted title\"
  end
  subgraph A title with spaces
  end
  subgraph plain
  end
  subgraph with_id[\"Bracket title\"]
  end",
    );
    let clusters: Vec<(&str, Vec<String>)> = graph
        .clusters
        .iter()
        .map(|cluster| (cluster.id.as_str(), cluster.label.clone()))
        .collect();
    assert_eq!(
        clusters,
        [
            ("subgraph 1", vec!["Quoted title".to_owned()]),
            ("subgraph 2", vec!["A title with spaces".to_owned()]),
            ("plain", vec!["plain".to_owned()]),
            ("with_id", vec!["Bracket title".to_owned()]),
        ]
    );
}

#[test]
fn an_edge_can_name_a_subgraph_before_and_after_it_is_defined() {
    let graph = graph(
        "flowchart
  a --> s
  subgraph s
    b
  end
  s --> c",
    );
    assert_eq!(ids(&graph), ["a", "b", "c"]);
    assert_eq!(ends(&graph), [("a", "s"), ("s", "c")]);
}

#[test]
fn a_direction_inside_a_subgraph_belongs_to_the_subgraph() {
    let graph = graph(
        "flowchart TD
  subgraph s
    direction LR
    a
  end",
    );
    assert_eq!(graph.direction, Direction::Down);
    assert_eq!(graph.clusters[0].direction, Some(Direction::Right));
}

#[test]
fn ignores_styles_classes_and_clicks() {
    let graph = graph(
        "flowchart
  a:::hot --> b
  classDef hot fill:#f00
  class a hot
  style b fill:#0f0
  linkStyle 0 stroke:#00f
  click a \"https://example.com\" \"tooltip\"",
    );
    assert_eq!(ends(&graph), [("a", "b")]);
    let a = node(&graph, "a");
    assert_eq!((a.link.as_deref(), a.classes.len()), (None, 0));
}

#[test]
fn skips_comments_and_blank_lines() {
    let graph = graph("flowchart\n  %% the start\n\n  a --> b\n  %% the end");
    assert_eq!(ends(&graph), [("a", "b")]);
}

#[test]
fn a_semicolon_splits_the_statements_of_one_line() {
    let graph = graph("flowchart\n  a --> b; b --> c");
    assert_eq!(ends(&graph), [("a", "b"), ("b", "c")]);
}

#[test]
fn names_the_line_and_the_column_of_an_unclosed_shape() {
    let unclosed = error("flowchart\n  a --> b[unclosed");
    assert_eq!((unclosed.line, unclosed.column), (2, 10));
    assert_eq!(unclosed.message, "the text of `b` has no closing `]`");
}

#[test]
fn refuses_an_empty_node_text() {
    assert_eq!(
        error("flowchart\n  a[ ]").message,
        "the text of `a` is empty"
    );
}

#[test]
fn refuses_a_link_it_does_not_know() {
    let refused = error("flowchart\n  a -> b");
    assert_eq!((refused.line, refused.column), (2, 5));
    assert_eq!(
        refused.message,
        "expected a link such as `-->`, or the end of the statement"
    );
    assert_eq!(
        error("flowchart\n  a b").message,
        "expected a link such as `-->`, or the end of the statement"
    );
}

#[test]
fn refuses_a_link_text_with_no_end() {
    assert_eq!(
        error("flowchart\n  a -- text b").message,
        "this link text has no end, such as `-->`"
    );
    assert_eq!(
        error("flowchart\n  a -- text -- b").message,
        "the link text needs an end such as `-->`"
    );
}

#[test]
fn refuses_heads_that_do_not_match() {
    for source in ["a <--x b", "a <--- b", "a o--> b"] {
        assert_eq!(
            error(&format!("flowchart\n  {source}")).message,
            "a link with a head at its start needs the same head at its end, as in `<-->`",
            "{source}"
        );
    }
}

#[test]
fn refuses_an_end_with_no_subgraph() {
    let refused = error("flowchart\n  a\n  end");
    assert_eq!((refused.line, refused.column), (3, 3));
    assert_eq!(refused.message, "this `end` closes no subgraph");
}

#[test]
fn refuses_a_subgraph_with_no_end_at_the_subgraph() {
    let refused = error("flowchart\n  subgraph s\n    a");
    assert_eq!((refused.line, refused.column), (2, 3));
    assert_eq!(refused.message, "this subgraph has no `end`");
}

#[test]
fn refuses_a_shape_on_a_subgraph_id() {
    assert_eq!(
        error("flowchart\n  subgraph s\n  end\n  s[Box]").message,
        "`s` names a subgraph, so it cannot have a node shape"
    );
    assert_eq!(
        error("flowchart\n  s[Box]\n  subgraph s\n  end").message,
        "`s` is a node with a shape, so it cannot name a subgraph"
    );
}

#[test]
fn refuses_the_same_subgraph_twice() {
    assert_eq!(
        error("flowchart\n  subgraph s\n  end\n  subgraph s\n  end").message,
        "the subgraph `s` is already defined"
    );
}

#[test]
fn refuses_the_shape_syntax_of_newer_mermaid() {
    assert_eq!(
        error("flowchart\n  a@{ shape: rect }").message,
        "the `@{ }` shape syntax is not supported"
    );
}

#[test]
fn refuses_an_unknown_direction() {
    assert_eq!(
        error("flowchart XY\n  a").message,
        "`XY` is not a direction; use `TB`, `TD`, `BT`, `LR`, or `RL`"
    );
}
