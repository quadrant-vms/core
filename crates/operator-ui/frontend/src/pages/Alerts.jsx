import React, { useEffect, useState } from 'react';
import { api } from '../services/api';

function Alerts() {
  const [alerts, setAlerts] = useState([]);
  const [rules, setRules] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [activeTab, setActiveTab] = useState('alerts');

  useEffect(() => {
    loadData();
  }, [activeTab]);

  const loadData = async () => {
    setLoading(true);
    try {
      if (activeTab === 'alerts') {
        const data = await api.getAlerts({});
        setAlerts(data);
      } else {
        const data = await api.getRules();
        setRules(data);
      }
      setError(null);
    } catch (err) {
      setError(`Failed to load ${activeTab}`);
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const handleToggleRule = async (ruleId, enabled) => {
    try {
      if (enabled) {
        await api.disableRule(ruleId);
      } else {
        await api.enableRule(ruleId);
      }
      loadData();
    } catch (err) {
      alert('Failed to toggle rule: ' + err.message);
    }
  };

  const getSeverityBadge = (severity) => {
    const severityMap = {
      critical: 'error',
      high: 'error',
      medium: 'warning',
      low: 'info',
    };
    return <span className={`badge ${severityMap[severity] || 'info'}`}>{severity}</span>;
  };

  return (
    <div>
      <div className="header">
        <h2>Alerts</h2>
        <div className="header-actions">
          <button className="btn btn-secondary" onClick={loadData}>
            Refresh
          </button>
        </div>
      </div>
      <div className="content">
        {error && <div className="error">{error}</div>}

        <div style={{ marginBottom: '20px', display: 'flex', gap: '10px' }}>
          <button
            className={`btn ${activeTab === 'alerts' ? 'btn-primary' : 'btn-secondary'}`}
            onClick={() => setActiveTab('alerts')}
          >
            Alerts
          </button>
          <button
            className={`btn ${activeTab === 'rules' ? 'btn-primary' : 'btn-secondary'}`}
            onClick={() => setActiveTab('rules')}
          >
            Rules
          </button>
        </div>

        {loading ? (
          <div className="loading">Loading {activeTab}...</div>
        ) : activeTab === 'alerts' ? (
          <div className="card">
            <div className="card-header">
              <h3 className="card-title">Recent Alerts ({alerts.length})</h3>
            </div>
            <table className="table">
              <thead>
                <tr>
                  <th>Timestamp</th>
                  <th>Rule</th>
                  <th>Severity</th>
                  <th>Message</th>
                  <th>Source</th>
                </tr>
              </thead>
              <tbody>
                {alerts.length === 0 ? (
                  <tr>
                    <td colSpan="5" style={{ textAlign: 'center', padding: '20px' }}>
                      No alerts found
                    </td>
                  </tr>
                ) : (
                  alerts.map((alert) => (
                    <tr key={alert.id}>
                      <td>
                        {alert.timestamp ? new Date(alert.timestamp).toLocaleString() : '-'}
                      </td>
                      <td>{alert.rule_name || alert.rule_id}</td>
                      <td>{getSeverityBadge(alert.severity)}</td>
                      <td>{alert.message}</td>
                      <td>{alert.source || '-'}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="card">
            <div className="card-header">
              <h3 className="card-title">Alert Rules ({rules.length})</h3>
            </div>
            <table className="table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Description</th>
                  <th>Severity</th>
                  <th>Channels</th>
                  <th>Status</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {rules.length === 0 ? (
                  <tr>
                    <td colSpan="6" style={{ textAlign: 'center', padding: '20px' }}>
                      No rules found
                    </td>
                  </tr>
                ) : (
                  rules.map((rule) => (
                    <tr key={rule.id}>
                      <td>{rule.name}</td>
                      <td>{rule.description}</td>
                      <td>{getSeverityBadge(rule.severity)}</td>
                      <td>{rule.channels?.join(', ') || '-'}</td>
                      <td>
                        <span className={`badge ${rule.enabled ? 'success' : 'error'}`}>
                          {rule.enabled ? 'Enabled' : 'Disabled'}
                        </span>
                      </td>
                      <td>
                        <button
                          className={`btn ${rule.enabled ? 'btn-danger' : 'btn-primary'}`}
                          onClick={() => handleToggleRule(rule.id, rule.enabled)}
                          style={{ padding: '6px 12px', fontSize: '12px' }}
                        >
                          {rule.enabled ? 'Disable' : 'Enable'}
                        </button>
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}

export default Alerts;
