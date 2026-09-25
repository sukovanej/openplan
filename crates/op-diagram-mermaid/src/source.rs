use crate::ParseError;

pub(crate) struct Line<'a> {
    pub(crate) number: usize,
    pub(crate) text: &'a str,
}

// Mermaid reads a `%%` comment only on a line of its own, and `%%{ }%%` is a directive that sets a
// theme or a layout this renderer would ignore, so it is refused rather than dropped.
pub(crate) fn lines(source: &str) -> Result<Vec<Line<'_>>, ParseError> {
    let mut kept = Vec::new();
    for (index, text) in source.lines().enumerate() {
        let line = Line {
            number: index + 1,
            text,
        };
        let trimmed = text.trim_start();
        if trimmed.starts_with("%%{") {
            let mut cursor = Cursor::new(&line);
            cursor.skip_spaces();
            return Err(cursor.error("directives such as `%%{init}%%` are not supported"));
        }
        if !trimmed.is_empty() && !trimmed.starts_with("%%") {
            kept.push(line);
        }
    }
    Ok(kept)
}

#[derive(Clone)]
pub(crate) struct Cursor<'a> {
    line: usize,
    text: &'a str,
    at: usize,
    end: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(line: &Line<'a>) -> Self {
        Self {
            line: line.number,
            text: line.text,
            at: 0,
            end: line.text.len(),
        }
    }

    pub(crate) fn until(&self, end: usize) -> Self {
        Self {
            end,
            ..self.clone()
        }
    }

    pub(crate) fn offset(&self) -> usize {
        self.at
    }

    pub(crate) fn rest(&self) -> &'a str {
        &self.text[self.at..self.end]
    }

    pub(crate) fn at_end(&self) -> bool {
        self.at >= self.end
    }

    pub(crate) fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    pub(crate) fn starts_with(&self, prefix: &str) -> bool {
        self.rest().starts_with(prefix)
    }

    pub(crate) fn advance(&mut self, bytes: usize) {
        self.at = (self.at + bytes).min(self.end);
    }

    pub(crate) fn eat(&mut self, prefix: &str) -> bool {
        let found = self.starts_with(prefix);
        if found {
            self.advance(prefix.len());
        }
        found
    }

    pub(crate) fn skip_spaces(&mut self) {
        self.take_while(char::is_whitespace);
    }

    pub(crate) fn take_while(&mut self, keep: impl Fn(char) -> bool) -> &'a str {
        let rest = self.rest();
        let taken = rest.find(|c| !keep(c)).unwrap_or(rest.len());
        self.advance(taken);
        &rest[..taken]
    }

    pub(crate) fn keyword(&mut self, word: &str) -> bool {
        let Some(after) = self.rest().strip_prefix(word) else {
            return false;
        };
        let bounded = after
            .chars()
            .next()
            .is_none_or(|c| c.is_whitespace() || c == ';');
        if bounded {
            self.advance(word.len());
        }
        bounded
    }

    pub(crate) fn error(&self, message: impl Into<String>) -> ParseError {
        self.error_at(self.at, message)
    }

    pub(crate) fn error_at(&self, offset: usize, message: impl Into<String>) -> ParseError {
        ParseError {
            line: self.line,
            column: self.text[..offset].chars().count() + 1,
            message: message.into(),
        }
    }
}

pub(crate) fn quoted<'a>(cursor: &mut Cursor<'a>) -> Result<&'a str, ParseError> {
    let opened = cursor.offset();
    cursor.eat("\"");
    let rest = cursor.rest();
    let Some(close) = rest.find('"') else {
        return Err(cursor.error_at(opened, "this `\"` has no closing `\"`"));
    };
    cursor.advance(close + 1);
    Ok(&rest[..close])
}
