//! State migration tool for upgrading Quadrant VMS deployments
//!
//! This tool helps migrate state data between schema versions and perform
//! maintenance operations on the state store.
//!
//! Usage:
//!   state-migrate check                    - Check current schema version
//!   state-migrate list-orphans             - List all orphaned resources
//!   state-migrate cleanup-orphans          - Clean up orphaned resources
//!   state-migrate export <path>            - Export all state to JSON file
//!   state-migrate import <path>            - Import state from JSON file
//!   state-migrate vacuum                   - Vacuum and analyze database

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use common::{
    recordings::RecordingInfo,
    state_store::StateStore,
    streams::StreamInfo,
};
use coordinator::pg_state_store::PgStateStore;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{fs, path::PathBuf};
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "state-migrate")]
#[command(about = "State migration tool for Quadrant VMS", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Database URL
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Check current schema version and migrations
    Check,

    /// List all orphaned resources (non-active state with lease_id)
    ListOrphans {
        /// Filter by node ID
        #[arg(long)]
        node_id: Option<String>,
    },

    /// Clean up orphaned resources
    CleanupOrphans {
        /// Filter by node ID
        #[arg(long)]
        node_id: Option<String>,

        /// Dry run (don't actually delete)
        #[arg(long)]
        dry_run: bool,
    },

    /// Export all state to JSON file
    Export {
        /// Output file path
        path: PathBuf,

        /// Pretty-print JSON
        #[arg(long)]
        pretty: bool,
    },

    /// Import state from JSON file
    Import {
        /// Input file path
        path: PathBuf,

        /// Skip existing resources (don't overwrite)
        #[arg(long)]
        skip_existing: bool,
    },

    /// Vacuum and analyze database
    Vacuum {
        /// Full vacuum (requires exclusive lock)
        #[arg(long)]
        full: bool,
    },

    /// Show statistics about state store
    Stats,
}

#[derive(Debug, Serialize, Deserialize)]
struct StateExport {
    version: String,
    exported_at: DateTime<Utc>,
    streams: Vec<StreamInfo>,
    recordings: Vec<RecordingInfo>,
}

#[derive(Debug, Serialize)]
struct OrphanStats {
    total_streams: usize,
    total_recordings: usize,
    orphaned_streams: usize,
    orphaned_recordings: usize,
    orphans: Vec<OrphanedResource>,
}

#[derive(Debug, Serialize)]
struct OrphanedResource {
    resource_type: String,
    id: String,
    node_id: String,
    state: String,
    lease_id: Option<String>,
    last_error: Option<String>,
}

#[derive(Debug, Serialize)]
struct Stats {
    total_streams: usize,
    active_streams: usize,
    error_streams: usize,
    total_recordings: usize,
    active_recordings: usize,
    error_recordings: usize,
    streams_by_node: std::collections::HashMap<String, usize>,
    recordings_by_node: std::collections::HashMap<String, usize>,
}

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init();
    let cli = Cli::parse();

    // Get database URL from CLI or environment
    let database_url = cli.database_url
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .context("DATABASE_URL must be provided via --database-url or DATABASE_URL env var")?;

    // Connect to database
    let pool = PgPool::connect(&database_url)
        .await
        .context("failed to connect to database")?;

    let state_store = PgStateStore::new(pool.clone());

    match cli.command {
        Commands::Check => check_schema(&pool).await?,
        Commands::ListOrphans { node_id } => list_orphans(&state_store, node_id.as_deref()).await?,
        Commands::CleanupOrphans { node_id, dry_run } => {
            cleanup_orphans(&state_store, node_id.as_deref(), dry_run).await?
        }
        Commands::Export { path, pretty } => export_state(&state_store, &path, pretty).await?,
        Commands::Import {
            path,
            skip_existing,
        } => import_state(&state_store, &path, skip_existing).await?,
        Commands::Vacuum { full } => vacuum_database(&pool, full).await?,
        Commands::Stats => show_stats(&state_store).await?,
    }

    Ok(())
}

async fn check_schema(pool: &PgPool) -> Result<()> {
    info!("Checking database schema...");

    // Query migration version table
    let result: Option<(i64, String, chrono::DateTime<Utc>)> = sqlx::query_as(
        r#"
        SELECT version, description, installed_on
        FROM _sqlx_migrations
        ORDER BY installed_on DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    if let Some((version, description, installed_on)) = result {
        info!(
            version = version,
            description = %description,
            installed_on = %installed_on,
            "Latest migration"
        );
    } else {
        warn!("No migrations found - database may not be initialized");
    }

    // Check tables exist
    let tables: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT table_name
        FROM information_schema.tables
        WHERE table_schema = 'public'
        AND table_name IN ('leases', 'streams', 'recordings', 'ai_tasks')
        "#,
    )
    .fetch_all(pool)
    .await?;

    info!("Found {} required tables", tables.len());
    for (table_name,) in tables {
        info!(table = %table_name, "Table exists");
    }

    Ok(())
}

async fn list_orphans(state_store: &PgStateStore, node_id: Option<&str>) -> Result<()> {
    info!("Listing orphaned resources...");

    let streams = state_store.list_streams(node_id).await?;
    let recordings = state_store.list_recordings(node_id).await?;

    let mut orphans = Vec::new();

    // Find orphaned streams
    for stream in &streams {
        if stream.lease_id.is_some() && !stream.state.is_active() {
            orphans.push(OrphanedResource {
                resource_type: "stream".to_string(),
                id: stream.config.id.clone(),
                node_id: stream.node_id.clone().unwrap_or_else(|| "unknown".to_string()),
                state: format!("{:?}", stream.state),
                lease_id: stream.lease_id.clone(),
                last_error: stream.last_error.clone(),
            });
        }
    }

    // Find orphaned recordings
    for recording in &recordings {
        if recording.lease_id.is_some() && !recording.state.is_active() {
            orphans.push(OrphanedResource {
                resource_type: "recording".to_string(),
                id: recording.config.id.clone(),
                node_id: recording.node_id.clone().unwrap_or_else(|| "unknown".to_string()),
                state: format!("{:?}", recording.state),
                lease_id: recording.lease_id.clone(),
                last_error: recording.last_error.clone(),
            });
        }
    }

    let stats = OrphanStats {
        total_streams: streams.len(),
        total_recordings: recordings.len(),
        orphaned_streams: orphans.iter().filter(|o| o.resource_type == "stream").count(),
        orphaned_recordings: orphans.iter().filter(|o| o.resource_type == "recording").count(),
        orphans,
    };

    println!("{}", serde_json::to_string_pretty(&stats)?);

    Ok(())
}

async fn cleanup_orphans(
    state_store: &PgStateStore,
    node_id: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        info!("DRY RUN: No resources will be deleted");
    } else {
        info!("Cleaning up orphaned resources...");
    }

    let streams = state_store.list_streams(node_id).await?;
    let recordings = state_store.list_recordings(node_id).await?;

    let mut cleaned_streams = 0;
    let mut cleaned_recordings = 0;

    // Clean orphaned streams
    for stream in streams {
        if stream.lease_id.is_some() && !stream.state.is_active() {
            info!(
                stream_id = %stream.config.id,
                state = ?stream.state,
                "Cleaning orphaned stream"
            );

            if !dry_run {
                state_store.delete_stream(&stream.config.id).await?;
            }
            cleaned_streams += 1;
        }
    }

    // Clean orphaned recordings
    for recording in recordings {
        if recording.lease_id.is_some() && !recording.state.is_active() {
            info!(
                recording_id = %recording.config.id,
                state = ?recording.state,
                "Cleaning orphaned recording"
            );

            if !dry_run {
                state_store.delete_recording(&recording.config.id).await?;
            }
            cleaned_recordings += 1;
        }
    }

    if dry_run {
        info!(
            streams = cleaned_streams,
            recordings = cleaned_recordings,
            "Would clean up orphaned resources (dry run)"
        );
    } else {
        info!(
            streams = cleaned_streams,
            recordings = cleaned_recordings,
            "Cleaned up orphaned resources"
        );
    }

    Ok(())
}

async fn export_state(
    state_store: &PgStateStore,
    path: &PathBuf,
    pretty: bool,
) -> Result<()> {
    info!("Exporting state to {:?}", path);

    let streams = state_store.list_streams(None).await?;
    let recordings = state_store.list_recordings(None).await?;

    let export = StateExport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        exported_at: Utc::now(),
        streams,
        recordings,
    };

    let json = if pretty {
        serde_json::to_string_pretty(&export)?
    } else {
        serde_json::to_string(&export)?
    };

    fs::write(path, json).context("failed to write export file")?;

    info!(
        streams = export.streams.len(),
        recordings = export.recordings.len(),
        "State exported successfully"
    );

    Ok(())
}

async fn import_state(
    state_store: &PgStateStore,
    path: &PathBuf,
    skip_existing: bool,
) -> Result<()> {
    info!("Importing state from {:?}", path);

    let json = fs::read_to_string(path).context("failed to read import file")?;
    let export: StateExport = serde_json::from_str(&json)?;

    info!(
        export_version = %export.version,
        export_date = %export.exported_at,
        "Loaded export file"
    );

    let mut imported_streams = 0;
    let mut skipped_streams = 0;
    let mut imported_recordings = 0;
    let mut skipped_recordings = 0;

    // Import streams
    for stream in export.streams {
        if skip_existing {
            let existing = state_store.get_stream(&stream.config.id).await?;
            if existing.is_some() {
                skipped_streams += 1;
                continue;
            }
        }

        state_store.save_stream(&stream).await?;
        imported_streams += 1;
    }

    // Import recordings
    for recording in export.recordings {
        if skip_existing {
            let existing = state_store.get_recording(&recording.config.id).await?;
            if existing.is_some() {
                skipped_recordings += 1;
                continue;
            }
        }

        state_store.save_recording(&recording).await?;
        imported_recordings += 1;
    }

    info!(
        imported_streams,
        skipped_streams,
        imported_recordings,
        skipped_recordings,
        "State imported successfully"
    );

    Ok(())
}

async fn vacuum_database(pool: &PgPool, full: bool) -> Result<()> {
    if full {
        info!("Running VACUUM FULL ANALYZE (this may take a while)...");
        sqlx::query("VACUUM FULL ANALYZE").execute(pool).await?;
    } else {
        info!("Running VACUUM ANALYZE...");
        sqlx::query("VACUUM ANALYZE").execute(pool).await?;
    }

    info!("Database vacuum completed");

    Ok(())
}

async fn show_stats(state_store: &PgStateStore) -> Result<()> {
    info!("Gathering statistics...");

    let streams = state_store.list_streams(None).await?;
    let recordings = state_store.list_recordings(None).await?;

    let mut streams_by_node = std::collections::HashMap::new();
    let mut recordings_by_node = std::collections::HashMap::new();

    let active_streams = streams.iter().filter(|s| s.state.is_active()).count();
    let error_streams = streams
        .iter()
        .filter(|s| matches!(s.state, common::streams::StreamState::Error))
        .count();

    for stream in &streams {
        let node_id = stream.node_id.clone().unwrap_or_else(|| "unknown".to_string());
        *streams_by_node.entry(node_id).or_insert(0) += 1;
    }

    let active_recordings = recordings.iter().filter(|r| r.state.is_active()).count();
    let error_recordings = recordings
        .iter()
        .filter(|r| matches!(r.state, common::recordings::RecordingState::Error))
        .count();

    for recording in &recordings {
        let node_id = recording.node_id.clone().unwrap_or_else(|| "unknown".to_string());
        *recordings_by_node
            .entry(node_id)
            .or_insert(0) += 1;
    }

    let stats = Stats {
        total_streams: streams.len(),
        active_streams,
        error_streams,
        total_recordings: recordings.len(),
        active_recordings,
        error_recordings,
        streams_by_node,
        recordings_by_node,
    };

    println!("{}", serde_json::to_string_pretty(&stats)?);

    Ok(())
}
