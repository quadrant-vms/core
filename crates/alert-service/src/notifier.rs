use crate::store::AlertStore;
use crate::types::*;
use anyhow::{Context, Result};
use async_trait::async_trait;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

#[async_trait]
pub trait NotificationChannel: Send + Sync {
    async fn send(&self, event: &AlertEvent, action: &AlertAction) -> Result<()>;
    fn channel_type(&self) -> ActionType;
}

pub struct EmailChannel {
    smtp_host: String,
    smtp_port: u16,
    smtp_username: String,
    smtp_password: String,
    from_address: String,
}

impl EmailChannel {
    pub fn new(
        smtp_host: String,
        smtp_port: u16,
        smtp_username: String,
        smtp_password: String,
        from_address: String,
    ) -> Self {
        Self {
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_password,
            from_address,
        }
    }

    fn render_template(&self, template: &str, event: &AlertEvent) -> String {
        template
            .replace("{severity}", &event.severity.to_string())
            .replace("{message}", &event.message)
            .replace("{trigger_type}", &event.trigger_type.to_string())
            .replace("{event_id}", &event.id.to_string())
            .replace("{fired_at}", &event.fired_at.to_string())
    }
}

#[async_trait]
impl NotificationChannel for EmailChannel {
    async fn send(&self, event: &AlertEvent, action: &AlertAction) -> Result<()> {
        let config: EmailActionConfig = serde_json::from_value(action.config_json.clone())
            .context("Invalid email action config")?;

        let subject = config
            .subject
            .unwrap_or_else(|| format!("Alert: {}", event.severity));

        let body = if let Some(template) = config.template {
            self.render_template(&template, event)
        } else {
            format!(
                "Alert Notification\n\n\
                Severity: {}\n\
                Trigger: {}\n\
                Message: {}\n\n\
                Event ID: {}\n\
                Fired At: {}\n\n\
                Context:\n{}\n",
                event.severity,
                event.trigger_type,
                event.message,
                event.id,
                event.fired_at,
                serde_json::to_string_pretty(&event.context_json)?
            )
        };

        // Build email message
        let mut email_builder = Message::builder()
            .from(self.from_address.parse()?)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN);

        for to in &config.to {
            email_builder = email_builder.to(to.parse()?);
        }

        let email = email_builder.body(body)?;

        // Send via SMTP
        let creds = Credentials::new(self.smtp_username.clone(), self.smtp_password.clone());

        let mailer = SmtpTransport::relay(&self.smtp_host)?
            .port(self.smtp_port)
            .credentials(creds)
            .build();

        mailer.send(&email)?;

        info!(
            event_id = %event.id,
            recipients = ?config.to,
            "Email notification sent"
        );

        Ok(())
    }

    fn channel_type(&self) -> ActionType {
        ActionType::Email
    }
}

pub struct WebhookChannel {
    client: reqwest::Client,
}

impl WebhookChannel {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    fn render_template(&self, template: &str, event: &AlertEvent) -> String {
        template
            .replace("{severity}", &event.severity.to_string())
            .replace("{message}", &event.message)
            .replace("{trigger_type}", &event.trigger_type.to_string())
            .replace("{event_id}", &event.id.to_string())
            .replace("{fired_at}", &event.fired_at.to_string())
            .replace("{context}", &event.context_json.to_string())
    }
}

impl Default for WebhookChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NotificationChannel for WebhookChannel {
    async fn send(&self, event: &AlertEvent, action: &AlertAction) -> Result<()> {
        let config: WebhookActionConfig = serde_json::from_value(action.config_json.clone())
            .context("Invalid webhook action config")?;

        let method = config.method.unwrap_or_else(|| "POST".to_string());

        // Build payload
        let payload = if let Some(template) = config.template {
            self.render_template(&template, event)
        } else {
            serde_json::to_string(&serde_json::json!({
                "event_id": event.id,
                "rule_id": event.rule_id,
                "tenant_id": event.tenant_id,
                "severity": event.severity,
                "trigger_type": event.trigger_type,
                "message": event.message,
                "context": event.context_json,
                "fired_at": event.fired_at,
            }))?
        };

        // Build request
        let mut request = self.client.request(
            method.parse().context("Invalid HTTP method")?,
            &config.url,
        );

        // Add headers
        if let Some(headers) = config.headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        } else {
            request = request.header("Content-Type", "application/json");
        }

        request = request.body(payload);

        // Send request
        let response = request.send().await?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Webhook request failed with status: {}",
                response.status()
            );
        }

        info!(
            event_id = %event.id,
            url = %config.url,
            status = %response.status(),
            "Webhook notification sent"
        );

        Ok(())
    }

    fn channel_type(&self) -> ActionType {
        ActionType::Webhook
    }
}

pub struct MqttChannel {
    // We'll keep a cache of MQTT clients per broker
    clients: Arc<tokio::sync::Mutex<HashMap<String, AsyncClient>>>,
}

impl MqttChannel {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    async fn get_or_create_client(&self, config: &MqttActionConfig) -> Result<AsyncClient> {
        let mut clients = self.clients.lock().await;

        if let Some(client) = clients.get(&config.broker) {
            return Ok(client.clone());
        }

        // Parse broker URL
        let broker_url = url::Url::parse(&config.broker)?;
        let host = broker_url.host_str().context("Invalid broker host")?;
        let port = broker_url.port().unwrap_or(1883);

        let client_id = format!("quadrant-alert-{}", Uuid::new_v4());
        let mut mqtt_options = MqttOptions::new(client_id, host, port);

        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            mqtt_options.set_credentials(username, password);
        }

        mqtt_options.set_keep_alive(Duration::from_secs(30));

        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

        // Spawn eventloop task
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("MQTT eventloop error: {}", e);
                        break;
                    }
                }
            }
        });

        clients.insert(config.broker.clone(), client.clone());

        Ok(client)
    }
}

impl Default for MqttChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NotificationChannel for MqttChannel {
    async fn send(&self, event: &AlertEvent, action: &AlertAction) -> Result<()> {
        let config: MqttActionConfig = serde_json::from_value(action.config_json.clone())
            .context("Invalid MQTT action config")?;

        let client = self.get_or_create_client(&config).await?;

        // Render topic (support template variables)
        let topic = config
            .topic
            .replace("{severity}", &event.severity.to_string())
            .replace("{trigger_type}", &event.trigger_type.to_string())
            .replace("{tenant_id}", &event.tenant_id.to_string());

        // Build payload
        let payload = serde_json::to_string(&serde_json::json!({
            "event_id": event.id,
            "rule_id": event.rule_id,
            "tenant_id": event.tenant_id,
            "severity": event.severity,
            "trigger_type": event.trigger_type,
            "message": event.message,
            "context": event.context_json,
            "fired_at": event.fired_at,
        }))?;

        let qos = match config.qos.unwrap_or(1) {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => QoS::AtLeastOnce,
        };

        client.publish(topic.clone(), qos, false, payload).await?;

        info!(
            event_id = %event.id,
            topic = %topic,
            qos = ?qos,
            "MQTT notification sent"
        );

        Ok(())
    }

    fn channel_type(&self) -> ActionType {
        ActionType::Mqtt
    }
}

pub struct Notifier {
    store: AlertStore,
    channels: HashMap<ActionType, Arc<dyn NotificationChannel>>,
}

impl Notifier {
    pub fn new(store: AlertStore) -> Self {
        let mut channels: HashMap<ActionType, Arc<dyn NotificationChannel>> = HashMap::new();

        // Add webhook channel (always available)
        channels.insert(ActionType::Webhook, Arc::new(WebhookChannel::new()));

        // Add MQTT channel (always available)
        channels.insert(ActionType::Mqtt, Arc::new(MqttChannel::new()));

        Self { store, channels }
    }

    pub fn add_email_channel(
        &mut self,
        smtp_host: String,
        smtp_port: u16,
        smtp_username: String,
        smtp_password: String,
        from_address: String,
    ) {
        let channel = EmailChannel::new(
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_password,
            from_address,
        );
        self.channels
            .insert(ActionType::Email, Arc::new(channel));
    }

    pub async fn notify(&self, event: &AlertEvent) -> Result<()> {
        if event.suppressed {
            info!(event_id = %event.id, "Event is suppressed, skipping notifications");
            return Ok(());
        }

        // Get all actions for this rule
        let actions = self.store.list_actions(event.rule_id).await?;

        for action in actions {
            if !action.enabled {
                continue;
            }

            // Create notification record
            let notification = self.store.create_notification(event.id, action.id).await?;

            // Get channel
            let channel = match self.channels.get(&action.action_type) {
                Some(c) => c,
                None => {
                    error!(
                        action_type = ?action.action_type,
                        "No channel available for action type"
                    );
                    self.store
                        .update_notification_status(
                            notification.id,
                            &NotificationStatus::Failed,
                            Some("Channel not configured".to_string()),
                        )
                        .await?;
                    self.store.increment_notifications_failed(event.id).await?;
                    continue;
                }
            };

            // Send notification
            match channel.send(event, &action).await {
                Ok(_) => {
                    self.store
                        .update_notification_status(notification.id, &NotificationStatus::Sent, None)
                        .await?;
                    self.store.increment_notifications_sent(event.id).await?;
                }
                Err(e) => {
                    error!(
                        event_id = %event.id,
                        action_id = %action.id,
                        error = %e,
                        "Failed to send notification"
                    );
                    self.store
                        .update_notification_status(
                            notification.id,
                            &NotificationStatus::Failed,
                            Some(e.to_string()),
                        )
                        .await?;
                    self.store.increment_notifications_failed(event.id).await?;
                }
            }
        }

        Ok(())
    }
}
