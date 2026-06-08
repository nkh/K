// ─── Log Viewer ───
// Log WebSocket connection, HTTP log loading, log line parsing, search.
(function() {
    'use strict';

// ─── Logs ───

// Log WebSocket: connect, disconnect, and indicator helpers

function _updateLogTransportIndicator(mode) {
    const el = document.getElementById('logTransportIndicator');
    if (!el) return;
    el.textContent = mode.toUpperCase();
    el.dataset.mode = mode;
}

function connectLogWs() {
    // Don't connect if already connected or if there's an active search filter
    if (state.logWs && state.logWs.readyState === WebSocket.OPEN) return;

    disconnectLogWs();

    const wsUrl = getBaseUrl().replace(/^http/, 'ws');
    const token = state.authToken || (state.connections[0] || {}).token || '';
    const sep = token ? '?' : '';
    const url = `${wsUrl}/api/ws/logs${sep}${token ? 'token=' + encodeURIComponent(token) : ''}`;

    try {
        const ws = new WebSocket(url);
        state.logWs = ws;

        ws.onopen = () => {
            state._logWsReconnectAttempts = 0;
            _updateLogTransportIndicator('ws');
            // Append a connected indicator line
            const container = document.getElementById('logContent');
            if (container && container.querySelector('.log-line')) {
                const indicator = document.createElement('div');
                indicator.className = 'log-line log-ws-indicator';
                indicator.innerHTML = '<span class="timestamp">[' + new Date().toISOString().replace('T', ' ').replace(/\.\d+Z$/, '') + ']</span> <span class="details" style="color:var(--green);">Connected to log stream</span>';
                container.appendChild(indicator);
                _autoScrollLog(container);
            }
            // Start a ping interval to keep the connection alive
            clearInterval(state._logWsPingTimer);
            state._logWsPingTimer = setInterval(() => {
                if (state.logWs && state.logWs.readyState === WebSocket.OPEN) {
                    state.logWs.send(JSON.stringify({ type: 'ping' }));
                }
            }, 30000);
        };

        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                if (msg.type === 'log_entry' && msg.data) {
                    const container = document.getElementById('logContent');
                    if (!container) return;
                    // Remove the "no entries" placeholder if present
                    const placeholder = container.querySelector('[style*="text-align:center"]');
                    if (placeholder && !placeholder.classList.contains('log-line')) {
                        placeholder.remove();
                    }
                    const parsed = parseLogLine(msg.data);
                    const div = document.createElement('div');
                    div.className = 'log-line';
                    div.innerHTML = formatLogLine(parsed, msg.data);
                    container.appendChild(div);
                    _autoScrollLog(container);
                    // Update line count
                    const countEl = document.getElementById('logCount');
                    if (countEl) {
                        const current = container.querySelectorAll('.log-line').length;
                        countEl.textContent = `${current} lines (streaming)`;
                    }
                } else if (msg.type === 'connected') {
                    // Server confirmed connection — nothing extra to do
                } else if (msg.type === 'pong') {
                    // Heartbeat response — ignore
                }
            } catch (e) {
                console.error('Log WS message parse error:', e);
            }
        };

        ws.onclose = () => {
            _updateLogTransportIndicator('http');
            clearInterval(state._logWsPingTimer);
            state._logWsPingTimer = null;
            if (state.logWs === ws) {
                state.logWs = null;
            }
            _scheduleLogWsReconnect();
        };

        ws.onerror = () => {
            _updateLogTransportIndicator('http');
            clearInterval(state._logWsPingTimer);
            state._logWsPingTimer = null;
            if (state.logWs === ws) {
                state.logWs = null;
            }
            _scheduleLogWsReconnect();
        };
    } catch (e) {
        console.error('Log WebSocket connect failed:', e);
        _updateLogTransportIndicator('http');
        _scheduleLogWsReconnect();
    }
}

function _scheduleLogWsReconnect() {
    if (state.logWsReconnectTimer) return; // already scheduled
    if (state.currentView !== 'log') return; // don't reconnect if not viewing logs

    const delay = Math.min(1000 * Math.pow(2, state._logWsReconnectAttempts), 30000);
    state._logWsReconnectAttempts++;
    state.logWsReconnectTimer = setTimeout(() => {
        state.logWsReconnectTimer = null;
        if (state.currentView === 'log') {
            connectLogWs();
        }
    }, delay);
}

function disconnectLogWs() {
    if (state.logWsReconnectTimer) {
        clearTimeout(state.logWsReconnectTimer);
        state.logWsReconnectTimer = null;
    }
    clearInterval(state._logWsPingTimer);
    state._logWsPingTimer = null;
    if (state.logWs) {
        state.logWs.onclose = null;
        state.logWs.onerror = null;
        state.logWs.close();
        state.logWs = null;
    }
    _updateLogTransportIndicator('http');
}

function _autoScrollLog(container) {
    // Only auto-scroll if user is already near the bottom
    if (container.scrollHeight - container.scrollTop - container.clientHeight < 50) {
        container.scrollTop = container.scrollHeight;
    }
}

async function loadLog() {
    _updateLogTransportIndicator('http');
    try {
        const search = document.getElementById('logSearch').value;
        const params = new URLSearchParams();
        if (search) params.set('search', search);
        params.set('limit', '500');

        const res = await fetch(apiUrl('/api/log?' + params.toString()), { headers: authHeaders() });
        const json = await res.json();

        if (json.status === 'ok' && json.data) {
            const container = document.getElementById('logContent');
            const lines = json.data.lines || [];
            document.getElementById('logCount').textContent = `${json.data.filtered_lines}/${json.data.total_lines} lines`;

            if (lines.length === 0) {
                container.innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;">No log entries found.' + (json.data.message ? ' ' + json.data.message : '') + '</div>';
            } else {
                container.innerHTML = lines.map(line => {
                    const parsed = parseLogLine(line);
                    if (search && line.toLowerCase().includes(search.toLowerCase())) {
                        return `<div class="log-line highlight">${formatLogLine(parsed, line)}</div>`;
                    }
                    return `<div class="log-line">${formatLogLine(parsed, line)}</div>`;
                }).join('');

                // Auto-scroll to bottom
                container.scrollTop = container.scrollHeight;
            }

            // Start WebSocket streaming after HTTP load if no search filter is active
            if (!search) {
                connectLogWs();
            }
        }
    } catch (e) {
        document.getElementById('logContent').innerHTML = `<div style="padding:1rem;color:var(--red);">Failed to load log: ${escHtml(e.message)}</div>`;
    }
}

function parseLogLine(line) {
    // Try to parse [timestamp] command: details
    const match = line.match(/^\[([^\]]+)\]\s+(\w+):\s+(.*)$/);
    if (match) {
        return { timestamp: match[1], command: match[2], details: match[3], raw: line };
    }
    return { timestamp: '', command: '', details: line, raw: line };
}

function formatLogLine(parsed, raw) {
    if (parsed.timestamp) {
        return `<span class="timestamp">[${escHtml(parsed.timestamp)}]</span> <span class="cmd-type">${escHtml(parsed.command)}</span> <span class="details">${escHtml(parsed.details)}</span>`;
    }
    return escHtml(raw);
}

function searchLogs() {
    // Disconnect WS during search — user is filtering, streaming would bypass the filter
    disconnectLogWs();
    state._logWsReconnectAttempts = 0; // reset for after search
    loadLog();
}

function clearLogSearch() {
    document.getElementById('logSearch').value = '';
    loadLog();
    // Reconnect WS after search is cleared (debounced)
    clearTimeout(state._logSearchReconnectTimer);
    state._logSearchReconnectTimer = setTimeout(() => {
        state._logSearchReconnectTimer = null;
        if (state.currentView === 'log') {
            connectLogWs();
        }
    }, 500);
}

    // Expose to global scope
    window.connectLogWs = connectLogWs;
    window.disconnectLogWs = disconnectLogWs;
    window.loadLog = loadLog;
    window.searchLogs = searchLogs;
    window.clearLogSearch = clearLogSearch;
    window._updateLogTransportIndicator = _updateLogTransportIndicator;
    window._scheduleLogWsReconnect = _scheduleLogWsReconnect;
})();
