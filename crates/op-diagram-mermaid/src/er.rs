use std::collections::HashMap;

use op_diagram::{Direction, Edge, Graph, Head, Node, Row, Shape, Stroke};

use crate::ParseError;
use crate::flowchart::direction;
use crate::source::{Cursor, Line, quoted};
use crate::text::label;

const KEYS: [&str; 3] = ["PK", "FK", "UK"];

pub(crate) fn parse(mut header: Cursor<'_>, body: &[Line<'_>]) -> Result<Graph, ParseError> {
    header.skip_spaces();
    if !header.at_end() {
        return Err(header.error("expected a new line after `erDiagram`"));
    }
    let mut tokens = Vec::new();
    for line in body {
        tokenize(Cursor::new(line), &mut tokens)?;
    }
    let mut chart = Chart {
        tokens,
        at: 0,
        graph: Graph {
            direction: Direction::Down,
            ..Graph::default()
        },
        entity_at: HashMap::new(),
    };
    chart.statements()?;
    Ok(chart.graph)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind<'a> {
    Word(&'a str),
    Quoted(&'a str),
    Open,
    Close,
    Colon,
    Comma,
    Relationship(Head, Stroke, Head),
    Direction(Direction),
    Newline,
}

struct Token<'a> {
    kind: Kind<'a>,
    error: Cursor<'a>,
}

fn word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '[' | ']' | '(' | ')' | '*')
}

fn tokenize<'a>(mut cursor: Cursor<'a>, into: &mut Vec<Token<'a>>) -> Result<(), ParseError> {
    loop {
        cursor.skip_spaces();
        let at = cursor.clone();
        let Some(c) = cursor.peek() else {
            into.push(Token {
                kind: Kind::Newline,
                error: at,
            });
            return Ok(());
        };
        let kind = match c {
            '"' => Kind::Quoted(quoted(&mut cursor)?),
            ':' => single(&mut cursor, Kind::Colon),
            ',' => single(&mut cursor, Kind::Comma),
            '{' => single(&mut cursor, Kind::Open),
            '}' if !cursor.rest()[1..].starts_with(['o', '|']) => single(&mut cursor, Kind::Close),
            '|' | '}' => relationship(&mut cursor)?,
            _ if word_char(c) => {
                let word = cursor.take_while(word_char);
                if word == "direction" && into.last().is_none_or(|last| last.kind == Kind::Newline)
                {
                    cursor.skip_spaces();
                    Kind::Direction(direction(&mut cursor)?)
                } else {
                    Kind::Word(word)
                }
            }
            _ => return Err(cursor.error(format!("unexpected `{c}`"))),
        };
        into.push(Token { kind, error: at });
    }
}

fn single<'a>(cursor: &mut Cursor<'_>, kind: Kind<'a>) -> Kind<'a> {
    cursor.advance(1);
    kind
}

// `||--o{` reads from the left entity to the right one: exactly one on the left, zero or more on
// the right. `..` in the middle marks a relationship that is not identifying.
fn relationship<'a>(cursor: &mut Cursor<'_>) -> Result<Kind<'a>, ParseError> {
    let rest = cursor.rest();
    let tail = match rest.get(..2) {
        Some("|o") => Head::ZeroOrOne,
        Some("||") => Head::ExactlyOne,
        Some("}o") => Head::ZeroOrMore,
        Some("}|") => Head::OneOrMore,
        _ => Head::None,
    };
    let stroke = match rest.get(2..4) {
        Some("--") => Some(Stroke::Solid),
        Some("..") => Some(Stroke::Dotted),
        _ => None,
    };
    let head = match rest.get(4..6) {
        Some("o|") => Head::ZeroOrOne,
        Some("||") => Head::ExactlyOne,
        Some("o{") => Head::ZeroOrMore,
        Some("|{") => Head::OneOrMore,
        _ => Head::None,
    };
    match (tail, stroke, head) {
        (Head::None, ..) | (_, None, _) | (.., Head::None) => Err(cursor.error(
            "expected a relationship such as `||--o{`: `|o`, `||`, `}o`, or `}|`, then `--` or `..`, then `o|`, `||`, `o{`, or `|{`",
        )),
        (tail, Some(stroke), head) => {
            cursor.advance(6);
            Ok(Kind::Relationship(tail, stroke, head))
        }
    }
}

struct Chart<'a> {
    tokens: Vec<Token<'a>>,
    at: usize,
    graph: Graph,
    entity_at: HashMap<String, usize>,
}

impl<'a> Chart<'a> {
    fn peek(&self) -> Kind<'a> {
        self.tokens
            .get(self.at)
            .map_or(Kind::Newline, |token| token.kind)
    }

    fn next(&mut self) -> Kind<'a> {
        let kind = self.peek();
        self.at += 1;
        kind
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        match self.tokens.get(self.at).or(self.tokens.last()) {
            Some(token) => token.error.error(message),
            None => ParseError {
                line: 1,
                column: 1,
                message: message.into(),
            },
        }
    }

    fn statements(&mut self) -> Result<(), ParseError> {
        while self.at < self.tokens.len() {
            match self.next() {
                Kind::Newline => {}
                Kind::Direction(direction) => {
                    self.graph.direction = direction;
                    self.end_of_line()?;
                }
                Kind::Word(name) | Kind::Quoted(name) => self.entity_statement(name)?,
                _ => {
                    self.at -= 1;
                    return Err(self.error("expected an entity or a relationship"));
                }
            }
        }
        Ok(())
    }

    fn entity_statement(&mut self, name: &str) -> Result<(), ParseError> {
        let from = self.entity(name);
        match self.next() {
            Kind::Newline => Ok(()),
            Kind::Open => self.attributes(from),
            Kind::Relationship(tail, stroke, head) => {
                let to = match self.next() {
                    Kind::Word(name) | Kind::Quoted(name) => self.entity(name),
                    _ => {
                        self.at -= 1;
                        return Err(
                            self.error("expected the entity on the right of the relationship")
                        );
                    }
                };
                if self.next() != Kind::Colon {
                    self.at -= 1;
                    return Err(self.error("expected `:` and the label of the relationship"));
                }
                let text = match self.next() {
                    Kind::Word(text) | Kind::Quoted(text) => label(text),
                    _ => {
                        self.at -= 1;
                        return Err(self.error("expected the label of the relationship"));
                    }
                };
                self.end_of_line()
                    .map_err(|_| self.error_before("put a label with spaces in quotes"))?;
                self.graph.edges.push(Edge {
                    from: self.graph.nodes[from].id.clone(),
                    to: self.graph.nodes[to].id.clone(),
                    label: text,
                    stroke,
                    tail,
                    head,
                    min_length: 1,
                });
                Ok(())
            }
            _ => {
                self.at -= 1;
                Err(self.error(format!("expected `{{` or a relationship after `{name}`")))
            }
        }
    }

    fn error_before(&self, message: &str) -> ParseError {
        self.tokens[self.at.saturating_sub(1).min(self.tokens.len() - 1)]
            .error
            .error(message)
    }

    fn end_of_line(&mut self) -> Result<(), ParseError> {
        match self.next() {
            Kind::Newline => Ok(()),
            _ => {
                self.at -= 1;
                Err(self.error("expected the end of the line"))
            }
        }
    }

    fn entity(&mut self, name: &str) -> usize {
        if let Some(&at) = self.entity_at.get(name) {
            return at;
        }
        let at = self.graph.nodes.len();
        self.entity_at.insert(name.to_owned(), at);
        self.graph.nodes.push(Node {
            id: name.to_owned(),
            shape: Shape::Table { rows: Vec::new() },
            label: vec![name.to_owned()],
            ..Node::default()
        });
        at
    }

    // One attribute on each line: `type name`, then keys such as `PK, FK`, then a quoted comment.
    fn attributes(&mut self, entity: usize) -> Result<(), ParseError> {
        loop {
            match self.next() {
                Kind::Newline => {}
                Kind::Close => return self.end_of_line(),
                Kind::Word(kind) => {
                    let Kind::Word(name) = self.next() else {
                        self.at -= 1;
                        return Err(self.error("expected the name of the attribute after its type"));
                    };
                    let mut cells = vec![
                        kind.to_owned(),
                        name.to_owned(),
                        String::new(),
                        String::new(),
                    ];
                    let mut keys = Vec::new();
                    while let Kind::Word(key) = self.peek() {
                        if !KEYS.contains(&key) {
                            return Err(self
                                .error(format!("`{key}` is not a key; use `PK`, `FK`, or `UK`")));
                        }
                        keys.push(key);
                        self.at += 1;
                        if self.peek() != Kind::Comma {
                            break;
                        }
                        self.at += 1;
                    }
                    cells[2] = keys.join(", ");
                    if let Kind::Quoted(comment) = self.peek() {
                        cells[3] = comment.to_owned();
                        self.at += 1;
                    }
                    while cells.last().is_some_and(String::is_empty) {
                        cells.pop();
                    }
                    if let Shape::Table { rows } = &mut self.graph.nodes[entity].shape {
                        rows.push(Row { cells });
                    }
                    match self.peek() {
                        Kind::Newline => self.at += 1,
                        Kind::Close => {}
                        _ => return Err(self.error("expected the end of the attribute")),
                    }
                }
                Kind::Quoted(_)
                | Kind::Open
                | Kind::Colon
                | Kind::Comma
                | Kind::Relationship(..)
                | Kind::Direction(_) => {
                    self.at -= 1;
                    return Err(self.error("expected an attribute such as `string name` or `}`"));
                }
            }
            if self.at >= self.tokens.len() {
                return Err(self.error("this entity has no closing `}`"));
            }
        }
    }
}
