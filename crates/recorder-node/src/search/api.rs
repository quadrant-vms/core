use axum::{extract::State, http::StatusCode, Json};
use common::search::*;
use std::sync::Arc;
use tracing::{error, info};
use super::store::SearchStore;
use super::indexer::SearchIndexer;

pub struct SearchApiState {
  pub store: Arc<dyn SearchStore>,
  pub indexer: Arc<SearchIndexer>,
}

pub async fn search_recordings(
  State(state): State<Arc<SearchApiState>>,
  Json(query): Json<RecordingSearchQuery>,
) -> Result<Json<RecordingSearchResponse>, StatusCode> {
  info!("searching recordings");
  match state.store.search_recordings(&query).await {
    Ok(response) => Ok(Json(response)),
    Err(e) => {
      error!(error = %e, "failed to search recordings");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

pub async fn search_events(
  State(state): State<Arc<SearchApiState>>,
  Json(query): Json<EventSearchQuery>,
) -> Result<Json<EventSearchResponse>, StatusCode> {
  info!("searching events");
  match state.store.search_events(&query).await {
    Ok(response) => Ok(Json(response)),
    Err(e) => {
      error!(error = %e, "failed to search events");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

pub async fn search_objects(
  State(state): State<Arc<SearchApiState>>,
  Json(query): Json<ObjectSearchQuery>,
) -> Result<Json<ObjectSearchResponse>, StatusCode> {
  info!(object_type = %query.object_type, "searching objects");
  match state.store.search_objects(&query).await {
    Ok(response) => Ok(Json(response)),
    Err(e) => {
      error!(error = %e, "failed to search objects");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

pub async fn reindex_recordings(
  State(state): State<Arc<SearchApiState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
  info!("reindexing all recordings");
  match state.indexer.index_all_recordings().await {
    Ok(count) => Ok(Json(serde_json::json!({
      "indexed": count,
      "message": "Reindexing completed successfully"
    }))),
    Err(e) => {
      error!(error = %e, "failed to reindex recordings");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

pub async fn get_search_stats(
  State(state): State<Arc<SearchApiState>>,
) -> Result<Json<SearchStatsResponse>, StatusCode> {
  match state.store.get_search_stats().await {
    Ok(stats) => Ok(Json(stats)),
    Err(e) => {
      error!(error = %e, "failed to get search stats");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}
