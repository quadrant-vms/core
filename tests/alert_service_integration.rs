use alert_service::{create_router, AlertStore, AppState, Notifier, RuleEngine, Severity, TriggerType};
use anyhow::Result;
use axum_test::TestServer;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

async fn setup_test_db() -> Result<sqlx::PgPool> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/quadrant_vms".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("./crates/alert-service/migrations")
        .run(&pool)
        .await?;

    // Clean up test data
    sqlx::query("DELETE FROM alert_events WHERE 1=1")
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM alert_actions WHERE 1=1")
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM alert_rules WHERE 1=1")
        .execute(&pool)
        .await?;

    Ok(pool)
}

async fn create_test_server() -> Result<TestServer> {
    let pool = setup_test_db().await?;

    let store = AlertStore::new(pool);
    let engine = Arc::new(RuleEngine::new(store.clone()));
    let notifier = Arc::new(Notifier::new(store.clone()));

    let state = AppState {
        store,
        engine,
        notifier,
    };

    let app = create_router(state);
    Ok(TestServer::new(app)?)
}

#[tokio::test]
async fn test_health_check() -> Result<()> {
    let server = create_test_server().await?;

    let response = server.get("/health").await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["status"], "healthy");

    Ok(())
}

#[tokio::test]
async fn test_create_and_get_rule() -> Result<()> {
    let pool = setup_test_db().await?;
    let store = AlertStore::new(pool);

    let tenant_id = uuid::Uuid::new_v4();
    let user_id = uuid::Uuid::new_v4();

    // Create a rule
    let req = alert_service::CreateAlertRuleRequest {
        name: "Test Alert Rule".to_string(),
        description: Some("A test alert rule".to_string()),
        enabled: Some(true),
        severity: Severity::Warning,
        trigger_type: TriggerType::DeviceOffline,
        condition_json: json!({
            "device_id": tenant_id.to_string()
        }),
        suppress_duration_secs: Some(300),
        max_alerts_per_hour: Some(10),
        schedule_cron: None,
    };

    let rule = store.create_rule(tenant_id, &req, Some(user_id)).await?;

    assert_eq!(rule.name, "Test Alert Rule");
    assert_eq!(rule.severity, Severity::Warning);
    assert_eq!(rule.trigger_type, TriggerType::DeviceOffline);
    assert!(rule.enabled);

    // Get the rule
    let fetched_rule = store.get_rule(rule.id, tenant_id).await?;
    assert!(fetched_rule.is_some());

    let fetched_rule = fetched_rule.unwrap();
    assert_eq!(fetched_rule.id, rule.id);
    assert_eq!(fetched_rule.name, "Test Alert Rule");

    Ok(())
}

#[tokio::test]
async fn test_create_action_for_rule() -> Result<()> {
    let pool = setup_test_db().await?;
    let store = AlertStore::new(pool);

    let tenant_id = uuid::Uuid::new_v4();

    // Create a rule first
    let rule_req = alert_service::CreateAlertRuleRequest {
        name: "Rule with Actions".to_string(),
        description: None,
        enabled: Some(true),
        severity: Severity::Error,
        trigger_type: TriggerType::RecordingFailed,
        condition_json: json!({}),
        suppress_duration_secs: None,
        max_alerts_per_hour: None,
        schedule_cron: None,
    };

    let rule = store.create_rule(tenant_id, &rule_req, None).await?;

    // Create a webhook action
    let action_req = alert_service::CreateAlertActionRequest {
        action_type: alert_service::ActionType::Webhook,
        config_json: json!({
            "url": "https://example.com/webhook",
            "method": "POST"
        }),
        enabled: Some(true),
    };

    let action = store.create_action(rule.id, &action_req).await?;

    assert_eq!(action.rule_id, rule.id);
    assert_eq!(action.action_type, alert_service::ActionType::Webhook);
    assert!(action.enabled);

    // List actions
    let actions = store.list_actions(rule.id).await?;
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].id, action.id);

    Ok(())
}

#[tokio::test]
async fn test_rule_engine_evaluation() -> Result<()> {
    let pool = setup_test_db().await?;
    let store = AlertStore::new(pool.clone());
    let engine = RuleEngine::new(store.clone());

    let tenant_id = uuid::Uuid::new_v4();
    let device_id = uuid::Uuid::new_v4();

    // Create a rule that matches when device goes offline
    let rule_req = alert_service::CreateAlertRuleRequest {
        name: "Device Offline Alert".to_string(),
        description: Some("Alert when device goes offline".to_string()),
        enabled: Some(true),
        severity: Severity::Critical,
        trigger_type: TriggerType::DeviceOffline,
        condition_json: json!({
            "device_id": device_id.to_string()
        }),
        suppress_duration_secs: None,
        max_alerts_per_hour: None,
        schedule_cron: None,
    };

    let _rule = store.create_rule(tenant_id, &rule_req, None).await?;

    // Trigger the alert
    let mut context = std::collections::HashMap::new();
    context.insert("device_id".to_string(), json!(device_id.to_string()));
    context.insert("device_name".to_string(), json!("Camera 1"));

    let events = engine
        .evaluate_and_fire(
            tenant_id,
            &TriggerType::DeviceOffline,
            "Camera 1 went offline".to_string(),
            context,
        )
        .await?;

    assert_eq!(events.len(), 1);

    let event = &events[0];
    assert_eq!(event.severity, Severity::Critical);
    assert_eq!(event.message, "Camera 1 went offline");
    assert!(!event.suppressed);

    Ok(())
}

#[tokio::test]
async fn test_rule_suppression() -> Result<()> {
    let pool = setup_test_db().await?;
    let store = AlertStore::new(pool.clone());
    let engine = RuleEngine::new(store.clone());

    let tenant_id = uuid::Uuid::new_v4();

    // Create a rule with suppression
    let rule_req = alert_service::CreateAlertRuleRequest {
        name: "Suppressed Alert".to_string(),
        description: None,
        enabled: Some(true),
        severity: Severity::Info,
        trigger_type: TriggerType::Custom,
        condition_json: json!({}),
        suppress_duration_secs: Some(10), // 10 seconds cooldown
        max_alerts_per_hour: None,
        schedule_cron: None,
    };

    let _rule = store.create_rule(tenant_id, &rule_req, None).await?;

    // First trigger should fire
    let events1 = engine
        .evaluate_and_fire(
            tenant_id,
            &TriggerType::Custom,
            "Test message".to_string(),
            std::collections::HashMap::new(),
        )
        .await?;

    assert_eq!(events1.len(), 1);
    assert!(!events1[0].suppressed);

    // Immediate second trigger should be suppressed
    let events2 = engine
        .evaluate_and_fire(
            tenant_id,
            &TriggerType::Custom,
            "Test message 2".to_string(),
            std::collections::HashMap::new(),
        )
        .await?;

    assert_eq!(events2.len(), 1);
    assert!(events2[0].suppressed);
    assert!(events2[0].suppressed_reason.is_some());

    Ok(())
}

#[tokio::test]
async fn test_rate_limiting() -> Result<()> {
    let pool = setup_test_db().await?;
    let store = AlertStore::new(pool.clone());
    let engine = RuleEngine::new(store.clone());

    let tenant_id = uuid::Uuid::new_v4();

    // Create a rule with rate limiting
    let rule_req = alert_service::CreateAlertRuleRequest {
        name: "Rate Limited Alert".to_string(),
        description: None,
        enabled: Some(true),
        severity: Severity::Warning,
        trigger_type: TriggerType::MotionDetected,
        condition_json: json!({}),
        suppress_duration_secs: None,
        max_alerts_per_hour: Some(2), // Max 2 alerts per hour
        schedule_cron: None,
    };

    let _rule = store.create_rule(tenant_id, &rule_req, None).await?;

    // First alert should fire
    let events1 = engine
        .evaluate_and_fire(
            tenant_id,
            &TriggerType::MotionDetected,
            "Motion 1".to_string(),
            std::collections::HashMap::new(),
        )
        .await?;
    assert!(!events1[0].suppressed);

    // Second alert should fire
    let events2 = engine
        .evaluate_and_fire(
            tenant_id,
            &TriggerType::MotionDetected,
            "Motion 2".to_string(),
            std::collections::HashMap::new(),
        )
        .await?;
    assert!(!events2[0].suppressed);

    // Third alert should be rate limited
    let events3 = engine
        .evaluate_and_fire(
            tenant_id,
            &TriggerType::MotionDetected,
            "Motion 3".to_string(),
            std::collections::HashMap::new(),
        )
        .await?;
    assert!(events3[0].suppressed);
    assert!(events3[0]
        .suppressed_reason
        .as_ref()
        .unwrap()
        .contains("Rate limit"));

    Ok(())
}

#[tokio::test]
async fn test_list_rules_and_events() -> Result<()> {
    let pool = setup_test_db().await?;
    let store = AlertStore::new(pool.clone());

    let tenant_id = uuid::Uuid::new_v4();

    // Create multiple rules
    for i in 0..3 {
        let req = alert_service::CreateAlertRuleRequest {
            name: format!("Rule {}", i),
            description: None,
            enabled: Some(true),
            severity: Severity::Info,
            trigger_type: TriggerType::Custom,
            condition_json: json!({}),
            suppress_duration_secs: None,
            max_alerts_per_hour: None,
            schedule_cron: None,
        };

        store.create_rule(tenant_id, &req, None).await?;
    }

    // List all rules
    let rules = store.list_rules(tenant_id, false).await?;
    assert_eq!(rules.len(), 3);

    // List only enabled rules
    let enabled_rules = store.list_rules(tenant_id, true).await?;
    assert_eq!(enabled_rules.len(), 3);

    Ok(())
}

#[tokio::test]
async fn test_create_slack_action() -> Result<()> {
    let pool = setup_test_db().await?;
    let store = AlertStore::new(pool);

    let tenant_id = uuid::Uuid::new_v4();

    // Create a rule
    let rule_req = alert_service::CreateAlertRuleRequest {
        name: "Slack Notification Rule".to_string(),
        description: None,
        enabled: Some(true),
        severity: Severity::Warning,
        trigger_type: TriggerType::AiDetection,
        condition_json: json!({}),
        suppress_duration_secs: None,
        max_alerts_per_hour: None,
        schedule_cron: None,
    };

    let rule = store.create_rule(tenant_id, &rule_req, None).await?;

    // Create a Slack action
    let action_req = alert_service::CreateAlertActionRequest {
        action_type: alert_service::ActionType::Slack,
        config_json: json!({
            "webhook_url": "https://hooks.slack.com/services/TEST/TEST/TEST",
            "channel": "#alerts",
            "username": "Quadrant VMS",
            "icon_emoji": ":camera:"
        }),
        enabled: Some(true),
    };

    let action = store.create_action(rule.id, &action_req).await?;

    assert_eq!(action.rule_id, rule.id);
    assert_eq!(action.action_type, alert_service::ActionType::Slack);
    assert!(action.enabled);

    // Verify config JSON
    let config = action.config_json.as_object().unwrap();
    assert_eq!(
        config.get("webhook_url").unwrap().as_str().unwrap(),
        "https://hooks.slack.com/services/TEST/TEST/TEST"
    );
    assert_eq!(config.get("channel").unwrap().as_str().unwrap(), "#alerts");

    Ok(())
}

#[tokio::test]
async fn test_create_discord_action() -> Result<()> {
    let pool = setup_test_db().await?;
    let store = AlertStore::new(pool);

    let tenant_id = uuid::Uuid::new_v4();

    // Create a rule
    let rule_req = alert_service::CreateAlertRuleRequest {
        name: "Discord Notification Rule".to_string(),
        description: None,
        enabled: Some(true),
        severity: Severity::Critical,
        trigger_type: TriggerType::DeviceOffline,
        condition_json: json!({}),
        suppress_duration_secs: None,
        max_alerts_per_hour: None,
        schedule_cron: None,
    };

    let rule = store.create_rule(tenant_id, &rule_req, None).await?;

    // Create a Discord action
    let action_req = alert_service::CreateAlertActionRequest {
        action_type: alert_service::ActionType::Discord,
        config_json: json!({
            "webhook_url": "https://discord.com/api/webhooks/TEST/TEST",
            "username": "Quadrant VMS Bot",
            "avatar_url": "https://example.com/avatar.png"
        }),
        enabled: Some(true),
    };

    let action = store.create_action(rule.id, &action_req).await?;

    assert_eq!(action.rule_id, rule.id);
    assert_eq!(action.action_type, alert_service::ActionType::Discord);
    assert!(action.enabled);

    // Verify config JSON
    let config = action.config_json.as_object().unwrap();
    assert_eq!(
        config.get("webhook_url").unwrap().as_str().unwrap(),
        "https://discord.com/api/webhooks/TEST/TEST"
    );

    Ok(())
}

#[tokio::test]
async fn test_create_sms_action() -> Result<()> {
    let pool = setup_test_db().await?;
    let store = AlertStore::new(pool);

    let tenant_id = uuid::Uuid::new_v4();

    // Create a rule
    let rule_req = alert_service::CreateAlertRuleRequest {
        name: "SMS Notification Rule".to_string(),
        description: None,
        enabled: Some(true),
        severity: Severity::Critical,
        trigger_type: TriggerType::StreamFailed,
        condition_json: json!({}),
        suppress_duration_secs: None,
        max_alerts_per_hour: None,
        schedule_cron: None,
    };

    let rule = store.create_rule(tenant_id, &rule_req, None).await?;

    // Create an SMS action
    let action_req = alert_service::CreateAlertActionRequest {
        action_type: alert_service::ActionType::Sms,
        config_json: json!({
            "to": ["+15551234567", "+15559876543"],
            "template": "[{severity}] {trigger_type}: {message}"
        }),
        enabled: Some(true),
    };

    let action = store.create_action(rule.id, &action_req).await?;

    assert_eq!(action.rule_id, rule.id);
    assert_eq!(action.action_type, alert_service::ActionType::Sms);
    assert!(action.enabled);

    // Verify config JSON
    let config = action.config_json.as_object().unwrap();
    let to_numbers = config.get("to").unwrap().as_array().unwrap();
    assert_eq!(to_numbers.len(), 2);
    assert_eq!(to_numbers[0].as_str().unwrap(), "+15551234567");

    Ok(())
}

#[tokio::test]
async fn test_multiple_notification_channels() -> Result<()> {
    let pool = setup_test_db().await?;
    let store = AlertStore::new(pool);

    let tenant_id = uuid::Uuid::new_v4();

    // Create a rule
    let rule_req = alert_service::CreateAlertRuleRequest {
        name: "Multi-Channel Alert".to_string(),
        description: Some("Alert sent to multiple channels".to_string()),
        enabled: Some(true),
        severity: Severity::Error,
        trigger_type: TriggerType::RecordingFailed,
        condition_json: json!({}),
        suppress_duration_secs: None,
        max_alerts_per_hour: None,
        schedule_cron: None,
    };

    let rule = store.create_rule(tenant_id, &rule_req, None).await?;

    // Create Slack action
    let slack_action = alert_service::CreateAlertActionRequest {
        action_type: alert_service::ActionType::Slack,
        config_json: json!({
            "webhook_url": "https://hooks.slack.com/services/TEST/TEST/TEST",
            "channel": "#critical-alerts"
        }),
        enabled: Some(true),
    };
    store.create_action(rule.id, &slack_action).await?;

    // Create Discord action
    let discord_action = alert_service::CreateAlertActionRequest {
        action_type: alert_service::ActionType::Discord,
        config_json: json!({
            "webhook_url": "https://discord.com/api/webhooks/TEST/TEST"
        }),
        enabled: Some(true),
    };
    store.create_action(rule.id, &discord_action).await?;

    // Create Webhook action
    let webhook_action = alert_service::CreateAlertActionRequest {
        action_type: alert_service::ActionType::Webhook,
        config_json: json!({
            "url": "https://example.com/alert-webhook",
            "method": "POST"
        }),
        enabled: Some(true),
    };
    store.create_action(rule.id, &webhook_action).await?;

    // List all actions for the rule
    let actions = store.list_actions(rule.id).await?;
    assert_eq!(actions.len(), 3);

    // Verify we have one of each type
    let action_types: Vec<_> = actions.iter().map(|a| a.action_type.clone()).collect();
    assert!(action_types.contains(&alert_service::ActionType::Slack));
    assert!(action_types.contains(&alert_service::ActionType::Discord));
    assert!(action_types.contains(&alert_service::ActionType::Webhook));

    Ok(())
}
