import React, { useEffect, useState } from 'react';
import { api } from '../services/api';
import { wsClient } from '../services/websocket';

function Dashboard() {
  const [stats, setStats] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  useEffect(() => {
    loadStats();

    // Subscribe to real-time updates
    const unsubscribe = wsClient.subscribe('dashboard', (data) => {
      console.log('Received dashboard update:', data);
      loadStats(); // Reload stats when update received
    });

    return unsubscribe;
  }, []);

  const loadStats = async () => {
    try {
      const data = await api.getDashboardStats();
      setStats(data);
      setError(null);
    } catch (err) {
      setError('Failed to load dashboard stats');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div>
        <div className="header">
          <h2>Dashboard</h2>
        </div>
        <div className="content">
          <div className="loading">Loading dashboard...</div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div>
        <div className="header">
          <h2>Dashboard</h2>
        </div>
        <div className="content">
          <div className="error">{error}</div>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div className="header">
        <h2>Dashboard</h2>
        <div className="header-actions">
          <button className="btn btn-secondary" onClick={loadStats}>
            Refresh
          </button>
        </div>
      </div>
      <div className="content">
        <div className="stats-grid">
          <div className="stat-card">
            <div className="stat-label">Total Devices</div>
            <div className="stat-value">{stats?.devices?.total || 0}</div>
            <div className="stat-change positive">
              {stats?.devices?.online || 0} online
            </div>
          </div>

          <div className="stat-card">
            <div className="stat-label">Active Streams</div>
            <div className="stat-value">{stats?.streams?.active || 0}</div>
            <div className="stat-change">
              {stats?.streams?.total || 0} total
            </div>
          </div>

          <div className="stat-card">
            <div className="stat-label">Recordings</div>
            <div className="stat-value">{stats?.recordings?.total || 0}</div>
            <div className="stat-change">
              {stats?.recordings?.today || 0} today
            </div>
          </div>

          <div className="stat-card">
            <div className="stat-label">AI Tasks</div>
            <div className="stat-value">{stats?.ai_tasks?.active || 0}</div>
            <div className="stat-change">
              {stats?.ai_tasks?.detections_today || 0} detections today
            </div>
          </div>

          <div className="stat-card">
            <div className="stat-label">Active Alerts</div>
            <div className="stat-value">{stats?.alerts?.active_rules || 0}</div>
            <div className="stat-change">
              {stats?.alerts?.alerts_today || 0} triggered today
            </div>
          </div>

          <div className="stat-card">
            <div className="stat-label">Open Incidents</div>
            <div className="stat-value">{stats?.incidents?.open || 0}</div>
            <div className="stat-change">
              {stats?.incidents?.acknowledged || 0} acknowledged
            </div>
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <h3 className="card-title">System Status</h3>
          </div>
          <table className="table">
            <thead>
              <tr>
                <th>Component</th>
                <th>Status</th>
                <th>Details</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>Devices</td>
                <td>
                  <span className="badge success">Healthy</span>
                </td>
                <td>
                  {stats?.devices?.online || 0}/{stats?.devices?.total || 0} online
                  {stats?.devices?.offline > 0 && `, ${stats.devices.offline} offline`}
                  {stats?.devices?.degraded > 0 && `, ${stats.devices.degraded} degraded`}
                </td>
              </tr>
              <tr>
                <td>Streams</td>
                <td>
                  <span className="badge success">Active</span>
                </td>
                <td>{stats?.streams?.active || 0} active streams</td>
              </tr>
              <tr>
                <td>AI Processing</td>
                <td>
                  <span className="badge success">Running</span>
                </td>
                <td>{stats?.ai_tasks?.active || 0} active tasks</td>
              </tr>
              <tr>
                <td>Alerts</td>
                <td>
                  <span className="badge info">Monitoring</span>
                </td>
                <td>{stats?.alerts?.active_rules || 0} active rules</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

export default Dashboard;
