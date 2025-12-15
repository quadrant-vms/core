import React, { useEffect, useState } from 'react';
import { api } from '../services/api';

function AiTasks() {
  const [tasks, setTasks] = useState([]);
  const [detections, setDetections] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [activeTab, setActiveTab] = useState('tasks');

  useEffect(() => {
    loadData();
  }, [activeTab]);

  const loadData = async () => {
    setLoading(true);
    try {
      if (activeTab === 'tasks') {
        const data = await api.getAiTasks({});
        setTasks(data);
      } else {
        const data = await api.getDetections({});
        setDetections(data);
      }
      setError(null);
    } catch (err) {
      setError(`Failed to load ${activeTab}`);
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const getStatusBadge = (status) => {
    const statusMap = {
      running: 'success',
      completed: 'info',
      failed: 'error',
      pending: 'warning',
    };
    return <span className={`badge ${statusMap[status] || 'info'}`}>{status}</span>;
  };

  return (
    <div>
      <div className="header">
        <h2>AI Tasks</h2>
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
            className={`btn ${activeTab === 'tasks' ? 'btn-primary' : 'btn-secondary'}`}
            onClick={() => setActiveTab('tasks')}
          >
            Tasks
          </button>
          <button
            className={`btn ${activeTab === 'detections' ? 'btn-primary' : 'btn-secondary'}`}
            onClick={() => setActiveTab('detections')}
          >
            Detections
          </button>
        </div>

        {loading ? (
          <div className="loading">Loading {activeTab}...</div>
        ) : activeTab === 'tasks' ? (
          <div className="card">
            <div className="card-header">
              <h3 className="card-title">AI Tasks ({tasks.length})</h3>
            </div>
            <table className="table">
              <thead>
                <tr>
                  <th>ID</th>
                  <th>Type</th>
                  <th>Source</th>
                  <th>Status</th>
                  <th>Created</th>
                  <th>Frames Processed</th>
                </tr>
              </thead>
              <tbody>
                {tasks.length === 0 ? (
                  <tr>
                    <td colSpan="6" style={{ textAlign: 'center', padding: '20px' }}>
                      No AI tasks found
                    </td>
                  </tr>
                ) : (
                  tasks.map((task) => (
                    <tr key={task.id}>
                      <td>{task.id}</td>
                      <td>{task.plugin_name || 'Object Detection'}</td>
                      <td>{task.source || '-'}</td>
                      <td>{getStatusBadge(task.status)}</td>
                      <td>{task.created_at ? new Date(task.created_at).toLocaleString() : '-'}</td>
                      <td>{task.frames_processed || 0}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="card">
            <div className="card-header">
              <h3 className="card-title">Detections ({detections.length})</h3>
            </div>
            <table className="table">
              <thead>
                <tr>
                  <th>Timestamp</th>
                  <th>Task ID</th>
                  <th>Object Class</th>
                  <th>Confidence</th>
                  <th>Location</th>
                </tr>
              </thead>
              <tbody>
                {detections.length === 0 ? (
                  <tr>
                    <td colSpan="5" style={{ textAlign: 'center', padding: '20px' }}>
                      No detections found
                    </td>
                  </tr>
                ) : (
                  detections.map((detection, idx) => (
                    <tr key={idx}>
                      <td>
                        {detection.timestamp ? new Date(detection.timestamp).toLocaleString() : '-'}
                      </td>
                      <td>{detection.task_id}</td>
                      <td>{detection.class_name || '-'}</td>
                      <td>
                        {detection.confidence
                          ? `${(detection.confidence * 100).toFixed(1)}%`
                          : '-'}
                      </td>
                      <td>
                        {detection.bbox
                          ? `x:${detection.bbox.x}, y:${detection.bbox.y}`
                          : '-'}
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

export default AiTasks;
