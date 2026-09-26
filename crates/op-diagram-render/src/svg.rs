use std::fmt::Write;

use op_diagram::{Head, Stroke};

use crate::measure::Weight;
use crate::scene::{
    Anchor, ClusterBox, EdgePath, GuideKind, NodeBox, Outline, Point, Rect, Scene, Text, TextRole,
};

const CORNER: f32 = 8.0;
const ARROW_LENGTH: f32 = 9.0;
const ARROW_HALF: f32 = 4.5;
const MARK: f32 = 5.0;
const RING: f32 = 4.0;
const ROUNDED: f32 = 10.0;
const SQUARE: f32 = 3.0;
const CYLINDER_CAP: f32 = 7.0;
const DOUBLE_RING: f32 = 5.0;
const SUBROUTINE_BAR: f32 = 8.0;
const NOTE_FOLD: f32 = 8.0;

// The SVG carries classes and no colors, so the page styles it in its own theme. Every text and
// attribute value passes through `escape`, and a link that is not a path on this site is left out:
// what a diagram says can never run as markup or send the reader elsewhere.
pub fn svg(scene: &Scene) -> String {
    let mut out = String::new();
    let (width, height) = (number(scene.width), number(scene.height));
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" class="op-diagram" viewBox="0 0 {width} {height}" width="{width}" height="{height}" font-family="Inter Diagram, sans-serif" text-rendering="geometricPrecision">"#
    );
    let mut clusters: Vec<&ClusterBox> = scene.clusters.iter().collect();
    clusters.sort_by_key(|cluster| cluster.depth);
    for cluster in &clusters {
        let _ = write!(
            out,
            r#"<g class="{}">"#,
            classes("cluster", &cluster.classes)
        );
        write_rect(&mut out, "cluster-box", &cluster.rect, 6.0);
        out.push_str("</g>");
    }
    for guide in &scene.guides {
        let class = match guide.kind {
            GuideKind::Lifeline => "lifeline",
            GuideKind::Divider => "divider",
        };
        let _ = write!(
            out,
            r#"<path class="{class}" d="M{} {}L{} {}"/>"#,
            number(guide.from.x),
            number(guide.from.y),
            number(guide.to.x),
            number(guide.to.y)
        );
    }
    for edge in &scene.edges {
        write_edge(&mut out, edge);
    }
    for node in &scene.nodes {
        write_node(&mut out, node);
    }
    // An edge into a cluster has to cross its header, so the header goes on top of the edges and
    // the page can give its text a halo that breaks the line around the words.
    for cluster in &clusters {
        write_cluster_header(&mut out, cluster);
    }
    for edge in &scene.edges {
        if let Some(label) = &edge.label {
            out.push_str(r#"<g class="edge-label">"#);
            write_rect(&mut out, "edge-label-box", &label.rect, 3.0);
            write_texts(&mut out, &label.texts);
            out.push_str("</g>");
        }
    }
    out.push_str("</svg>");
    out
}

fn classes(base: &str, extra: &[String]) -> String {
    let mut all = base.to_owned();
    for class in extra {
        all.push(' ');
        all.push_str(class);
    }
    escape(&all)
}

fn safe_link(link: &Option<String>) -> Option<String> {
    link.as_ref()
        .filter(|link| link.starts_with('/') && !link.starts_with("//"))
        .map(|link| escape(link))
}

fn write_cluster_header(out: &mut String, cluster: &ClusterBox) {
    let link = safe_link(&cluster.link);
    out.push_str(r#"<g class="cluster-header">"#);
    if let Some(href) = &link {
        let _ = write!(out, r#"<a href="{href}">"#);
    }
    write_texts(out, &cluster.texts);
    if link.is_some() {
        out.push_str("</a>");
    }
    out.push_str("</g>");
}

fn write_node(out: &mut String, node: &NodeBox) {
    let link = safe_link(&node.link);
    if let Some(href) = &link {
        let _ = write!(out, r#"<a href="{href}">"#);
    }
    let _ = write!(
        out,
        r#"<g class="{}">"#,
        classes(
            &format!("node shape-{}", shape_name(&node.outline)),
            &node.classes
        )
    );
    write_outline(out, &node.outline, &node.rect);
    write_texts(out, &node.texts);
    out.push_str("</g>");
    if link.is_some() {
        out.push_str("</a>");
    }
}

fn shape_name(outline: &Outline) -> &'static str {
    match outline {
        Outline::Rectangle => "rectangle",
        Outline::Rounded => "rounded",
        Outline::Stadium => "stadium",
        Outline::Subroutine => "subroutine",
        Outline::Cylinder => "cylinder",
        Outline::Circle => "circle",
        Outline::DoubleCircle => "double-circle",
        Outline::Asymmetric => "asymmetric",
        Outline::Diamond => "diamond",
        Outline::Hexagon => "hexagon",
        Outline::LeanRight => "lean-right",
        Outline::LeanLeft => "lean-left",
        Outline::Trapezoid => "trapezoid",
        Outline::InvertedTrapezoid => "inverted-trapezoid",
        Outline::Table { .. } => "table",
        Outline::Actor => "actor",
        Outline::Note => "note",
    }
}

fn write_rect(out: &mut String, class: &str, rect: &Rect, radius: f32) {
    let _ = write!(
        out,
        r#"<rect class="{class}" x="{}" y="{}" width="{}" height="{}" rx="{}"/>"#,
        number(rect.x),
        number(rect.y),
        number(rect.width),
        number(rect.height),
        number(radius)
    );
}

fn write_polygon(out: &mut String, points: &[(f32, f32)]) {
    let list: Vec<String> = points
        .iter()
        .map(|(x, y)| format!("{},{}", number(*x), number(*y)))
        .collect();
    let _ = write!(
        out,
        r#"<polygon class="outline" points="{}"/>"#,
        list.join(" ")
    );
}

fn write_outline(out: &mut String, outline: &Outline, rect: &Rect) {
    let Rect {
        x,
        y,
        width: w,
        height: h,
    } = *rect;
    match outline {
        Outline::Rectangle => write_rect(out, "outline", rect, SQUARE),
        Outline::Rounded => write_rect(out, "outline", rect, ROUNDED.min(h / 2.0)),
        Outline::Stadium => write_rect(out, "outline", rect, h / 2.0),
        Outline::Subroutine => {
            write_rect(out, "outline", rect, SQUARE);
            for bar in [x + SUBROUTINE_BAR, x + w - SUBROUTINE_BAR] {
                let _ = write!(
                    out,
                    r#"<path class="outline-detail" d="M{} {}V{}"/>"#,
                    number(bar),
                    number(y),
                    number(y + h)
                );
            }
        }
        Outline::Cylinder => {
            let (rx, ry) = (w / 2.0, CYLINDER_CAP);
            let _ = write!(
                out,
                r#"<path class="outline" d="M{x0} {top}A{rx} {ry} 0 0 0 {x1} {top}A{rx} {ry} 0 0 0 {x0} {top}V{bottom}A{rx} {ry} 0 0 0 {x1} {bottom}V{top}"/>"#,
                x0 = number(x),
                x1 = number(x + w),
                top = number(y + ry),
                bottom = number(y + h - ry),
                rx = number(rx),
                ry = number(ry),
            );
        }
        Outline::Circle | Outline::DoubleCircle => {
            let center = rect.center();
            let mut radius = w / 2.0;
            let rings = if matches!(outline, Outline::DoubleCircle) {
                2
            } else {
                1
            };
            for _ in 0..rings {
                let _ = write!(
                    out,
                    r#"<circle class="outline" cx="{}" cy="{}" r="{}"/>"#,
                    number(center.x),
                    number(center.y),
                    number(radius)
                );
                radius -= DOUBLE_RING;
            }
        }
        Outline::Asymmetric => write_polygon(
            out,
            &[
                (x, y),
                (x + w, y),
                (x + w, y + h),
                (x, y + h),
                (x + h / 2.0, y + h / 2.0),
            ],
        ),
        Outline::Diamond => write_polygon(
            out,
            &[
                (x + w / 2.0, y),
                (x + w, y + h / 2.0),
                (x + w / 2.0, y + h),
                (x, y + h / 2.0),
            ],
        ),
        Outline::Hexagon => {
            let side = h / 4.0;
            write_polygon(
                out,
                &[
                    (x + side, y),
                    (x + w - side, y),
                    (x + w, y + h / 2.0),
                    (x + w - side, y + h),
                    (x + side, y + h),
                    (x, y + h / 2.0),
                ],
            );
        }
        Outline::LeanRight => {
            let slant = h / 2.0;
            write_polygon(
                out,
                &[
                    (x + slant, y),
                    (x + w, y),
                    (x + w - slant, y + h),
                    (x, y + h),
                ],
            );
        }
        Outline::LeanLeft => {
            let slant = h / 2.0;
            write_polygon(
                out,
                &[
                    (x, y),
                    (x + w - slant, y),
                    (x + w, y + h),
                    (x + slant, y + h),
                ],
            );
        }
        Outline::Trapezoid => {
            let slant = h / 2.0;
            write_polygon(
                out,
                &[
                    (x + slant, y),
                    (x + w - slant, y),
                    (x + w, y + h),
                    (x, y + h),
                ],
            );
        }
        Outline::InvertedTrapezoid => {
            let slant = h / 2.0;
            write_polygon(
                out,
                &[
                    (x, y),
                    (x + w, y),
                    (x + w - slant, y + h),
                    (x + slant, y + h),
                ],
            );
        }
        Outline::Actor => {
            let center = x + w / 2.0;
            let _ = write!(
                out,
                r#"<circle class="outline" cx="{}" cy="{}" r="6"/><path class="actor-figure" d="M{c} {neck}V{hip}M{left} {arms}H{right}M{c} {hip}L{left_foot} {foot}M{c} {hip}L{right_foot} {foot}"/>"#,
                number(center),
                number(y + 7.0),
                c = number(center),
                neck = number(y + 13.0),
                hip = number(y + 24.0),
                arms = number(y + 17.0),
                left = number(center - 10.0),
                right = number(center + 10.0),
                left_foot = number(center - 8.0),
                right_foot = number(center + 8.0),
                foot = number(y + 33.0),
            );
        }
        Outline::Note => {
            let fold = NOTE_FOLD.min(h / 2.0);
            write_polygon(
                out,
                &[
                    (x, y),
                    (x + w - fold, y),
                    (x + w, y + fold),
                    (x + w, y + h),
                    (x, y + h),
                ],
            );
            let _ = write!(
                out,
                r#"<path class="outline-detail" d="M{} {}V{}H{}"/>"#,
                number(x + w - fold),
                number(y),
                number(y + fold),
                number(x + w)
            );
        }
        Outline::Table { dividers } => {
            write_rect(out, "outline", rect, SQUARE);
            for divider in dividers {
                let _ = write!(
                    out,
                    r#"<path class="outline-detail" d="M{} {}H{}"/>"#,
                    number(x),
                    number(y + divider),
                    number(x + w)
                );
            }
        }
    }
}

fn role_class(role: TextRole) -> &'static str {
    match role {
        TextRole::Label => "text-label",
        TextRole::Caption => "text-caption",
        TextRole::Header => "text-header",
        TextRole::Cell => "text-cell",
        TextRole::EdgeLabel => "text-edge-label",
        TextRole::Number => "text-number",
    }
}

fn write_texts(out: &mut String, texts: &[Text]) {
    for text in texts {
        let anchor = match text.anchor {
            Anchor::Start => "start",
            Anchor::Middle => "middle",
        };
        let weight = match text.weight {
            Weight::Regular => "400",
            Weight::SemiBold => "600",
        };
        let _ = write!(
            out,
            r#"<text class="{}" x="{}" y="{}" font-size="{}" font-weight="{weight}" text-anchor="{anchor}">{}</text>"#,
            role_class(text.role),
            number(text.x),
            number(text.baseline),
            number(text.size),
            escape(&text.content)
        );
    }
}

fn stroke_name(stroke: Stroke) -> &'static str {
    match stroke {
        Stroke::Solid => "solid",
        Stroke::Dotted => "dotted",
        Stroke::Thick => "thick",
        Stroke::Invisible => "invisible",
    }
}

fn write_edge(out: &mut String, edge: &EdgePath) {
    if edge.points.len() < 2 {
        return;
    }
    let mut points = edge.points.clone();
    let last = points.len() - 1;
    points[0] = shorten(points[0], points[1], edge.tail);
    points[last] = shorten(points[last], points[last - 1], edge.head);
    let _ = write!(
        out,
        r#"<g class="edge stroke-{}"><path class="edge-line" d="{}"/>"#,
        stroke_name(edge.stroke),
        rounded_path(&points)
    );
    write_head(out, edge.points[1], edge.points[0], edge.tail);
    write_head(out, edge.points[last - 1], edge.points[last], edge.head);
    out.push_str("</g>");
}

// A filled arrow covers the end of its line, so the line stops where the arrow's base begins and
// its square end never shows past the tip.
fn shorten(end: Point, before: Point, head: Head) -> Point {
    let cut = match head {
        Head::Arrow => ARROW_LENGTH - 1.0,
        Head::Circle => 2.0 * RING,
        _ => return end,
    };
    let (dx, dy) = (end.x - before.x, end.y - before.y);
    let length = dx.hypot(dy);
    if length <= cut {
        return end;
    }
    Point {
        x: end.x - dx / length * cut,
        y: end.y - dy / length * cut,
    }
}

fn rounded_path(points: &[Point]) -> String {
    let mut path = format!("M{} {}", number(points[0].x), number(points[0].y));
    for at in 1..points.len() - 1 {
        let (from, turn, to) = (points[at - 1], points[at], points[at + 1]);
        let back = CORNER.min(distance(from, turn) / 2.0);
        let on = CORNER.min(distance(turn, to) / 2.0);
        let enter = toward(turn, from, back);
        let leave = toward(turn, to, on);
        let _ = write!(
            path,
            "L{} {}Q{} {} {} {}",
            number(enter.x),
            number(enter.y),
            number(turn.x),
            number(turn.y),
            number(leave.x),
            number(leave.y)
        );
    }
    let last = points[points.len() - 1];
    let _ = write!(path, "L{} {}", number(last.x), number(last.y));
    path
}

fn distance(a: Point, b: Point) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn toward(from: Point, to: Point, by: f32) -> Point {
    let length = distance(from, to);
    if length < f32::EPSILON {
        return from;
    }
    Point {
        x: from.x + (to.x - from.x) / length * by,
        y: from.y + (to.y - from.y) / length * by,
    }
}

fn write_head(out: &mut String, before: Point, end: Point, head: Head) {
    let length = distance(before, end);
    if length < f32::EPSILON || head == Head::None {
        return;
    }
    let (ux, uy) = ((end.x - before.x) / length, (end.y - before.y) / length);
    let (nx, ny) = (-uy, ux);
    let at = |along: f32, across: f32| Point {
        x: end.x - ux * along + nx * across,
        y: end.y - uy * along + ny * across,
    };
    let line = |from: Point, to: Point| {
        format!(
            "M{} {}L{} {}",
            number(from.x),
            number(from.y),
            number(to.x),
            number(to.y)
        )
    };
    let ring = |along: f32| {
        let center = at(along, 0.0);
        format!(
            r#"<circle class="head-ring" cx="{}" cy="{}" r="{}"/>"#,
            number(center.x),
            number(center.y),
            number(RING)
        )
    };
    let bar = |along: f32| line(at(along, MARK), at(along, -MARK));
    let crow = || {
        let root = at(2.4 * MARK, 0.0);
        format!(
            "{}{}{}",
            line(root, at(0.0, MARK)),
            line(root, end),
            line(root, at(0.0, -MARK))
        )
    };
    let (class, shape) = match head {
        Head::None => return,
        Head::Arrow => {
            let (left, right) = (at(ARROW_LENGTH, ARROW_HALF), at(ARROW_LENGTH, -ARROW_HALF));
            (
                "head-arrow",
                format!(
                    r#"<path d="M{} {}L{} {}L{} {}Z"/>"#,
                    number(end.x),
                    number(end.y),
                    number(left.x),
                    number(left.y),
                    number(right.x),
                    number(right.y)
                ),
            )
        }
        Head::OpenArrow => (
            "head-open",
            format!(
                r#"<path d="{}{}"/>"#,
                line(at(ARROW_LENGTH, ARROW_HALF), end),
                line(at(ARROW_LENGTH, -ARROW_HALF), end)
            ),
        ),
        Head::Circle => ("head-circle", ring(RING)),
        Head::Cross => {
            let middle = MARK + 1.0;
            (
                "head-cross",
                format!(
                    r#"<path d="{}{}"/>"#,
                    line(
                        at(middle - MARK * 0.7, MARK * 0.7),
                        at(middle + MARK * 0.7, -MARK * 0.7)
                    ),
                    line(
                        at(middle - MARK * 0.7, -MARK * 0.7),
                        at(middle + MARK * 0.7, MARK * 0.7)
                    )
                ),
            )
        }
        Head::ExactlyOne => (
            "head-cardinality",
            format!(r#"<path d="{}{}"/>"#, bar(1.2 * MARK), bar(2.0 * MARK)),
        ),
        Head::ZeroOrOne => (
            "head-cardinality",
            format!(
                r#"<path d="{}"/>{}"#,
                bar(1.2 * MARK),
                ring(2.0 * MARK + RING)
            ),
        ),
        Head::ZeroOrMore => (
            "head-cardinality",
            format!(r#"<path d="{}"/>{}"#, crow(), ring(2.4 * MARK + RING + 2.0)),
        ),
        Head::OneOrMore => (
            "head-cardinality",
            format!(r#"<path d="{}{}"/>"#, crow(), bar(2.4 * MARK + 2.0)),
        ),
    };
    let _ = write!(out, r#"<g class="head {class}">{shape}</g>"#);
}

fn number(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let text = format!("{rounded:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}
