use crate::store::{AlertStore, SuppressionState};
use crate::types::*;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

pub struct RuleEngine {
    store: AlertStore,
}

impl RuleEngine {
    pub fn new(store: AlertStore) -> Self {
        Self { store }
    }

    /// Evaluate if alert should fire based on context
    pub async fn evaluate_and_fire(
        &self,
        tenant_id: Uuid,
        trigger_type: &TriggerType,
        message: String,
        context: HashMap<String, serde_json::Value>,
    ) -> Result<Vec<AlertEvent>> {
        // Find all enabled rules for this trigger type
        let rules = self
            .store
            .find_rules_by_trigger(trigger_type, tenant_id)
            .await?;

        let mut fired_events = Vec::new();

        for rule in rules {
            // Check schedule (if rule has time windows)
            if !self.is_within_schedule(&rule)? {
                tracing::debug!(
                    rule_id = %rule.id,
                    "Rule outside schedule window, skipping"
                );
                continue;
            }

            // Check condition matching
            if !self.matches_condition(&rule, &context) {
                tracing::debug!(
                    rule_id = %rule.id,
                    "Condition not matched, skipping"
                );
                continue;
            }

            // Check suppression
            let (suppressed, suppressed_reason) = self.check_suppression(&rule).await?;

            if suppressed {
                tracing::info!(
                    rule_id = %rule.id,
                    reason = ?suppressed_reason,
                    "Alert suppressed"
                );

                // Still create event but mark as suppressed
                let event = self
                    .store
                    .create_event(
                        rule.id,
                        tenant_id,
                        &rule.severity,
                        &rule.trigger_type,
                        &message,
                        &serde_json::to_value(&context)?,
                        true,
                        suppressed_reason,
                    )
                    .await?;

                fired_events.push(event);
                continue;
            }

            // Create alert event
            let event = self
                .store
                .create_event(
                    rule.id,
                    tenant_id,
                    &rule.severity,
                    &rule.trigger_type,
                    &message,
                    &serde_json::to_value(&context)?,
                    false,
                    None,
                )
                .await?;

            tracing::info!(
                event_id = %event.id,
                rule_id = %rule.id,
                severity = ?rule.severity,
                "Alert fired"
            );

            // Update suppression state
            self.update_suppression_state(&rule).await?;

            fired_events.push(event);
        }

        Ok(fired_events)
    }

    /// Check if current time is within the rule's schedule
    fn is_within_schedule(&self, rule: &AlertRule) -> Result<bool> {
        let Some(ref schedule_cron) = rule.schedule_cron else {
            return Ok(true); // No schedule = always active
        };

        // Parse cron expression
        let schedule = cron::Schedule::from_str(schedule_cron)
            .context("Failed to parse cron expression")?;

        let now = Utc::now();

        // Check if current time matches the schedule
        // We check if there's a matching time in the past minute
        let upcoming = schedule.upcoming(Utc).next();
        let previous = schedule.after(&(now - Duration::minutes(1))).next();

        Ok(upcoming.is_some() || previous.is_some())
    }

    /// Check if context matches rule conditions
    fn matches_condition(
        &self,
        rule: &AlertRule,
        context: &HashMap<String, serde_json::Value>,
    ) -> bool {
        let condition = &rule.condition_json;

        // Empty condition = always match
        if condition.is_null() || (condition.is_object() && condition.as_object().unwrap().is_empty()) {
            return true;
        }

        // Match each condition field
        let Some(condition_obj) = condition.as_object() else {
            return false;
        };

        for (key, expected_value) in condition_obj {
            match context.get(key) {
                Some(actual_value) => {
                    if !self.value_matches(expected_value, actual_value) {
                        return false;
                    }
                }
                None => return false, // Required field missing
            }
        }

        true
    }

    /// Compare two JSON values with operator support
    fn value_matches(&self, expected: &serde_json::Value, actual: &serde_json::Value) -> bool {
        // Direct equality
        if expected == actual {
            return true;
        }

        // Handle operator-based comparisons
        if let Some(expected_obj) = expected.as_object() {
            // Check for operator syntax: {"operator": ">", "value": 80}
            if let (Some(op), Some(threshold)) = (expected_obj.get("operator"), expected_obj.get("value")) {
                return self.apply_operator(op.as_str().unwrap_or("="), actual, threshold);
            }
        }

        // String pattern matching
        if let (Some(expected_str), Some(actual_str)) = (expected.as_str(), actual.as_str()) {
            if expected_str.contains('*') || expected_str.contains('?') {
                // Simple wildcard matching
                return self.wildcard_match(expected_str, actual_str);
            }
        }

        false
    }

    /// Apply comparison operator
    fn apply_operator(&self, op: &str, actual: &serde_json::Value, threshold: &serde_json::Value) -> bool {
        match (actual.as_f64(), threshold.as_f64()) {
            (Some(a), Some(t)) => match op {
                ">" => a > t,
                ">=" => a >= t,
                "<" => a < t,
                "<=" => a <= t,
                "=" | "==" => (a - t).abs() < f64::EPSILON,
                "!=" => (a - t).abs() >= f64::EPSILON,
                _ => false,
            },
            _ => false,
        }
    }

    /// Simple wildcard matching (* and ? support)
    fn wildcard_match(&self, pattern: &str, text: &str) -> bool {
        // Validate regex pattern to prevent ReDoS attacks
        if let Err(e) = common::validation::validate_regex_pattern(pattern) {
            tracing::warn!(pattern=%pattern, error=%e, "invalid regex pattern");
            return false;
        }

        let pattern = pattern.replace('*', ".*").replace('?', ".");
        match regex::Regex::new(&format!("^{}$", pattern)) {
            Ok(re) => re.is_match(text),
            Err(e) => {
                tracing::warn!(pattern=%pattern, error=%e, "failed to compile regex");
                false
            }
        }
    }

    /// Check if alert should be suppressed
    async fn check_suppression(&self, rule: &AlertRule) -> Result<(bool, Option<String>)> {
        let now = Utc::now();

        // Get current suppression state
        let state = self.store.get_suppression_state(rule.id).await?;

        // Check cooldown suppression
        if let Some(state) = &state {
            if state.suppressed_until > now {
                let remaining = (state.suppressed_until - now).num_seconds();
                return Ok((
                    true,
                    Some(format!("Cooldown active ({} seconds remaining)", remaining)),
                ));
            }
        }

        // Check rate limiting
        if let Some(max_per_hour) = rule.max_alerts_per_hour {
            if let Some(state) = &state {
                // Check if we're still in the same hour window
                if state.hour_window_start + Duration::hours(1) > now {
                    if state.alert_count_this_hour >= max_per_hour {
                        return Ok((
                            true,
                            Some(format!(
                                "Rate limit exceeded ({} alerts in the past hour)",
                                state.alert_count_this_hour
                            )),
                        ));
                    }
                }
            }
        }

        Ok((false, None))
    }

    /// Update suppression state after firing an alert
    async fn update_suppression_state(&self, rule: &AlertRule) -> Result<()> {
        let now = Utc::now();

        let state = self.store.get_suppression_state(rule.id).await?;

        let new_state = match state {
            Some(mut state) => {
                // Update cooldown
                state.last_fired_at = now;
                if let Some(suppress_secs) = rule.suppress_duration_secs {
                    state.suppressed_until = now + Duration::seconds(suppress_secs as i64);
                } else {
                    state.suppressed_until = now; // No suppression
                }

                // Update rate limit counter
                if state.hour_window_start + Duration::hours(1) > now {
                    // Same hour window
                    state.alert_count_this_hour += 1;
                } else {
                    // New hour window
                    state.hour_window_start = now;
                    state.alert_count_this_hour = 1;
                }

                state.updated_at = now;
                state
            }
            None => {
                // Create new state
                let suppressed_until = if let Some(suppress_secs) = rule.suppress_duration_secs {
                    now + Duration::seconds(suppress_secs as i64)
                } else {
                    now
                };

                SuppressionState {
                    rule_id: rule.id,
                    last_fired_at: now,
                    suppressed_until,
                    alert_count_this_hour: 1,
                    hour_window_start: now,
                    updated_at: now,
                }
            }
        };

        self.store.upsert_suppression_state(&new_state).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wildcard_match() {
        let engine = RuleEngine {
            store: AlertStore::new(sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap()),
        };

        assert!(engine.wildcard_match("Camera*", "Camera1"));
        assert!(engine.wildcard_match("Camera*", "Camera123"));
        assert!(!engine.wildcard_match("Camera*", "Recorder1"));

        assert!(engine.wildcard_match("Cam?ra", "Camera"));
        assert!(engine.wildcard_match("Cam?ra", "Camara"));
        assert!(!engine.wildcard_match("Cam?ra", "Cam12ra"));
    }

    #[tokio::test]
    async fn test_operator_matching() {
        let engine = RuleEngine {
            store: AlertStore::new(sqlx::PgPool::connect_lazy("postgres://localhost/test").unwrap()),
        };

        let actual = serde_json::json!(85.5);
        let threshold = serde_json::json!(80.0);

        assert!(engine.apply_operator(">", &actual, &threshold));
        assert!(engine.apply_operator(">=", &actual, &threshold));
        assert!(!engine.apply_operator("<", &actual, &threshold));
        assert!(!engine.apply_operator("<=", &actual, &threshold));
    }
}
