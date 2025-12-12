-- Alert Rules Table
CREATE TABLE IF NOT EXISTS alert_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,

    -- Severity: info, warning, error, critical
    severity VARCHAR(20) NOT NULL DEFAULT 'info',

    -- Trigger type: device_offline, device_online, motion_detected, ai_detection,
    -- recording_started, recording_stopped, recording_failed, stream_started, stream_stopped,
    -- stream_failed, health_check_failed, custom
    trigger_type VARCHAR(50) NOT NULL,

    -- Condition (JSON): flexible rule matching
    -- Examples:
    -- {"device_id": "uuid", "consecutive_failures": 3}
    -- {"zone": "entrance", "object_type": "person", "confidence": 0.8}
    -- {"metric": "cpu_usage", "operator": ">", "threshold": 80}
    condition_json JSONB NOT NULL DEFAULT '{}',

    -- Suppression settings
    suppress_duration_secs INTEGER, -- How long to suppress after firing (cooldown)
    max_alerts_per_hour INTEGER, -- Rate limiting

    -- Schedule (cron expression for when rule is active)
    -- NULL means always active
    -- Examples: "0 9-17 * * MON-FRI" (business hours)
    schedule_cron VARCHAR(255),

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,

    UNIQUE(tenant_id, name)
);

CREATE INDEX idx_alert_rules_tenant ON alert_rules(tenant_id);
CREATE INDEX idx_alert_rules_enabled ON alert_rules(enabled);
CREATE INDEX idx_alert_rules_trigger_type ON alert_rules(trigger_type);
CREATE INDEX idx_alert_rules_condition ON alert_rules USING GIN(condition_json);

-- Alert Actions Table (notification channels)
CREATE TABLE IF NOT EXISTS alert_actions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id UUID NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,

    -- Action type: email, webhook, mqtt, sms (future)
    action_type VARCHAR(20) NOT NULL,

    -- Configuration (JSON):
    -- Email: {"to": ["user@example.com"], "subject": "Alert: {rule_name}"}
    -- Webhook: {"url": "https://example.com/webhook", "method": "POST", "headers": {...}}
    -- MQTT: {"broker": "mqtt://broker:1883", "topic": "alerts/{severity}", "qos": 1}
    config_json JSONB NOT NULL,

    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(rule_id, action_type, config_json)
);

CREATE INDEX idx_alert_actions_rule ON alert_actions(rule_id);
CREATE INDEX idx_alert_actions_type ON alert_actions(action_type);

-- Alert Events Table (fired alerts history)
CREATE TABLE IF NOT EXISTS alert_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id UUID NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL,

    -- Event details
    severity VARCHAR(20) NOT NULL,
    trigger_type VARCHAR(50) NOT NULL,
    message TEXT NOT NULL,

    -- Context data (JSON): what triggered the alert
    -- Examples:
    -- {"device_id": "uuid", "device_name": "Camera 1", "status": "offline"}
    -- {"recording_id": "uuid", "error": "storage full"}
    context_json JSONB NOT NULL DEFAULT '{}',

    -- Firing state
    fired_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    suppressed BOOLEAN NOT NULL DEFAULT false,
    suppressed_reason TEXT,

    -- Notification delivery tracking
    notifications_sent INTEGER NOT NULL DEFAULT 0,
    notifications_failed INTEGER NOT NULL DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_alert_events_rule ON alert_events(rule_id);
CREATE INDEX idx_alert_events_tenant ON alert_events(tenant_id);
CREATE INDEX idx_alert_events_fired_at ON alert_events(fired_at DESC);
CREATE INDEX idx_alert_events_severity ON alert_events(severity);
CREATE INDEX idx_alert_events_trigger ON alert_events(trigger_type);
CREATE INDEX idx_alert_events_context ON alert_events USING GIN(context_json);

-- Alert Notifications Table (individual notification attempts)
CREATE TABLE IF NOT EXISTS alert_notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL REFERENCES alert_events(id) ON DELETE CASCADE,
    action_id UUID NOT NULL REFERENCES alert_actions(id) ON DELETE CASCADE,

    -- Delivery status: pending, sent, failed
    status VARCHAR(20) NOT NULL DEFAULT 'pending',

    -- Delivery details
    sent_at TIMESTAMPTZ,
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_alert_notifications_event ON alert_notifications(event_id);
CREATE INDEX idx_alert_notifications_status ON alert_notifications(status);
CREATE INDEX idx_alert_notifications_sent_at ON alert_notifications(sent_at DESC);

-- Alert Suppression State Table (tracking cooldowns)
CREATE TABLE IF NOT EXISTS alert_suppression_state (
    rule_id UUID PRIMARY KEY REFERENCES alert_rules(id) ON DELETE CASCADE,
    last_fired_at TIMESTAMPTZ NOT NULL,
    suppressed_until TIMESTAMPTZ NOT NULL,
    alert_count_this_hour INTEGER NOT NULL DEFAULT 1,
    hour_window_start TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_alert_suppression_until ON alert_suppression_state(suppressed_until);

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_alert_rules_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER alert_rules_updated_at
    BEFORE UPDATE ON alert_rules
    FOR EACH ROW
    EXECUTE FUNCTION update_alert_rules_updated_at();
