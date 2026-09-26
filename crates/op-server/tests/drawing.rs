mod common;

use axum::http::StatusCode;
use common::*;
use op_server::{AppState, DrawingCache};
use serde_json::{Value, json};

async fn draw(state: &AppState, source: &str) -> (StatusCode, Value) {
    let response = send(
        state,
        "POST",
        "/api/diagram",
        Some(json!({ "source": source })),
    )
    .await;
    let status = response.status();
    (status, body_json(response).await)
}

fn two_projects() -> (tempfile::TempDir, tempfile::TempDir, AppState) {
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    let state = AppState::new([
        local_project("alpha", alpha.path(), "AAA"),
        local_project("beta", beta.path(), "BBB"),
    ]);
    (alpha, beta, state)
}

async fn todo(state: &AppState, project: &str, title: &str, dependencies: &[&str]) -> String {
    create_in(
        state,
        project,
        json!({ "title": title, "status": "todo", "dependencies": dependencies }),
    )
    .await
}

#[tokio::test]
async fn a_posted_diagram_comes_back_as_svg() {
    let (_dir, state) = local_state();
    let (status, body) = draw(&state, "flowchart LR\n  a[Parse] --> b[Draw]\n").await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let svg = body["svg"].as_str().unwrap();
    assert!(svg.starts_with("<svg "), "{svg}");
    assert!(svg.contains(">Parse</text>"), "{svg}");
    assert!(body["width"].as_f64().unwrap() > 0.0);
    assert!(body["height"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn a_source_that_does_not_parse_gives_the_place_it_stops() {
    let (_dir, state) = local_state();
    let (status, body) = draw(&state, "flowchart LR\n  a --> b[unclosed\n").await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["position"], json!({ "line": 2, "column": 10 }));
    assert_eq!(
        message_of(&body),
        "line 2, column 10: the text of `b` has no closing `]`"
    );
}

#[tokio::test]
async fn another_refusal_sends_no_position() {
    let (_alpha, _beta, state) = two_projects();
    let body = body_json(send(&state, "GET", "/api/flow/drawing?project=gamma", None).await).await;

    assert!(body.get("position").is_none());
}

#[tokio::test]
async fn an_empty_diagram_has_no_size() {
    let (_dir, state) = local_state();
    let (status, body) = draw(&state, "flowchart TD\n").await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["width"], json!(0.0));
    assert_eq!(body["height"], json!(0.0));
}

#[tokio::test]
async fn the_daemon_keeps_a_drawing_for_the_next_request() {
    let (_dir, state) = local_state();
    let source = "sequenceDiagram\n  Alice ->> Bob: Hello\n";
    let diagram = op_diagram_mermaid::parse(source).unwrap();
    assert!(!state.drawings().holds(&diagram));

    let (_, first) = draw(&state, source).await;
    assert!(state.drawings().holds(&diagram));
    let (_, second) = draw(&state, source).await;

    assert_eq!(first, second);
}

#[tokio::test]
async fn the_flow_drawing_links_each_task_to_its_page() {
    let (_alpha, _beta, state) = two_projects();
    let first = todo(&state, "alpha", "alpha one", &[]).await;
    todo(&state, "alpha", "alpha two", &[&first]).await;
    todo(&state, "beta", "beta one", &[]).await;

    let body = json_of(&state, "/api/flow/drawing").await;
    let svg = body["svg"].as_str().unwrap();

    for page in ["/alpha/task/AAA-1", "/alpha/task/AAA-2", "/beta/task/BBB-1"] {
        assert!(
            svg.contains(&format!(r#"<a href="{page}">"#)),
            "{page}: {svg}"
        );
    }
    assert!(svg.contains(">alpha two</text>"), "{svg}");
    assert!(svg.contains("status-todo"), "{svg}");
}

#[tokio::test]
async fn the_flow_drawing_fills_the_shape_of_the_page() {
    let (_alpha, _beta, state) = two_projects();
    for number in 1..=9 {
        todo(&state, "alpha", &format!("task {number}"), &[]).await;
    }

    let row = json_of(&state, "/api/flow/drawing").await;
    let square = json_of(&state, "/api/flow/drawing?width=900&height=900").await;
    let shape = |body: &Value| body["width"].as_f64().unwrap() / body["height"].as_f64().unwrap();

    assert!(shape(&row) > 5.0, "{}", shape(&row));
    assert!((0.5..2.0).contains(&shape(&square)), "{}", shape(&square));
}

#[tokio::test]
async fn a_page_takes_a_width_and_a_height_above_zero() {
    let (_alpha, _beta, state) = two_projects();
    for query in [
        "width=900",
        "height=900",
        "width=0&height=900",
        "width=wide&height=900",
    ] {
        let response = send(&state, "GET", &format!("/api/flow/drawing?{query}"), None).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{query}");
    }
}

#[tokio::test]
async fn an_empty_flow_has_no_size() {
    let (_alpha, _beta, state) = two_projects();
    let body = json_of(&state, "/api/flow/drawing").await;

    assert_eq!(body["width"], json!(0.0));
    assert_eq!(body["height"], json!(0.0));
}

fn diagram(source: &str) -> op_diagram::Diagram {
    op_diagram_mermaid::parse(source).unwrap()
}

fn svg_size(diagram: &op_diagram::Diagram) -> usize {
    DrawingCache::new(DrawingCache::BUDGET)
        .draw(diagram.clone())
        .svg
        .len()
}

#[test]
fn the_cache_drops_the_drawing_used_longest_ago() {
    let first = diagram("flowchart LR\n  a --> b\n");
    let second = diagram("flowchart LR\n  c --> d\n");
    let third = diagram("flowchart LR\n  e --> f\n");
    let cache = DrawingCache::new(svg_size(&first) + svg_size(&second) + svg_size(&third) - 1);

    cache.draw(first.clone());
    cache.draw(second.clone());
    cache.draw(first.clone());
    cache.draw(third.clone());

    assert!(cache.holds(&first));
    assert!(!cache.holds(&second));
    assert!(cache.holds(&third));
}

#[test]
fn the_cache_does_not_keep_a_drawing_larger_than_its_budget() {
    let large = diagram("flowchart LR\n  a --> b\n");
    let cache = DrawingCache::new(svg_size(&large) - 1);

    assert!(cache.draw(large.clone()).svg.starts_with("<svg "));
    assert!(!cache.holds(&large));
}
