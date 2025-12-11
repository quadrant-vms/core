use anyhow::{Context, Result};
use auth_service::{AuthConfig, AuthRepository, AuthService, AuthState};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init();

    let config = AuthConfig::from_env()?;
    let bind_addr = config.bind_addr;

    // Create database connection pool
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    // Run migrations
    // NOTE: Migrations manually applied - SQLx has issues with shared migration table across services
    // info!("running database migrations");
    // sqlx::migrate!("./migrations")
    //     .run(&pool)
    //     .await
    //     .context("failed to run migrations")?;

    // Create repository and service
    let repository = AuthRepository::new(pool);
    let service = Arc::new(AuthService::new(repository, config.clone()));
    let state = AuthState::new(service);

    // Build router
    let app = auth_service::routes::router(state);
    let listener = TcpListener::bind(bind_addr).await?;

    info!(
        addr = %bind_addr,
        "auth-service listening"
    );

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("signal received, starting graceful shutdown");
}
