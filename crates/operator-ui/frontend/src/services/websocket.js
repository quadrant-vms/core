export class WebSocketClient {
  constructor(url = 'ws://localhost:8090/ws') {
    this.url = url;
    this.ws = null;
    this.reconnectInterval = 5000;
    this.listeners = new Map();
    this.reconnectTimeout = null;
  }

  connect() {
    try {
      this.ws = new WebSocket(this.url);

      this.ws.onopen = () => {
        console.log('WebSocket connected');
        this.notifyListeners('connection', { status: 'connected' });
      };

      this.ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);
          this.handleMessage(message);
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error);
        }
      };

      this.ws.onerror = (error) => {
        console.error('WebSocket error:', error);
        this.notifyListeners('error', { error });
      };

      this.ws.onclose = () => {
        console.log('WebSocket disconnected');
        this.notifyListeners('connection', { status: 'disconnected' });
        this.scheduleReconnect();
      };
    } catch (error) {
      console.error('Failed to create WebSocket:', error);
      this.scheduleReconnect();
    }
  }

  scheduleReconnect() {
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
    }

    this.reconnectTimeout = setTimeout(() => {
      console.log('Attempting to reconnect...');
      this.connect();
    }, this.reconnectInterval);
  }

  handleMessage(message) {
    if (message.type === 'update') {
      this.notifyListeners(message.topic, message.data);
    } else if (message.type === 'error') {
      console.error('WebSocket error:', message.message);
    }
  }

  subscribe(topic, callback) {
    if (!this.listeners.has(topic)) {
      this.listeners.set(topic, new Set());
    }
    this.listeners.get(topic).add(callback);

    // Send subscribe message to server
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.send({
        type: 'subscribe',
        topics: [topic],
      });
    }

    return () => this.unsubscribe(topic, callback);
  }

  unsubscribe(topic, callback) {
    const listeners = this.listeners.get(topic);
    if (listeners) {
      listeners.delete(callback);
      if (listeners.size === 0) {
        this.listeners.delete(topic);
      }
    }

    // Send unsubscribe message to server
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.send({
        type: 'unsubscribe',
        topics: [topic],
      });
    }
  }

  notifyListeners(topic, data) {
    const listeners = this.listeners.get(topic);
    if (listeners) {
      listeners.forEach((callback) => callback(data));
    }
  }

  send(message) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    }
  }

  disconnect() {
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }
}

export const wsClient = new WebSocketClient();
