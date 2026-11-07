// SPDX-License-Identifier: MIT OR Apache-2.0

//! Starts and stops the HTTP server.

use crate::app::{AppState, StartupError};
use crate::clock::SystemClock;
use crate::config::Config;
use crate::web;
use std::fs;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::info;

/// Runs the server until SIGTERM or Ctrl+C.
///
/// # Errors
///
/// Returns [`StartupError`] if the service can't start or the listener fails.
pub(crate) async fn run(config: Config) -> Result<(), StartupError> {
    fs::create_dir_all(&config.state_dir).map_err(|source| StartupError::Io {
        context: format!(
            "can't create state directory {}",
            config.state_dir.display()
        ),
        source,
    })?;
    info!(
        public_url = config.base_url.origin(),
        state_dir = %config.state_dir.display(),
        analytics = config.analytics.is_some(),
        "starting praxis-website {}",
        env!("CARGO_PKG_VERSION")
    );

    let bind = config.bind;
    let dev = config.dev;
    let state = AppState::new(config, Arc::new(SystemClock))?;
    let base = state.config.base_url.clone();
    let listener = TcpListener::bind(bind)
        .await
        .map_err(|source| StartupError::Io {
            context: format!("can't listen on {bind}"),
            source,
        })?;
    info!("listening on {bind}");
    if dev {
        info!("development mode: open {}", base.join("/"));
    }

    axum::serve(listener, web::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|source| StartupError::Io {
            context: "the server failed".to_string(),
            source,
        })?;
    info!("stopped");
    Ok(())
}

/// Completes on SIGTERM (systemd) or Ctrl+C (terminal).
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("installing the Ctrl+C handler should succeed");
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        signal(SignalKind::terminate())
            .expect("installing the SIGTERM handler should succeed")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    info!("shutting down, finishing open requests");
}
