use op_diagram::Icon;

pub(crate) const GRID: f32 = 24.0;

// Copied from Lucide 1.24.0 under the licenses in Lucide-LICENSE.txt. Each icon is drawn on a grid
// of 24 by 24 with a stroke of 2 and no fill.
pub(crate) fn shapes(icon: Icon) -> &'static str {
    match icon {
        Icon::CircleAlert => {
            r#"<circle cx="12" cy="12" r="10"/><line x1="12" x2="12" y1="8" y2="12"/><line x1="12" x2="12.01" y1="16" y2="16"/>"#
        }
        Icon::CircleCheck => r#"<circle cx="12" cy="12" r="10"/><path d="m9 12 2 2 4-4"/>"#,
        Icon::CircleDashed => {
            r#"<path d="M10.1 2.182a10 10 0 0 1 3.8 0"/><path d="M13.9 21.818a10 10 0 0 1-3.8 0"/><path d="M17.609 3.721a10 10 0 0 1 2.69 2.7"/><path d="M2.182 13.9a10 10 0 0 1 0-3.8"/><path d="M20.279 17.609a10 10 0 0 1-2.7 2.69"/><path d="M21.818 10.1a10 10 0 0 1 0 3.8"/><path d="M3.721 6.391a10 10 0 0 1 2.7-2.69"/><path d="M6.391 20.279a10 10 0 0 1-2.69-2.7"/>"#
        }
        Icon::CircleDot => r#"<circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="1"/>"#,
        Icon::CircleEllipsis => {
            r#"<circle cx="12" cy="12" r="10"/><path d="M17 12h.01"/><path d="M12 12h.01"/><path d="M7 12h.01"/>"#
        }
        Icon::CircleX => {
            r#"<circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>"#
        }
        Icon::Clock => r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>"#,
        Icon::Eye => {
            r#"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/>"#
        }
    }
}

pub(crate) fn name(icon: Icon) -> &'static str {
    match icon {
        Icon::CircleAlert => "circle-alert",
        Icon::CircleCheck => "circle-check",
        Icon::CircleDashed => "circle-dashed",
        Icon::CircleDot => "circle-dot",
        Icon::CircleEllipsis => "circle-ellipsis",
        Icon::CircleX => "circle-x",
        Icon::Clock => "clock",
        Icon::Eye => "eye",
    }
}
