use crate::notifier::Notifier;
use crate::rule_engine::RuleEngine;
use crate::store::AlertStore;
use crate::types::*;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use common::auth_middleware::RequireAuth;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub store: AlertStore,
    pub engine: Arc<RuleEngine>,
    pub notifier: Arc<Notifier>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health check
        .route("/health", axum::routing::get(health_check))
        .route("/readyz", axum::routing::get(ready_check))
        // Alert Rules
        .route("/v1/rules", axum::routing::post(create_rule))
        .route("/v1/rules", axum::routing::get(list_rules))
        .route("/v1/rules/:rule_id", axum::routing::get(get_rule))
        .route("/v1/rules/:rule_id", axum::routing::put(update_rule))
        .route("/v1/rules/:rule_id", axum::routing::delete(delete_rule))
        // Alert Actions
        .route("/v1/rules/:rule_id/actions", axum::routing::post(create_action))
        .route("/v1/rules/:rule_id/actions", axum::routing::get(list_actions))
        .route("/v1/actions/:action_id", axum::routing::delete(delete_action))
        // Alert Events
        .route("/v1/events", axum::routing::get(list_events))
        .route("/v1/events/:event_id", axum::routing::get(get_event))
        // Trigger alerts (for integration)
        .route("/v1/trigger", axum::routing::post(trigger_alert))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "alert-service"
    }))
}

async fn ready_check(State(state): State<AppState>) -> impl IntoResponse {
    // Check database connectivity
    match sqlx::query("SELECT 1").execute(&state.store.pool).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "status": "ready",
                "database": "connected"
            })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "not ready",
                "database": "disconnected"
            })),
        )
            .into_response(),
    }
}

// Alert Rules endpoints

async fn create_rule(
    State(state): State<AppState>,
    RequireAuth(auth_ctx): RequireAuth,
    Json(req): Json<CreateAlertRuleRequest>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("alert:create") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let tenant_id = Uuid::parse_str(&auth_ctx.tenant_id).unwrap();
    let user_id = Uuid::parse_str(&auth_ctx.user_id).unwrap();

    match state.store.create_rule(tenant_id, &req, Some(user_id)).await {
        Ok(rule) => (StatusCode::CREATED, Json(rule)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn get_rule(
    State(state): State<AppState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(rule_id): Path<Uuid>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("alert:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let tenant_id = Uuid::parse_str(&auth_ctx.tenant_id).unwrap();

    match state.store.get_rule(rule_id, tenant_id).await {
        Ok(Some(rule)) => Json(rule).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "rule not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct ListRulesQuery {
    #[serde(default)]
    enabled_only: bool,
}

async fn list_rules(
    State(state): State<AppState>,
    RequireAuth(auth_ctx): RequireAuth,
    Query(query): Query<ListRulesQuery>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("alert:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let tenant_id = Uuid::parse_str(&auth_ctx.tenant_id).unwrap();

    match state.store.list_rules(tenant_id, query.enabled_only).await {
        Ok(rules) => Json(rules).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn update_rule(
    State(state): State<AppState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(rule_id): Path<Uuid>,
    Json(req): Json<UpdateAlertRuleRequest>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("alert:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let tenant_id = Uuid::parse_str(&auth_ctx.tenant_id).unwrap();

    match state.store.update_rule(rule_id, tenant_id, &req).await {
        Ok(Some(rule)) => Json(rule).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "rule not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn delete_rule(
    State(state): State<AppState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(rule_id): Path<Uuid>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("alert:delete") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let tenant_id = Uuid::parse_str(&auth_ctx.tenant_id).unwrap();

    match state.store.delete_rule(rule_id, tenant_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "rule not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// Alert Actions endpoints

async fn create_action(
    State(state): State<AppState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(rule_id): Path<Uuid>,
    Json(req): Json<CreateAlertActionRequest>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("alert:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let tenant_id = Uuid::parse_str(&auth_ctx.tenant_id).unwrap();

    // Verify rule exists and belongs to tenant
    match state.store.get_rule(rule_id, tenant_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "rule not found"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }

    match state.store.create_action(rule_id, &req).await {
        Ok(action) => (StatusCode::CREATED, Json(action)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn list_actions(
    State(state): State<AppState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(rule_id): Path<Uuid>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("alert:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let tenant_id = Uuid::parse_str(&auth_ctx.tenant_id).unwrap();

    // Verify rule exists and belongs to tenant
    match state.store.get_rule(rule_id, tenant_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "rule not found"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }

    match state.store.list_actions(rule_id).await {
        Ok(actions) => Json(actions).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn delete_action(
    State(state): State<AppState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(action_id): Path<Uuid>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("alert:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.store.delete_action(action_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "action not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// Alert Events endpoints

#[derive(Deserialize)]
struct ListEventsQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    100
}

async fn list_events(
    State(state): State<AppState>,
    RequireAuth(auth_ctx): RequireAuth,
    Query(query): Query<ListEventsQuery>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("alert:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let tenant_id = Uuid::parse_str(&auth_ctx.tenant_id).unwrap();

    match state
        .store
        .list_events(tenant_id, query.limit, query.offset)
        .await
    {
        Ok(events) => Json(events).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn get_event(
    State(state): State<AppState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(event_id): Path<Uuid>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("alert:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.store.get_event(event_id).await {
        Ok(Some(event)) => Json(event).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "event not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// Trigger alert endpoint (for integration with other services)

async fn trigger_alert(
    State(state): State<AppState>,
    RequireAuth(auth_ctx): RequireAuth,
    Json(req): Json<TriggerAlertRequest>,
) -> impl IntoResponse {
    let tenant_id = Uuid::parse_str(&auth_ctx.tenant_id).unwrap();

    // Evaluate and fire alerts
    let events = match state
        .engine
        .evaluate_and_fire(tenant_id, &req.trigger_type, req.message, req.context)
        .await
    {
        Ok(events) => events,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    // Send notifications for each event
    for event in &events {
        if let Err(e) = state.notifier.notify(event).await {
            tracing::error!(
                event_id = %event.id,
                error = %e,
                "Failed to send notifications"
            );
        }
    }

    Json(json!({
        "fired_count": events.len(),
        "events": events,
    }))
    .into_response()
}
