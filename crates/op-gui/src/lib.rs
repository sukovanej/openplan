use std::process::ExitCode;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
use anyhow::bail;
use anyhow::{Context, Result};
#[cfg(target_os = "windows")]
use op_client::{base_url, default_port};
#[cfg(not(target_os = "windows"))]
use op_daemon::{Control, base_url, default_port};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager as _, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt as _, MessageDialogKind};

#[cfg(target_os = "windows")]
use std::time::{Duration, Instant};

const TITLE: &str = "OpenPlan";
const WINDOW: &str = "main";

// The window opens on the splash and moves to the daemon, and either half can be ready first. A
// navigation the webview receives before its first page settles is dropped without a word, so the
// move waits for both.
#[derive(Default)]
struct Handover {
    splash_shown: bool,
    daemon: Option<Url>,
}

pub fn run() -> ExitCode {
    let handover = Arc::new(Mutex::new(Handover::default()));
    let shown = handover.clone();
    let started = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let window = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::default())
                .title(TITLE)
                .inner_size(1280.0, 860.0)
                .min_inner_size(720.0, 480.0)
                .on_page_load(move |window, payload| {
                    if payload.event() != PageLoadEvent::Finished {
                        return;
                    }
                    let mut handover = lock(&shown);
                    handover.splash_shown = true;
                    hand_over(&window, &mut handover);
                })
                .build()?;

            // A cold start spawns the daemon and waits for it to answer /health, which together run
            // for seconds. The main thread has to keep drawing the splash meanwhile.
            let handle = app.handle().clone();
            let found = handover.clone();
            std::thread::spawn(move || {
                let url = daemon_url();
                let main = handle.clone();
                let _ = handle.run_on_main_thread(move || match url {
                    Ok(url) => {
                        let mut handover = lock(&found);
                        handover.daemon = Some(url);
                        hand_over(&window, &mut handover);
                    }
                    Err(reason) => refuse(&main, &format!("{reason:#}")),
                });
            });
            Ok(())
        })
        .run(tauri::generate_context!());

    match started {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

// The daemon serves the same SPA the browser gets, so the window moves to its URL instead of
// loading assets of its own. One build of `web/packages/app` then answers both.
#[cfg(not(target_os = "windows"))]
fn daemon_url() -> Result<Url> {
    let control = Control::resolve().context("Cannot find the openplan home directory.")?;
    let info = control
        .ensure(default_port()?)
        .context("Cannot start the openplan daemon.")?
        .into_info();
    base_url(info.port)
        .parse()
        .context("The daemon reported a port that makes no URL.")
}

// WSL forwards its loopback listeners to the Windows host. The Windows bundle has no daemon of
// its own: the CLI in WSL owns both its state and lifecycle, while this window only waits for the
// daemon the user already started there.
#[cfg(target_os = "windows")]
fn daemon_url() -> Result<Url> {
    let base = base_url(windows_daemon_port()?);
    let client = op_client::Client::default();
    let deadline = Instant::now() + Duration::from_secs(5);

    while client.health(&base).is_none() {
        if Instant::now() >= deadline {
            bail!(
                "Cannot reach the openplan daemon at {base}. Start it in WSL with `openplan server start`; \
                 Windows needs WSL localhost forwarding enabled."
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    base.parse()
        .context("The Windows-to-WSL daemon URL is not valid.")
}

#[cfg(target_os = "windows")]
fn windows_daemon_port() -> Result<u16> {
    let port = default_port()?;
    if port == 0 {
        bail!(
            "OPENPLAN_PORT=0 chooses a random port in WSL, which the Windows GUI cannot discover. \
             Start the daemon on a fixed port instead."
        );
    }
    Ok(port)
}

fn hand_over(window: &WebviewWindow, handover: &mut Handover) {
    if !handover.splash_shown {
        return;
    }
    let Some(url) = handover.daemon.take() else {
        return;
    };
    if let Err(err) = window.navigate(url) {
        refuse(window.app_handle(), &err.to_string());
    }
}

fn lock(handover: &Mutex<Handover>) -> std::sync::MutexGuard<'_, Handover> {
    handover.lock().unwrap_or_else(|held| held.into_inner())
}

fn refuse(app: &AppHandle, message: &str) {
    let handle = app.clone();
    app.dialog()
        .message(message)
        .kind(MessageDialogKind::Error)
        .title(TITLE)
        .show(move |_| handle.exit(1));
}
