import React, { useEffect, useState } from 'react';
import { api } from '../services/api';

function Streams() {
  const [streams, setStreams] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  useEffect(() => {
    loadStreams();
  }, []);

  const loadStreams = async () => {
    try {
      const data = await api.getStreams();
      setStreams(data);
      setError(null);
    } catch (err) {
      setError('Failed to load streams');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const handleStopStream = async (streamId) => {
    try {
      await api.stopStream(streamId);
      loadStreams(); // Reload list
    } catch (err) {
      alert('Failed to stop stream: ' + err.message);
    }
  };

  const getStatusBadge = (status) => {
    const statusMap = {
      active: 'success',
      starting: 'warning',
      stopped: 'error',
      error: 'error',
    };
    return <span className={`badge ${statusMap[status] || 'info'}`}>{status}</span>;
  };

  if (loading) {
    return (
      <div>
        <div className="header">
          <h2>Live Streams</h2>
        </div>
        <div className="content">
          <div className="loading">Loading streams...</div>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div className="header">
        <h2>Live Streams</h2>
        <div className="header-actions">
          <button className="btn btn-secondary" onClick={loadStreams}>
            Refresh
          </button>
        </div>
      </div>
      <div className="content">
        {error && <div className="error">{error}</div>}

        <div className="card">
          <div className="card-header">
            <h3 className="card-title">Active Streams ({streams.length})</h3>
          </div>
          <table className="table">
            <thead>
              <tr>
                <th>ID</th>
                <th>Source</th>
                <th>Status</th>
                <th>Format</th>
                <th>Started</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {streams.length === 0 ? (
                <tr>
                  <td colSpan="6" style={{ textAlign: 'center', padding: '20px' }}>
                    No active streams
                  </td>
                </tr>
              ) : (
                streams.map((stream) => (
                  <tr key={stream.id}>
                    <td>{stream.id}</td>
                    <td>{stream.source_uri || '-'}</td>
                    <td>{getStatusBadge(stream.status)}</td>
                    <td>{stream.format || 'HLS'}</td>
                    <td>{stream.started_at ? new Date(stream.started_at).toLocaleString() : '-'}</td>
                    <td>
                      <button
                        className="btn btn-danger"
                        onClick={() => handleStopStream(stream.id)}
                        style={{ padding: '6px 12px', fontSize: '12px' }}
                      >
                        Stop
                      </button>
                    </td>
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

export default Streams;
