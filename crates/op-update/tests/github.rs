use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use op_update::{Channel, Github, Step};
use semver::Version;
use sha2::{Digest as _, Sha256};

struct FakeRelease {
    tag: &'static str,
    title: &'static str,
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
        .route("/repos/acme/openplan/releases/latest", get(release_json))
        .route(
            "/repos/acme/openplan/releases/tags/canary",
            get(release_json),
        )
        .route("/dl/{name}", get(asset))
        .with_state(state);
    runtime.spawn(async move { axum::serve(listener, app).await.unwrap() });
    (runtime, base)
}

async fn release_json(State(state): State<Arc<(FakeRelease, String)>>) -> impl IntoResponse {
    let (release, base) = &*state;
    let assets: Vec<serde_json::Value> = release
        .assets
        .keys()
        .map(|name| {
            serde_json::json!({ "name": name, "browser_download_url": format!("{base}/dl/{name}") })
        })
        .collect();
    axum::Json(
        serde_json::json!({ "tag_name": release.tag, "name": release.title, "assets": assets }),
    )
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
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{hex} *{name}\n").into_bytes()
}

fn release_with(archive: &[u8], digest: &[u8]) -> FakeRelease {
    FakeRelease {
        tag: "v1.2.3",
        title: "v1.2.3",
        assets: HashMap::from([
            ("openplan-x.tar.gz".to_owned(), archive.to_vec()),
            ("openplan-x.tar.gz.sha256".to_owned(), digest.to_vec()),
        ]),
    }
}

#[test]
fn the_latest_release_parses_its_tag_as_a_version() {
    let (_runtime, base) = serve(release_with(b"bytes", b""));
    let release = Github::at(&base, "acme/openplan")
        .release(Channel::Stable)
        .unwrap();
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
    let release = github.release(Channel::Stable).unwrap();

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
    let release = github.release(Channel::Stable).unwrap();

    let err = github
        .download_verified(&release, "openplan-x.tar.gz")
        .unwrap_err();

    assert!(format!("{err:#}").contains("digest mismatch"), "{err:#}");
}

#[test]
fn a_missing_asset_names_the_release() {
    let (_runtime, base) = serve(release_with(b"bytes", b""));
    let github = Github::at(&base, "acme/openplan");
    let release = github.release(Channel::Stable).unwrap();

    let err = github
        .download_verified(&release, "OpenPlan-x.app.tar.gz")
        .unwrap_err();

    assert!(
        format!("{err:#}").contains("no asset named OpenPlan-x.app.tar.gz"),
        "{err:#}"
    );
}

#[test]
fn a_release_publishes_only_the_assets_it_lists() {
    let (_runtime, base) = serve(release_with(b"bytes", b""));
    let release = Github::at(&base, "acme/openplan")
        .release(Channel::Stable)
        .unwrap();

    assert!(release.publishes("openplan-x.tar.gz"));
    assert!(!release.publishes("OpenPlan-x.app.tar.gz"));
}

fn release_titled(tag: &'static str, title: &'static str) -> FakeRelease {
    FakeRelease {
        tag,
        title,
        assets: HashMap::new(),
    }
}

fn installed(version: &str) -> Version {
    Version::parse(version).unwrap()
}

#[test]
fn the_canary_release_takes_its_version_from_the_title() {
    let (_runtime, base) = serve(release_titled("canary", "0.4.1-canary.57"));
    let release = Github::at(&base, "acme/openplan")
        .release(Channel::Canary)
        .unwrap();

    assert_eq!(release.version, installed("0.4.1-canary.57"));
    assert_eq!(release.tag, "canary");
    assert_eq!(release.channel, Channel::Canary);
}

#[test]
fn a_canary_title_that_is_not_a_version_fails() {
    let (_runtime, base) = serve(release_titled("canary", "Canary build"));

    let err = Github::at(&base, "acme/openplan")
        .release(Channel::Canary)
        .map(|_| ())
        .unwrap_err();

    assert!(
        format!("{err:#}").contains(r#"the canary release title "Canary build" is not a version"#),
        "{err:#}"
    );
}

#[test]
fn the_canary_build_installs_when_it_is_not_the_installed_version() {
    let (_runtime, base) = serve(release_titled("canary", "0.4.1-canary.57"));
    let release = Github::at(&base, "acme/openplan")
        .release(Channel::Canary)
        .unwrap();

    assert_eq!(release.step_from(&installed("0.4.0")), Step::Install);
    assert_eq!(
        release.step_from(&installed("0.4.1-canary.56")),
        Step::Install
    );
    assert_eq!(
        release.step_from(&installed("0.4.1-canary.57")),
        Step::UpToDate
    );
}

#[test]
fn a_canary_build_goes_back_to_the_older_stable_release() {
    let (_runtime, base) = serve(release_titled("v0.4.0", "Version 0.4.0"));
    let release = Github::at(&base, "acme/openplan")
        .release(Channel::Stable)
        .unwrap();

    assert_eq!(
        release.step_from(&installed("0.4.1-canary.57")),
        Step::BackToStable
    );
}

#[test]
fn a_canary_build_moves_on_to_the_stable_release_of_its_version() {
    let (_runtime, base) = serve(release_titled("v0.4.1", "Version 0.4.1"));
    let release = Github::at(&base, "acme/openplan")
        .release(Channel::Stable)
        .unwrap();

    assert_eq!(
        release.step_from(&installed("0.4.1-canary.57")),
        Step::Install
    );
}

#[test]
fn a_stable_install_stays_on_the_newest_stable_release() {
    let (_runtime, base) = serve(release_titled("v0.4.0", "Version 0.4.0"));
    let release = Github::at(&base, "acme/openplan")
        .release(Channel::Stable)
        .unwrap();

    assert_eq!(release.step_from(&installed("0.4.0")), Step::UpToDate);
    assert_eq!(release.step_from(&installed("0.3.9")), Step::Install);
}
