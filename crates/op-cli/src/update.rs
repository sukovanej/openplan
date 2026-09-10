use std::path::Path;
use std::process::Command;

use anyhow::{Context as _, Result, bail};
use op_daemon::{AppInfo, Control, StopOutcome};
use op_update::{Environment, Github, Release, owner_of};
use semver::Version;

pub fn run() -> Result<()> {
    let exe = std::env::current_exe()
        .and_then(|exe| exe.canonicalize())
        .context("locating this executable")?;
    if let Some(owner) = owner_of(&exe, &Environment::detect()) {
        bail!("{}", owner.refusal(&exe));
    }
    let target = op_update::target().with_context(|| {
        format!(
            "no release is built for {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;

    let current =
        Version::parse(env!("CARGO_PKG_VERSION")).context("parsing the built-in version")?;
    let github = Github::default();
    let release = github
        .latest_release()
        .context("looking up the newest release")?;
    if release.version <= current {
        println!("openplan {current} is the newest release");
        return Ok(());
    }
    println!("openplan {current} -> {}", release.version);

    let control = Control::resolve()?;
    let app = control.app().filter(|app| app.bundle.is_dir());

    let cli_archive = download(&github, &release, &op_update::cli_archive_name(target))?;
    let app_archive = app
        .as_ref()
        .map(|_| download(&github, &release, &op_update::app_archive_name(target)))
        .transpose()?;

    let app_was_running = match &app {
        Some(app) if control.app_is_running(app) => {
            println!("quitting {}", app.bundle.display());
            control.quit_app(app)?;
            true
        }
        _ => false,
    };
    match control.stop()? {
        StopOutcome::Stopped { pid, .. } => println!("stopped the daemon (pid {pid})"),
        StopOutcome::NotRunning | StopOutcome::RemovedStale { .. } => {}
    }

    op_update::replace_executable(&cli_archive, &exe)?;
    println!("replaced {}", exe.display());
    if let (Some(app), Some(archive)) = (&app, &app_archive) {
        op_update::replace_bundle(archive, &app.bundle)?;
        println!("replaced {}", app.bundle.display());
    }

    start_daemon(&exe, &control, &release.version)?;
    if app_was_running && let Some(app) = &app {
        open_app(app)?;
    }
    println!("updated openplan to {}", release.version);
    Ok(())
}

fn download(github: &Github, release: &Release, name: &str) -> Result<Vec<u8>> {
    println!("downloading {name}");
    let bytes = github.download_verified(release, name)?;
    println!("verified {name}");
    Ok(bytes)
}

// The new binary spawns the daemon, so the daemon runs the new code; this process would spawn its
// own, already replaced, executable image.
fn start_daemon(exe: &Path, control: &Control, expected: &Version) -> Result<()> {
    let status = Command::new(exe)
        .args(["server", "start"])
        .status()
        .with_context(|| format!("running {} server start", exe.display()))?;
    if !status.success() {
        bail!("the new openplan could not start the daemon");
    }
    let info = control
        .running()
        .context("the new daemon does not answer /health")?;
    if info.version != expected.to_string() {
        bail!(
            "the daemon reports version {}, expected {expected}",
            info.version
        );
    }
    Ok(())
}

fn open_app(app: &AppInfo) -> Result<()> {
    let status = Command::new("open")
        .arg(&app.bundle)
        .status()
        .with_context(|| format!("opening {}", app.bundle.display()))?;
    if !status.success() {
        bail!("open refused {}", app.bundle.display());
    }
    Ok(())
}
