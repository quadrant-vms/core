use crate::types::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct AlertStore {
    pub(crate) pool: PgPool,
}

impl AlertStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // Alert Rules
    pub async fn create_rule(&self, tenant_id: Uuid, req: &CreateAlertRuleRequest, created_by: Option<Uuid>) -> Result<AlertRule> {
        let id = Uuid::new_v4();
        let enabled = req.enabled.unwrap_or(true);

        let rule = sqlx::query_as!(
            AlertRule,
            r#"
            INSERT INTO alert_rules (id, tenant_id, name, description, enabled, severity, trigger_type, condition_json, suppress_duration_secs, max_alerts_per_hour, schedule_cron, created_by)
            VALUES ($1, $2, $3, $4, $5, $6::text, $7::text, $8, $9, $10, $11, $12)
            RETURNING id, tenant_id, name, description, enabled, severity as "severity: Severity", trigger_type as "trigger_type: TriggerType", condition_json, suppress_duration_secs, max_alerts_per_hour, schedule_cron, created_at, updated_at, created_by
            "#,
            id,
            tenant_id,
            req.name,
            req.description,
            enabled,
            req.severity.to_string(),
            req.trigger_type.to_string(),
            req.condition_json,
            req.suppress_duration_secs,
            req.max_alerts_per_hour,
            req.schedule_cron,
            created_by
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(rule)
    }

    pub async fn get_rule(&self, id: Uuid, tenant_id: Uuid) -> Result<Option<AlertRule>> {
        let rule = sqlx::query_as!(
            AlertRule,
            r#"
            SELECT id, tenant_id, name, description, enabled, severity as "severity: Severity", trigger_type as "trigger_type: TriggerType", condition_json, suppress_duration_secs, max_alerts_per_hour, schedule_cron, created_at, updated_at, created_by
            FROM alert_rules
            WHERE id = $1 AND tenant_id = $2
            "#,
            id,
            tenant_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(rule)
    }

    pub async fn list_rules(&self, tenant_id: Uuid, enabled_only: bool) -> Result<Vec<AlertRule>> {
        let rules = if enabled_only {
            sqlx::query_as!(
                AlertRule,
                r#"
                SELECT id, tenant_id, name, description, enabled, severity as "severity: Severity", trigger_type as "trigger_type: TriggerType", condition_json, suppress_duration_secs, max_alerts_per_hour, schedule_cron, created_at, updated_at, created_by
                FROM alert_rules
                WHERE tenant_id = $1 AND enabled = true
                ORDER BY created_at DESC
                "#,
                tenant_id
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as!(
                AlertRule,
                r#"
                SELECT id, tenant_id, name, description, enabled, severity as "severity: Severity", trigger_type as "trigger_type: TriggerType", condition_json, suppress_duration_secs, max_alerts_per_hour, schedule_cron, created_at, updated_at, created_by
                FROM alert_rules
                WHERE tenant_id = $1
                ORDER BY created_at DESC
                "#,
                tenant_id
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rules)
    }

    pub async fn update_rule(&self, id: Uuid, tenant_id: Uuid, req: &UpdateAlertRuleRequest) -> Result<Option<AlertRule>> {
        let mut query = "UPDATE alert_rules SET ".to_string();
        let mut updates = Vec::new();
        let mut param_count = 3; // Starting after id and tenant_id

        if req.name.is_some() {
            updates.push(format!("name = ${}", param_count));
            param_count += 1;
        }
        if req.description.is_some() {
            updates.push(format!("description = ${}", param_count));
            param_count += 1;
        }
        if req.enabled.is_some() {
            updates.push(format!("enabled = ${}", param_count));
            param_count += 1;
        }
        if req.severity.is_some() {
            updates.push(format!("severity = ${}::text", param_count));
            param_count += 1;
        }
        if req.condition_json.is_some() {
            updates.push(format!("condition_json = ${}", param_count));
            param_count += 1;
        }
        if req.suppress_duration_secs.is_some() {
            updates.push(format!("suppress_duration_secs = ${}", param_count));
            param_count += 1;
        }
        if req.max_alerts_per_hour.is_some() {
            updates.push(format!("max_alerts_per_hour = ${}", param_count));
            param_count += 1;
        }
        if req.schedule_cron.is_some() {
            updates.push(format!("schedule_cron = ${}", param_count));
        }

        if updates.is_empty() {
            return self.get_rule(id, tenant_id).await;
        }

        query.push_str(&updates.join(", "));
        query.push_str(" WHERE id = $1 AND tenant_id = $2 RETURNING *");

        let mut query_builder = sqlx::query(&query).bind(id).bind(tenant_id);

        if let Some(ref name) = req.name {
            query_builder = query_builder.bind(name);
        }
        if let Some(ref description) = req.description {
            query_builder = query_builder.bind(description);
        }
        if let Some(enabled) = req.enabled {
            query_builder = query_builder.bind(enabled);
        }
        if let Some(ref severity) = req.severity {
            query_builder = query_builder.bind(severity.to_string());
        }
        if let Some(ref condition_json) = req.condition_json {
            query_builder = query_builder.bind(condition_json);
        }
        if let Some(suppress_duration_secs) = req.suppress_duration_secs {
            query_builder = query_builder.bind(suppress_duration_secs);
        }
        if let Some(max_alerts_per_hour) = req.max_alerts_per_hour {
            query_builder = query_builder.bind(max_alerts_per_hour);
        }
        if let Some(ref schedule_cron) = req.schedule_cron {
            query_builder = query_builder.bind(schedule_cron);
        }

        let row = query_builder.fetch_optional(&self.pool).await?;

        match row {
            Some(row) => {
                let severity_str: String = row.get("severity");
                let severity = severity_str.parse().map_err(|e| {
                    tracing::warn!(severity=%severity_str, error=%e, "invalid severity in database, using default");
                    e
                }).unwrap_or_default();

                let trigger_type_str: String = row.get("trigger_type");
                let trigger_type = trigger_type_str.parse().map_err(|e| {
                    tracing::warn!(trigger_type=%trigger_type_str, error=%e, "invalid trigger_type in database, using default");
                    e
                }).unwrap_or_default();

                let rule = AlertRule {
                    id: row.get("id"),
                    tenant_id: row.get("tenant_id"),
                    name: row.get("name"),
                    description: row.get("description"),
                    enabled: row.get("enabled"),
                    severity,
                    trigger_type,
                    condition_json: row.get("condition_json"),
                    suppress_duration_secs: row.get("suppress_duration_secs"),
                    max_alerts_per_hour: row.get("max_alerts_per_hour"),
                    schedule_cron: row.get("schedule_cron"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    created_by: row.get("created_by"),
                };
                Ok(Some(rule))
            }
            None => Ok(None),
        }
    }

    pub async fn delete_rule(&self, id: Uuid, tenant_id: Uuid) -> Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM alert_rules WHERE id = $1 AND tenant_id = $2",
            id,
            tenant_id
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // Alert Actions
    pub async fn create_action(&self, rule_id: Uuid, req: &CreateAlertActionRequest) -> Result<AlertAction> {
        let id = Uuid::new_v4();
        let enabled = req.enabled.unwrap_or(true);

        let action = sqlx::query_as!(
            AlertAction,
            r#"
            INSERT INTO alert_actions (id, rule_id, action_type, config_json, enabled)
            VALUES ($1, $2, $3::text, $4, $5)
            RETURNING id, rule_id, action_type as "action_type: ActionType", config_json, enabled, created_at
            "#,
            id,
            rule_id,
            req.action_type.to_string(),
            req.config_json,
            enabled
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(action)
    }

    pub async fn list_actions(&self, rule_id: Uuid) -> Result<Vec<AlertAction>> {
        let actions = sqlx::query_as!(
            AlertAction,
            r#"
            SELECT id, rule_id, action_type as "action_type: ActionType", config_json, enabled, created_at
            FROM alert_actions
            WHERE rule_id = $1
            ORDER BY created_at ASC
            "#,
            rule_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(actions)
    }

    pub async fn delete_action(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM alert_actions WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // Alert Events
    pub async fn create_event(
        &self,
        rule_id: Uuid,
        tenant_id: Uuid,
        severity: &Severity,
        trigger_type: &TriggerType,
        message: &str,
        context_json: &serde_json::Value,
        suppressed: bool,
        suppressed_reason: Option<String>,
    ) -> Result<AlertEvent> {
        let id = Uuid::new_v4();

        let event = sqlx::query_as!(
            AlertEvent,
            r#"
            INSERT INTO alert_events (id, rule_id, tenant_id, severity, trigger_type, message, context_json, suppressed, suppressed_reason)
            VALUES ($1, $2, $3, $4::text, $5::text, $6, $7, $8, $9)
            RETURNING id, rule_id, tenant_id, severity as "severity: Severity", trigger_type as "trigger_type: TriggerType", message, context_json, fired_at, suppressed, suppressed_reason, notifications_sent, notifications_failed, created_at
            "#,
            id,
            rule_id,
            tenant_id,
            severity.to_string(),
            trigger_type.to_string(),
            message,
            context_json,
            suppressed,
            suppressed_reason
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(event)
    }

    pub async fn get_event(&self, id: Uuid) -> Result<Option<AlertEvent>> {
        let event = sqlx::query_as!(
            AlertEvent,
            r#"
            SELECT id, rule_id, tenant_id, severity as "severity: Severity", trigger_type as "trigger_type: TriggerType", message, context_json, fired_at, suppressed, suppressed_reason, notifications_sent, notifications_failed, created_at
            FROM alert_events
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(event)
    }

    pub async fn list_events(&self, tenant_id: Uuid, limit: i64, offset: i64) -> Result<Vec<AlertEvent>> {
        let events = sqlx::query_as!(
            AlertEvent,
            r#"
            SELECT id, rule_id, tenant_id, severity as "severity: Severity", trigger_type as "trigger_type: TriggerType", message, context_json, fired_at, suppressed, suppressed_reason, notifications_sent, notifications_failed, created_at
            FROM alert_events
            WHERE tenant_id = $1
            ORDER BY fired_at DESC
            LIMIT $2 OFFSET $3
            "#,
            tenant_id,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    pub async fn increment_notifications_sent(&self, event_id: Uuid) -> Result<()> {
        sqlx::query!(
            "UPDATE alert_events SET notifications_sent = notifications_sent + 1 WHERE id = $1",
            event_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn increment_notifications_failed(&self, event_id: Uuid) -> Result<()> {
        sqlx::query!(
            "UPDATE alert_events SET notifications_failed = notifications_failed + 1 WHERE id = $1",
            event_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Alert Notifications
    pub async fn create_notification(&self, event_id: Uuid, action_id: Uuid) -> Result<AlertNotification> {
        let id = Uuid::new_v4();

        let notification = sqlx::query_as!(
            AlertNotification,
            r#"
            INSERT INTO alert_notifications (id, event_id, action_id, status)
            VALUES ($1, $2, $3, 'pending'::text)
            RETURNING id, event_id, action_id, status as "status: NotificationStatus", sent_at, error_message, retry_count, created_at
            "#,
            id,
            event_id,
            action_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(notification)
    }

    pub async fn update_notification_status(
        &self,
        id: Uuid,
        status: &NotificationStatus,
        error_message: Option<String>,
    ) -> Result<()> {
        let sent_at = if *status == NotificationStatus::Sent {
            Some(Utc::now())
        } else {
            None
        };

        sqlx::query!(
            "UPDATE alert_notifications SET status = $1::text, sent_at = $2, error_message = $3 WHERE id = $4",
            status.to_string(),
            sent_at,
            error_message,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn increment_notification_retry(&self, id: Uuid) -> Result<()> {
        sqlx::query!(
            "UPDATE alert_notifications SET retry_count = retry_count + 1 WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Suppression State
    pub async fn get_suppression_state(&self, rule_id: Uuid) -> Result<Option<SuppressionState>> {
        let state = sqlx::query_as!(
            SuppressionState,
            r#"
            SELECT rule_id, last_fired_at, suppressed_until, alert_count_this_hour, hour_window_start, updated_at
            FROM alert_suppression_state
            WHERE rule_id = $1
            "#,
            rule_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(state)
    }

    pub async fn upsert_suppression_state(&self, state: &SuppressionState) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO alert_suppression_state (rule_id, last_fired_at, suppressed_until, alert_count_this_hour, hour_window_start)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (rule_id) DO UPDATE
            SET last_fired_at = $2, suppressed_until = $3, alert_count_this_hour = $4, hour_window_start = $5, updated_at = NOW()
            "#,
            state.rule_id,
            state.last_fired_at,
            state.suppressed_until,
            state.alert_count_this_hour,
            state.hour_window_start
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn find_rules_by_trigger(&self, trigger_type: &TriggerType, tenant_id: Uuid) -> Result<Vec<AlertRule>> {
        let rules = sqlx::query_as!(
            AlertRule,
            r#"
            SELECT id, tenant_id, name, description, enabled, severity as "severity: Severity", trigger_type as "trigger_type: TriggerType", condition_json, suppress_duration_secs, max_alerts_per_hour, schedule_cron, created_at, updated_at, created_by
            FROM alert_rules
            WHERE tenant_id = $1 AND trigger_type = $2::text AND enabled = true
            ORDER BY created_at ASC
            "#,
            tenant_id,
            trigger_type.to_string()
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rules)
    }
}

#[derive(Debug, Clone)]
pub struct SuppressionState {
    pub rule_id: Uuid,
    pub last_fired_at: DateTime<Utc>,
    pub suppressed_until: DateTime<Utc>,
    pub alert_count_this_hour: i32,
    pub hour_window_start: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
