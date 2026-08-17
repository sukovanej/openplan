use std::fmt;
use std::ops::Range;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Code {
    Frontmatter,
    Status,
    Created,
    Parent,
    Dependencies,
    Rank,
    Reference,
    UnrewritableRef,
    Title,
    ParentCycle,
    DependencyCycle,
    DuplicateNumber,
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Frontmatter => "frontmatter",
            Code::Status => "status",
            Code::Created => "created",
            Code::Parent => "parent",
            Code::Dependencies => "dependencies",
            Code::Rank => "rank",
            Code::Reference => "reference",
            Code::UnrewritableRef => "unrewritable-ref",
            Code::Title => "title",
            Code::ParentCycle => "parent-cycle",
            Code::DependencyCycle => "dependency-cycle",
            Code::DuplicateNumber => "duplicate-number",
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }
}

impl From<Range<usize>> for Span {
    fn from(range: Range<usize>) -> Self {
        Span {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn in_source(source: &str, offset: usize) -> Self {
        let mut position = Position { line: 1, column: 1 };
        for (index, ch) in source.char_indices() {
            if index >= offset {
                break;
            }
            if ch == '\n' {
                position.line += 1;
                position.column = 1;
            } else {
                position.column += 1;
            }
        }
        position
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

const FIX_HINT: &str = "run `openplan lint --fix`";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: Code,
    pub path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    pub fixable: bool,
    pub severity: Severity,
}

impl Diagnostic {
    pub fn error(code: Code, path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Diagnostic {
            code,
            path: path.into(),
            span: None,
            position: None,
            message: message.into(),
            help: None,
            fixable: false,
            severity: Severity::Error,
        }
    }

    pub fn at(mut self, span: impl Into<Span>, source: &str) -> Self {
        let span = span.into();
        self.position = Some(Position::in_source(source, span.start));
        self.span = Some(span);
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn mark_fixable(mut self) -> Self {
        self.fixable = true;
        self
    }

    fn advice(&self) -> Option<&str> {
        self.help
            .as_deref()
            .or_else(|| self.fixable.then_some(FIX_HINT))
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.display())?;
        if let Some(position) = self.position {
            write!(f, ":{position}")?;
        }
        write!(f, ": {}[{}]: {}", self.severity, self.code, self.message)?;
        if let Some(advice) = self.advice() {
            write!(f, "\n  help: {advice}")?;
        }
        Ok(())
    }
}
