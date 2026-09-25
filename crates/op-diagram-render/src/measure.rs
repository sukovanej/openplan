mod inter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    SemiBold,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineMetrics {
    pub ascent: f32,
    pub descent: f32,
}

// The browser draws a character outside the table in a fallback font the table knows nothing of.
// One em is wider than most such glyphs, so the label still fits its shape.
const FALLBACK: f32 = inter::UNITS_PER_EM;

// Chromium lays SVG text out on a grid of 1/64 px and rounds the width of a run up to it. The same
// rounding here keeps the width the layout reserves from ever falling short of the drawn one.
const GRID: f32 = 64.0;

pub fn text_width(text: &str, size: f32, weight: Weight) -> f32 {
    let table = match weight {
        Weight::Regular => inter::REGULAR,
        Weight::SemiBold => inter::SEMI_BOLD,
    };
    let units: f32 = text.chars().map(|c| advance(table, c)).sum();
    (units * size / inter::UNITS_PER_EM * GRID).ceil() / GRID
}

pub fn line_metrics(size: f32) -> LineMetrics {
    LineMetrics {
        ascent: inter::ASCENDER * size / inter::UNITS_PER_EM,
        descent: -inter::DESCENDER * size / inter::UNITS_PER_EM,
    }
}

fn advance(table: &[(u32, &[u16])], c: char) -> f32 {
    let code = u32::from(c);
    table
        .iter()
        .find_map(|(first, widths)| {
            let at = code.checked_sub(*first)?;
            widths.get(usize::try_from(at).ok()?)
        })
        .map_or(FALLBACK, |width| f32::from(*width))
}
