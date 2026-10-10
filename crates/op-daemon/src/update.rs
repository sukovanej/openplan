use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use op_server::AppState;
use op_update::{Channel, Environment, Github, Step, owner_of};
use semver::Version;

use crate::home::{Home, LastCheck};
use crate::now_unix;

const FIRST_CHECK: Duration = Duration::from_secs(5 * 60);
// Each push to `main` makes a canary build, so the interval also limits how often a canary daemon
// restarts.
const INTERVAL: Duration = Duration::from_secs(60 * 60);

pub enum Updates {
    Auto,
    Never(&'static str),
}

pub struct Updater {
    github: Github,
    exe: PathBuf,
    archive: String,
    installed: Version,
}

struct Ready {
    version: Version,
    archive: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Checked {
    Off,
    UpToDate,
    Installed(Version),
    Stopping,
}

impl Updater {
    pub fn new(
        github: Github,
        exe: PathBuf,
        installed: Version,
        env: &Environment,
    ) -> Result<Self, String> {
        if let Some(owner) = owner_of(&exe, env) {
            return Err(owner.refusal(&exe));
        }
        let Some(target) = op_update::target() else {
            return Err(format!(
                "no release is built for {}-{}",
                std::env::consts::OS,
                std::env::consts::ARCH
            ));
        };
        Ok(Self {
            github,
            exe,
            archive: op_update::cli_archive_name(target),
            installed,
        })
    }

    pub fn for_this_binary(updates: Updates) -> Result<Self, String> {
        if let Updates::Never(reason) = updates {
            return Err(reason.to_owned());
        }
        let exe = std::env::current_exe()
            .and_then(|exe| exe.canonicalize())
            .map_err(|err| format!("cannot locate this executable: {err}"))?;
        let installed = Version::parse(env!("CARGO_PKG_VERSION"))
            .map_err(|err| format!("the built-in version does not parse: {err}"))?;
        Self::new(Github::default(), exe, installed, &Environment::detect())
    }

    pub(crate) fn exe(&self) -> &Path {
        &self.exe
    }

    fn channel(&self) -> Channel {
        Channel::of(&self.installed)
    }

    fn find(&self) -> Result<Option<Ready>> {
        let release = self
            .github
            .release(self.channel())
            .context("looking up the newest release")?;
        match release.step_from(&self.installed) {
            Step::UpToDate => Ok(None),
            Step::Install | Step::BackToStable => {
                let archive = self.github.download_verified(&release, &self.archive)?;
                Ok(Some(Ready {
                    version: release.version,
                    archive,
                }))
            }
        }
    }

    fn up_to_date(&self) -> String {
        match self.channel() {
            Channel::Stable => format!("openplan {} is the newest release", self.installed),
            Channel::Canary => format!("openplan {} is the newest canary build", self.installed),
        }
    }
}

pub(crate) async fn keep_updated(
    updater: Result<Arc<Updater>, String>,
    home: Arc<Home>,
    state: AppState,
) {
    let asking = state.self_update();
    let updater = match updater {
        Ok(updater) => updater,
        Err(reason) => {
            tracing::info!(%reason, "no automatic updates");
            record(&home, format!("no automatic updates: {reason}"));
            loop {
                asking.wanted().await;
                asking.record(format!("no automatic updates: {reason}"));
                state.report_faults();
            }
        }
    };
    let mut wait = FIRST_CHECK;
    loop {
        asking.set_automatic(home.read_update().auto);
        state.report_faults();
        // A project that needs a newer openplan asks for the check at once.
        let asked = tokio::select! {
            () = tokio::time::sleep(wait) => false,
            () = asking.wanted() => true,
        };
        wait = INTERVAL;
        let outcome = match check(&updater, &home, &state).await {
            Ok(Checked::Installed(version)) => {
                tracing::info!(%version, "installed a new release; starting again on it");
                return;
            }
            Ok(Checked::Stopping) => return,
            Ok(Checked::UpToDate) => updater.up_to_date(),
            Ok(Checked::Off) => "automatic updates are off".to_owned(),
            Err(err) => {
                let error = format!("{err:#}");
                tracing::warn!(%error, "the update failed; the daemon keeps its version");
                record(&home, format!("failed: {error}"));
                format!("the update failed: {error}")
            }
        };
        if asked {
            asking.record(outcome);
        }
    }
}

// Downloads and verifies the new release first, then replaces the executable and stops the daemon.
// `serve` then starts the new executable in this process.
pub async fn check(updater: &Arc<Updater>, home: &Arc<Home>, state: &AppState) -> Result<Checked> {
    if !home.read_update().auto {
        return Ok(Checked::Off);
    }
    let finding = Arc::clone(updater);
    let found = tokio::task::spawn_blocking(move || finding.find())
        .await
        .context("the update check panicked")??;
    let Some(ready) = found else {
        record(home, updater.up_to_date());
        return Ok(Checked::UpToDate);
    };
    let version = ready.version.clone();
    if !home.read_update().auto {
        return Ok(Checked::Off);
    }
    let (updater, home, state) = (Arc::clone(updater), Arc::clone(home), state.clone());
    // Unpacking the archive blocks.
    let installed =
        tokio::task::spawn_blocking(move || state.update(|| install(&updater, &home, &ready)))
            .await
            .context("the install panicked")??;
    Ok(match installed {
        true => Checked::Installed(version),
        false => Checked::Stopping,
    })
}

fn install(updater: &Updater, home: &Home, ready: &Ready) -> Result<()> {
    op_update::replace_executable(&ready.archive, &updater.exe)?;
    // The executable is already new, so a failure to record it must not keep the old daemon running.
    if let Err(err) = home.edit_update(|record| record.installing = Some(ready.version.to_string()))
    {
        tracing::warn!(error = format!("{err:#}"), "cannot record the update");
    }
    Ok(())
}

// The process that an update started reports whether it runs the version the update installed.
pub fn confirm_install(home: &Home) {
    let Some(installing) = home.read_update().installing else {
        return;
    };
    let running = env!("CARGO_PKG_VERSION");
    let result = if installing == running {
        tracing::info!(version = running, "the update is complete");
        format!("installed openplan {running}")
    } else {
        tracing::warn!(
            installing,
            running,
            "the daemon runs another version than the update installed"
        );
        format!("installed openplan {installing}, but the daemon runs {running}")
    };
    if let Err(err) = home.edit_update(|record| {
        record.installing = None;
        record.last_check = Some(LastCheck {
            at: now_unix(),
            result,
        });
    }) {
        tracing::warn!(error = format!("{err:#}"), "cannot record the update");
    }
}

fn record(home: &Home, result: String) {
    if let Err(err) = home.edit_update(|record| {
        record.last_check = Some(LastCheck {
            at: now_unix(),
            result,
        });
    }) {
        tracing::warn!(error = format!("{err:#}"), "cannot record the update check");
    }
}
