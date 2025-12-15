const API_BASE = '/api';

export const api = {
  // Dashboard
  getDashboardStats: () => fetch(`${API_BASE}/dashboard/stats`).then(r => r.json()),

  // Devices
  getDevices: () => fetch(`${API_BASE}/devices`).then(r => r.json()),
  getDevice: (id) => fetch(`${API_BASE}/devices/${id}`).then(r => r.json()),
  getDeviceHealth: (id) => fetch(`${API_BASE}/devices/${id}/health`).then(r => r.json()),

  // Streams
  getStreams: () => fetch(`${API_BASE}/streams`).then(r => r.json()),
  getStream: (id) => fetch(`${API_BASE}/streams/${id}`).then(r => r.json()),
  stopStream: (id) => fetch(`${API_BASE}/streams/${id}/stop`, { method: 'POST' }).then(r => r.json()),

  // Recordings
  getRecordings: (params) => {
    const query = new URLSearchParams(params).toString();
    return fetch(`${API_BASE}/recordings${query ? '?' + query : ''}`).then(r => r.json());
  },
  searchRecordings: (searchReq) =>
    fetch(`${API_BASE}/recordings/search`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(searchReq),
    }).then(r => r.json()),
  getRecording: (id) => fetch(`${API_BASE}/recordings/${id}`).then(r => r.json()),
  getThumbnail: (id) => fetch(`${API_BASE}/recordings/${id}/thumbnail`).then(r => r.json()),

  // AI
  getAiTasks: (params) => {
    const query = new URLSearchParams(params).toString();
    return fetch(`${API_BASE}/ai/tasks${query ? '?' + query : ''}`).then(r => r.json());
  },
  getAiTask: (id) => fetch(`${API_BASE}/ai/tasks/${id}`).then(r => r.json()),
  getDetections: (params) => {
    const query = new URLSearchParams(params).toString();
    return fetch(`${API_BASE}/ai/detections${query ? '?' + query : ''}`).then(r => r.json());
  },

  // Alerts
  getAlerts: (params) => {
    const query = new URLSearchParams(params).toString();
    return fetch(`${API_BASE}/alerts${query ? '?' + query : ''}`).then(r => r.json());
  },
  getAlert: (id) => fetch(`${API_BASE}/alerts/${id}`).then(r => r.json()),
  getRules: () => fetch(`${API_BASE}/alerts/rules`).then(r => r.json()),
  getRule: (id) => fetch(`${API_BASE}/alerts/rules/${id}`).then(r => r.json()),
  enableRule: (id) => fetch(`${API_BASE}/alerts/rules/${id}/enable`, { method: 'POST' }).then(r => r.json()),
  disableRule: (id) => fetch(`${API_BASE}/alerts/rules/${id}/disable`, { method: 'POST' }).then(r => r.json()),

  // Incidents
  getIncidents: () => fetch(`${API_BASE}/incidents`).then(r => r.json()),
  createIncident: (incident) =>
    fetch(`${API_BASE}/incidents`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(incident),
    }).then(r => r.json()),
  getIncident: (id) => fetch(`${API_BASE}/incidents/${id}`).then(r => r.json()),
  updateIncident: (id, updates) =>
    fetch(`${API_BASE}/incidents/${id}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(updates),
    }).then(r => r.json()),
  acknowledgeIncident: (id) =>
    fetch(`${API_BASE}/incidents/${id}/acknowledge`, { method: 'POST' }).then(r => r.json()),
  resolveIncident: (id) =>
    fetch(`${API_BASE}/incidents/${id}/resolve`, { method: 'POST' }).then(r => r.json()),
  addIncidentNote: (id, note) =>
    fetch(`${API_BASE}/incidents/${id}/notes`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(note),
    }).then(r => r.json()),
};
