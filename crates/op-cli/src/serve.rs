use std::net::SocketAddr;

use anyhow::Result;
use op_server::AppState;
use op_store::Store;
use op_watch::Watcher;
use tracing_subscriber::EnvFilter;

pub async fn run(port: u16) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init()
        .ok();

    let state = AppState::default();

    let (tx, rx) = std::sync::mpsc::channel();
    let _watcher = match Store::discover(".") {
        Ok(store) => Watcher::start(store.plan_dir(), tx)
            .inspect_err(|e| tracing::warn!("watch disabled: {e}"))
            .ok(),
        Err(_) => {
            tracing::warn!("no .plan/ store found; watch disabled");
            None
        }
    };
    std::thread::spawn(move || {
        for event in rx {
            tracing::debug!(?event, "change");
        }
    });

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!(%addr, "oplan serving");
    op_server::serve(addr, state).await?;
    Ok(())
}
