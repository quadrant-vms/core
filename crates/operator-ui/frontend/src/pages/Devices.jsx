import React, { useEffect, useState } from 'react';
import { api } from '../services/api';

function Devices() {
  const [devices, setDevices] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  useEffect(() => {
    loadDevices();
  }, []);

  const loadDevices = async () => {
    try {
      const data = await api.getDevices();
      setDevices(data);
      setError(null);
    } catch (err) {
      setError('Failed to load devices');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const getStatusBadge = (status) => {
    const statusMap = {
      online: 'success',
      offline: 'error',
      degraded: 'warning',
    };
    return <span className={`badge ${statusMap[status] || 'info'}`}>{status}</span>;
  };

  if (loading) {
    return (
      <div>
        <div className="header">
          <h2>Devices</h2>
        </div>
        <div className="content">
          <div className="loading">Loading devices...</div>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div className="header">
        <h2>Devices</h2>
        <div className="header-actions">
          <button className="btn btn-secondary" onClick={loadDevices}>
            Refresh
          </button>
        </div>
      </div>
      <div className="content">
        {error && <div className="error">{error}</div>}

        <div className="card">
          <div className="card-header">
            <h3 className="card-title">All Devices ({devices.length})</h3>
          </div>
          <table className="table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Type</th>
                <th>Location</th>
                <th>Status</th>
                <th>IP Address</th>
                <th>Last Seen</th>
              </tr>
            </thead>
            <tbody>
              {devices.length === 0 ? (
                <tr>
                  <td colSpan="6" style={{ textAlign: 'center', padding: '20px' }}>
                    No devices found
                  </td>
                </tr>
              ) : (
                devices.map((device) => (
                  <tr key={device.id}>
                    <td>{device.name || device.id}</td>
                    <td>{device.type || 'Camera'}</td>
                    <td>{device.location || '-'}</td>
                    <td>{getStatusBadge(device.status)}</td>
                    <td>{device.ip_address || '-'}</td>
                    <td>{device.last_seen ? new Date(device.last_seen).toLocaleString() : '-'}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

export default Devices;
