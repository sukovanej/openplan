use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use op_update::Github;
use semver::Version;
use sha2::{Digest as _, Sha256};

struct FakeRelease {
    tag: &'static str,
    assets: HashMap<String, Vec<u8>>,
}

fn serve(release: FakeRelease) -> (tokio::runtime::Runtime, String) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let state = Arc::new((release, base.clone()));
    let app = Router::new()
        .route("/repos/acme/openplan/releases/latest", get(latest))
        .route("/dl/{name}", get(asset))
        .with_state(state);
    runtime.spawn(async move { axum::serve(listener, app).await.unwrap() });
    (runtime, base)
}

async fn latest(State(state): State<Arc<(FakeRelease, String)>>) -> impl IntoResponse {
    let (release, base) = &*state;
    let assets: Vec<serde_json::Value> = release
        .assets
        .keys()
        .map(|name| {
            serde_json::json!({ "name": name, "browser_download_url": format!("{base}/dl/{name}") })
        })
        .collect();
    axum::Json(serde_json::json!({ "tag_name": release.tag, "assets": assets }))
}

async fn asset(
    State(state): State<Arc<(FakeRelease, String)>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.0.assets.get(&name) {
        Some(bytes) => (StatusCode::OK, bytes.clone()),
        None => (StatusCode::NOT_FOUND, Vec::new()),
    }
}

fn digest_of(bytes: &[u8], name: &str) -> Vec<u8> {
    format!("{:x} *{name}\n", Sha256::digest(bytes)).into_bytes()
}

fn release_with(archive: &[u8], digest: &[u8]) -> FakeRelease {
    FakeRelease {
        tag: "v1.2.3",
        assets: HashMap::from([
            ("openplan-x.tar.gz".to_owned(), archive.to_vec()),
            ("openplan-x.tar.gz.sha256".to_owned(), digest.to_vec()),
        ]),
    }
}

#[test]
fn the_latest_release_parses_its_tag_as_a_version() {
    let (_runtime, base) = serve(release_with(b"bytes", b""));
    let release = Github::at(&base, "acme/openplan").latest_release().unwrap();
    assert_eq!(release.version, Version::new(1, 2, 3));
    assert_eq!(release.tag, "v1.2.3");
}

#[test]
fn a_verified_download_returns_the_bytes() {
    let archive = b"the archive";
    let (_runtime, base) = serve(release_with(
        archive,
        &digest_of(archive, "openplan-x.tar.gz"),
    ));
    let github = Github::at(&base, "acme/openplan");
    let release = github.latest_release().unwrap();

    let bytes = github
        .download_verified(&release, "openplan-x.tar.gz")
        .unwrap();

    assert_eq!(bytes, archive);
}

#[test]
fn a_download_that_does_not_match_its_digest_fails() {
    let (_runtime, base) = serve(release_with(
        b"tampered",
        &digest_of(b"original", "openplan-x.tar.gz"),
    ));
    let github = Github::at(&base, "acme/openplan");
    let release = github.latest_release().unwrap();

    let err = github
        .download_verified(&release, "openplan-x.tar.gz")
        .unwrap_err();

    assert!(format!("{err:#}").contains("digest mismatch"), "{err:#}");
}

#[test]
fn a_missing_asset_names_the_release() {
    let (_runtime, base) = serve(release_with(b"bytes", b""));
    let github = Github::at(&base, "acme/openplan");
    let release = github.latest_release().unwrap();

    let err = github
        .download_verified(&release, "OpenPlan-x.app.tar.gz")
        .unwrap_err();

    assert!(
        format!("{err:#}").contains("no asset named OpenPlan-x.app.tar.gz"),
        "{err:#}"
    );
}
