mod er;
mod flowchart;
mod sequence;
mod source;
mod text;

use op_diagram::Diagram;

use source::Cursor;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}, column {column}: {message}")]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

pub fn parse(source: &str) -> Result<Diagram, ParseError> {
    let lines = source::lines(source)?;
    let Some((header, body)) = lines.split_first() else {
        return Err(ParseError {
            line: 1,
            column: 1,
            message: "the diagram is empty".to_owned(),
        });
    };
    let mut cursor = Cursor::new(header);
    cursor.skip_spaces();
    let start = cursor.offset();
    if cursor.starts_with("---") {
        return Err(cursor.error("front matter is not supported"));
    }
    match cursor.take_while(|c| !c.is_whitespace() && c != ';') {
        "flowchart" | "graph" => flowchart::parse(cursor, body).map(Diagram::Graph),
        "sequenceDiagram" => sequence::parse(cursor, body).map(Diagram::Sequence),
        "erDiagram" => er::parse(cursor, body).map(Diagram::Graph),
        other => Err(cursor.error_at(
            start,
            format!(
                "`{other}` is not a supported diagram type; use `flowchart`, `sequenceDiagram`, or `erDiagram`"
            ),
        )),
    }
}
