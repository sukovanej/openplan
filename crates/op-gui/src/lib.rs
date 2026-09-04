pub mod cli;
pub mod daemon;

use tauri::{AppHandle, Manager as _, Url, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt as _, MessageDialogKind};

const TITLE: &str = "OpenPlan";
const WINDOW: &str = "main";

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let resources = app.path().resource_dir().ok();
            match daemon::url(resources.as_deref()) {
                Ok(url) => open(app.handle(), &url),
                Err(err) => refuse(app.handle(), &err.to_string()),
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("starting the openplan window");
}

// The daemon serves the same SPA the browser gets, so the window loads its URL instead of assets of
// its own. One build of `web/packages/app` then answers both.
fn open(app: &AppHandle, url: &str) {
    let built = url
        .parse::<Url>()
        .map_err(|err| err.to_string())
        .and_then(|url| {
            WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::External(url))
                .title(TITLE)
                .inner_size(1280.0, 860.0)
                .min_inner_size(720.0, 480.0)
                .build()
                .map_err(|err| err.to_string())
        });
    if let Err(reason) = built {
        refuse(app, &reason);
    }
}

fn refuse(app: &AppHandle, message: &str) {
    let handle = app.clone();
    app.dialog()
        .message(message)
        .kind(MessageDialogKind::Error)
        .title(TITLE)
        .show(move |_| handle.exit(1));
}
