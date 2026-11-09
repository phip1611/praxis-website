// SPDX-License-Identifier: MIT OR Apache-2.0

//! Starts and stops the HTTP server.

use crate::app::{AppState, StartupError};
use crate::clock::SystemClock;
use crate::config::Config;
use crate::{stats, web};
use std::fs;
use std::future::IntoFuture;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::{signal, time};
use tracing::{info, warn};

/// How long open requests may take after SIGTERM. Below systemd's
/// `TimeoutStopSec`, so that the statistics are saved before it kills us.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

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

    let stats = Arc::clone(&state.stats);
    tokio::spawn(stats::report::run(Arc::clone(&stats)));

    // After the signal, open requests get a grace period, so that a stuck
    // connection can't delay saving the statistics until systemd kills us.
    let stopping = Arc::new(Notify::new());
    let signal = {
        let stopping = Arc::clone(&stopping);
        async move {
            shutdown_signal().await;
            stopping.notify_one();
        }
    };
    let server = axum::serve(listener, web::router(state)).with_graceful_shutdown(signal);
    tokio::select! {
        result = server.into_future() => result.map_err(|source| StartupError::Io {
            context: "the server failed".to_string(),
            source,
        })?,
        () = async {
            stopping.notified().await;
            time::sleep(SHUTDOWN_GRACE).await;
        } => warn!(
            "open requests didn't finish within {} seconds, stopping anyway",
            SHUTDOWN_GRACE.as_secs()
        ),
    }
    if let Err(err) = stats.flush() {
        warn!("can't write the access statistics: {err}");
    }
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
