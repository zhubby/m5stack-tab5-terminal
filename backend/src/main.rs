use std::net::SocketAddr;

use anyhow::Context;
use tab5_stock_backend::{AppConfig, AppState, app, spawn_provider};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tab5_stock_backend=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env().map_err(anyhow::Error::msg)?;
    let bind_addr: SocketAddr = config.bind_addr;
    let state = AppState::new(config);
    let provider_handle = spawn_provider(state.clone());
    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;
    info!(%bind_addr, "starting Tab5 stock backend");

    axum::serve(listener, app(state.clone()))
        .with_graceful_shutdown(shutdown_signal(state))
        .await?;
    provider_handle.abort();

    Ok(())
}

async fn shutdown_signal(state: AppState) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install terminate signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }

    state.shutdown();
}
