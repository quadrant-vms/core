use alert_service::{create_router, AlertStore, AppState, Notifier, RuleEngine};
use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    // Load environment variables
    let database_url = env::var("DATABASE_URL")
        .context("DATABASE_URL must be set")?;

    let bind_addr = env::var("ALERT_SERVICE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8085".to_string());

    info!("Starting alert-service");
    info!("Bind address: {}", bind_addr);

    // Create database connection pool
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    info!("Connected to database");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("Failed to run migrations")?;

    info!("Migrations complete");

    // Create store
    let store = AlertStore::new(pool);

    // Create rule engine
    let engine = Arc::new(RuleEngine::new(store.clone()));

    // Create notifier
    let mut notifier = Notifier::new(store.clone());

    // Configure email channel if SMTP settings are provided
    if let (Ok(smtp_host), Ok(smtp_username), Ok(smtp_password), Ok(from_address)) = (
        env::var("SMTP_HOST"),
        env::var("SMTP_USERNAME"),
        env::var("SMTP_PASSWORD"),
        env::var("SMTP_FROM"),
    ) {
        let smtp_port = env::var("SMTP_PORT")
            .unwrap_or_else(|_| "587".to_string())
            .parse()
            .unwrap_or(587);

        notifier.add_email_channel(
            smtp_host.clone(),
            smtp_port,
            smtp_username,
            smtp_password,
            from_address,
        );

        info!("Email channel configured (SMTP: {}:{})", smtp_host, smtp_port);
    } else {
        info!("Email channel not configured (SMTP settings missing)");
    }

    // Configure SMS channel if Twilio settings are provided
    if let (Ok(account_sid), Ok(auth_token), Ok(from_number)) = (
        env::var("TWILIO_ACCOUNT_SID"),
        env::var("TWILIO_AUTH_TOKEN"),
        env::var("TWILIO_FROM_NUMBER"),
    ) {
        notifier.add_sms_channel(
            account_sid,
            auth_token,
            from_number.clone(),
        );

        info!("SMS channel configured (Twilio from: {})", from_number);
    } else {
        info!("SMS channel not configured (Twilio settings missing)");
    }

    info!("Slack and Discord channels configured (webhook-based)");

    let notifier = Arc::new(notifier);

    // Create app state
    let state = AppState {
        store,
        engine,
        notifier,
    };

    // Create router
    let app = create_router(state);

    // Start server
    let listener = TcpListener::bind(&bind_addr)
        .await
        .context("Failed to bind to address")?;

    info!("Alert service listening on {}", bind_addr);

    axum::serve(listener, app)
        .await
        .context("Server error")?;

    Ok(())
}
