import React, { useEffect, useState } from 'react';
import { api } from '../services/api';

function Incidents() {
  const [incidents, setIncidents] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [selectedIncident, setSelectedIncident] = useState(null);
  const [showModal, setShowModal] = useState(false);
  const [noteContent, setNoteContent] = useState('');

  useEffect(() => {
    loadIncidents();
  }, []);

  const loadIncidents = async () => {
    try {
      const data = await api.getIncidents();
      setIncidents(data);
      setError(null);
    } catch (err) {
      setError('Failed to load incidents');
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const handleAcknowledge = async (incidentId) => {
    try {
      await api.acknowledgeIncident(incidentId);
      loadIncidents();
    } catch (err) {
      alert('Failed to acknowledge incident: ' + err.message);
    }
  };

  const handleResolve = async (incidentId) => {
    try {
      await api.resolveIncident(incidentId);
      loadIncidents();
      setShowModal(false);
      setSelectedIncident(null);
    } catch (err) {
      alert('Failed to resolve incident: ' + err.message);
    }
  };

  const handleAddNote = async (e) => {
    e.preventDefault();
    if (!noteContent.trim() || !selectedIncident) return;

    try {
      await api.addIncidentNote(selectedIncident.id, {
        author: 'operator',
        content: noteContent,
      });
      setNoteContent('');
      const updated = await api.getIncident(selectedIncident.id);
      setSelectedIncident(updated.incident);
      loadIncidents();
    } catch (err) {
      alert('Failed to add note: ' + err.message);
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

  const getStatusBadge = (status) => {
    const statusMap = {
      open: 'error',
      acknowledged: 'warning',
      investigating: 'warning',
      resolved: 'success',
      closed: 'info',
    };
    return <span className={`badge ${statusMap[status] || 'info'}`}>{status}</span>;
  };

  const openIncidentModal = async (incident) => {
    try {
      const data = await api.getIncident(incident.id);
      setSelectedIncident(data.incident);
      setShowModal(true);
    } catch (err) {
      alert('Failed to load incident details: ' + err.message);
    }
  };

  if (loading) {
    return (
      <div>
        <div className="header">
          <h2>Incidents</h2>
        </div>
        <div className="content">
          <div className="loading">Loading incidents...</div>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div className="header">
        <h2>Incidents</h2>
        <div className="header-actions">
          <button className="btn btn-secondary" onClick={loadIncidents}>
            Refresh
          </button>
        </div>
      </div>
      <div className="content">
        {error && <div className="error">{error}</div>}

        <div className="card">
          <div className="card-header">
            <h3 className="card-title">All Incidents ({incidents.length})</h3>
          </div>
          <table className="table">
            <thead>
              <tr>
                <th>Title</th>
                <th>Severity</th>
                <th>Status</th>
                <th>Source</th>
                <th>Created</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {incidents.length === 0 ? (
                <tr>
                  <td colSpan="6" style={{ textAlign: 'center', padding: '20px' }}>
                    No incidents found
                  </td>
                </tr>
              ) : (
                incidents.map((incident) => (
                  <tr key={incident.id}>
                    <td>
                      <button
                        onClick={() => openIncidentModal(incident)}
                        style={{
                          background: 'none',
                          border: 'none',
                          color: '#4a9eff',
                          cursor: 'pointer',
                          textDecoration: 'underline',
                        }}
                      >
                        {incident.title}
                      </button>
                    </td>
                    <td>{getSeverityBadge(incident.severity)}</td>
                    <td>{getStatusBadge(incident.status)}</td>
                    <td>{incident.source}</td>
                    <td>{new Date(incident.created_at).toLocaleString()}</td>
                    <td>
                      {incident.status === 'open' && (
                        <button
                          className="btn btn-primary"
                          onClick={() => handleAcknowledge(incident.id)}
                          style={{ padding: '6px 12px', fontSize: '12px' }}
                        >
                          Acknowledge
                        </button>
                      )}
                      {incident.status === 'acknowledged' && (
                        <button
                          className="btn btn-primary"
                          onClick={() => handleResolve(incident.id)}
                          style={{ padding: '6px 12px', fontSize: '12px' }}
                        >
                          Resolve
                        </button>
                      )}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {showModal && selectedIncident && (
          <div
            style={{
              position: 'fixed',
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              backgroundColor: 'rgba(0, 0, 0, 0.8)',
              display: 'flex',
              justifyContent: 'center',
              alignItems: 'center',
              zIndex: 1000,
            }}
            onClick={() => setShowModal(false)}
          >
            <div
              className="card"
              style={{ maxWidth: '800px', width: '90%', maxHeight: '90vh', overflow: 'auto' }}
              onClick={(e) => e.stopPropagation()}
            >
              <div className="card-header">
                <h3 className="card-title">{selectedIncident.title}</h3>
                <button
                  onClick={() => setShowModal(false)}
                  style={{
                    background: 'none',
                    border: 'none',
                    color: '#e8eaed',
                    fontSize: '24px',
                    cursor: 'pointer',
                  }}
                >
                  &times;
                </button>
              </div>

              <div style={{ padding: '20px' }}>
                <div style={{ marginBottom: '20px' }}>
                  <strong>Description:</strong>
                  <p>{selectedIncident.description}</p>
                </div>

                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '15px', marginBottom: '20px' }}>
                  <div>
                    <strong>Severity:</strong> {getSeverityBadge(selectedIncident.severity)}
                  </div>
                  <div>
                    <strong>Status:</strong> {getStatusBadge(selectedIncident.status)}
                  </div>
                  <div>
                    <strong>Source:</strong> {selectedIncident.source}
                  </div>
                  <div>
                    <strong>Created:</strong> {new Date(selectedIncident.created_at).toLocaleString()}
                  </div>
                </div>

                <div style={{ marginBottom: '20px' }}>
                  <h4>Notes ({selectedIncident.notes?.length || 0})</h4>
                  {selectedIncident.notes?.length > 0 ? (
                    <div style={{ maxHeight: '200px', overflowY: 'auto' }}>
                      {selectedIncident.notes.map((note) => (
                        <div
                          key={note.id}
                          style={{
                            padding: '10px',
                            marginBottom: '10px',
                            backgroundColor: '#2d3748',
                            borderRadius: '6px',
                          }}
                        >
                          <div style={{ fontSize: '12px', color: '#a0aec0', marginBottom: '5px' }}>
                            {note.author} - {new Date(note.created_at).toLocaleString()}
                          </div>
                          <div>{note.content}</div>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p style={{ color: '#a0aec0' }}>No notes yet</p>
                  )}
                </div>

                <form onSubmit={handleAddNote}>
                  <textarea
                    value={noteContent}
                    onChange={(e) => setNoteContent(e.target.value)}
                    placeholder="Add a note..."
                    style={{
                      width: '100%',
                      padding: '10px',
                      borderRadius: '6px',
                      border: '1px solid #2d3748',
                      backgroundColor: '#1a1f2e',
                      color: '#e8eaed',
                      minHeight: '80px',
                      marginBottom: '10px',
                    }}
                  />
                  <button type="submit" className="btn btn-primary">
                    Add Note
                  </button>
                </form>

                <div style={{ marginTop: '20px', display: 'flex', gap: '10px' }}>
                  {selectedIncident.status === 'open' && (
                    <button
                      className="btn btn-primary"
                      onClick={() => handleAcknowledge(selectedIncident.id)}
                    >
                      Acknowledge
                    </button>
                  )}
                  {(selectedIncident.status === 'acknowledged' || selectedIncident.status === 'investigating') && (
                    <button
                      className="btn btn-primary"
                      onClick={() => handleResolve(selectedIncident.id)}
                    >
                      Resolve
                    </button>
                  )}
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default Incidents;
