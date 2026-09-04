pub mod cli;
pub mod daemon;

use std::sync::{Arc, Mutex};

use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager as _, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt as _, MessageDialogKind};

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

pub fn run() {
    let handover = Arc::new(Mutex::new(Handover::default()));
    let shown = handover.clone();
    tauri::Builder::default()
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

            // A cold start probes the daemon and waits for `openplan server start`, which together
            // run for seconds. The main thread has to keep drawing the splash meanwhile.
            let handle = app.handle().clone();
            let found = handover.clone();
            std::thread::spawn(move || {
                let resources = handle.path().resource_dir().ok();
                let url = daemon::url(resources.as_deref());
                let main = handle.clone();
                let _ = handle.run_on_main_thread(move || match reached(url) {
                    Ok(url) => {
                        let mut handover = lock(&found);
                        handover.daemon = Some(url);
                        hand_over(&window, &mut handover);
                    }
                    Err(reason) => refuse(&main, &reason),
                });
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("starting the openplan window");
}

// The daemon serves the same SPA the browser gets, so the window moves to its URL instead of
// loading assets of its own. One build of `web/packages/app` then answers both.
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

fn reached(url: Result<String, daemon::Unreachable>) -> Result<Url, String> {
    let url = url.map_err(|err| err.to_string())?;
    url.parse()
        .map_err(|err: <Url as std::str::FromStr>::Err| err.to_string())
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
