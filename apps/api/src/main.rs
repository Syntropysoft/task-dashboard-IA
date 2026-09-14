use std::sync::Arc;

use task_dashboard_api::{app, auth::Validator, config::Config, db, state::AppState};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env(|k| std::env::var(k).ok())?;
    // Conectar y migrar ANTES de escuchar: si la base no está, el proceso muere y Railway lo
    // reintenta (restartPolicy ON_FAILURE) — nunca un /health verde sin base.
    let pool = db::connect_and_migrate(&config.database_url).await?;
    info!("base conectada y migraciones al día");

    // El JWKS se intenta al arrancar pero NO bloquea: si syntroAuth está caído, /health y /mcp
    // (PAT, ítem 3c) siguen; /api responde 503 hasta poder validar. Nunca acepta sin firma.
    let auth = Arc::new(Validator::new(config.jwt.clone()));
    if let Err(e) = auth.refresh().await {
        warn!(error = %e, "arranco sin JWKS; /api dará 503 hasta que syntroAuth responda");
    }
    let state = AppState {
        pool,
        auth,
        mcp_allowed_hosts: config.mcp_allowed_hosts.clone(),
    };
    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    info!(addr = %config.addr, "task-dashboard-api escuchando");

    // Railway manda SIGTERM al redeployar y al dormir el servicio: cerrar limpio evita cortar
    // una transacción a la mitad cuando exista la base.
    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut sig) = signal(SignalKind::terminate()) {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("señal de apagado recibida, cerrando");
}
