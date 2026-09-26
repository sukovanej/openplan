use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use flate2::Compression;
use flate2::write::GzEncoder;
use op_api::StopReason;
use op_daemon::{Checked, Home, Updater, check, confirm_install};
use op_server::{AppState, Project};
use op_update::{Environment, Github};
use semver::Version;
use sha2::{Digest as _, Sha256};

const OLD_BINARY: &[u8] = b"the installed openplan";
const NEW_BINARY: &[u8] = b"the released openplan";

struct FakeRelease {
    tag: &'static str,
    title: &'static str,
    assets: HashMap<String, Vec<u8>>,
}

fn serve(runtime: &tokio::runtime::Runtime, release: FakeRelease) -> String {
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
    base
}

async fn release_json(State(state): State<Arc<(FakeRelease, String)>>) -> impl IntoResponse {
    let (release, base) = &*state;
    let assets: Vec<serde_json::Value> = release
        .assets
        .keys()
        .map(|name| serde_json::json!({ "name": name, "browser_download_url": format!("{base}/dl/{name}") }))
        .collect();
    axum::Json(
        serde_json::json!({ "tag_name": release.tag, "name": release.title, "assets": assets }),
    )
}

async fn asset(
    State(state): State<Arc<(FakeRelease, String)>>,
    UrlPath(name): UrlPath<String>,
) -> impl IntoResponse {
    match state.0.assets.get(&name) {
        Some(bytes) => (StatusCode::OK, bytes.clone()),
        None => (StatusCode::NOT_FOUND, Vec::new()),
    }
}

fn archive_name() -> String {
    op_update::cli_archive_name(op_update::target().expect("the tests run on a released target"))
}

fn cli_archive(body: &[u8]) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    let mut header = tar::Header::new_gnu();
    header.set_size(body.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    builder
        .append_data(&mut header, "openplan-release/openplan", body)
        .unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

fn digest_of(bytes: &[u8]) -> Vec<u8> {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{hex} *{}\n", archive_name()).into_bytes()
}

fn release(tag: &'static str, title: &'static str, digest: Option<Vec<u8>>) -> FakeRelease {
    let archive = cli_archive(NEW_BINARY);
    let digest = digest.unwrap_or_else(|| digest_of(&archive));
    FakeRelease {
        tag,
        title,
        assets: HashMap::from([
            (archive_name(), archive),
            (format!("{}.sha256", archive_name()), digest),
        ]),
    }
}

fn no_owners() -> Environment {
    Environment {
        cargo_home: None,
        homebrew_prefix: None,
        wsl: false,
    }
}

// The updater's blocking HTTP client must be dropped off the runtime, so each test drives the
// runtime by hand, as the daemon does.
struct Machine {
    runtime: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
    exe: PathBuf,
    home: Arc<Home>,
    state: AppState,
}

impl Machine {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("openplan");
        std::fs::write(&exe, OLD_BINARY).unwrap();
        let home = Arc::new(Home::at(dir.path().join("home")));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let state = runtime.block_on(async { AppState::new(Vec::<Project>::new()) });
        Self {
            runtime,
            _dir: dir,
            exe,
            home,
            state,
        }
    }

    fn updater(&self, base: &str, installed: &str) -> Arc<Updater> {
        Arc::new(
            Updater::new(
                Github::at(base, "acme/openplan"),
                self.exe.clone(),
                Version::parse(installed).unwrap(),
                &no_owners(),
            )
            .unwrap(),
        )
    }

    fn serve(&self, release: FakeRelease) -> String {
        serve(&self.runtime, release)
    }

    fn check(&self, updater: &Arc<Updater>) -> anyhow::Result<Checked> {
        self.runtime
            .block_on(check(updater, &self.home, &self.state))
    }

    fn binary(&self) -> Vec<u8> {
        std::fs::read(&self.exe).unwrap()
    }

    fn last_result(&self) -> String {
        self.home.read_update().last_check.unwrap().result
    }
}

fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

#[test]
fn a_new_release_replaces_the_binary_and_stops_the_daemon_for_the_update() {
    let machine = Machine::new();
    let base = machine.serve(release("v1.1.0", "v1.1.0", None));

    let checked = machine.check(&machine.updater(&base, "1.0.0")).unwrap();

    assert_eq!(checked, Checked::Installed(version("1.1.0")));
    assert_eq!(machine.binary(), NEW_BINARY);
    assert_eq!(machine.state.stop_reason(), Some(StopReason::Update));
    assert_eq!(
        machine.home.read_update().installing.as_deref(),
        Some("1.1.0")
    );
}

#[test]
fn the_newest_release_installs_nothing() {
    let machine = Machine::new();
    let base = machine.serve(release("v1.0.0", "v1.0.0", None));

    let checked = machine.check(&machine.updater(&base, "1.0.0")).unwrap();

    assert_eq!(checked, Checked::UpToDate);
    assert_eq!(machine.binary(), OLD_BINARY);
    assert_eq!(machine.state.stop_reason(), None);
    assert_eq!(
        machine.last_result(),
        "openplan 1.0.0 is the newest release"
    );
}

#[test]
fn a_canary_build_installs_the_newer_canary_build() {
    let machine = Machine::new();
    let base = machine.serve(release("canary", "0.4.1-canary.57", None));

    let checked = machine
        .check(&machine.updater(&base, "0.4.1-canary.56"))
        .unwrap();

    assert_eq!(checked, Checked::Installed(version("0.4.1-canary.57")));
    assert_eq!(machine.binary(), NEW_BINARY);
}

#[test]
fn a_digest_that_does_not_match_keeps_the_old_binary() {
    let machine = Machine::new();
    let base = machine.serve(release(
        "v1.1.0",
        "v1.1.0",
        Some(digest_of(b"something else")),
    ));

    let err = machine.check(&machine.updater(&base, "1.0.0")).unwrap_err();

    assert!(format!("{err:#}").contains("digest mismatch"), "{err:#}");
    assert_eq!(machine.binary(), OLD_BINARY);
    assert_eq!(machine.state.stop_reason(), None);
}

#[test]
fn the_opt_out_checks_nothing() {
    let machine = Machine::new();
    machine
        .home
        .edit_update(|record| record.auto = false)
        .unwrap();

    let checked = machine
        .check(&machine.updater("http://127.0.0.1:9", "1.0.0"))
        .unwrap();

    assert_eq!(checked, Checked::Off);
    assert_eq!(machine.binary(), OLD_BINARY);
}

#[test]
fn the_updater_refuses_a_binary_that_cargo_install_owns() {
    let cargo_home = tempfile::tempdir().unwrap();
    let exe = cargo_home.path().join("bin").join("openplan");
    let env = Environment {
        cargo_home: Some(cargo_home.path().to_path_buf()),
        homebrew_prefix: None,
        wsl: false,
    };

    let refusal = Updater::new(
        Github::at("http://127.0.0.1:9", "acme/openplan"),
        exe,
        version("1.0.0"),
        &env,
    )
    .err()
    .expect("cargo install owns the binary");

    assert!(refusal.contains("cargo install owns"), "{refusal}");
}

fn home_installing(dir: &Path, installing: &str) -> Home {
    let home = Home::at(dir);
    home.edit_update(|record| record.installing = Some(installing.to_owned()))
        .unwrap();
    home
}

#[test]
fn the_new_daemon_confirms_the_version_the_update_installed() {
    let dir = tempfile::tempdir().unwrap();
    let home = home_installing(dir.path(), env!("CARGO_PKG_VERSION"));

    confirm_install(&home);

    let record = home.read_update();
    assert_eq!(record.installing, None);
    assert_eq!(
        record.last_check.unwrap().result,
        format!("installed openplan {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_new_daemon_reports_a_version_the_update_did_not_install() {
    let dir = tempfile::tempdir().unwrap();
    let home = home_installing(dir.path(), "99.0.0");

    confirm_install(&home);

    assert_eq!(
        home.read_update().last_check.unwrap().result,
        format!(
            "installed openplan 99.0.0, but the daemon runs {}",
            env!("CARGO_PKG_VERSION")
        )
    );
}
