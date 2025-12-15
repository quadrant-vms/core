use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IncidentSeverity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IncidentStatus {
    Open,
    Acknowledged,
    Investigating,
    Resolved,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentNote {
    pub id: String,
    pub author: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: String,
    pub title: String,
    pub description: String,
    pub severity: IncidentSeverity,
    pub status: IncidentStatus,
    pub source: String,
    pub device_id: Option<String>,
    pub alert_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
    pub notes: Vec<IncidentNote>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Incident {
    pub fn new(
        title: String,
        description: String,
        severity: IncidentSeverity,
        source: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            title,
            description,
            severity,
            status: IncidentStatus::Open,
            source,
            device_id: None,
            alert_id: None,
            created_at: now,
            updated_at: now,
            acknowledged_at: None,
            acknowledged_by: None,
            resolved_at: None,
            resolved_by: None,
            notes: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn acknowledge(&mut self, acknowledged_by: String) {
        self.status = IncidentStatus::Acknowledged;
        self.acknowledged_at = Some(Utc::now());
        self.acknowledged_by = Some(acknowledged_by);
        self.updated_at = Utc::now();
    }

    pub fn resolve(&mut self, resolved_by: String) {
        self.status = IncidentStatus::Resolved;
        self.resolved_at = Some(Utc::now());
        self.resolved_by = Some(resolved_by);
        self.updated_at = Utc::now();
    }

    pub fn add_note(&mut self, author: String, content: String) {
        let note = IncidentNote {
            id: Uuid::new_v4().to_string(),
            author,
            content,
            created_at: Utc::now(),
        };
        self.notes.push(note);
        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Default)]
pub struct IncidentStore {
    incidents: HashMap<String, Incident>,
}

impl IncidentStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&mut self, incident: Incident) -> Incident {
        let id = incident.id.clone();
        self.incidents.insert(id, incident.clone());
        incident
    }

    pub fn get(&self, id: &str) -> Option<&Incident> {
        self.incidents.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Incident> {
        self.incidents.get_mut(id)
    }

    pub fn list(&self) -> Vec<&Incident> {
        let mut incidents: Vec<&Incident> = self.incidents.values().collect();
        incidents.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        incidents
    }

    pub fn update(&mut self, id: &str, incident: Incident) -> Option<Incident> {
        if self.incidents.contains_key(id) {
            self.incidents.insert(id.to_string(), incident.clone());
            Some(incident)
        } else {
            None
        }
    }
}
