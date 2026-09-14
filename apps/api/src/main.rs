use task_dashboard_api::{app, config::Config};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env(|k| std::env::var(k).ok())?;
    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    info!(addr = %config.addr, "task-dashboard-api escuchando");

    // Railway manda SIGTERM al redeployar y al dormir el servicio: cerrar limpio evita cortar
    // una transacción a la mitad cuando exista la base.
    axum::serve(listener, app())
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
