import React, { useEffect, useState } from 'react';
import { api } from '../services/api';

function Recordings() {
  const [recordings, setRecordings] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [searchQuery, setSearchQuery] = useState('');

  useEffect(() => {
    loadRecordings();
  }, []);

  const loadRecordings = async () => {
    try {
      const data = await api.getRecordings({});
      setRecordings(data);
      setError(null);
    } catch (err) {
      setError('Failed to load recordings');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const handleSearch = async (e) => {
    e.preventDefault();
    if (!searchQuery.trim()) {
      loadRecordings();
      return;
    }

    try {
      const data = await api.searchRecordings({
        query: searchQuery,
        filters: {},
      });
      setRecordings(data);
      setError(null);
    } catch (err) {
      setError('Search failed');
      console.error(err);
    }
  };

  const formatFileSize = (bytes) => {
    if (!bytes) return '-';
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / Math.pow(1024, i)).toFixed(2)} ${sizes[i]}`;
  };

  const formatDuration = (seconds) => {
    if (!seconds) return '-';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  };

  if (loading) {
    return (
      <div>
        <div className="header">
          <h2>Recordings</h2>
        </div>
        <div className="content">
          <div className="loading">Loading recordings...</div>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div className="header">
        <h2>Recordings</h2>
        <div className="header-actions">
          <form onSubmit={handleSearch} style={{ display: 'flex', gap: '10px' }}>
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search recordings..."
              style={{
                padding: '10px',
                borderRadius: '6px',
                border: '1px solid #2d3748',
                backgroundColor: '#1a1f2e',
                color: '#e8eaed',
                minWidth: '300px',
              }}
            />
            <button type="submit" className="btn btn-primary">
              Search
            </button>
            <button type="button" className="btn btn-secondary" onClick={loadRecordings}>
              Clear
            </button>
          </form>
        </div>
      </div>
      <div className="content">
        {error && <div className="error">{error}</div>}

        <div className="card">
          <div className="card-header">
            <h3 className="card-title">All Recordings ({recordings.length})</h3>
          </div>
          <table className="table">
            <thead>
              <tr>
                <th>ID</th>
                <th>Device</th>
                <th>Started</th>
                <th>Duration</th>
                <th>Size</th>
                <th>Format</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {recordings.length === 0 ? (
                <tr>
                  <td colSpan="7" style={{ textAlign: 'center', padding: '20px' }}>
                    No recordings found
                  </td>
                </tr>
              ) : (
                recordings.map((recording) => (
                  <tr key={recording.id}>
                    <td>{recording.id}</td>
                    <td>{recording.device_id || '-'}</td>
                    <td>
                      {recording.started_at ? new Date(recording.started_at).toLocaleString() : '-'}
                    </td>
                    <td>{formatDuration(recording.duration_seconds)}</td>
                    <td>{formatFileSize(recording.file_size_bytes)}</td>
                    <td>{recording.format || 'MP4'}</td>
                    <td>
                      <span className={`badge ${recording.status === 'completed' ? 'success' : 'warning'}`}>
                        {recording.status}
                      </span>
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

export default Recordings;
