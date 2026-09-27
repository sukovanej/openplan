use crate::scene::{Outline, Point, Rect};

const CORNER: f32 = 5.0;
const ARC_STEPS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

pub(crate) struct Bend {
    pub(crate) start: Point,
    pub(crate) end: Point,
    pub(crate) radius: f32,
    pub(crate) clockwise: bool,
    center: Point,
}

pub(crate) fn corners(outline: &Outline, rect: &Rect) -> Option<Vec<Point>> {
    let Rect {
        x,
        y,
        width: w,
        height: h,
    } = *rect;
    let slant = h / 2.0;
    let points: &[(f32, f32)] = match outline {
        Outline::Asymmetric => &[
            (x, y),
            (x + w, y),
            (x + w, y + h),
            (x, y + h),
            (x + h / 2.0, y + h / 2.0),
        ],
        Outline::Diamond => &[
            (x + w / 2.0, y),
            (x + w, y + h / 2.0),
            (x + w / 2.0, y + h),
            (x, y + h / 2.0),
        ],
        Outline::Hexagon => {
            let side = h / 4.0;
            &[
                (x + side, y),
                (x + w - side, y),
                (x + w, y + h / 2.0),
                (x + w - side, y + h),
                (x + side, y + h),
                (x, y + h / 2.0),
            ]
        }
        Outline::LeanRight => &[
            (x + slant, y),
            (x + w, y),
            (x + w - slant, y + h),
            (x, y + h),
        ],
        Outline::LeanLeft => &[
            (x, y),
            (x + w - slant, y),
            (x + w, y + h),
            (x + slant, y + h),
        ],
        Outline::Trapezoid => &[
            (x + slant, y),
            (x + w - slant, y),
            (x + w, y + h),
            (x, y + h),
        ],
        Outline::InvertedTrapezoid => &[
            (x, y),
            (x + w, y),
            (x + w - slant, y + h),
            (x + slant, y + h),
        ],
        _ => return None,
    };
    Some(points.iter().map(|&(x, y)| Point { x, y }).collect())
}

// Each corner becomes an arc tangent to its two sides. A short side takes a smaller arc, so two arcs
// never overlap.
pub(crate) fn bends(corners: &[Point]) -> Vec<Bend> {
    let count = corners.len();
    (0..count)
        .map(|at| {
            let corner = corners[at];
            let before = corners[(at + count - 1) % count];
            let after = corners[(at + 1) % count];
            let (to_before, before_length) = unit(corner, before);
            let (to_after, after_length) = unit(corner, after);
            let half = (to_before.x * to_after.x + to_before.y * to_after.y)
                .clamp(-1.0, 1.0)
                .acos()
                / 2.0;
            let reach = (CORNER / half.tan())
                .min(before_length / 2.0)
                .min(after_length / 2.0);
            let radius = reach * half.tan();
            let (bisector, _) = unit(
                Point { x: 0.0, y: 0.0 },
                Point {
                    x: to_before.x + to_after.x,
                    y: to_before.y + to_after.y,
                },
            );
            let center_distance = radius / half.sin();
            let start = along(corner, to_before, reach);
            let end = along(corner, to_after, reach);
            let center = along(corner, bisector, center_distance);
            let clockwise = (start.x - center.x) * (end.y - center.y)
                - (start.y - center.y) * (end.x - center.x)
                > 0.0;
            Bend {
                start,
                end,
                radius,
                clockwise,
                center,
            }
        })
        .collect()
}

// How far in from the side of its box an outline starts, on the line that crosses that side
// `offset` from its middle. An edge that ends there touches the outline.
pub(crate) fn inset(outline: &Outline, width: f32, height: f32, side: Side, offset: f32) -> f32 {
    if let Outline::Cylinder { cap } = outline {
        if matches!(side, Side::Left | Side::Right) {
            return 0.0;
        }
        let reach = offset / (width / 2.0);
        return cap * (1.0 - (1.0 - reach * reach).max(0.0).sqrt());
    }
    let rect = Rect {
        x: 0.0,
        y: 0.0,
        width,
        height,
    };
    let Some(corners) = corners(outline, &rect) else {
        return 0.0;
    };
    let boundary = boundary(&bends(&corners));
    let crossings = boundary
        .iter()
        .zip(boundary.iter().cycle().skip(1))
        .filter_map(|(&a, &b)| match side {
            Side::Top | Side::Bottom => cross(a.x, a.y, b.x, b.y, width / 2.0 + offset),
            Side::Left | Side::Right => cross(a.y, a.x, b.y, b.x, height / 2.0 + offset),
        });
    let found = match side {
        Side::Top | Side::Left => crossings.reduce(f32::min),
        Side::Bottom => crossings.reduce(f32::max).map(|y| height - y),
        Side::Right => crossings.reduce(f32::max).map(|x| width - x),
    };
    found.unwrap_or(0.0)
}

fn boundary(bends: &[Bend]) -> Vec<Point> {
    bends
        .iter()
        .flat_map(|bend| {
            let first = (bend.start.y - bend.center.y).atan2(bend.start.x - bend.center.x);
            let mut sweep = (bend.end.y - bend.center.y).atan2(bend.end.x - bend.center.x) - first;
            if sweep > std::f32::consts::PI {
                sweep -= std::f32::consts::TAU;
            } else if sweep < -std::f32::consts::PI {
                sweep += std::f32::consts::TAU;
            }
            (0..=ARC_STEPS).map(move |step| {
                let angle = first + sweep * step as f32 / ARC_STEPS as f32;
                Point {
                    x: bend.center.x + bend.radius * angle.cos(),
                    y: bend.center.y + bend.radius * angle.sin(),
                }
            })
        })
        .collect()
}

// Where the segment from (a_along, a_across) to (b_along, b_across) crosses the line at `along`.
fn cross(a_along: f32, a_across: f32, b_along: f32, b_across: f32, along: f32) -> Option<f32> {
    let (low, high) = (a_along.min(b_along), a_along.max(b_along));
    if high <= low || !(low..=high).contains(&along) {
        return None;
    }
    Some(a_across + (b_across - a_across) * (along - a_along) / (b_along - a_along))
}

fn unit(from: Point, to: Point) -> (Point, f32) {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = dx.hypot(dy);
    (
        Point {
            x: dx / length,
            y: dy / length,
        },
        length,
    )
}

fn along(from: Point, direction: Point, distance: f32) -> Point {
    Point {
        x: from.x + direction.x * distance,
        y: from.y + direction.y * distance,
    }
}
