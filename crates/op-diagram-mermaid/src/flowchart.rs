use std::collections::{HashMap, HashSet};

use op_diagram::{Cluster, Direction, Edge, Graph, Head, Node, Shape, Stroke};

use crate::ParseError;
use crate::source::{Cursor, Line, quoted};
use crate::text::label;

const IGNORED: [&str; 5] = ["classDef", "class", "style", "linkStyle", "click"];

pub(crate) fn parse(mut header: Cursor<'_>, body: &[Line<'_>]) -> Result<Graph, ParseError> {
    let mut chart = Chart::default();
    header.skip_spaces();
    if !header.at_end() && !header.starts_with(";") {
        chart.direction = direction(&mut header)?;
    }
    header.skip_spaces();
    if !header.at_end() && !header.eat(";") {
        return Err(header.error("expected a new line or `;` after the direction"));
    }
    chart.statements(header)?;
    for line in body {
        chart.statements(Cursor::new(line))?;
    }
    chart.finish()
}

pub(crate) fn direction(cursor: &mut Cursor<'_>) -> Result<Direction, ParseError> {
    let start = cursor.offset();
    match cursor.take_while(|c| c.is_ascii_alphabetic()) {
        "TB" | "TD" => Ok(Direction::Down),
        "BT" => Ok(Direction::Up),
        "LR" => Ok(Direction::Right),
        "RL" => Ok(Direction::Left),
        other => Err(cursor.error_at(
            start,
            format!("`{other}` is not a direction; use `TB`, `TD`, `BT`, `LR`, or `RL`"),
        )),
    }
}

#[derive(Default)]
struct Chart {
    direction: Direction,
    nodes: Vec<Node>,
    node_at: HashMap<String, usize>,
    shaped: HashSet<String>,
    clusters: Vec<Cluster>,
    cluster_at: HashMap<String, usize>,
    edges: Vec<Edge>,
    // Where each edge's link starts, for an error that only the whole chart can find.
    edge_places: Vec<ParseError>,
    open: Vec<Frame>,
    claimed: HashSet<String>,
    untitled: usize,
}

struct Frame {
    cluster: usize,
    mentioned: Vec<String>,
    error: ParseError,
}

struct Link {
    stroke: Stroke,
    tail: Head,
    head: Head,
    min_length: usize,
    label: Vec<String>,
}

impl Chart {
    fn statements(&mut self, mut cursor: Cursor<'_>) -> Result<(), ParseError> {
        loop {
            cursor.skip_spaces();
            if cursor.at_end() {
                return Ok(());
            }
            if cursor.eat(";") {
                continue;
            }
            if IGNORED.iter().any(|word| cursor.keyword(word)) {
                return Ok(());
            }
            let at = cursor.offset();
            if cursor.keyword("subgraph") {
                self.open_subgraph(&mut cursor, at)?;
            } else if cursor.keyword("end") {
                self.close_subgraph(&cursor, at)?;
            } else if cursor.keyword("direction") {
                cursor.skip_spaces();
                let direction = direction(&mut cursor)?;
                match self.open.last() {
                    Some(frame) => self.clusters[frame.cluster].direction = Some(direction),
                    None => self.direction = direction,
                }
            } else {
                self.edge_statement(&mut cursor)?;
            }
            cursor.skip_spaces();
            if !cursor.at_end() && !cursor.eat(";") {
                return Err(cursor.error("expected a new line or `;`"));
            }
        }
    }

    // Mermaid gives a subgraph a title with spaces and no id an id of its own. The generated id holds
    // a space, which no node id can, so no edge can name it by accident.
    fn open_subgraph(&mut self, cursor: &mut Cursor<'_>, at: usize) -> Result<(), ParseError> {
        let opened = cursor.error_at(at, "this subgraph has no `end`");
        cursor.skip_spaces();
        let start = cursor.offset();
        let title_start = cursor.clone();
        let (id, title) = if cursor.peek() == Some('"') {
            (self.untitled_id(), label(quoted(cursor)?))
        } else {
            let id = node_id(cursor);
            if id.is_empty() {
                return Err(cursor.error("expected the id or the title of the subgraph"));
            }
            cursor.skip_spaces();
            if cursor.eat("[") {
                let opened_at = cursor.offset() - 1;
                let (text, _) = enclosed(cursor, &["]"], opened_at, "the subgraph title")?;
                (id.to_owned(), label(text))
            } else if cursor.at_end() || cursor.starts_with(";") {
                (id.to_owned(), vec![id.to_owned()])
            } else {
                *cursor = title_start;
                let title = cursor.take_while(|c| c != ';');
                (self.untitled_id(), label(title))
            }
        };
        if self.cluster_at.contains_key(&id) {
            return Err(cursor.error_at(start, format!("the subgraph `{id}` is already defined")));
        }
        if self.shaped.contains(&id) {
            return Err(cursor.error_at(
                start,
                format!("`{id}` is a node with a shape, so it cannot name a subgraph"),
            ));
        }
        let parent = self
            .open
            .last()
            .map(|frame| self.clusters[frame.cluster].id.clone());
        self.cluster_at.insert(id.clone(), self.clusters.len());
        self.open.push(Frame {
            cluster: self.clusters.len(),
            mentioned: Vec::new(),
            error: opened,
        });
        self.clusters.push(Cluster {
            id,
            label: title,
            parent,
            ..Cluster::default()
        });
        Ok(())
    }

    fn untitled_id(&mut self) -> String {
        self.untitled += 1;
        format!("subgraph {}", self.untitled)
    }

    // A node belongs to the first subgraph to close that mentions it, which is the innermost one. A
    // later subgraph that mentions it again only draws an edge to it.
    fn close_subgraph(&mut self, cursor: &Cursor<'_>, at: usize) -> Result<(), ParseError> {
        let Some(frame) = self.open.pop() else {
            return Err(cursor.error_at(at, "this `end` closes no subgraph"));
        };
        let cluster = self.clusters[frame.cluster].id.clone();
        for id in frame.mentioned {
            if id == cluster
                || self.cluster_at.contains_key(&id)
                || !self.claimed.insert(id.clone())
            {
                continue;
            }
            if let Some(&at) = self.node_at.get(&id) {
                self.nodes[at].parent = Some(cluster.clone());
            }
        }
        Ok(())
    }

    fn edge_statement(&mut self, cursor: &mut Cursor<'_>) -> Result<(), ParseError> {
        let mut from = self.vertex_group(cursor)?;
        loop {
            cursor.skip_spaces();
            if cursor.at_end() || cursor.starts_with(";") {
                return Ok(());
            }
            let place = cursor.error_at(cursor.offset(), "");
            let link = link(cursor)?;
            cursor.skip_spaces();
            let to = self.vertex_group(cursor)?;
            for tail in &from {
                for head in &to {
                    self.edges.push(Edge {
                        from: tail.clone(),
                        to: head.clone(),
                        label: link.label.clone(),
                        stroke: link.stroke,
                        tail: link.tail,
                        head: link.head,
                        min_length: link.min_length,
                    });
                    self.edge_places.push(place.clone());
                }
            }
            from = to;
        }
    }

    fn vertex_group(&mut self, cursor: &mut Cursor<'_>) -> Result<Vec<String>, ParseError> {
        let mut ids = vec![self.vertex(cursor)?];
        loop {
            let before = cursor.clone();
            cursor.skip_spaces();
            if !cursor.eat("&") {
                *cursor = before;
                return Ok(ids);
            }
            cursor.skip_spaces();
            ids.push(self.vertex(cursor)?);
        }
    }

    fn vertex(&mut self, cursor: &mut Cursor<'_>) -> Result<String, ParseError> {
        let start = cursor.offset();
        let id = node_id(cursor).to_owned();
        if id.is_empty() {
            return Err(cursor.error("expected a node id"));
        }
        if cursor.starts_with("@{") {
            return Err(cursor.error("the `@{ }` shape syntax is not supported"));
        }
        let shape = shape(cursor, &id)?;
        if cursor.eat(":::") {
            cursor.take_while(|c| c.is_alphanumeric() || c == '_' || c == '-');
        }
        if let Some(frame) = self.open.last_mut() {
            frame.mentioned.push(id.clone());
        }
        match shape {
            Some((shape, text)) => {
                if self.cluster_at.contains_key(&id) {
                    return Err(cursor.error_at(
                        start,
                        format!("`{id}` names a subgraph, so it cannot have a node shape"),
                    ));
                }
                self.shaped.insert(id.clone());
                match self.node_at.get(&id) {
                    Some(&at) => {
                        self.nodes[at].shape = shape;
                        self.nodes[at].label = text;
                    }
                    None => self.push_node(Node {
                        id: id.clone(),
                        shape,
                        label: text,
                        ..Node::default()
                    }),
                }
            }
            None => {
                if !self.node_at.contains_key(&id) && !self.cluster_at.contains_key(&id) {
                    self.push_node(Node {
                        id: id.clone(),
                        label: vec![id.clone()],
                        ..Node::default()
                    });
                }
            }
        }
        Ok(id)
    }

    // Such a link would start inside its own target and end on that target's border, so no drawing
    // of it reads as a link.
    fn joins_its_own_subgraph(&self, edge: &Edge) -> Option<String> {
        for (inner, outer) in [(&edge.from, &edge.to), (&edge.to, &edge.from)] {
            if !self.cluster_at.contains_key(outer) {
                continue;
            }
            if inner == outer {
                return Some(format!("the subgraph `{outer}` cannot link to itself"));
            }
            let mut above = self.parent(inner);
            while let Some(at) = above {
                if at == outer {
                    return Some(format!(
                        "`{inner}` is inside the subgraph `{outer}`, so no link can join them"
                    ));
                }
                above = self.parent(at);
            }
        }
        None
    }

    fn parent(&self, id: &str) -> Option<&str> {
        match self.cluster_at.get(id) {
            Some(&at) => self.clusters[at].parent.as_deref(),
            None => self
                .node_at
                .get(id)
                .and_then(|&at| self.nodes[at].parent.as_deref()),
        }
    }

    fn push_node(&mut self, node: Node) {
        self.node_at.insert(node.id.clone(), self.nodes.len());
        self.nodes.push(node);
    }

    fn finish(self) -> Result<Graph, ParseError> {
        if let Some(frame) = self.open.last() {
            return Err(frame.error.clone());
        }
        for (edge, place) in self.edges.iter().zip(&self.edge_places) {
            if let Some(message) = self.joins_its_own_subgraph(edge) {
                return Err(ParseError {
                    message,
                    ..place.clone()
                });
            }
        }
        let clusters = self.cluster_at;
        Ok(Graph {
            direction: self.direction,
            pack: None,
            nodes: self
                .nodes
                .into_iter()
                .filter(|node| !clusters.contains_key(&node.id))
                .collect(),
            clusters: self.clusters,
            edges: self.edges,
        })
    }
}

// A dash belongs to the id unless it starts a link: `a-b` is one node, `a-->b` and `a-.->b` are two.
fn node_id<'a>(cursor: &mut Cursor<'a>) -> &'a str {
    let rest = cursor.rest();
    let mut end = 0;
    let mut chars = rest.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        let next = chars.peek().map(|(_, next)| *next);
        let part = c.is_alphanumeric()
            || c == '_'
            || (c == '-' && at > 0 && !matches!(next, Some('-' | '.' | '>' | '=')));
        if !part {
            break;
        }
        end = at + c.len_utf8();
    }
    cursor.advance(end);
    &rest[..end]
}

fn openings() -> [(&'static str, Vec<(&'static str, Shape)>); 12] {
    [
        ("(((", vec![(")))", Shape::DoubleCircle)]),
        ("((", vec![("))", Shape::Circle)]),
        ("([", vec![("])", Shape::Stadium)]),
        ("(", vec![(")", Shape::Rounded)]),
        ("[[", vec![("]]", Shape::Subroutine)]),
        ("[(", vec![(")]", Shape::Cylinder)]),
        (
            "[/",
            vec![("/]", Shape::LeanRight), ("\\]", Shape::Trapezoid)],
        ),
        (
            "[\\",
            vec![("\\]", Shape::LeanLeft), ("/]", Shape::InvertedTrapezoid)],
        ),
        ("[", vec![("]", Shape::Rectangle)]),
        ("{{", vec![("}}", Shape::Hexagon)]),
        ("{", vec![("}", Shape::Diamond)]),
        (">", vec![("]", Shape::Asymmetric)]),
    ]
}

fn shape(cursor: &mut Cursor<'_>, id: &str) -> Result<Option<(Shape, Vec<String>)>, ParseError> {
    let Some((opener, closers)) = openings()
        .into_iter()
        .find(|(opener, _)| cursor.starts_with(opener))
    else {
        return Ok(None);
    };
    let opened_at = cursor.offset();
    cursor.advance(opener.len());
    let ends: Vec<&str> = closers.iter().map(|(closer, _)| *closer).collect();
    let (text, which) = enclosed(cursor, &ends, opened_at, &format!("the text of `{id}`"))?;
    if text.trim().is_empty() {
        return Err(cursor.error_at(opened_at, format!("the text of `{id}` is empty")));
    }
    let (_, shape) = closers
        .into_iter()
        .nth(which)
        .expect("`enclosed` returns one of the closers");
    Ok(Some((shape, label(text))))
}

// The text runs to the first closer, unless it is quoted: then the quotes protect a closer inside it.
fn enclosed<'a>(
    cursor: &mut Cursor<'a>,
    closers: &[&str],
    opened_at: usize,
    what: &str,
) -> Result<(&'a str, usize), ParseError> {
    let before = cursor.clone();
    cursor.skip_spaces();
    if cursor.peek() == Some('"') {
        let text = quoted(cursor)?;
        cursor.skip_spaces();
        return match closers.iter().position(|closer| cursor.eat(closer)) {
            Some(which) => Ok((text, which)),
            None => Err(cursor.error(format!("expected `{}` after the quoted text", closers[0]))),
        };
    }
    *cursor = before;
    let rest = cursor.rest();
    let Some((at, which)) = closers
        .iter()
        .enumerate()
        .filter_map(|(which, closer)| rest.find(closer).map(|at| (at, which)))
        .min()
    else {
        return Err(cursor.error_at(opened_at, format!("{what} has no closing `{}`", closers[0])));
    };
    cursor.advance(at + closers[which].len());
    Ok((&rest[..at], which))
}

fn head_of(c: Option<char>) -> Option<Head> {
    match c {
        Some('>') => Some(Head::Arrow),
        Some('o') => Some(Head::Circle),
        Some('x') => Some(Head::Cross),
        _ => None,
    }
}

// `-->` spans one rank and every extra `-`, `=`, or `.` one more, as in Mermaid. A letter `o` or `x`
// right after the dashes is a head, so `a --oak` is a circle head on the node `ak`.
fn link(cursor: &mut Cursor<'_>) -> Result<Link, ParseError> {
    let start = cursor.offset();
    let not_a_link = |cursor: &Cursor<'_>| {
        cursor.error_at(
            start,
            "expected a link such as `-->`, or the end of the statement",
        )
    };
    let tildes = cursor.rest().chars().take_while(|c| *c == '~').count();
    if tildes >= 3 {
        cursor.advance(tildes);
        return Ok(Link {
            stroke: Stroke::Invisible,
            tail: Head::None,
            head: Head::None,
            min_length: tildes - 2,
            label: Vec::new(),
        });
    }
    let marker = match cursor.peek() {
        Some('<') => Some(Head::Arrow),
        Some(c @ ('o' | 'x')) => head_of(Some(c)),
        _ => None,
    };
    if marker.is_some() {
        let next = cursor.rest()[1..].chars().next();
        if !matches!(next, Some('-' | '=')) {
            return Err(not_a_link(cursor));
        }
        cursor.advance(1);
    }
    let (stroke, head, min_length, label) = match cursor.peek() {
        Some('=') if cursor.rest()[1..].starts_with('=') => {
            solid_or_thick(cursor, '=', Stroke::Thick)?
        }
        Some('-') if cursor.rest()[1..].starts_with('.') => dotted(cursor)?,
        Some('-') if cursor.rest()[1..].starts_with('-') => {
            solid_or_thick(cursor, '-', Stroke::Solid)?
        }
        _ => return Err(not_a_link(cursor)),
    };
    let tail = match marker {
        None => Head::None,
        Some(marker) if marker == head => marker,
        Some(_) => {
            return Err(cursor.error_at(
                start,
                "a link with a head at its start needs the same head at its end, as in `<-->`",
            ));
        }
    };
    Ok(Link {
        stroke,
        tail,
        head,
        min_length,
        label,
    })
}

type Body = (Stroke, Head, usize, Vec<String>);

fn solid_or_thick(cursor: &mut Cursor<'_>, dash: char, stroke: Stroke) -> Result<Body, ParseError> {
    let run = cursor.take_while(|c| c == dash).len();
    if let Some(head) = head_of(cursor.peek()) {
        cursor.advance(1);
        return Ok((stroke, head, run - 1, pipe_label(cursor)?));
    }
    if run >= 3 {
        return Ok((stroke, Head::None, run - 2, pipe_label(cursor)?));
    }
    let doubled: String = [dash, dash].iter().collect();
    let text = link_text(cursor, &doubled)?;
    let run = cursor.take_while(|c| c == dash).len();
    match head_of(cursor.peek()) {
        Some(head) => {
            cursor.advance(1);
            Ok((stroke, head, run - 1, text))
        }
        None if run >= 3 => Ok((stroke, Head::None, run - 2, text)),
        None => Err(cursor.error(format!("the link text needs an end such as `{doubled}>`"))),
    }
}

fn dotted(cursor: &mut Cursor<'_>) -> Result<Body, ParseError> {
    cursor.advance(1);
    let dots = cursor.take_while(|c| c == '.').len();
    if cursor.eat("-") {
        let head = head_of(cursor.peek()).unwrap_or(Head::None);
        if head != Head::None {
            cursor.advance(1);
        }
        return Ok((Stroke::Dotted, head, dots, pipe_label(cursor)?));
    }
    let text = link_text(cursor, ".-")?;
    let dots = cursor.take_while(|c| c == '.').len();
    if !cursor.eat("-") {
        return Err(cursor.error("the link text needs an end such as `.->`"));
    }
    let head = head_of(cursor.peek()).unwrap_or(Head::None);
    if head != Head::None {
        cursor.advance(1);
    }
    Ok((Stroke::Dotted, head, dots, text))
}

// The text of `-- text -->` runs to the first `--`; of `-. text .->`, to the dots before its `-`.
fn link_text(cursor: &mut Cursor<'_>, end: &str) -> Result<Vec<String>, ParseError> {
    let start = cursor.offset();
    cursor.skip_spaces();
    let text = if cursor.peek() == Some('"') {
        let text = quoted(cursor)?;
        cursor.skip_spaces();
        if !cursor.starts_with(end) && !(end == ".-" && cursor.starts_with(".")) {
            return Err(cursor.error("expected the end of the link after the quoted text"));
        }
        text
    } else {
        let rest = cursor.rest();
        let Some(mut at) = rest.find(end) else {
            return Err(cursor.error_at(start, "this link text has no end, such as `-->`"));
        };
        if end == ".-" {
            while at > 0 && rest[..at].ends_with('.') {
                at -= 1;
            }
        }
        cursor.advance(at);
        &rest[..at]
    };
    if text.trim().is_empty() {
        return Err(cursor.error_at(start, "the link text is empty"));
    }
    Ok(label(text))
}

fn pipe_label(cursor: &mut Cursor<'_>) -> Result<Vec<String>, ParseError> {
    let before = cursor.clone();
    cursor.skip_spaces();
    if !cursor.starts_with("|") {
        *cursor = before;
        return Ok(Vec::new());
    }
    let opened_at = cursor.offset();
    cursor.advance(1);
    let (text, _) = enclosed(cursor, &["|"], opened_at, "the link label")?;
    if text.trim().is_empty() {
        return Err(cursor.error_at(opened_at, "the link label is empty"));
    }
    Ok(label(text))
}
