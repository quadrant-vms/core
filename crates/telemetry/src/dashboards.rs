//! Grafana dashboard templates for SLO monitoring
//!
//! This module provides pre-built Grafana dashboard JSON templates for monitoring
//! Service Level Objectives across all Quadrant VMS services.

use serde_json::{json, Value};

/// Generate a comprehensive SLO dashboard for Quadrant VMS
pub fn generate_slo_dashboard() -> Value {
    json!({
        "dashboard": {
            "title": "Quadrant VMS - Service Level Objectives",
            "tags": ["quadrant-vms", "slo", "monitoring"],
            "timezone": "browser",
            "schemaVersion": 16,
            "version": 1,
            "refresh": "30s",
            "time": {
                "from": "now-1h",
                "to": "now"
            },
            "templating": {
                "list": [
                    {
                        "name": "service",
                        "type": "query",
                        "datasource": "Prometheus",
                        "query": "label_values(slo_service_up, service)",
                        "refresh": 1,
                        "multi": false,
                        "includeAll": false
                    },
                    {
                        "name": "tenant_id",
                        "type": "query",
                        "datasource": "Prometheus",
                        "query": "label_values(slo_service_up{service=\"$service\"}, tenant_id)",
                        "refresh": 1,
                        "multi": true,
                        "includeAll": true
                    },
                    {
                        "name": "node_id",
                        "type": "query",
                        "datasource": "Prometheus",
                        "query": "label_values(slo_service_up{service=\"$service\"}, node_id)",
                        "refresh": 1,
                        "multi": true,
                        "includeAll": true
                    }
                ]
            },
            "panels": [
                // Row 1: Availability Overview
                {
                    "type": "row",
                    "title": "Availability",
                    "gridPos": {"x": 0, "y": 0, "w": 24, "h": 1}
                },
                {
                    "title": "Service Uptime",
                    "type": "stat",
                    "gridPos": {"x": 0, "y": 1, "w": 6, "h": 4},
                    "targets": [{
                        "expr": "avg(slo_service_up{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}) * 100",
                        "legendFormat": "Uptime %"
                    }],
                    "fieldConfig": {
                        "defaults": {
                            "unit": "percent",
                            "thresholds": {
                                "mode": "absolute",
                                "steps": [
                                    {"value": 0, "color": "red"},
                                    {"value": 95, "color": "yellow"},
                                    {"value": 99, "color": "green"}
                                ]
                            }
                        }
                    }
                },
                {
                    "title": "Health Check Success Rate",
                    "type": "stat",
                    "gridPos": {"x": 6, "y": 1, "w": 6, "h": 4},
                    "targets": [{
                        "expr": "sum(rate(slo_health_check_success_total{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m])) / (sum(rate(slo_health_check_success_total{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m])) + sum(rate(slo_health_check_failure_total{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m]))) * 100",
                        "legendFormat": "Success Rate %"
                    }],
                    "fieldConfig": {
                        "defaults": {
                            "unit": "percent",
                            "thresholds": {
                                "mode": "absolute",
                                "steps": [
                                    {"value": 0, "color": "red"},
                                    {"value": 95, "color": "yellow"},
                                    {"value": 99, "color": "green"}
                                ]
                            }
                        }
                    }
                },
                {
                    "title": "Dependency Status",
                    "type": "stat",
                    "gridPos": {"x": 12, "y": 1, "w": 6, "h": 4},
                    "targets": [{
                        "expr": "avg(slo_dependency_up{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}) * 100",
                        "legendFormat": "Dependencies Up %"
                    }],
                    "fieldConfig": {
                        "defaults": {
                            "unit": "percent",
                            "thresholds": {
                                "mode": "absolute",
                                "steps": [
                                    {"value": 0, "color": "red"},
                                    {"value": 90, "color": "yellow"},
                                    {"value": 99, "color": "green"}
                                ]
                            }
                        }
                    }
                },
                {
                    "title": "Uptime by Node",
                    "type": "table",
                    "gridPos": {"x": 18, "y": 1, "w": 6, "h": 4},
                    "targets": [{
                        "expr": "slo_service_up{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}",
                        "format": "table",
                        "instant": true
                    }]
                },

                // Row 2: Latency Metrics
                {
                    "type": "row",
                    "title": "Latency",
                    "gridPos": {"x": 0, "y": 5, "w": 24, "h": 1}
                },
                {
                    "title": "Request Latency (p50, p95, p99)",
                    "type": "graph",
                    "gridPos": {"x": 0, "y": 6, "w": 12, "h": 6},
                    "targets": [
                        {
                            "expr": "histogram_quantile(0.50, sum(rate(slo_request_latency_seconds_bucket{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m])) by (le, endpoint)) * 1000",
                            "legendFormat": "p50 - {{endpoint}}"
                        },
                        {
                            "expr": "histogram_quantile(0.95, sum(rate(slo_request_latency_seconds_bucket{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m])) by (le, endpoint)) * 1000",
                            "legendFormat": "p95 - {{endpoint}}"
                        },
                        {
                            "expr": "histogram_quantile(0.99, sum(rate(slo_request_latency_seconds_bucket{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m])) by (le, endpoint)) * 1000",
                            "legendFormat": "p99 - {{endpoint}}"
                        }
                    ],
                    "yaxes": [{
                        "label": "Latency (ms)",
                        "format": "ms"
                    }]
                },
                {
                    "title": "Database Query Latency (p95)",
                    "type": "graph",
                    "gridPos": {"x": 12, "y": 6, "w": 12, "h": 6},
                    "targets": [{
                        "expr": "histogram_quantile(0.95, sum(rate(slo_db_query_latency_seconds_bucket{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m])) by (le, operation)) * 1000",
                        "legendFormat": "{{operation}}"
                    }],
                    "yaxes": [{
                        "label": "Latency (ms)",
                        "format": "ms"
                    }]
                },

                // Row 3: Error Rate
                {
                    "type": "row",
                    "title": "Error Rate",
                    "gridPos": {"x": 0, "y": 12, "w": 24, "h": 1}
                },
                {
                    "title": "Request Error Rate (4xx, 5xx)",
                    "type": "graph",
                    "gridPos": {"x": 0, "y": 13, "w": 12, "h": 6},
                    "targets": [
                        {
                            "expr": "sum(rate(slo_requests_failed_total{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\", status_class=\"4xx\"}[5m])) by (endpoint)",
                            "legendFormat": "4xx - {{endpoint}}"
                        },
                        {
                            "expr": "sum(rate(slo_requests_failed_total{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\", status_class=\"5xx\"}[5m])) by (endpoint)",
                            "legendFormat": "5xx - {{endpoint}}"
                        }
                    ],
                    "yaxes": [{
                        "label": "Errors/sec",
                        "format": "reqps"
                    }]
                },
                {
                    "title": "Overall Error Rate %",
                    "type": "stat",
                    "gridPos": {"x": 12, "y": 13, "w": 6, "h": 6},
                    "targets": [{
                        "expr": "(sum(rate(slo_requests_failed_total{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m])) / sum(rate(slo_requests_total{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m]))) * 100",
                        "legendFormat": "Error Rate %"
                    }],
                    "fieldConfig": {
                        "defaults": {
                            "unit": "percent",
                            "thresholds": {
                                "mode": "absolute",
                                "steps": [
                                    {"value": 0, "color": "green"},
                                    {"value": 1, "color": "yellow"},
                                    {"value": 5, "color": "red"}
                                ]
                            }
                        }
                    }
                },
                {
                    "title": "Pipeline Failures",
                    "type": "graph",
                    "gridPos": {"x": 18, "y": 13, "w": 6, "h": 6},
                    "targets": [{
                        "expr": "sum(rate(slo_pipeline_failures_total{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m])) by (pipeline_type, failure_reason)",
                        "legendFormat": "{{pipeline_type}} - {{failure_reason}}"
                    }]
                },

                // Row 4: Throughput
                {
                    "type": "row",
                    "title": "Throughput",
                    "gridPos": {"x": 0, "y": 19, "w": 24, "h": 1}
                },
                {
                    "title": "Request Rate",
                    "type": "graph",
                    "gridPos": {"x": 0, "y": 20, "w": 8, "h": 6},
                    "targets": [{
                        "expr": "sum(rate(slo_request_rate_total{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m])) by (tenant_id)",
                        "legendFormat": "{{tenant_id}}"
                    }],
                    "yaxes": [{
                        "label": "Requests/sec",
                        "format": "reqps"
                    }]
                },
                {
                    "title": "Bytes Processed",
                    "type": "graph",
                    "gridPos": {"x": 8, "y": 20, "w": 8, "h": 6},
                    "targets": [{
                        "expr": "sum(rate(slo_bytes_processed_total{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}[5m])) by (operation)",
                        "legendFormat": "{{operation}}"
                    }],
                    "yaxes": [{
                        "label": "Bytes/sec",
                        "format": "Bps"
                    }]
                },
                {
                    "title": "Concurrent Operations",
                    "type": "graph",
                    "gridPos": {"x": 16, "y": 20, "w": 8, "h": 6},
                    "targets": [{
                        "expr": "slo_concurrent_operations{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}",
                        "legendFormat": "{{operation_type}}"
                    }]
                },

                // Row 5: Resource Utilization
                {
                    "type": "row",
                    "title": "Resource Utilization",
                    "gridPos": {"x": 0, "y": 26, "w": 24, "h": 1}
                },
                {
                    "title": "CPU Utilization by Node",
                    "type": "graph",
                    "gridPos": {"x": 0, "y": 27, "w": 12, "h": 6},
                    "targets": [{
                        "expr": "slo_cpu_utilization_percent{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}",
                        "legendFormat": "{{node_id}}"
                    }],
                    "yaxes": [{
                        "label": "CPU %",
                        "format": "percent",
                        "max": 100
                    }]
                },
                {
                    "title": "Memory Usage by Node",
                    "type": "graph",
                    "gridPos": {"x": 12, "y": 27, "w": 12, "h": 6},
                    "targets": [{
                        "expr": "slo_memory_usage_bytes{service=\"$service\", tenant_id=~\"$tenant_id\", node_id=~\"$node_id\"}",
                        "legendFormat": "{{node_id}}"
                    }],
                    "yaxes": [{
                        "label": "Memory",
                        "format": "bytes"
                    }]
                }
            ]
        },
        "overwrite": true
    })
}

/// Generate a tenant-specific SLO dashboard
pub fn generate_tenant_slo_dashboard(tenant_id: &str) -> Value {
    json!({
        "dashboard": {
            "title": format!("Tenant SLO Dashboard - {}", tenant_id),
            "tags": ["quadrant-vms", "slo", "tenant", tenant_id],
            "timezone": "browser",
            "schemaVersion": 16,
            "version": 1,
            "refresh": "30s",
            "time": {
                "from": "now-24h",
                "to": "now"
            },
            "templating": {
                "list": [
                    {
                        "name": "service",
                        "type": "query",
                        "datasource": "Prometheus",
                        "query": format!("label_values(slo_service_up{{tenant_id=\"{}\"}}, service)", tenant_id),
                        "refresh": 1,
                        "multi": true,
                        "includeAll": true
                    }
                ]
            },
            "panels": [
                {
                    "title": "Overall SLO Compliance",
                    "type": "stat",
                    "gridPos": {"x": 0, "y": 0, "w": 24, "h": 4},
                    "targets": [{
                        "expr": format!("(1 - (sum(rate(slo_requests_failed_total{{tenant_id=\"{}\"}}[24h])) / sum(rate(slo_requests_total{{tenant_id=\"{}\"}}[24h])))) * 100", tenant_id, tenant_id),
                        "legendFormat": "SLO Compliance %"
                    }],
                    "fieldConfig": {
                        "defaults": {
                            "unit": "percent",
                            "thresholds": {
                                "mode": "absolute",
                                "steps": [
                                    {"value": 0, "color": "red"},
                                    {"value": 99, "color": "yellow"},
                                    {"value": 99.9, "color": "green"}
                                ]
                            }
                        }
                    }
                },
                {
                    "title": "Request Volume by Service",
                    "type": "graph",
                    "gridPos": {"x": 0, "y": 4, "w": 12, "h": 6},
                    "targets": [{
                        "expr": format!("sum(rate(slo_requests_total{{tenant_id=\"{}\"}}[5m])) by (service)", tenant_id),
                        "legendFormat": "{{service}}"
                    }]
                },
                {
                    "title": "Error Budget Remaining",
                    "type": "gauge",
                    "gridPos": {"x": 12, "y": 4, "w": 12, "h": 6},
                    "targets": [{
                        "expr": format!("100 - ((sum(rate(slo_requests_failed_total{{tenant_id=\"{}\"}}[24h])) / (sum(rate(slo_requests_total{{tenant_id=\"{}\"}}[24h])) * 0.01)) * 100)", tenant_id, tenant_id),
                        "legendFormat": "Error Budget %"
                    }],
                    "fieldConfig": {
                        "defaults": {
                            "unit": "percent",
                            "min": 0,
                            "max": 100,
                            "thresholds": {
                                "mode": "absolute",
                                "steps": [
                                    {"value": 0, "color": "red"},
                                    {"value": 25, "color": "yellow"},
                                    {"value": 50, "color": "green"}
                                ]
                            }
                        }
                    }
                }
            ]
        },
        "overwrite": true
    })
}

/// Generate a node-specific SLO dashboard
pub fn generate_node_slo_dashboard(node_id: &str) -> Value {
    json!({
        "dashboard": {
            "title": format!("Node SLO Dashboard - {}", node_id),
            "tags": ["quadrant-vms", "slo", "node", node_id],
            "timezone": "browser",
            "schemaVersion": 16,
            "version": 1,
            "refresh": "30s",
            "time": {
                "from": "now-1h",
                "to": "now"
            },
            "panels": [
                {
                    "title": "Node Health",
                    "type": "stat",
                    "gridPos": {"x": 0, "y": 0, "w": 12, "h": 4},
                    "targets": [{
                        "expr": format!("avg(slo_service_up{{node_id=\"{}\"}}) * 100", node_id),
                        "legendFormat": "Node Health %"
                    }],
                    "fieldConfig": {
                        "defaults": {
                            "unit": "percent",
                            "thresholds": {
                                "mode": "absolute",
                                "steps": [
                                    {"value": 0, "color": "red"},
                                    {"value": 95, "color": "yellow"},
                                    {"value": 99, "color": "green"}
                                ]
                            }
                        }
                    }
                },
                {
                    "title": "Resource Utilization",
                    "type": "graph",
                    "gridPos": {"x": 12, "y": 0, "w": 12, "h": 8},
                    "targets": [
                        {
                            "expr": format!("slo_cpu_utilization_percent{{node_id=\"{}\"}}", node_id),
                            "legendFormat": "CPU %"
                        },
                        {
                            "expr": format!("(slo_memory_usage_bytes{{node_id=\"{}\"}} / (8 * 1024 * 1024 * 1024)) * 100", node_id),
                            "legendFormat": "Memory % (assuming 8GB)"
                        }
                    ]
                },
                {
                    "title": "Workload Distribution",
                    "type": "graph",
                    "gridPos": {"x": 0, "y": 4, "w": 12, "h": 4},
                    "targets": [{
                        "expr": format!("slo_concurrent_operations{{node_id=\"{}\"}}", node_id),
                        "legendFormat": "{{service}} - {{operation_type}}"
                    }]
                }
            ]
        },
        "overwrite": true
    })
}

/// Export all dashboards as JSON files
pub fn export_dashboards_json() -> std::collections::HashMap<String, Value> {
    let mut dashboards = std::collections::HashMap::new();
    dashboards.insert("slo-overview".to_string(), generate_slo_dashboard());
    dashboards
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_slo_dashboard() {
        let dashboard = generate_slo_dashboard();
        assert!(dashboard["dashboard"]["title"].as_str().unwrap().contains("Service Level Objectives"));
        assert_eq!(dashboard["dashboard"]["tags"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_generate_tenant_dashboard() {
        let dashboard = generate_tenant_slo_dashboard("tenant-123");
        assert!(dashboard["dashboard"]["title"].as_str().unwrap().contains("tenant-123"));
    }

    #[test]
    fn test_generate_node_dashboard() {
        let dashboard = generate_node_slo_dashboard("node-1");
        assert!(dashboard["dashboard"]["title"].as_str().unwrap().contains("node-1"));
    }

    #[test]
    fn test_export_dashboards() {
        let dashboards = export_dashboards_json();
        assert!(dashboards.contains_key("slo-overview"));
    }
}
