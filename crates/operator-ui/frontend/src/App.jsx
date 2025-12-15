import React, { useEffect } from 'react';
import { BrowserRouter as Router, Routes, Route, Link, useLocation } from 'react-router-dom';
import { wsClient } from './services/websocket';
import Dashboard from './pages/Dashboard';
import Devices from './pages/Devices';
import Streams from './pages/Streams';
import Recordings from './pages/Recordings';
import AiTasks from './pages/AiTasks';
import Alerts from './pages/Alerts';
import Incidents from './pages/Incidents';

function Sidebar() {
  const location = useLocation();

  const navItems = [
    { path: '/', label: 'Dashboard', icon: '📊' },
    { path: '/devices', label: 'Devices', icon: '📹' },
    { path: '/streams', label: 'Live Streams', icon: '🎥' },
    { path: '/recordings', label: 'Recordings', icon: '📼' },
    { path: '/ai', label: 'AI Tasks', icon: '🤖' },
    { path: '/alerts', label: 'Alerts', icon: '🔔' },
    { path: '/incidents', label: 'Incidents', icon: '🚨' },
  ];

  return (
    <div className="sidebar">
      <div className="sidebar-header">
        <h1>Quadrant VMS</h1>
      </div>
      <nav className="sidebar-nav">
        <ul>
          {navItems.map((item) => (
            <li key={item.path}>
              <Link
                to={item.path}
                className={location.pathname === item.path ? 'active' : ''}
              >
                <span style={{ marginRight: '10px' }}>{item.icon}</span>
                {item.label}
              </Link>
            </li>
          ))}
        </ul>
      </nav>
    </div>
  );
}

function App() {
  useEffect(() => {
    wsClient.connect();
    return () => wsClient.disconnect();
  }, []);

  return (
    <Router>
      <div className="app">
        <Sidebar />
        <div className="main-content">
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/devices" element={<Devices />} />
            <Route path="/streams" element={<Streams />} />
            <Route path="/recordings" element={<Recordings />} />
            <Route path="/ai" element={<AiTasks />} />
            <Route path="/alerts" element={<Alerts />} />
            <Route path="/incidents" element={<Incidents />} />
          </Routes>
        </div>
      </div>
    </Router>
  );
}

export default App;
