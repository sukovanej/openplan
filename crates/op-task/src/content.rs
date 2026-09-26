use crate::conflict;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TitleError {
    #[error("a task must have a non-empty title")]
    Empty,
    #[error("a title must be one line; remove the line break")]
    LineBreak,
    #[error("the title is inside a conflict block; resolve the block first")]
    InConflict,
}

pub fn title(body: &str) -> Option<String> {
    op_md::title(&conflict::published(body))
}

pub fn split(body: &str) -> String {
    Layout::of(body).map_or_else(|| body.to_owned(), |layout| layout.description())
}

pub fn join(template: &str, title: &str, description: &str) -> Result<String, TitleError> {
    let Some(layout) = Layout::of(template) else {
        if title == self::title(template).unwrap_or_default() {
            return Ok(description.to_owned());
        }
        if self::title(template).is_some() {
            return Err(TitleError::InConflict);
        }
        let heading = heading_line(title, "\n")?;
        return Ok(titled("", &heading, "\n", description));
    };
    let newline = match line_ending(layout.heading) {
        "" => "\n",
        ending => ending,
    };
    let heading = match title == layout.title {
        true => layout.heading.to_owned(),
        false => heading_line(title, line_ending(layout.heading))?,
    };
    if description == layout.description() {
        return Ok(format!(
            "{}{heading}{}{}",
            layout.lead, layout.gap, layout.rest
        ));
    }
    let (lead, gap) = match (layout.lead.trim().is_empty(), layout.rest.is_empty()) {
        (true, false) => (layout.lead, layout.gap),
        (true, true) => (layout.lead, newline),
        (false, _) => ("", newline),
    };
    Ok(titled(lead, &heading, gap, description))
}

fn titled(lead: &str, heading: &str, gap: &str, description: &str) -> String {
    if description.is_empty() {
        return format!("{lead}{heading}");
    }
    let heading = match heading.ends_with('\n') {
        true => heading.to_owned(),
        false => format!("{heading}\n"),
    };
    format!("{lead}{heading}{gap}{description}")
}

fn heading_line(title: &str, ending: &str) -> Result<String, TitleError> {
    if title.contains(['\n', '\r']) {
        return Err(TitleError::LineBreak);
    }
    if title.trim().is_empty() {
        return Err(TitleError::Empty);
    }
    Ok(format!("# {title}{ending}"))
}

fn line_ending(line: &str) -> &str {
    if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

struct Layout<'a> {
    lead: &'a str,
    heading: &'a str,
    gap: &'a str,
    rest: &'a str,
    title: String,
}

impl<'a> Layout<'a> {
    // Marker lines read as markdown too, and `=======` under a line of text makes a setext heading,
    // so a heading that touches a conflict block is no title line.
    fn of(body: &'a str) -> Option<Self> {
        let blocks = conflict::in_body(body);
        let heading = op_md::headings(body).into_iter().find(|heading| {
            heading.level == 1
                && !blocks
                    .iter()
                    .any(|block| block.range.start < heading.end && heading.start < block.range.end)
        })?;
        let start = body[..heading.start].rfind('\n').map_or(0, |at| at + 1);
        let end = match body[..heading.end].ends_with('\n') {
            true => heading.end,
            false => body[heading.end..]
                .find('\n')
                .map_or(body.len(), |at| heading.end + at + 1),
        };
        let gap = body[end..]
            .split_inclusive('\n')
            .take_while(|line| line.trim().is_empty())
            .map(str::len)
            .sum::<usize>();
        Some(Self {
            lead: &body[..start],
            heading: &body[start..end],
            gap: &body[end..end + gap],
            rest: &body[end + gap..],
            title: heading.text,
        })
    }

    fn description(&self) -> String {
        match self.lead.trim().is_empty() {
            true => self.rest.to_owned(),
            false => format!("{}{}{}", self.lead, self.gap, self.rest),
        }
    }
}
