use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use op_server::{AppState, app};
use tower::ServiceExt;

async fn get(uri: &str) -> axum::response::Response {
    app(AppState::default())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn health_returns_ok() {
    let response = get("/health").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"ok");
}

#[tokio::test]
async fn spa_index_served_with_charset() {
    let response = get("/").await;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(content_type, "text/html; charset=utf-8");
}

#[tokio::test]
async fn matrix_endpoint_returns_empty() {
    let response = get("/api/matrix").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], br#"{"cells":[]}"#);
}

#[tokio::test]
async fn health_reports_identity_when_set() {
    let info = op_api::DaemonInfo {
        pid: 4242,
        port: 9,
        version: "9.9.9".to_owned(),
        started_at: 5,
    };
    let response = app(AppState::default().with_health(info.clone()))
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let served: op_api::DaemonInfo = serde_json::from_slice(&body).unwrap();
    assert_eq!(served, info);
}

#[tokio::test]
async fn admin_shutdown_returns_ok_with_admin_header() {
    let response = app(AppState::default())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/shutdown")
                .header(op_api::ADMIN_HEADER, "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_shutdown_forbidden_without_admin_header() {
    let response = app(AppState::default())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/shutdown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn serve_stops_on_external_shutdown() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(op_server::serve(
        listener,
        AppState::default(),
        async move {
            let _ = rx.await;
        },
    ));

    tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}
