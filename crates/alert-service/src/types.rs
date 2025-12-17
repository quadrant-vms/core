use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, Default)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Info,
    Warning,
    Error,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

impl std::str::FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "info" => Ok(Severity::Info),
            "warning" => Ok(Severity::Warning),
            "error" => Ok(Severity::Error),
            "critical" => Ok(Severity::Critical),
            _ => Err(format!("Invalid severity: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, Default)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    DeviceOffline,
    DeviceOnline,
    MotionDetected,
    AiDetection,
    RecordingStarted,
    RecordingStopped,
    RecordingFailed,
    StreamStarted,
    StreamStopped,
    StreamFailed,
    HealthCheckFailed,
    #[default]
    Custom,
}

impl std::fmt::Display for TriggerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TriggerType::DeviceOffline => "device_offline",
            TriggerType::DeviceOnline => "device_online",
            TriggerType::MotionDetected => "motion_detected",
            TriggerType::AiDetection => "ai_detection",
            TriggerType::RecordingStarted => "recording_started",
            TriggerType::RecordingStopped => "recording_stopped",
            TriggerType::RecordingFailed => "recording_failed",
            TriggerType::StreamStarted => "stream_started",
            TriggerType::StreamStopped => "stream_stopped",
            TriggerType::StreamFailed => "stream_failed",
            TriggerType::HealthCheckFailed => "health_check_failed",
            TriggerType::Custom => "custom",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for TriggerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "device_offline" => Ok(TriggerType::DeviceOffline),
            "device_online" => Ok(TriggerType::DeviceOnline),
            "motion_detected" => Ok(TriggerType::MotionDetected),
            "ai_detection" => Ok(TriggerType::AiDetection),
            "recording_started" => Ok(TriggerType::RecordingStarted),
            "recording_stopped" => Ok(TriggerType::RecordingStopped),
            "recording_failed" => Ok(TriggerType::RecordingFailed),
            "stream_started" => Ok(TriggerType::StreamStarted),
            "stream_stopped" => Ok(TriggerType::StreamStopped),
            "stream_failed" => Ok(TriggerType::StreamFailed),
            "health_check_failed" => Ok(TriggerType::HealthCheckFailed),
            "custom" => Ok(TriggerType::Custom),
            _ => Err(format!("Invalid trigger type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Email,
    Webhook,
    Mqtt,
    Slack,
    Discord,
    Sms,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionType::Email => write!(f, "email"),
            ActionType::Webhook => write!(f, "webhook"),
            ActionType::Mqtt => write!(f, "mqtt"),
            ActionType::Slack => write!(f, "slack"),
            ActionType::Discord => write!(f, "discord"),
            ActionType::Sms => write!(f, "sms"),
        }
    }
}

impl std::str::FromStr for ActionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "email" => Ok(ActionType::Email),
            "webhook" => Ok(ActionType::Webhook),
            "mqtt" => Ok(ActionType::Mqtt),
            "slack" => Ok(ActionType::Slack),
            "discord" => Ok(ActionType::Discord),
            "sms" => Ok(ActionType::Sms),
            _ => Err(format!("Invalid action type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub severity: Severity,
    pub trigger_type: TriggerType,
    #[serde(default)]
    pub condition_json: serde_json::Value,
    pub suppress_duration_secs: Option<i32>,
    pub max_alerts_per_hour: Option<i32>,
    pub schedule_cron: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlertRuleRequest {
    pub name: String,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub severity: Severity,
    pub trigger_type: TriggerType,
    #[serde(default)]
    pub condition_json: serde_json::Value,
    pub suppress_duration_secs: Option<i32>,
    pub max_alerts_per_hour: Option<i32>,
    pub schedule_cron: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAlertRuleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub severity: Option<Severity>,
    pub condition_json: Option<serde_json::Value>,
    pub suppress_duration_secs: Option<i32>,
    pub max_alerts_per_hour: Option<i32>,
    pub schedule_cron: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertAction {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub action_type: ActionType,
    pub config_json: serde_json::Value,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlertActionRequest {
    pub action_type: ActionType,
    pub config_json: serde_json::Value,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub tenant_id: Uuid,
    pub severity: Severity,
    pub trigger_type: TriggerType,
    pub message: String,
    #[serde(default)]
    pub context_json: serde_json::Value,
    pub fired_at: DateTime<Utc>,
    pub suppressed: bool,
    pub suppressed_reason: Option<String>,
    pub notifications_sent: i32,
    pub notifications_failed: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerAlertRequest {
    pub trigger_type: TriggerType,
    pub message: String,
    #[serde(default)]
    pub context: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case")]
pub enum NotificationStatus {
    Pending,
    Sent,
    Failed,
}

impl std::fmt::Display for NotificationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationStatus::Pending => write!(f, "pending"),
            NotificationStatus::Sent => write!(f, "sent"),
            NotificationStatus::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertNotification {
    pub id: Uuid,
    pub event_id: Uuid,
    pub action_id: Uuid,
    pub status: NotificationStatus,
    pub sent_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub created_at: DateTime<Utc>,
}

// Configuration structures for action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailActionConfig {
    pub to: Vec<String>,
    pub subject: Option<String>,
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookActionConfig {
    pub url: String,
    pub method: Option<String>, // Default: POST
    pub headers: Option<HashMap<String, String>>,
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttActionConfig {
    pub broker: String,
    pub topic: String,
    pub qos: Option<u8>, // 0, 1, or 2
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackActionConfig {
    pub webhook_url: String,
    pub channel: Option<String>,
    pub username: Option<String>,
    pub icon_emoji: Option<String>,
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordActionConfig {
    pub webhook_url: String,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsActionConfig {
    pub to: Vec<String>, // Phone numbers in E.164 format
    pub template: Option<String>,
}

// Alert context helpers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertContext {
    pub device_id: Option<Uuid>,
    pub device_name: Option<String>,
    pub recording_id: Option<Uuid>,
    pub stream_id: Option<String>,
    pub ai_task_id: Option<Uuid>,
    pub zone: Option<String>,
    pub object_type: Option<String>,
    pub confidence: Option<f64>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl AlertContext {
    pub fn new() -> Self {
        Self {
            device_id: None,
            device_name: None,
            recording_id: None,
            stream_id: None,
            ai_task_id: None,
            zone: None,
            object_type: None,
            confidence: None,
            extra: HashMap::new(),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

impl Default for AlertContext {
    fn default() -> Self {
        Self::new()
    }
}
