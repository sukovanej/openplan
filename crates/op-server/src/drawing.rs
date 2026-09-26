use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::{Query, State};
use op_api::{ApiErrorBody, DiagramSource, Drawing, Status};
use op_diagram::{Diagram, Page};

use crate::{ApiError, AppState, blocking, tasks};

// The layout is the slow part of a drawing, and a page asks for the same diagrams each time it
// shows a task. The cache keeps the drawings that were used last, up to `budget` bytes of SVG.
pub struct DrawingCache {
    budget: usize,
    held: Mutex<Held>,
}

#[derive(Default)]
struct Held {
    drawings: HashMap<Diagram, Kept>,
    bytes: usize,
    clock: u64,
}

struct Kept {
    drawing: Arc<Drawing>,
    used: u64,
}

impl DrawingCache {
    pub const BUDGET: usize = 16 * 1024 * 1024;

    pub fn new(budget: usize) -> Self {
        Self {
            budget,
            held: Mutex::default(),
        }
    }

    pub fn draw(&self, diagram: Diagram) -> Arc<Drawing> {
        if let Some(drawing) = self.used(&diagram) {
            return drawing;
        }
        let scene = op_diagram_render::layout(&diagram);
        let drawing = Arc::new(Drawing {
            svg: op_diagram_render::svg(&scene),
            width: scene.width,
            height: scene.height,
        });
        self.keep(diagram, Arc::clone(&drawing));
        drawing
    }

    pub fn holds(&self, diagram: &Diagram) -> bool {
        self.lock().drawings.contains_key(diagram)
    }

    fn used(&self, diagram: &Diagram) -> Option<Arc<Drawing>> {
        let mut held = self.lock();
        held.clock += 1;
        let now = held.clock;
        let kept = held.drawings.get_mut(diagram)?;
        kept.used = now;
        Some(Arc::clone(&kept.drawing))
    }

    fn keep(&self, diagram: Diagram, drawing: Arc<Drawing>) {
        let size = drawing.svg.len();
        if size > self.budget {
            return;
        }
        let mut held = self.lock();
        held.clock += 1;
        let used = held.clock;
        if let Some(replaced) = held.drawings.insert(diagram, Kept { drawing, used }) {
            held.bytes -= replaced.drawing.svg.len();
        }
        held.bytes += size;
        while held.bytes > self.budget {
            let Some(oldest) = held
                .drawings
                .iter()
                .min_by_key(|(_, kept)| kept.used)
                .map(|(diagram, _)| diagram.clone())
            else {
                break;
            };
            if let Some(dropped) = held.drawings.remove(&oldest) {
                held.bytes -= dropped.drawing.svg.len();
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Held> {
        self.held.lock().expect("drawing cache poisoned")
    }
}

#[utoipa::path(
    post,
    path = "/api/diagram",
    request_body = DiagramSource,
    responses(
        (status = 200, description = "The SVG of the diagram", body = Drawing),
        (status = 422, description = "The source is not Mermaid that the daemon draws; the body gives the line and the column", body = ApiErrorBody)
    )
)]
pub(crate) async fn draw_diagram(
    State(state): State<AppState>,
    Json(body): Json<DiagramSource>,
) -> Result<Json<Drawing>, ApiError> {
    let drawings = state.drawings();
    let drawing = blocking(move || {
        let diagram = op_diagram_mermaid::parse(&body.source)?;
        Ok(drawings.draw(diagram))
    })
    .await?;
    Ok(Json(drawing.as_ref().clone()))
}

#[utoipa::path(
    get,
    path = "/api/flow/drawing",
    params(
        ("project" = Option<Vec<String>>, Query, description = "Project name; omit to take every project the daemon serves"),
        ("status" = Option<Vec<Status>>, Query, description = "Seed status; omit it to seed every task that is not done or cancelled"),
        ("task" = Option<Vec<String>>, Query, description = "Task key; it needs a project"),
        ("tag" = Option<Vec<String>>, Query, description = "Tag name"),
        ("width" = Option<u32>, Query, description = "Width of the page in CSS pixels. With the height, each part of the flow that no edge joins to another is laid out on its own, and the parts fill the shape of the page"),
        ("height" = Option<u32>, Query, description = "Height of the page in CSS pixels; it comes with the width")
    ),
    responses(
        (status = 200, description = "The SVG of the flow: one row for each wave, and a link to the page of each task", body = Drawing),
        (status = 400, description = "The query names an unknown parameter, an unknown status, a task without a project, or a page size that is not two whole numbers above zero", body = ApiErrorBody),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 422, description = "The dependencies of the selected tasks form a cycle", body = ApiErrorBody),
        (status = 503, description = "A named project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn draw_flow(
    State(state): State<AppState>,
    Query(parameters): Query<Vec<(String, String)>>,
) -> Result<Json<Drawing>, ApiError> {
    let (page, parameters) = page_of(parameters)?;
    let flow = tasks::flow(&state, &parameters).await?;
    let drawings = state.drawings();
    let drawing = blocking(move || Ok(drawings.draw(flow.diagram(page)))).await?;
    Ok(Json(drawing.as_ref().clone()))
}

type Parameters = Vec<(String, String)>;

fn page_of(parameters: Parameters) -> Result<(Option<Page>, Parameters), ApiError> {
    let (mut width, mut height) = (None, None);
    let mut rest = Vec::new();
    for (name, value) in parameters {
        match name.as_str() {
            "width" => width = Some(pixels(&name, &value)?),
            "height" => height = Some(pixels(&name, &value)?),
            _ => rest.push((name, value)),
        }
    }
    let page = match (width, height) {
        (Some(width), Some(height)) => Some(Page { width, height }),
        (None, None) => None,
        _ => {
            return Err(ApiError::bad_request(
                "give the width and the height of the page together: the flow fills its shape",
            ));
        }
    };
    Ok((page, rest))
}

fn pixels(name: &str, value: &str) -> Result<u32, ApiError> {
    value
        .parse::<u32>()
        .ok()
        .filter(|pixels| *pixels > 0)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "the {name} of the page must be a whole number of pixels above zero, not {value:?}"
            ))
        })
}
