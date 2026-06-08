// ─── Miscellaneous ───
// Logs, Docs, Search, Sound, Onboarding, Shortcuts, Resources,
// Notifications, Global Search, Command Manager, Workspaces, Environments,
// Groups, Templates, Refresh Loop, Keyboard/Mouse handling
(function() {
    'use strict';

// ─── UI Controls ───
// ─── Token Management ───
function saveToken() {
    state.authToken = document.getElementById('authToken').value.trim();
    if (state.authToken) {
        localStorage.setItem('vrw_auth_token', state.authToken);
    } else {
        localStorage.removeItem('vrw_auth_token');
    }
}

// ─── Font Size ───
function changeFontSize(delta) {
    state.fontSize = Math.max(8, Math.min(28, state.fontSize + delta));
    applyFontSize();
}

function applyFontSize() {
    document.documentElement.style.setProperty('--font-size', state.fontSize + 'px');
    const label = document.getElementById('fontSizeLabel');
    if (label) label.textContent = state.fontSize + 'px';
    localStorage.setItem('vrw_font_size', state.fontSize.toString());
}

// Per-panel font size: changes only the specified panel's font size.
function changePanelFontSize(panelId, delta) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    panelObj.fontSize = Math.max(8, Math.min(28, panelObj.fontSize + delta));
    localStorage.setItem('vrw_panel_font_' + panelId, panelObj.fontSize.toString());
    // Apply inline style on the VTTY container
    const vttyEl = document.getElementById('vtty-' + panelId);
    if (vttyEl) vttyEl.style.fontSize = panelObj.fontSize + 'px';
    // Update the shared toolbar font size label
    const stFontSize = document.getElementById('stFontSize');
    if (stFontSize && panelId === getActivePanelId()) {
        stFontSize.textContent = panelObj.fontSize + 'px';
    }
    // Update the label in the panel header (if per-panel label still exists)
    const label = document.querySelector(`#${panelId} .panel-font-size`);
    if (label) label.textContent = panelObj.fontSize + 'px';
}

// ─── Refresh throttle ───
// Controls how often VTTY updates are applied to the DOM.
// 0 = no throttle (updates applied immediately on every server push).
// 100–2000 = throttle interval in milliseconds (updates batched and applied
// at most once per interval).
function changeRefreshMs(delta) {
    state.refreshMs = Math.max(0, Math.min(2000, state.refreshMs + delta));
    // Snap to 100ms steps (0 stays 0)
    if (state.refreshMs > 0 && state.refreshMs % 100 !== 0) {
        state.refreshMs = Math.round(state.refreshMs / 100) * 100;
    }
    localStorage.setItem('vrw_refresh_ms', state.refreshMs.toString());
    // Update all panel widgets
    _syncRefreshMsUI();
}

/// Apply the refresh throttle from the input field (called on change).
function applyRefreshMs() {
    const val = parseInt(document.getElementById('refreshMs').value) || 0;
    state.refreshMs = Math.max(0, Math.min(2000, val));
    // Snap to 100ms steps (0 stays 0)
    if (state.refreshMs > 0 && state.refreshMs % 100 !== 0) {
        state.refreshMs = Math.round(state.refreshMs / 100) * 100;
    }
    localStorage.setItem('vrw_refresh_ms', state.refreshMs.toString());
    document.getElementById('refreshMs').value = state.refreshMs;
    _syncRefreshMsUI();
}

/// Sync all refresh throttle UI elements with state.refreshMs.
function _syncRefreshMsUI() {
    const input = document.getElementById('refreshMs');
    if (input) input.value = state.refreshMs;
    document.querySelectorAll('.refresh-val').forEach(el => {
        el.textContent = state.refreshMs || 'off';
    });
}

/// Throttled wrapper: if a refresh throttle is active, buffer the update and
/// apply it after the throttle window.  Returns true if the update was
/// throttled (caller should not apply it now), false if it should be applied
/// immediately.
function _throttleRefresh() {
    if (state.refreshMs <= 0) return false; // no throttle
    if (state._refreshThrottleTimer) return true; // already pending
    state._refreshThrottleTimer = setTimeout(() => {
        state._refreshThrottleTimer = null;
        _flushThrottledRefresh();
    }, state.refreshMs);
    return true;
}

/// Called when the throttle timer fires: fetch the latest VTTY state.
function _flushThrottledRefresh() {
    if (state.selectedInstUrl && state.selectedCmdId) {
        scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 0);
    }
}

// ─── Selection Mode ───
// When active, mouse events are NOT forwarded to PTY, enabling native text selection.
function toggleSelectionMode(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    panelObj.selectionMode = !panelObj.selectionMode;
    localStorage.setItem('vrw_panel_sel_' + panelId, panelObj.selectionMode.toString());
    const vttyEl = document.getElementById('vtty-' + panelId);
    if (vttyEl) vttyEl.classList.toggle('selection-mode', panelObj.selectionMode);
    const btn = document.getElementById('selectBtn-' + panelId);
    if (btn) {
        btn.classList.toggle('btn-primary', panelObj.selectionMode);
        btn.textContent = panelObj.selectionMode ? '✓ Select' : 'Select';
    }
    // Update shared toolbar button if this is the active panel
    if (panelId === getActivePanelId()) {
        const stBtn = document.getElementById('stSelectBtn');
        if (stBtn) {
            stBtn.classList.toggle('btn-primary', panelObj.selectionMode);
            stBtn.textContent = panelObj.selectionMode ? '✓ Select' : 'Select';
        }
    }
}

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

// ─── Documentation ───
function showDocs() {
    const btn = document.getElementById('docsBtn');
    const vtty = document.getElementById('view-vtty');
    const log = document.getElementById('view-log');
    const docs = document.getElementById('view-docs');
    if (state.currentView === 'docs') {
        // Switch back to terminal
        state.currentView = 'vtty';
        vtty.style.display = 'flex';
        docs.style.display = 'none';
        if (btn) { btn.style.background = ''; btn.style.color = ''; }
    } else {
        // Disconnect log WS if active
        if (state.currentView === 'log') {
            disconnectLogWs();
            if (log) log.style.display = 'none';
        }
        state.currentView = 'docs';
        vtty.style.display = 'none';
        docs.style.display = 'block';
        if (btn) { btn.style.background = 'var(--accent)'; btn.style.color = '#fff'; }
        loadDocs();
    }
}

async function loadDocs() {
    const container = document.getElementById('view-docs');
    container.innerHTML = '<div style="padding:2rem;text-align:center;color:var(--text-muted);">Loading documentation...</div>';

    // Try fetching docs from the server, fall back to embedded docs
    try {
        const res = await fetch('/admin/docs.md', { headers: authHeaders() });
        if (res.ok) {
            const text = await res.text();
            container.innerHTML = renderMarkdown(text);
            return;
        }
    } catch (e) { /* fall through */ }

    // Embedded documentation
    container.innerHTML = renderEmbeddedDocs();
}

function renderMarkdown(md) {
    // Simple markdown to HTML (no external lib)
    let html = md
        .replace(/^### (.+)$/gm, '<h3>$1</h3>')
        .replace(/^## (.+)$/gm, '<h2>$1</h2>')
        .replace(/^# (.+)$/gm, '<h1>$1</h1>')
        .replace(/```(\w*)\n([\s\S]*?)```/g, '<pre><code>$2</code></pre>')
        .replace(/`([^`]+)`/g, '<code>$1</code>')
        .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
        .replace(/\[(.+?)\]\((.+?)\)/g, '<a href="$2" target="_blank" style="color:var(--accent);">$1</a>')
        .replace(/^\- (.+)$/gm, '<li>$1</li>')
        .replace(/^(\d+)\. (.+)$/gm, '<li>$2</li>')
        .replace(/\n\n/g, '</p><p>')
        .replace(/\n/g, '<br>');
    return '<p>' + html + '</p>';
}

function renderEmbeddedDocs() {
    return `
<h1>vrw Administration</h1>

<h2>Overview</h2>
<p>vrw is a virtual terminal runner with a web control plane. It manages terminal applications, exposing their output through a web interface and REST API. This admin panel provides real-time monitoring and control of all running commands.</p>

<h2>Getting Started</h2>
<p>The admin panel connects to one or more vrw instances. Each instance manages its own set of terminal commands. Use the <strong>+ Panel</strong> button in the top bar to add connections to additional vrw instances.</p>

<h3>Connecting to an Instance</h3>
<p>By default, the admin panel connects to the vrw instance serving it. To add more instances:</p>
<ol>
    <li>Click <strong>+ Panel</strong> in the top bar</li>
    <li>Enter the instance URL (e.g., <code>http://localhost:9090</code>)</li>
    <li>Optionally set a label and auth token</li>
    <li>Click <strong>Add Panel</strong></li>
</ol>
<p>You can also use URL arguments: <code>?instance=http://host:8080&label=Prod&instance=http://host:9090&label=Dev</code></p>

<h2>URL Arguments for Multi-Instance</h2>
<p>The admin page accepts query parameters to pre-configure multi-panel views:</p>
<table>
    <tr><th>Parameter</th><th>Description</th><th>Example</th></tr>
    <tr><td><code>instance</code></td><td>vrw instance URL (repeatable)</td><td><code>?instance=http://host:8080</code></td></tr>
    <tr><td><code>label</code></td><td>Panel label (matches instance order)</td><td><code>&label=Production</code></td></tr>
    <tr><td><code>token</code></td><td>Auth token for instance (matches order)</td><td><code>&token=abc123</code></td></tr>
</table>
<p><strong>Full example:</strong> <code>/admin?instance=http://prod:8080&label=Production&instance=http://dev:9090&label=Development</code></p>

<h2>Managing Commands</h2>

<h3>Viewing Terminal Output</h3>
<p>Click on a command in the sidebar to view its real-time ANSI-rendered terminal output. The terminal emulator supports:</p>
<ul>
    <li>Full ANSI color rendering (16, 256, and 24-bit truecolor)</li>
    <li>Cursor position indicator (blue highlight)</li>
    <li>Text attributes: bold, italic, underline, strikethrough</li>
    <li>Scrollback buffer navigation via scrollbar</li>
</ul>

<h3>Spawning Commands</h3>
<p>Switch to the <strong>Spawn</strong> tab in the sidebar to create new commands. Specify the command path, optional arguments, an optional certificate for access control, and the target vrw instance.</p>

<h3>Sending Keystrokes</h3>
<p>Use the key input field in the panel header to send keystrokes to the selected command. Press <strong>Enter</strong> or click <strong>Send</strong> to transmit. Supports special keys using angle bracket notation:</p>
<ul>
    <li><code>&lt;Enter&gt;</code>, <code>&lt;Esc&gt;</code>, <code>&lt;Tab&gt;</code>, <code>&lt;Backspace&gt;</code></li>
    <li><code>&lt;Up&gt;</code>, <code>&lt;Down&gt;</code>, <code>&lt;Left&gt;</code>, <code>&lt;Right&gt;</code></li>
    <li><code>&lt;C-c&gt;</code> (Ctrl+C), <code>&lt;C-d&gt;</code> (Ctrl+D)</li>
    <li><code>&lt;F1&gt;</code> through <code>&lt;F12&gt;</code></li>
</ul>

<h3>Resizing the Terminal</h3>
<p>Use the <strong>R</strong> (rows) and <strong>C</strong> (columns) inputs in the top bar to resize the virtual terminal. Click <strong>Resize</strong> to apply. Valid ranges: rows 1-200, columns 1-500.</p>

<h3>Killing Commands</h3>
<p>Click the <strong>&#x2715;</strong> button next to a command in the sidebar to send SIGINT (Ctrl+C) to the process.</p>

<h2>Certificates</h2>
<p>The <strong>Certs</strong> tab in the sidebar shows all certificates configured in the connected instances' certificate pools. Certificates provide per-command access control — only clients presenting a certificate's derived token can interact with commands bound to that certificate.</p>
<p>When spawning a command, you can select a certificate to bind it. The certificate badge next to each command in the sidebar shows its binding status.</p>

<h2>Log Viewer</h2>
<p>The <strong>Logs</strong> tab provides access to the vrw command log. Use the search bar to filter log entries by content. Each entry shows a timestamp, the command type (spawn, kill, send_keys, etc.), and relevant details.</p>

<h2>Font Size</h2>
<p>Use the <strong>A-</strong> and <strong>A+</strong> buttons in the top bar to adjust the terminal font size (8px-28px). Your preference is saved in localStorage.</p>

<h2>VTTY Update Modes</h2>
<p>The web UI supports two modes for detecting when a terminal buffer has changed. You can switch between them using the <strong>Update</strong> dropdown in the bottom status bar. Your choice is saved in localStorage and will be restored on the next visit.</p>

<h3>Push Mode (default)</h3>
<p>In push mode, the server monitors each command's VTTY buffer at a configurable interval (default 200ms). When changes are detected, the server sends a lightweight <code>vtty_dirty</code> signal over the existing WebSocket connection. The signal contains only the command ID — no cell data, no HTML. The web UI then fetches the full HTML via <code>GET /api/commands/:id/vtty/html</code> at its own pace (debounced at 50ms). This is the most efficient mode because no polling is required; the server only sends when something actually changed.</p>
<p>Push mode is the default and is recommended for most use cases. It provides the lowest latency and lowest bandwidth overhead.</p>

<h3>Poll Mode</h3>
<p>In poll mode, the web client periodically calls <code>GET /api/commands/:id/vtty/changed</code> to ask "has the buffer changed since the last check?". The response is a simple <code>{ "changed": true/false }</code> with no HTML or diff data. If changed, the client then fetches the full HTML via the standard endpoint. The poll interval is configurable via the input next to the mode dropdown (50ms–5000ms, default 500ms).</p>
<p>Poll mode is useful when WebSocket connections are unreliable — for example, when a reverse proxy buffers WebSocket frames, when network conditions cause frequent WS reconnections, or when the client wants full control over refresh timing. The bandwidth overhead is slightly higher than push mode because the changed-check runs continuously even when nothing is changing.</p>

<h3>Server Configuration</h3>
<p>The server-side update settings can be configured in the vrw config file under the <code>web</code> section:</p>
<pre><code>web:
  update_mode: push       # "push" (default) or "poll"
  dirty_check_ms: 200     # server dirty-check interval (push mode)
  default_poll_ms: 500    # suggested client poll interval (poll mode)
</code></pre>
<p>The <code>dirty_check_ms</code> controls how often the server compares the VTTY buffer against the last-known snapshot in push mode. Lower values provide faster updates but increase CPU usage slightly. The <code>default_poll_ms</code> is the suggested interval that the web UI will use when in poll mode, but the user can override it via the UI controls at any time.</p>

<h2>API Reference</h2>
<table>
    <tr><th>Method</th><th>Endpoint</th><th>Description</th></tr>
    <tr><td>GET</td><td><code>/api/commands</code></td><td>List all running commands</td></tr>
    <tr><td>POST</td><td><code>/api/commands</code></td><td>Spawn a new command</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/vtty</code></td><td>Get VTTY output as ANSI text</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/vtty/html</code></td><td>Get VTTY as rendered HTML + cursor</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/vtty/changed</code></td><td>Check if VTTY buffer changed (poll mode)</td></tr>
    <tr><td>POST</td><td><code>/api/commands/:id/keys</code></td><td>Send keystrokes to a command</td></tr>
    <tr><td>POST</td><td><code>/api/commands/:id/kill</code></td><td>Kill a running command</td></tr>
    <tr><td>POST</td><td><code>/api/commands/:id/resize</code></td><td>Resize virtual terminal</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/handles</code></td><td>List handles for a command</td></tr>
    <tr><td>GET</td><td><code>/api/certificates</code></td><td>List certificate pool</td></tr>
    <tr><td>GET</td><td><code>/api/info</code></td><td>Instance info and stats</td></tr>
    <tr><td>GET</td><td><code>/api/log</code></td><td>Command log with search</td></tr>
    <tr><td>POST</td><td><code>/api/shutdown</code></td><td>Graceful shutdown</td></tr>
</table>

<h2>Keyboard Shortcuts</h2>
<table>
    <tr><th>Shortcut</th><th>Action</th></tr>
    <tr><td><code>Enter</code> in key input</td><td>Send keystrokes</td></tr>
</table>
`;
}

// ─── Refresh Loop ───
function startRefresh() {
    // First call uses the snapshot endpoint (1 request = commands + VTTY + resources)
    loadSnapshot();
    if (state.refreshInterval) clearInterval(state.refreshInterval);
    state.refreshInterval = setInterval(() => {
        loadCommands();
        checkForExitedCommands();
    }, 1000);

    // Start resource polling (every 2 seconds) — first poll fires immediately
    if (state._resourceInterval) clearInterval(state._resourceInterval);
    pollResources(); // immediate first poll
    state._resourceInterval = setInterval(pollResources, 2000);
}

// ─── Keyboard handling ───
document.addEventListener('keydown', (e) => {
    // Direct terminal keyboard input: when a panel is focused,
    // capture keystrokes and send them to the PTY directly.
    if (state.currentView === 'vtty') {
        const panel = getSelectedPanel();
        if (panel) {
            const panelObj = state.panels.find(p => p.id === panel.id);
            if (panelObj && panelObj.focused && state.selectedCmdId) {
                // Skip if user is in a search input
                const searchBar = document.getElementById('searchBar-' + panel.id);
                if (searchBar && searchBar.classList.contains('visible') &&
                    document.activeElement && document.activeElement.id === 'searchInput-' + panel.id) {
                    // Let search input handle the key
                } else if (e.key === 'Escape') {
                    // Close Add Panel modal if open
                    const panelModal = document.getElementById('panelModal');
                    if (panelModal && panelModal.style.display !== 'none') {
                        closePanelModal();
                        return;
                    }
                    // Close Command Picker if open
                    const cmdPicker = document.getElementById('cmdPicker');
                    if (cmdPicker) {
                        releaseCurrentFocusTrap();
                        cmdPicker.remove();
                        return;
                    }
                    vttySearchClose(panel.id);
                    closeContextMenu();
                    closeShortcuts();
                    return;
                } else if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
                    e.preventDefault();
                    const sb = document.getElementById('searchBar-' + panel.id);
                    if (sb) {
                        sb.classList.add('visible');
                        // Trap focus inside the search bar
                        const vttyContainer = panel.querySelector('.vtty-container');
                        if (vttyContainer) trapFocus(vttyContainer);
                        const si = document.getElementById('searchInput-' + panel.id);
                        if (si) { si.focus(); si.select(); }
                    }
                    return;
                } else {
                    e.preventDefault();
                    sendDirectKey(e, panelObj);
                    return;
                }
            }
        }
    }

    // Focus key input when not in an input field and a command is selected
    if (state.currentView === 'vtty' && state.selectedCmdId &&
        !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        const panel = getSelectedPanel();
        if (panel) {
            const input = document.getElementById('keyInput-' + panel.id);
            if (input) input.focus();
        }
    }
    // Ctrl+F — open terminal search bar
    if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        const vttyContainer = e.target.closest && e.target.closest('.vtty-container');
        if (vttyContainer || state.currentView === 'vtty') {
            e.preventDefault();
            const panel = getSelectedPanel();
            if (panel) {
                const searchBar = document.getElementById('searchBar-' + panel.id);
                if (searchBar) {
                    searchBar.classList.add('visible');
                    // Trap focus inside the search bar area
                    const vtty = panel.querySelector('.vtty-container');
                    if (vtty) trapFocus(vtty);
                    const searchInput = document.getElementById('searchInput-' + panel.id);
                    if (searchInput) { searchInput.focus(); searchInput.select(); }
                }
            }
        }
    }
    // Shift+F10 or ContextMenu key — open context menu on focused cmd-item or panel-header
    if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
        e.preventDefault();
        const target = document.activeElement;
        if (!target) return;
        // Panel header context menu
        if (target.classList.contains('panel-header') && target.dataset.panelId) {
            const rect = target.getBoundingClientRect();
            showPanelContextMenu({ preventDefault: () => {}, clientX: rect.left + rect.width / 2, clientY: rect.bottom }, target.dataset.panelId);
        }
        // Command item context menu
        if (target.classList.contains('cmd-item') && target.dataset.instUrl) {
            const rect = target.getBoundingClientRect();
            showCmdContextMenu({ preventDefault: () => {}, clientX: rect.left + rect.width / 2, clientY: rect.bottom }, target.dataset.instUrl, target.dataset.cmdId, target.dataset.cmdName, target.dataset.cmdAlive === 'true');
        }
    }
    // Escape — close terminal search bar, panel modal, command picker, shortcuts
    if (e.key === 'Escape') {
        // Close Add Panel modal if open
        const panelModal = document.getElementById('panelModal');
        if (panelModal && panelModal.style.display !== 'none') {
            closePanelModal();
            return;
        }
        // Close Command Picker if open
        const cmdPicker = document.getElementById('cmdPicker');
        if (cmdPicker) {
            releaseCurrentFocusTrap();
            cmdPicker.remove();
            return;
        }
        const panel = getSelectedPanel();
        if (panel) {
            vttySearchClose(panel.id);
        }
        closeContextMenu();
        closeShortcuts();
    }
    // Ctrl+Shift+C — copy terminal selection
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'C' || e.key === 'c')) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            copyTerminalSelection(panel.id);
            return;
        }
    }
    // Ctrl+Shift+S — toggle selection mode
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'S' || e.key === 's')) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            toggleSelectionMode(panel.id);
            return;
        }
    }
    // Alt+S — toggle selection mode (alternative shortcut)
    if (e.altKey && (e.key === 's' || e.key === 'S') && !e.ctrlKey && !e.metaKey) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            toggleSelectionMode(panel.id);
            return;
        }
    }
    // ? — show keyboard shortcuts
    if (e.key === '?' && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        showShortcuts();
    }
    // Ctrl+Shift+E — export terminal as text
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'E' || e.key === 'e')) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            exportTerminal(panel.id);
            return;
        }
    }
    // Ctrl+Shift+R — restart command (only when not in input)
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'R' || e.key === 'r') && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            restartCommand(panel.id);
            return;
        }
    }
    // Alt+T — toggle panel theme (only when not in input)
    if (e.altKey && (e.key === 't' || e.key === 'T') && !e.ctrlKey && !e.metaKey && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        const panelId = getActivePanelId();
        if (panelId) {
            e.preventDefault();
            togglePanelTheme(panelId);
            return;
        }
    }
    // Alt+N — add new panel (only when not in input)
    if (e.altKey && (e.key === 'n' || e.key === 'N') && !e.ctrlKey && !e.metaKey && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        e.preventDefault();
        addPanel();
        return;
    }
    // Alt+Left / Alt+Right — navigate prev/next command (only when not focused on terminal)
    if (e.altKey && !e.ctrlKey && !e.metaKey && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        const panel = getSelectedPanel();
        const panelObj = panel && state.panels.find(p => p.id === panel.id);
        if (e.key === 'ArrowLeft' && !(panelObj && panelObj.focused)) {
            e.preventDefault();
            navigatePrevCommand();
            return;
        }
        if (e.key === 'ArrowRight' && !(panelObj && panelObj.focused)) {
            e.preventDefault();
            navigateNextCommand();
            return;
        }
    }
});


// ─── Direct key sending (when terminal is focused) ───
// Encodes a KeyboardEvent into escape sequences and sends to the PTY.
async function sendDirectKey(e, panelObj) {
    if (!state.selectedCmdId || !panelObj.selectedInstUrl) return;

    // Map common special keys to escape sequences
    const keyMap = {
        'Enter': '\r',
        'Backspace': '\x7f',
        'Tab': '\t',
        'Escape': '\x1b',
        'Home': '\x1b[H',
        'End': '\x1b[F',
        'Delete': '\x1b[3~',
        'ArrowUp': '\x1b[A',
        'ArrowDown': '\x1b[B',
        'ArrowRight': '\x1b[C',
        'ArrowLeft': '\x1b[D',
        'PageUp': '\x1b[5~',
        'PageDown': '\x1b[6~',
        'Insert': '\x1b[2~',
        'F1': '\x1bOP',
        'F2': '\x1bOQ',
        'F3': '\x1bOR',
        'F4': '\x1bOS',
        'F5': '\x1b[15~',
        'F6': '\x1b[17~',
        'F7': '\x1b[18~',
        'F8': '\x1b[19~',
        'F9': '\x1b[20~',
        'F10': '\x1b[21~',
        'F11': '\x1b[23~',
        'F12': '\x1b[24~',
    };

    let seq = '';
    if (e.ctrlKey && !e.altKey && !e.metaKey) {
        // Ctrl+letter
        if (e.key.length === 1 && e.key >= 'a' && e.key <= 'z') {
            seq = String.fromCharCode(e.key.charCodeAt(0) - 96);
        } else if (e.key === '[') seq = '\x1b'; // Ctrl+[ = ESC
        else if (e.key === '\\') seq = '\x1c';
        else if (e.key === ']') seq = '\x1d';
        else if (e.key === '^') seq = '\x1e';
        else if (e.key === '_') seq = '\x1f';
    } else if (e.altKey && !e.ctrlKey && !e.metaKey) {
        // Alt+letter = ESC + letter
        if (e.key.length === 1) seq = '\x1b' + e.key;
    } else if (keyMap[e.key]) {
        seq = keyMap[e.key];
    } else if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
        // Regular printable character
        seq = e.key;
    }

    if (!seq) return;

    try {
        const res = await fetch(apiUrl(`/api/commands/${state.selectedCmdId}/keys`, { url: panelObj.selectedInstUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: panelObj.selectedInstUrl }),
            body: JSON.stringify({ keys: seq }),
        });
        const json = await res.json();
        if (json.status === 'ok') {
            // Trigger a refresh
            scheduleVttyHttp(panelObj.selectedInstUrl, state.selectedCmdId, 50);
        }
    } catch (err) {
        console.error('Direct key send error:', err);
    }
}

// ─── Click-to-focus terminal ───
// Clicking on the VTTY container focuses the terminal for direct keyboard input.
// A second click on an already-focused terminal blurs it.
document.addEventListener('click', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (vttyContainer && state.currentView === 'vtty') {
        const panelEl = vttyContainer.closest('.panel');
        if (panelEl) {
            const panelObj = state.panels.find(p => p.id === panelEl.id);
            if (panelObj) {
                // Check if click is on a button inside the vtty container (search bar, scroll btn)
                if (e.target.closest('button') || e.target.closest('input')) return;

                if (panelObj.focused) {
                    // Already focused — blur
                    panelObj.focused = false;
                    vttyContainer.style.outline = '';
                } else {
                    // Focus this panel's terminal
                    state.panels.forEach(p => p.focused = false);
                    document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
                    panelObj.focused = true;
                    vttyContainer.style.outline = '2px solid var(--accent)';
                    vttyContainer.setAttribute('tabindex', '0');
                    vttyContainer.focus();
                }
            }
        }
    } else if (!vttyContainer) {
        // Click outside any terminal — blur all
        state.panels.forEach(p => p.focused = false);
        document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
    }
});

// ─── Mouse wheel handling on terminal ───
// Level 1 optimization: Don't block native scroll when viewing the live buffer.
// Only intercept wheel events at the top edge (scroll into scrollback history)
// or when mouse tracking is enabled (forward to PTY).
//
// When in scrollback view (scrollbackOffset > 0), scroll wheel navigates
// scrollback history via server-side offset (debounced with rAF).
//
// Native scroll provides smooth inertia and momentum — the browser handles
// repaint timing, which is far more efficient than per-tick HTTP round-trips.
let _wheelScrollRafId = null;
let _wheelScrollPanel = null;   // panel object for the pending rAF callback
let _wheelScrollAccum = 0;      // accumulated signed vertical delta

document.addEventListener('wheel', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') return;

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    // If selection mode is active, let browser handle wheel natively (no scrollback, no PTY)
    if (panelObj.selectionMode) return;

    // If the child has mouse tracking enabled, forward wheel events to the PTY
    if (panelObj.mouseTracking) {
        e.preventDefault();
        const wheelEvent = e.deltaY < 0 ? 'wheel_up' : 'wheel_down';
        sendMouseEvent(panelObj, wheelEvent, 0, e);
        return;
    }

    // ── Live buffer view (scrollbackOffset === 0) ──
    // Allow native scroll. Only intercept when user scrolls up past the top
    // edge, which means they want to enter scrollback history.
    if (panelObj.scrollbackOffset === 0) {
        const atTop = vttyContainer.scrollTop <= 0;
        if (e.deltaY < 0 && atTop) {
            // User scrolled up at the top edge — enter scrollback history
            e.preventDefault();
            panelObj.scrollbackOffset += 3;
            sessionStorage.setItem('vrw_scrollback_' + state.selectedCmdId, panelObj.scrollbackOffset.toString());
            loadVttyHttp(panelObj.selectedInstUrl, state.selectedCmdId);
            // Show scrollback indicator
            const sbIndicator = document.getElementById('scrollbackIndicator');
            if (sbIndicator) { sbIndicator.style.display = ''; sbIndicator.textContent = 'SCROLLBACK -' + panelObj.scrollbackOffset + ' rows'; }
            const btn = panelEl.querySelector('.scroll-bottom-btn');
            if (btn) btn.classList.add('visible');
        }
        // else: let browser handle native scroll (no preventDefault)
        return;
    }

    // ── Scrollback history view (scrollbackOffset > 0) ──
    e.preventDefault();

    // Accumulate scroll delta — will be processed in the next animation frame.
    // This coalesces rapid wheel ticks into a single HTTP round-trip.
    _wheelScrollPanel = panelObj;
    _wheelScrollAccum += e.deltaY;

    if (_wheelScrollRafId) cancelAnimationFrame(_wheelScrollRafId);
    _wheelScrollRafId = requestAnimationFrame(() => {
        _wheelScrollRafId = null;
        const p = _wheelScrollPanel;
        if (!p) return;

        // Snapshot and reset the accumulator before processing.
        const accum = _wheelScrollAccum;
        _wheelScrollAccum = 0;

        // Convert accumulated pixel delta to scrollback lines.
        // ~100px of scroll ≈ 3 lines (same ratio as the previous per-tick behavior).
        const lines = Math.max(1, Math.round(Math.abs(accum) / 100) * 3);

        if (accum > 0) {
            // Wheel down: decrease scrollback offset (move toward live view)
            const newOffset = Math.max(0, p.scrollbackOffset - lines);
            if (newOffset === 0) {
                // Reached the live buffer — restore native scroll
                p.scrollbackOffset = 0;
                sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);
                loadVttyHttpForPanel(panel.id, p.selectedInstUrl, p.selectedCmdId);
                // Scroll to bottom after returning to live view
                const vtty = panelEl.querySelector('.vtty-container');
                if (vtty) vtty.scrollTop = vtty.scrollHeight;
            } else {
                p.scrollbackOffset = newOffset;
                sessionStorage.setItem('vrw_scrollback_' + state.selectedCmdId, p.scrollbackOffset.toString());
                loadVttyHttpForPanel(panel.id, p.selectedInstUrl, p.selectedCmdId);
            }
        } else {
            // Wheel up: increase scrollback offset (move into history)
            p.scrollbackOffset += lines;
            sessionStorage.setItem('vrw_scrollback_' + state.selectedCmdId, p.scrollbackOffset.toString());
            loadVttyHttpForPanel(panel.id, p.selectedInstUrl, p.selectedCmdId);
        }

        // Update scroll-to-bottom button visibility and scrollback indicator
        const btn = panelEl.querySelector('.scroll-bottom-btn');
        if (btn) btn.classList.toggle('visible', p.scrollbackOffset > 0);
        const sbIndicator = document.getElementById('scrollbackIndicator');
        if (sbIndicator) {
            sbIndicator.style.display = p.scrollbackOffset > 0 ? '' : 'none';
            if (p.scrollbackOffset > 0) sbIndicator.textContent = 'SCROLLBACK -' + p.scrollbackOffset + ' rows';
        }
    });
}, { passive: false });

// ─── Mouse event forwarding to PTY ───
// Forwards mousedown, mouseup, mousemove events to the PTY when the child
// has enabled mouse tracking mode. Events are sent as escape sequences via
// POST /api/commands/:id/mouse.

let _mouseDownButton = null; // Track which button is pressed

document.addEventListener('mousedown', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') {
        _mouseDownButton = null;
        return;
    }

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    // Skip if clicking on buttons/inputs inside vtty container
    if (e.target.closest('button') || e.target.closest('input')) return;

    // If selection mode is active, skip PTY forwarding — let browser handle selection
    if (panelObj.selectionMode) return;

    // If mouse tracking is enabled, forward the event to PTY
    if (panelObj.mouseTracking) {
        e.preventDefault();
        _mouseDownButton = e.button; // 0=left, 1=middle, 2=right
        sendMouseEvent(panelObj, 'down', e.button, e);
    }
});

document.addEventListener('mouseup', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') {
        _mouseDownButton = null;
        return;
    }

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    // If selection mode is active, skip PTY forwarding — auto-copy on select
    if (panelObj.selectionMode) {
        _mouseDownButton = null;
        // Copy-on-select: if user just selected text, copy it automatically
        setTimeout(() => {
            const sel = window.getSelection();
            const text = sel ? sel.toString().trim() : '';
            if (text) copyTerminalSelection(panelEl.id);
        }, 0);
        return;
    }

    if (panelObj.mouseTracking && _mouseDownButton !== null) {
        e.preventDefault();
        sendMouseEvent(panelObj, 'up', _mouseDownButton, e);
        _mouseDownButton = null;
    }
});

document.addEventListener('mousemove', (e) => {
    if (_mouseDownButton === null) return; // Only track during drag

    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') return;

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId || !panelObj.mouseTracking) return;

    // If selection mode is active, skip PTY forwarding
    if (panelObj.selectionMode) return;

    // Throttle mouse move events to avoid flooding
    if (!panelObj._lastMoveTime || Date.now() - panelObj._lastMoveTime > 16) {
        panelObj._lastMoveTime = Date.now();
        sendMouseEvent(panelObj, 'move', _mouseDownButton, e);
    }
});

// Send a mouse event to the PTY via the API
async function sendMouseEvent(panelObj, eventType, button, e) {
    if (!state.selectedCmdId || !panelObj.selectedInstUrl) return;

    // Calculate terminal cell coordinates from pixel position
    const vttyEl = document.getElementById(panelObj.id)?.querySelector('.vtty-container');
    if (!vttyEl) return;

    const rect = vttyEl.getBoundingClientRect();
    const charW = state.fontSize * 0.6;
    const charH = state.fontSize * 1.2;

    const x = Math.max(1, Math.floor((e.clientX - rect.left) / charW) + 1);
    const y = Math.max(1, Math.floor((e.clientY - rect.top) / charH) + 1);

    try {
        await fetch(apiUrl(`/api/commands/${state.selectedCmdId}/mouse`, { url: panelObj.selectedInstUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: panelObj.selectedInstUrl }),
            body: JSON.stringify({
                event: eventType,
                button: button,
                x: x,
                y: y,
            }),
        });
        // Refresh display after mouse events (the child may have reacted)
        scheduleVttyHttp(panelObj.selectedInstUrl, state.selectedCmdId, 30);
    } catch (err) {
        // Silently ignore — mouse events are best-effort
    }
}


// ─── Terminal Search ───
const vttySearchState = { matchIndex: 0, matches: [], panelId: null };

function vttySearch(panelId) {
    const input = document.getElementById('searchInput-' + panelId);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (!input || !countEl) return;
    const query = input.value;
    vttySearchState.panelId = panelId;
    vttySearchState.matchIndex = 0;

    // Get the text content of the terminal
    const panel = document.getElementById(panelId);
    const pre = panel ? panel.querySelector('pre') : null;
    if (!pre) { countEl.textContent = '0/0'; return; }

    // Remove previous highlights
    vttyRemoveHighlights(pre);

    if (!query) {
        vttySearchState.matches = [];
        countEl.textContent = '';
        return;
    }

    // Find all text nodes and mark matches
    const text = pre.textContent || '';
    const lowerText = text.toLowerCase();
    const lowerQuery = query.toLowerCase();
    vttySearchState.matches = [];

    let pos = 0;
    while ((pos = lowerText.indexOf(lowerQuery, pos)) !== -1) {
        vttySearchState.matches.push(pos);
        pos += lowerQuery.length;
    }

    if (vttySearchState.matches.length > 0) {
        vttyApplyHighlights(pre, text, query);
        vttyScrollToMatch(panelId, 0);
        countEl.textContent = '1/' + vttySearchState.matches.length;
        _updateSearchProgress(panelId, 0, vttySearchState.matches.length);
    } else {
        countEl.textContent = '0/0';
        _updateSearchProgress(panelId, 0, 0);
    }
}

function vttyApplyHighlights(pre, text, query) {
    // Walk through text and highlight matches
    const lowerText = text.toLowerCase();
    const lowerQuery = query.toLowerCase();
    const fragment = document.createDocumentFragment();
    let lastIdx = 0;
    let matchIdx = 0;
    let pos = 0;

    // We need to rebuild using the pre's innerHTML which has spans for ANSI
    // Instead, work at the text level using a tree walker on text nodes
    const walker = document.createTreeWalker(pre, NodeFilter.SHOW_TEXT, null);
    const textNodes = [];
    while (walker.nextNode()) textNodes.push(walker.currentNode);

    if (textNodes.length === 0) return;

    // Simple approach: highlight by rebuilding innerHTML with mark spans
    // Get the full innerHTML and do string replacement on text portions
    let html = pre.innerHTML;
    const escaped = escHtml(query);
    // Use a regex that matches the query text (case insensitive) but only
    // within text content, not inside HTML tags
    const regex = new RegExp('(?![^<]*>)(' + escaped.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + ')', 'gi');
    const results = [];
    let match;
    while ((match = regex.exec(html)) !== null) {
        results.push(match.index);
    }

    // Apply highlights in reverse order to preserve indices
    for (let i = results.length - 1; i >= 0; i--) {
        const idx = results[i];
        const originalLen = query.length;
        // Find the end of the match in the HTML
        const endIdx = html.indexOf(match[1], idx) + match[1].length;
        if (endIdx <= idx) continue;
        const cls = i === 0 ? 'vtty-search-highlight current' : 'vtty-search-highlight';
        html = html.substring(0, idx) + '<mark class="' + cls + '" data-match-idx="' + i + '">' + html.substring(idx, endIdx) + '</mark>' + html.substring(endIdx);
    }

    pre.innerHTML = html;
}

function vttyRemoveHighlights(pre) {
    const marks = pre.querySelectorAll('mark.vtty-search-highlight');
    marks.forEach(mark => {
        const parent = mark.parentNode;
        parent.replaceChild(document.createTextNode(mark.textContent), mark);
        parent.normalize();
    });
}

function vttyScrollToMatch(panelId, idx) {
    const panel = document.getElementById(panelId);
    if (!panel) return;
    const mark = panel.querySelector('mark.vtty-search-highlight.current');
    if (mark) mark.classList.remove('current');
    const marks = panel.querySelectorAll('mark.vtty-search-highlight');
    if (marks[idx]) {
        marks[idx].classList.add('current');
        marks[idx].scrollIntoView({ block: 'center', behavior: 'smooth' });
    }
}

function vttySearchNext(panelId) {
    if (vttySearchState.matches.length === 0) return;
    vttySearchState.matchIndex = (vttySearchState.matchIndex + 1) % vttySearchState.matches.length;
    vttyScrollToMatch(panelId, vttySearchState.matchIndex);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = (vttySearchState.matchIndex + 1) + '/' + vttySearchState.matches.length;
    _updateSearchProgress(panelId, vttySearchState.matchIndex, vttySearchState.matches.length);
}

function vttySearchPrev(panelId) {
    if (vttySearchState.matches.length === 0) return;
    vttySearchState.matchIndex = (vttySearchState.matchIndex - 1 + vttySearchState.matches.length) % vttySearchState.matches.length;
    vttyScrollToMatch(panelId, vttySearchState.matchIndex);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = (vttySearchState.matchIndex + 1) + '/' + vttySearchState.matches.length;
    _updateSearchProgress(panelId, vttySearchState.matchIndex, vttySearchState.matches.length);
}

function _updateSearchProgress(panelId, currentIdx, totalMatches) {
    const bar = document.getElementById('searchProgress-' + panelId);
    if (!bar) return;
    if (totalMatches <= 1) {
        bar.style.display = 'none';
        return;
    }
    bar.style.display = '';
    const pct = ((currentIdx + 1) / totalMatches) * 100;
    bar.style.background = `linear-gradient(to right, var(--accent) ${pct}%, var(--border) ${pct}%)`;
}

function vttySearchClose(panelId) {
    releaseCurrentFocusTrap();
    const searchBar = document.getElementById('searchBar-' + panelId);
    if (searchBar) searchBar.classList.remove('visible');
    const panel = document.getElementById(panelId);
    const pre = panel ? panel.querySelector('pre') : null;
    if (pre) vttyRemoveHighlights(pre);
    vttySearchState.matches = [];
    vttySearchState.matchIndex = 0;
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = '';
    // Return focus to the VTTY container
    if (panel) {
        const vtty = panel.querySelector('.vtty-container');
        if (vtty) vtty.focus();
    }
}


// ─── Scroll to Bottom ───
function scrollTerminalBottom(panelId) {
    // Check if this is a secondary pane of a split panel
    const isSecondary = panelId.endsWith('-secondary');
    if (isSecondary) {
        const primaryPanelId = panelId.slice(0, -'-secondary'.length);
        const vtty = document.getElementById('vtty-' + panelId);
        if (vtty) {
            vtty.scrollTop = vtty.scrollHeight;
        }
        const panelObj = state.panels.find(p => p.id === primaryPanelId);
        if (panelObj && panelObj.split && panelObj.split.secondaryScrollbackOffset > 0) {
            panelObj.split.secondaryScrollbackOffset = 0;
            if (panelObj.split.secondaryCmdId) {
                _loadSecondaryVttyHttp(panelObj);
            }
        }
        return;
    }

    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;
    const vtty = panelEl.querySelector('.vtty-container');
    if (vtty) {
        vtty.scrollTop = vtty.scrollHeight;
    }
    // Reset scrollback offset and re-fetch
    const panelObj = state.panels.find(p => p.id === panelId);
    if (panelObj && panelObj.scrollbackOffset > 0) {
        panelObj.scrollbackOffset = 0;
        // Clear stored scrollback since we reset
        if (state.selectedCmdId) {
            sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);
        }
        const sbIndicator = document.getElementById('scrollbackIndicator');
        if (sbIndicator) sbIndicator.style.display = 'none';
        if (state.selectedCmdId && panelObj.selectedInstUrl) {
            loadVttyHttp(panelObj.selectedInstUrl, state.selectedCmdId);
        }
    }
}

// ─── Browser Notification on Command Exit ───
const _notifiedExits = new Set();

function notifyCommandEnded(cmdId) {
    if (!cmdId || _notifiedExits.has(cmdId)) return;
    _notifiedExits.add(cmdId);

    // Find command name and exit code
    let cmdName = cmdId;
    let exitCode = null;
    for (const inst of state.connections) {
        if (inst._commands) {
            const cmd = inst._commands.find(c => c.id === cmdId);
            if (cmd) { cmdName = cmd.name || cmdId; exitCode = cmd.exit_code; break; }
        }
    }

    // Play sound notification
    if (state.soundEnabled) {
        playExitSound(exitCode === 0);
    }

    if ('Notification' in window) {
        if (Notification.permission === 'granted') {
            new Notification('vrw: Command exited', { body: cmdName, icon: '/favicon.ico' });
        } else if (Notification.permission !== 'denied') {
            Notification.requestPermission().then(perm => {
                if (perm === 'granted') {
                    new Notification('vrw: Command exited', { body: cmdName, icon: '/favicon.ico' });
                }
            });
        }
    }
}

// Also detect command exits via polling — notify when a previously-alive command exits
// Auto-restart pinned commands on exit (with debounce to avoid restart loops)
const _autoRestartDebounce = new Map(); // cmdName → timeout ID

function checkForExitedCommands() {
    const pinnedNames = getPinnedNames();
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false && !_notifiedExits.has(cmd.id)) {
                notifyCommandEnded(cmd.id);
                // Auto-restart pinned commands
                const cmdName = cmd.name || cmd.id;
                if (pinnedNames.includes(cmdName)) {
                    _autoRestartCommand(inst.url, cmd, cmdName);
                }
            }
        }
    }
}

function _autoRestartCommand(instUrl, cmd, cmdName) {
    // Debounce: don't restart the same command name more than once every 10s
    // to avoid rapid restart loops on commands that exit immediately
    if (_autoRestartDebounce.has(cmdName)) return;
    _autoRestartDebounce.set(cmdName, setTimeout(() => {
        _autoRestartDebounce.delete(cmdName);
    }, 10000));

    restartCommandById(instUrl, cmd.id).then(() => {
        // Show a brief indicator that auto-restart happened
        const indicator = document.getElementById('autoRestartIndicator');
        if (indicator) {
            indicator.textContent = 'Auto-restarted: ' + cmdName;
            indicator.style.display = 'flex';
            setTimeout(() => { indicator.style.display = 'none'; }, 3000);
        }
    }).catch(() => {
        // Restart failed — remove debounce lock so it can retry
        const t = _autoRestartDebounce.get(cmdName);
        if (t) { clearTimeout(t); _autoRestartDebounce.delete(cmdName); }
    });
}


// ─── Onboarding Tutorial ───
const ONBOARDING_KEY = 'vrw_onboarding_done';
const ONBOARDING_STEPS = [
    { target: '#sidebar', title: 'Sidebar', body: 'Browse servers, running commands, and spawn new commands from here. Drag commands by their grip handle to reorder them. Pin important commands with the ◉ button.' },
    { target: '#tab-servers', title: 'Servers Tab', body: 'View all connected vrw instances. Click the connection indicator to add new servers. Resource polling shows live CPU and memory usage.' },
    { target: '#tab-spawn', title: 'Spawn Tab', body: 'Launch commands on any connected server. Set environment variables, working directory, and terminal size. Press Tab for path completion.' },
    { target: '#sharedToolbar', title: 'Shared Toolbar', body: 'Controls for the focused panel: restart, resize font, toggle selection mode, copy, export, screenshot, and layout presets.' },
    { target: '#view-vtty', title: 'Terminal Panels', body: 'Each panel shows a terminal. Click the panel header to focus it. Double-click the command name to rename the panel. Use the ☰ Commands button for a unified command manager.' },
    { target: '#bottomBar', title: 'Status Bar', body: 'Shows the active command name, arguments, PID, runtime, cursor position, terminal dimensions, and scrollback indicator. Toggle it with the Status button.' },
    { target: null, title: 'Keyboard Shortcuts', body: 'Ctrl+F — search terminal\nCtrl+Shift+C — copy selection\nCtrl+Shift+R — restart command\nAlt+S — toggle selection mode\nAlt+N — new panel\nAlt+T — toggle theme\nPress ? (on focus) to see all shortcuts' },
    { target: null, title: 'You\'re all set!', body: 'Right-click panels and commands for more options. Drag commands from the sidebar onto panels. Pin commands to auto-restart them on exit. Check the ☰ Commands button to manage all commands at once.' },
];

let _onboardingStep = 0;

function checkOnboarding() {
    if (localStorage.getItem(ONBOARDING_KEY)) return;
    // Only show after a short delay to let the UI settle
    setTimeout(() => {
        const sidebar = document.getElementById('sidebar');
        const viewVtty = document.getElementById('view-vtty');
        if (sidebar && viewVtty) openOnboarding();
    }, 1500);
}

function openOnboarding() {
    _onboardingStep = 0;
    document.getElementById('onboardingOverlay').style.display = '';
    document.getElementById('onboardingDontShow').checked = false;
    renderOnboardingStep();
}

function closeOnboarding() {
    document.getElementById('onboardingOverlay').style.display = 'none';
    if (document.getElementById('onboardingDontShow').checked) {
        localStorage.setItem(ONBOARDING_KEY, '1');
    }
}

function nextOnboardingStep() {
    _onboardingStep++;
    if (_onboardingStep >= ONBOARDING_STEPS.length) {
        closeOnboarding();
        return;
    }
    renderOnboardingStep();
}

function renderOnboardingStep() {
    const step = ONBOARDING_STEPS[_onboardingStep];
    const total = ONBOARDING_STEPS.length;
    document.getElementById('onboardingStep').textContent = (_onboardingStep + 1) + '/' + total;
    document.getElementById('onboardingTitle').textContent = step.title;
    // Support newlines in body text
    document.getElementById('onboardingBody').innerHTML = escHtml(step.body).replace(/\n/g, '<br>');
    const nextBtn = document.getElementById('onboardingNextBtn');
    nextBtn.textContent = _onboardingStep === total - 1 ? 'Done' : 'Next';

    // Position spotlight on target element
    const spotlight = document.getElementById('onboardingSpotlight');
    const tooltip = document.getElementById('onboardingTooltip');
    if (step.target) {
        const el = document.querySelector(step.target);
        if (el) {
            const rect = el.getBoundingClientRect();
            spotlight.style.display = 'block';
            spotlight.style.top = (rect.top - 4) + 'px';
            spotlight.style.left = (rect.left - 4) + 'px';
            spotlight.style.width = (rect.width + 8) + 'px';
            spotlight.style.height = (rect.height + 8) + 'px';

            // Position tooltip below or beside the spotlight
            const tooltipMaxWidth = Math.min(350, window.innerWidth - 40);
            tooltip.style.maxWidth = tooltipMaxWidth + 'px';
            if (rect.bottom + 16 + 200 < window.innerHeight) {
                tooltip.style.top = (rect.bottom + 12) + 'px';
                tooltip.style.left = Math.max(12, Math.min(rect.left, window.innerWidth - tooltipMaxWidth - 12)) + 'px';
            } else {
                tooltip.style.top = Math.max(12, rect.top - 200) + 'px';
                tooltip.style.left = Math.max(12, Math.min(rect.left, window.innerWidth - tooltipMaxWidth - 12)) + 'px';
            }
            return;
        }
    }
    // No target — center the tooltip
    spotlight.style.display = 'none';
    tooltip.style.top = '50%';
    tooltip.style.left = '50%';
    tooltip.style.transform = 'translate(-50%, -50%)';
    setTimeout(() => { tooltip.style.transform = ''; }, 0);
}

// ─── Keyboard Shortcuts Help ───
function showShortcuts() {
    closeShortcuts();
    const overlay = document.createElement('div');
    overlay.className = 'shortcuts-overlay';
    overlay.id = 'shortcutsOverlay';
    overlay.onclick = (e) => { if (e.target === overlay) closeShortcuts(); };
    overlay.innerHTML = `<div class="shortcuts-panel">
        <h2>Keyboard Shortcuts</h2>
        <table>
            <tr><td>?</td><td>Show this help</td></tr>
            <tr><td>Ctrl+F</td><td>Search in terminal</td></tr>
            <tr><td>Ctrl+Shift+C</td><td>Copy terminal selection</td></tr>
            <tr><td>Ctrl+Shift+S / Alt+S</td><td>Toggle selection mode</td></tr>
            <tr><td>Ctrl+Shift+E</td><td>Export terminal as text</td></tr>
            <tr><td>Ctrl+Shift+R</td><td>Restart command</td></tr>
            <tr><td>Escape</td><td>Close search / menu</td></tr>
            <tr><td>Alt+Left / Alt+Right</td><td>Navigate prev/next command</td></tr>
            <tr><td>Alt+T</td><td>Toggle panel theme</td></tr>
            <tr><td>Alt+N</td><td>Add new panel</td></tr>
            <tr><td>Any key</td><td>Focus key input (when not in a field)</td></tr>
            <tr><td>Enter</td><td>Send keystrokes to terminal</td></tr>
        </table>
        <p style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.5rem;">Click on the terminal to focus the key input field.</p>
        <div style="text-align:right;margin-top:0.75rem;">
            <button class="btn" onclick="closeShortcuts()">Close</button>
        </div>
    </div>`;
    document.body.appendChild(overlay);
    // Trap focus inside the shortcuts panel and focus the close button
    const shortcutsPanel = overlay.querySelector('.shortcuts-panel');
    if (shortcutsPanel) trapFocus(shortcutsPanel);
    const closeBtn = overlay.querySelector('button');
    if (closeBtn) closeBtn.focus();
}

function closeShortcuts() {
    releaseCurrentFocusTrap();
    const el = document.getElementById('shortcutsOverlay');
    if (el) el.remove();
}


// ─── Resource Polling ───
async function pollResources() {
    // Fetch all alive commands' resources in PARALLEL (not serial).
    const promises = [];
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false) continue;
            promises.push((async () => {
                try {
                    const res = await fetch(apiUrl(`/api/commands/${cmd.id}/resources`, { url: inst.url }), {
                        headers: authHeadersForInstance(inst),
                    });
                    const json = await res.json();
                    if (json.status === 'ok' && json.data) {
                        state._resourceCache[cmd.id] = json.data;
                    }
                } catch (e) {
                    // Silently ignore — resources are optional
                }
            })());
        }
    }
    await Promise.all(promises);
    // Update sidebar resource text without full DOM rebuild
    updateSidebarResourceText();
}

/// Update the .cmd-detail-inline text in sidebar command items to reflect
/// the latest resource data from state._resourceCache. This avoids a full
/// DOM rebuild (which the fingerprint optimization would skip anyway).
function updateSidebarResourceText() {
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false) continue;
            const res = state._resourceCache[cmd.id];
            const item = document.querySelector(`.cmd-item[data-cmd-id="${cmd.id}"]`);
            if (!item) continue;
            const isFrozen = cmd.frozen === true;
            const runtimeStr = cmd.runtime_secs > 0
                ? (cmd.runtime_secs < 60 ? Math.floor(cmd.runtime_secs) + 's'
                   : cmd.runtime_secs < 3600 ? Math.floor(cmd.runtime_secs / 60) + 'm ' + Math.floor(cmd.runtime_secs % 60) + 's'
                   : Math.floor(cmd.runtime_secs / 3600) + 'h ' + Math.floor((cmd.runtime_secs % 3600) / 60) + 'm')
                : '';
            const frozenBadge = isFrozen ? 'PAUSED' : '';
            // Compact: runtime · cpu% · memM · pid  (numeric only, no labels — must match
            // the format used in renderCmdList to avoid visual flipping)
            const detailParts = [];
            if (runtimeStr) detailParts.push(runtimeStr);
            if (frozenBadge) detailParts.push(frozenBadge);
            if (res && res.cpu_percent != null) detailParts.push(res.cpu_percent.toFixed(1) + '%');
            if (res && res.memory_mb != null) {
                const mb = res.memory_mb;
                detailParts.push(mb >= 1024 ? (mb / 1024).toFixed(1) + 'G' : mb.toFixed(1) + 'M');
            }
            if (cmd.pid) detailParts.push(String(cmd.pid));

            // Find or create the detail row
            let detailRow = item.querySelector('.cmd-detail-row');
            if (detailParts.length === 0) {
                if (detailRow) detailRow.remove();
            } else {
                if (!detailRow) {
                    detailRow = document.createElement('div');
                    detailRow.className = 'cmd-detail-row';
                    item.appendChild(detailRow);
                }
                detailRow.innerHTML = detailParts.join(' · ');
            }
        }
    }
}


// ─── Command Templates ───
// Server-side templates are loaded from the vrw config file ([[templates]]).
// User templates are stored in localStorage and are editable in the web UI.
let _serverTemplates = []; // cached from /api/templates

function getServerTemplates() {
    return _serverTemplates;
}

async function fetchServerTemplates() {
    try {
        const res = await fetch(apiUrl('/api/templates'), { headers: authHeaders() });
        const json = await res.json();
        if (json.status === 'ok') {
            _serverTemplates = json.data || [];
        }
    } catch { /* ignore — use cached */ }
}

function getUserTemplates() {
    try {
        return JSON.parse(localStorage.getItem('vrw_templates') || '[]');
    } catch { return []; }
}

function saveUserTemplates(templates) {
    localStorage.setItem('vrw_templates', JSON.stringify(templates));
}

function renderTemplates() {
    const container = document.getElementById('templateList');
    if (!container) return;

    const server = getServerTemplates();
    const user = getUserTemplates();
    const hasAny = server.length > 0 || user.length > 0;

    if (!hasAny) {
        container.innerHTML = '<div style="padding:0.5rem;color:var(--text-muted);font-size:0.7rem;text-align:center;">No templates configured. Add templates in your config file under [[templates]].</div>';
        return;
    }

    let html = '';

    // Server templates section
    if (server.length > 0) {
        html += '<div style="font-size:0.6rem;color:var(--text-muted);padding:0.2rem 0.3rem;text-transform:uppercase;letter-spacing:0.05em;">From config</div>';
        html += server.map((t, i) => {
            const detail = [t.cmd, t.args].filter(Boolean).join(' ');
            const extras = [];
            if (t.workdir) extras.push('dir: ' + t.workdir);
            if (t.certificate) extras.push('cert: ' + t.certificate);
            if (t.rows || t.cols) extras.push((t.rows || '?') + 'x' + (t.cols || '?'));
            const extraStr = extras.length > 0 ? extras.join(' | ') : '';
            return `<div class="template-card" onclick="spawnServerTemplate(${i})" title="Click to spawn this command">
                <div style="display:flex;align-items:center;gap:0.3rem;">
                    <div class="template-name">${escHtml(t.name)}</div>
                    <span style="font-size:0.5rem;background:var(--accent);color:#fff;padding:0 0.25rem;border-radius:2px;">config</span>
                </div>
                <div class="template-cmd">${escHtml(detail)}</div>
                ${extraStr ? `<div style="font-size:0.6rem;color:var(--text-muted);padding-left:0.2rem;">${escHtml(extraStr)}</div>` : ''}
            </div>`;
        }).join('');
    }

    // User templates section
    if (user.length > 0) {
        html += '<div style="font-size:0.6rem;color:var(--text-muted);padding:0.3rem 0.3rem 0.1rem;text-transform:uppercase;letter-spacing:0.05em;">Custom</div>';
        html += user.map((t, i) => `
            <div class="template-card" onclick="spawnUserTemplate(${i})" title="Click to spawn this command">
                <div class="template-name">${escHtml(t.name)}</div>
                <div class="template-cmd">${escHtml(t.cmd)}${t.args ? ' ' + escHtml(t.args) : ''}</div>
                <div class="template-actions">
                    <button class="btn btn-xs btn-danger" onclick="event.stopPropagation();deleteUserTemplate(${i})" title="Delete">&#x2715;</button>
                </div>
            </div>
        `).join('');
    }

    container.innerHTML = html;
}

function spawnServerTemplate(index) {
    const t = getServerTemplates()[index];
    if (!t) return;
    const instSelect = document.getElementById('spawnInstance');
    const instUrl = instSelect ? instSelect.value : getBaseUrl();
    const args = t.args ? t.args.split(/\s+/) : [];
    const body = { cmd: t.cmd, args };
    // Convert env from ["KEY=VALUE", ...] to { "KEY": "VALUE", ... }
    if (t.env && t.env.length > 0) {
        const envObj = {};
        for (const entry of t.env) {
            const eqIdx = entry.indexOf('=');
            if (eqIdx > 0) envObj[entry.substring(0, eqIdx)] = entry.substring(eqIdx + 1);
        }
        body.env = envObj;
    }
    if (t.workdir) body.workdir = t.workdir;
    if (t.certificate) body.certificate = t.certificate;
    if (t.rows) body.rows = t.rows;
    if (t.cols) body.cols = t.cols;
    fetch(apiUrl('/api/commands', { url: instUrl }), {
        method: 'POST',
        headers: authHeadersForInstance({ url: instUrl }),
        body: JSON.stringify(body),
    }).then(res => res.json()).then(json => {
        if (json.status === 'ok') {
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                _cacheTerminalForSwitch();
                state._pendingSelectId = newId;
            }
            loadCommands();
            const cmdTab = document.querySelector('.sidebar-tab');
            if (cmdTab) switchSidebarTab('commands', cmdTab);
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    }).catch(e => alert('Spawn failed: ' + e.message));
}

function spawnUserTemplate(index) {
    const user = getUserTemplates();
    const t = user[index];
    if (!t) return;
    const instSelect = document.getElementById('spawnInstance');
    const instUrl = instSelect ? instSelect.value : getBaseUrl();
    const args = t.args ? t.args.split(/\s+/) : [];
    const body = { cmd: t.cmd, args };
    fetch(apiUrl('/api/commands', { url: instUrl }), {
        method: 'POST',
        headers: authHeadersForInstance({ url: instUrl }),
        body: JSON.stringify(body),
    }).then(res => res.json()).then(json => {
        if (json.status === 'ok') {
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                _cacheTerminalForSwitch();
                state._pendingSelectId = newId;
            }
            loadCommands();
            const cmdTab = document.querySelector('.sidebar-tab');
            if (cmdTab) switchSidebarTab('commands', cmdTab);
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    }).catch(e => alert('Spawn failed: ' + e.message));
}

function deleteUserTemplate(index) {
    const templates = getUserTemplates();
    templates.splice(index, 1);
    saveUserTemplates(templates);
    renderTemplates();
}

function showAddTemplateForm() {
    const form = document.getElementById('templateAddForm');
    if (form) form.style.display = '';
}

function hideAddTemplateForm() {
    const form = document.getElementById('templateAddForm');
    if (form) form.style.display = 'none';
    document.getElementById('templateName').value = '';
    document.getElementById('templateCmd').value = '';
    document.getElementById('templateArgs').value = '';
}

function saveTemplate() {
    const name = document.getElementById('templateName').value.trim();
    const cmd = document.getElementById('templateCmd').value.trim();
    const args = document.getElementById('templateArgs').value.trim();
    if (!name || !cmd) { alert('Name and command are required'); return; }
    const templates = getUserTemplates();
    templates.push({ name, cmd, args });
    saveUserTemplates(templates);
    hideAddTemplateForm();
    renderTemplates();
}


// ─── Drag-and-Drop: Sidebar Commands to Panels ───
let _draggedCmd = null; // { instUrl, cmdId, cmdName }

function onCmdDragStart(e, instUrl, cmdId, cmdName) {
    _draggedCmd = { instUrl, cmdId, cmdName };
    e.dataTransfer.effectAllowed = 'copy';
    e.dataTransfer.setData('text/plain', cmdId);
    e.dataTransfer.setData('application/x-cmd', JSON.stringify({ instUrl, cmdId, cmdName }));
    e.target.style.opacity = '0.5';
    setTimeout(() => { if (e.target) e.target.style.opacity = ''; }, 0);
}

// Make panels accept command drops from sidebar
function initPanelDropTargets() {
    document.querySelectorAll('.panel').forEach(panelEl => {
        panelEl.addEventListener('dragover', (e) => {
            e.preventDefault();
            e.dataTransfer.dropEffect = 'copy';
            panelEl.classList.add('drag-over-left');
        });
        panelEl.addEventListener('dragleave', (e) => {
            panelEl.classList.remove('drag-over-left');
        });
        panelEl.addEventListener('drop', (e) => {
            e.preventDefault();
            panelEl.classList.remove('drag-over-left');
            try {
                const data = JSON.parse(e.dataTransfer.getData('application/x-cmd'));
                if (data && data.cmdId) {
                    // Assign command to this specific panel
                    const panelObj = state.panels.find(p => p.id === panelEl.id);
                    if (panelObj) {
                        _cacheTerminalForSwitch();
                        panelObj.selectedInstUrl = data.instUrl;
                        panelObj.selectedCmdId = data.cmdId;
                        focusPanel(panelObj.id);
                        state.selectedInstUrl = data.instUrl;
                        state.selectedCmdId = data.cmdId;
                        state._pendingVttyData = null;
                        state._pendingVttyDirty = false;
                        state.bufferView = 'current';
                        _restoreCachedDom(data.cmdId);
                        updatePanelCommandInfo();
                        updateTerminalDisconnectedOverlay();
                        updateSidebarSelection();
                        loadVttyHttpForPanel(panelObj.id, data.instUrl, data.cmdId);
                        startPanelUpdateMode(panelObj.id);
                    }
                }
            } catch (err) { /* ignore invalid drops */ }
            _draggedCmd = null;
        });
    });
}


// ─── Drag-and-Drop: Sidebar Command Reorder (mousedown-based) ───
// Commands can be reordered within the sidebar by dragging the grab handle.
// Uses mousedown/mousemove/mouseup instead of nested HTML5 DnD because nested
// draggable elements (cmd-item draggable for panel-drop + grab-handle draggable
// for reorder) is a well-known anti-pattern that fails silently in most browsers.
// The custom order is persisted in localStorage as 'vrw_cmd_order'.
// { instUrl: [cmdId1, cmdId2, ...] }
function getCmdOrder() {
    try { return JSON.parse(localStorage.getItem('vrw_cmd_order') || '{}'); } catch { return {}; }
}
function setCmdOrder(order) {
    localStorage.setItem('vrw_cmd_order', JSON.stringify(order));
}
function getOrderedCmds(instUrl, items) {
    const order = getCmdOrder();
    const instOrder = order[instUrl];
    if (!instOrder) return items;
    // items are { inst, cmd, cmdName } objects; order by cmd.id
    const ordered = [];
    const remaining = [];
    for (const item of items) {
        const idx = instOrder.indexOf(item.cmd.id);
        if (idx >= 0) {
            ordered.push({ item, idx });
        } else {
            remaining.push(item);
        }
    }
    ordered.sort((a, b) => a.idx - b.idx);
    return [...ordered.map(x => x.item), ...remaining];
}

// mousedown-based reorder state
let _reorderState = null; // { instUrl, cmdId, cmdName, srcEl, startY, startRect, placeholder, offsetY, overPane }

function _cmdReorderMouseDown(e, instUrl, cmdId, cmdName) {
    // Only left-click
    if (e.button !== 0) return;
    e.preventDefault(); // prevent text selection
    e.stopPropagation(); // don't trigger cmd-item onclick

    const srcEl = e.target.closest('.cmd-item');
    if (!srcEl) return;

    const rect = srcEl.getBoundingClientRect();
    _reorderState = {
        instUrl,
        cmdId,
        cmdName: cmdName || cmdId,
        srcEl,
        startY: e.clientY,
        startRect: rect,
        placeholder: null,
        offsetY: e.clientY - rect.top,
        overPane: false,
    };

    document.addEventListener('mousemove', _cmdReorderMouseMove);
    document.addEventListener('mouseup', _cmdReorderMouseUp);
}

function _cmdReorderMouseMove(e) {
    if (!_reorderState) return;

    const dy = e.clientY - _reorderState.startY;
    // Minimum 4px before starting visual drag
    if (Math.abs(dy) < 4 && !_reorderState.placeholder) return;

    const container = document.getElementById('commandList');
    if (!container) return;

    // First move: create placeholder and make source float
    if (!_reorderState.placeholder) {
        const srcEl = _reorderState.srcEl;
        _reorderState.placeholder = document.createElement('div');
        _reorderState.placeholder.style.cssText = 'border-top:2px solid var(--accent);margin:0;pointer-events:none;';
        _reorderState.placeholder.className = 'cmd-reorder-placeholder';
        srcEl.parentNode.insertBefore(_reorderState.placeholder, srcEl);
        srcEl.style.position = 'fixed';
        srcEl.style.left = _reorderState.startRect.left + 'px';
        srcEl.style.top = (e.clientY - _reorderState.offsetY) + 'px';
        srcEl.style.width = _reorderState.startRect.width + 'px';
        srcEl.style.zIndex = '1000';
        srcEl.style.opacity = '0.85';
        srcEl.style.pointerEvents = 'none';
        srcEl.classList.add('cmd-dragging');
    }

    // Move the floating element
    _reorderState.srcEl.style.top = (e.clientY - _reorderState.offsetY) + 'px';

    // Find the element we're hovering over (use elementFromPoint to see what's
    // under the floating ghost).
    _reorderState.srcEl.style.display = 'none';
    const underEl = document.elementFromPoint(e.clientX, e.clientY);
    _reorderState.srcEl.style.display = '';

    // Check if hovering over the pane area (for drop-to-open feature)
    const overPanel = underEl ? underEl.closest('.panel') : null;
    const wasOverPane = _reorderState.overPane;
    _reorderState.overPane = !!overPanel;

    // Toggle pane drop indicator
    if (_reorderState.overPane && !wasOverPane) {
        // Entered pane area — show drop indicator
        document.querySelectorAll('.panel').forEach(p => p.classList.add('drag-over-left'));
        // Clear sidebar indicators
        container.querySelectorAll('.cmd-item').forEach(el => {
            el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom');
        });
    } else if (!_reorderState.overPane && wasOverPane) {
        // Left pane area — remove drop indicator
        document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left'));
    }

    // If over a panel, don't try to reorder in sidebar
    if (_reorderState.overPane) return;

    // Clear old sidebar indicators
    container.querySelectorAll('.cmd-item').forEach(el => {
        el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom');
    });

    const target = underEl ? underEl.closest('.cmd-item') : null;
    if (!target || target === _reorderState.srcEl) return;

    // Move placeholder to indicate drop position
    const rect = target.getBoundingClientRect();
    const midY = rect.top + rect.height / 2;
    if (e.clientY < midY) {
        target.classList.add('cmd-drag-over-top');
        target.parentNode.insertBefore(_reorderState.placeholder, target);
    } else {
        target.classList.add('cmd-drag-over-bottom');
        const next = target.nextElementSibling;
        target.parentNode.insertBefore(_reorderState.placeholder, next);
    }
}

function _cmdReorderMouseUp(e) {
    document.removeEventListener('mousemove', _cmdReorderMouseMove);
    document.removeEventListener('mouseup', _cmdReorderMouseUp);

    if (!_reorderState) return;

    const container = document.getElementById('commandList');
    const placeholder = _reorderState.placeholder;
    const srcEl = _reorderState.srcEl;
    const droppedOnPane = _reorderState.overPane;

    // Clean up visual state on the source element
    if (srcEl) {
        srcEl.style.position = '';
        srcEl.style.left = '';
        srcEl.style.top = '';
        srcEl.style.width = '';
        srcEl.style.zIndex = '';
        srcEl.style.opacity = '';
        srcEl.style.pointerEvents = '';
        srcEl.classList.remove('cmd-dragging');
    }
    // Clean up sidebar indicators
    if (container) {
        container.querySelectorAll('.cmd-item').forEach(el => {
            el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom');
        });
    }
    // Clean up pane drop indicators
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left'));

    // ── Drop on pane area: create new panel with this command ──
    if (droppedOnPane && placeholder) {
        placeholder.remove();
        _openCommandInNewPane(_reorderState.instUrl, _reorderState.cmdId, _reorderState.cmdName);
        _reorderState = null;
        return;
    }

    // ── Drop on sidebar: perform reorder ──
    if (placeholder && container) {
        const targetItem = placeholder.nextElementSibling;
        const targetCmdId = targetItem && targetItem.classList.contains('cmd-item')
            ? targetItem.dataset.cmdId
            : null;

        // Remove placeholder before doing DOM operations
        placeholder.remove();

        // Only reorder if we moved to a different position
        if (targetCmdId && targetCmdId !== _reorderState.cmdId) {
            const order = getCmdOrder();
            let instOrder = order[_reorderState.instUrl] || [];
            // Remove source from current position
            instOrder = instOrder.filter(id => id !== _reorderState.cmdId);
            // Find target position
            const targetIdx = instOrder.indexOf(targetCmdId);
            instOrder.splice(targetIdx >= 0 ? targetIdx : instOrder.length, 0, _reorderState.cmdId);
            order[_reorderState.instUrl] = instOrder;
            setCmdOrder(order);
            _lastCommandState = ''; // force sidebar rebuild with new order
            loadCommands();
        } else if (placeholder.parentNode) {
            // Moved but dropped back to same spot — just remove placeholder
            placeholder.remove();
        }
    }

    _reorderState = null;
}

// ─── Open command in a new pane (used by grab-handle drop-to-pane) ───
function _openCommandInNewPane(instUrl, cmdId, cmdName) {
    // Create a new empty panel
    const newPanel = addPanelDirect();
    if (!newPanel) return;
    // Focus it and assign the command
    focusPanel(newPanel.id);
    newPanel.selectedInstUrl = instUrl;
    newPanel.selectedCmdId = cmdId;
    // Sync global state
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    state._pendingVttyData = null;
    state._pendingVttyDirty = false;
    state.bufferView = 'current';
    _restoreCachedDom(cmdId);
    updatePanelCommandInfo();
    updateTerminalDisconnectedOverlay();
    updateSidebarSelection();
    // Fetch VTTY content and start push/poll
    loadVttyHttpForPanel(newPanel.id, instUrl, cmdId);
    startPanelUpdateMode(newPanel.id);
}


// ─── Global Search ───
// When the search overlay opens, all panel VTTY updates are paused so text
// doesn't shift under the user's eyes. Optionally, the commands themselves
// can be frozen (SIGSTOP). On cancel, everything resumes. On result click,
// the selected panel stays frozen so the matched text remains stable.

function _freezeAllPanelsForSearch() {
    _searchFrozenPanelIds.clear();
    _searchFrozenCmdIds = [];
    for (const panel of state.panels) {
        if (panel.selectedInstUrl && panel.selectedCmdId) {
            stopPanelUpdateMode(panel.id);
            _searchFrozenPanelIds.add(panel.id);
        }
    }
}

async function _thawAllPanelsFromSearch() {
    for (const panelId of _searchFrozenPanelIds) {
        const panelObj = state.panels.find(p => p.id === panelId);
        if (panelObj && panelObj.selectedInstUrl && panelObj.selectedCmdId) {
            startPanelUpdateMode(panelId);
        }
    }
    _searchFrozenPanelIds.clear();
    // Thaw any commands that were frozen during search
    for (const entry of _searchFrozenCmdIds) {
        try {
            await fetch(apiUrl(`/api/commands/${entry.cmdId}/thaw`, { url: entry.instUrl }), {
                method: 'POST',
                headers: authHeadersForInstance({ url: entry.instUrl }),
                body: JSON.stringify({}),
            });
        } catch (e) { /* ignore */ }
    }
    _searchFrozenCmdIds = [];
}

// ─── Command Manager Dialog ───
function openCmdManager() {
    document.getElementById('cmdManagerModal').style.display = '';
    document.getElementById('cmdManagerFilter').value = '';
    renderCmdManagerList();
}

function closeCmdManager() {
    document.getElementById('cmdManagerModal').style.display = 'none';
}

function renderCmdManagerList() {
    const container = document.getElementById('cmdManagerList');
    const filter = (document.getElementById('cmdManagerFilter').value || '').toLowerCase();
    const sortBy = document.getElementById('cmdManagerSort').value;
    const footer = document.getElementById('cmdManagerFooter');

    // Collect all commands across all instances
    let cmds = [];
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            const res = state._resourceCache[cmd.id] || {};
            cmds.push({ ...cmd, instUrl: inst.url, cpu: res.cpu_percent || 0, mem: res.memory_mb || 0 });
        }
    }

    // Filter
    if (filter) {
        cmds = cmds.filter(c => {
            const name = (c.name || c.id).toLowerCase();
            const args = (c.args || []).join(' ').toLowerCase();
            return name.includes(filter) || args.includes(filter);
        });
    }

    // Sort
    if (sortBy === 'name') cmds.sort((a, b) => (a.name || a.id).localeCompare(b.name || b.id));
    else if (sortBy === 'runtime') cmds.sort((a, b) => (b.runtime_secs || 0) - (a.runtime_secs || 0));
    else if (sortBy === 'cpu') cmds.sort((a, b) => b.cpu - a.cpu);
    else if (sortBy === 'mem') cmds.sort((a, b) => b.mem - a.mem);

    // Stats
    const alive = cmds.filter(c => c.alive !== false).length;
    const total = cmds.length;
    const totalCpu = cmds.reduce((s, c) => s + c.cpu, 0);
    const totalMem = cmds.reduce((s, c) => s + c.mem, 0);
    footer.textContent = total + ' commands (' + alive + ' running) | CPU: ' + totalCpu.toFixed(1) + '% | Mem: ' + totalMem.toFixed(1) + 'MB';

    // Render rows
    if (cmds.length === 0) {
        container.innerHTML = '<div class="cmd-manager-empty">No commands found</div>';
        return;
    }

    let html = '<div class="cmd-manager-header"><span class="cm-col cm-name">Name</span><span class="cm-col cm-status">Status</span><span class="cm-col cm-runtime">Runtime</span><span class="cm-col cm-res">CPU</span><span class="cm-col cm-res">Mem</span><span class="cm-col cm-server">Server</span><span class="cm-col cm-actions">Actions</span></div>';
    for (const cmd of cmds) {
        const isAlive = cmd.alive !== false;
        const name = cmd.name || cmd.id;
        const args = (cmd.args || []).join(' ');
        const runtime = cmd.runtime_secs != null ? formatRuntime(cmd.runtime_secs) : '-';
        const statusClass = isAlive ? 'cm-running' : 'cm-exited';
        const statusText = isAlive ? (cmd.frozen ? 'frozen' : 'running') : ('exit ' + (cmd.exit_code != null ? cmd.exit_code : '?'));
        const exitCode = cmd.exit_code;
        const kept = cmd.exit && cmd.exit.retain_on_exit;
        const pinned = getPinnedNames().includes(name);
        const serverLabel = cmd.instUrl.replace(/^https?:\/\//, '').replace(/\/$/, '');

        html += `<div class="cmd-manager-row${isAlive ? '' : ' cm-row-dead'}" data-cmd-id="${escHtml(cmd.id)}" data-inst-url="${escHtml(cmd.instUrl)}">
            <span class="cm-col cm-name" title="${escHtml(name + (args ? ' ' + args : ''))}"><span class="cm-cmd-name">${escHtml(name)}</span>${args ? '<span class="cm-cmd-args">' + escHtml(args) + '</span>' : ''}</span>
            <span class="cm-col cm-status ${statusClass}">${statusText}</span>
            <span class="cm-col cm-runtime">${escHtml(runtime)}</span>
            <span class="cm-col cm-res">${cmd.cpu.toFixed(1)}%</span>
            <span class="cm-col cm-res">${cmd.mem.toFixed(1)}MB</span>
            <span class="cm-col cm-server" title="${escHtml(cmd.instUrl)}">${escHtml(serverLabel)}</span>
            <span class="cm-col cm-actions">
                ${isAlive ? `<button class="btn btn-xs" onclick="restartCommandById('${escHtml(cmd.instUrl)}','${escHtml(cmd.id)}')" title="Restart">&#x21BB;</button>` : ''}
                ${isAlive ? `<button class="btn btn-xs" onclick="toggleKeepCmd('${escHtml(cmd.instUrl)}','${escHtml(cmd.id)}')" title="${kept ? 'Unkeep' : 'Keep'}">${kept ? '★' : '☆'}</button>` : ''}
                <button class="btn btn-xs ${pinned ? 'btn-primary' : ''}" onclick="togglePinCmd('${escHtml(name)}')" title="Pin/Unpin">${pinned ? '◉' : '◎'}</button>
                ${isAlive ? `<button class="btn btn-xs btn-danger" onclick="killCommand('${escHtml(cmd.instUrl)}','${escHtml(cmd.id)}')" title="Kill">&#x2715;</button>` : ''}
                <button class="btn btn-xs" onclick="selectCommand('${escHtml(cmd.instUrl)}','${escHtml(cmd.id)}','${escHtml(name)}');closeCmdManager()" title="View">&#x25B6;</button>
            </span>
        </div>`;
    }
    container.innerHTML = html;
}

async function cmdManagerKillAll() {
    if (!confirm('Kill all running commands on all servers?')) return;
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive !== false) {
                try { await fetch(apiUrl('/api/commands/' + cmd.id, { url: inst.url }), { method: 'DELETE', headers: authHeadersForInstance(inst) }); } catch {}
            }
        }
    }
    loadCommands();
    renderCmdManagerList();
}

function openGlobalSearch() {
    _freezeAllPanelsForSearch();
    const modal = document.getElementById('globalSearchModal');
    modal.style.display = '';
    const input = document.getElementById('globalSearchInput');
    input.value = '';
    input.focus();
    document.getElementById('searchFreezeToggle').checked = false;
    document.getElementById('globalSearchResults').innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;font-size:0.75rem;">Type a query and press Enter to search across all command output</div>';
}

function closeGlobalSearch() {
    const modal = document.getElementById('globalSearchModal');
    modal.style.display = 'none';
    _thawAllPanelsFromSearch();
}

async function _toggleSearchFreezeCommands() {
    const freeze = document.getElementById('searchFreezeToggle').checked;
    if (freeze) {
        // Freeze all running commands across all servers
        for (const inst of state.connections) {
            if (!inst._commands) continue;
            for (const cmd of inst._commands) {
                if (!cmd.alive || cmd.frozen) continue;
                try {
                    const res = await fetch(apiUrl(`/api/commands/${cmd.id}/freeze`, { url: inst.url }), {
                        method: 'POST',
                        headers: authHeadersForInstance(inst),
                        body: JSON.stringify({}),
                    });
                    if (res.ok) {
                        _searchFrozenCmdIds.push({ instUrl: inst.url, cmdId: cmd.id, wasFrozen: false });
                    }
                } catch (e) { /* skip */ }
            }
        }
    } else {
        // Thaw all commands we froze
        for (const entry of _searchFrozenCmdIds) {
            if (!entry.wasFrozen) {
                try {
                    await fetch(apiUrl(`/api/commands/${entry.cmdId}/thaw`, { url: entry.instUrl }), {
                        method: 'POST',
                        headers: authHeadersForInstance({ url: entry.instUrl }),
                        body: JSON.stringify({}),
                    });
                } catch (e) { /* ignore */ }
            }
        }
        _searchFrozenCmdIds = [];
    }
}

function onSearchResultClick(instUrl, cmdId, cmdName) {
    const modal = document.getElementById('globalSearchModal');
    modal.style.display = 'none';

    // Select the command in the focused panel
    const activePanelId = getActivePanelId();
    selectCommand(instUrl, cmdId, cmdName);

    // Thaw all OTHER panels and commands, but keep the selected panel frozen
    const keepFrozenId = activePanelId;
    for (const panelId of _searchFrozenPanelIds) {
        if (panelId !== keepFrozenId) {
            const panelObj = state.panels.find(p => p.id === panelId);
            if (panelObj && panelObj.selectedInstUrl && panelObj.selectedCmdId) {
                startPanelUpdateMode(panelId);
            }
        }
    }
    // Thaw all frozen commands
    for (const entry of _searchFrozenCmdIds) {
        if (!entry.wasFrozen) {
            fetch(apiUrl(`/api/commands/${entry.cmdId}/thaw`, { url: entry.instUrl }), {
                method: 'POST',
                headers: authHeadersForInstance({ url: entry.instUrl }),
                body: JSON.stringify({}),
            }).catch(() => {});
        }
    }
    _searchFrozenCmdIds = [];
    _searchFrozenPanelIds.clear();

    // Keep only the active panel frozen
    if (keepFrozenId) {
        _searchFrozenPanelIds.add(keepFrozenId);
    }

    // Show a frozen indicator on the panel so the user knows updates are paused
    updateFrozenIndicator();
}

function updateFrozenIndicator() {
    // Remove any existing frozen indicators
    document.querySelectorAll('.search-frozen-indicator').forEach(el => el.remove());
    for (const panelId of _searchFrozenPanelIds) {
        const panelEl = document.getElementById(panelId);
        if (!panelEl) continue;
        const indicator = document.createElement('div');
        indicator.className = 'search-frozen-indicator';
        indicator.textContent = 'VTTY frozen (click to unfreeze)';
        indicator.onclick = () => {
            _searchFrozenPanelIds.delete(panelId);
            indicator.remove();
            const panelObj = state.panels.find(p => p.id === panelId);
            if (panelObj && panelObj.selectedInstUrl && panelObj.selectedCmdId) {
                startPanelUpdateMode(panelId);
            }
        };
        panelEl.style.position = 'relative';
        panelEl.appendChild(indicator);
    }
}

async function executeGlobalSearch() {
    const query = document.getElementById('globalSearchInput').value.trim();
    if (!query) return;
    const resultsContainer = document.getElementById('globalSearchResults');
    resultsContainer.innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;font-size:0.75rem;">Searching...</div>';
    let allResults = [];
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            try {
                const res = await fetch(apiUrl(`/api/commands/${cmd.id}/vtty/text`, { url: inst.url }), {
                    headers: authHeadersForInstance(inst),
                });
                if (!res.ok) continue;
                const json = await res.json();
                if (json.status !== 'ok' || !json.data || !json.data.text) continue;
                const lines = json.data.text.split('\n');
                const cmdName = cmd.name || cmd.id;
                const matchingLines = [];
                lines.forEach((line, idx) => {
                    if (line.toLowerCase().includes(query.toLowerCase())) {
                        matchingLines.push({ lineNum: idx + 1, text: line.trim() });
                    }
                });
                if (matchingLines.length > 0) {
                    allResults.push({ cmdName, cmdId: cmd.id, instUrl: inst.url, lines: matchingLines.slice(0, 50) });
                }
            } catch (e) { /* skip */ }
        }
    }
    if (allResults.length === 0) {
        resultsContainer.innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;font-size:0.75rem;">No results found</div>';
        return;
    }
    resultsContainer.innerHTML = allResults.map(group => `
        <div class="search-result-group">
            <div class="search-result-header" onclick="onSearchResultClick('${escHtml(group.instUrl)}','${escHtml(group.cmdId)}','${escHtml(group.cmdName)}')">
                ${escHtml(group.cmdName)} <span style="color:var(--text-muted);font-size:0.6rem;">(${group.lines.length} matches)</span>
            </div>
            ${group.lines.map(l => `<div class="search-result-line" title="${escHtml(l.text)}"><span style="color:var(--text-muted);">${l.lineNum}:</span> ${escHtml(l.text)}</div>`).join('')}
        </div>
    `).join('');
}


// ─── Sound Notifications ───
function initSoundToggle() {
    const btn = document.getElementById('soundBtn');
    if (!btn) return;
    if (state.soundEnabled) btn.classList.add('sound-btn-active');
}

function toggleSoundNotifications() {
    state.soundEnabled = !state.soundEnabled;
    localStorage.setItem('vrw_sound', state.soundEnabled.toString());
    const btn = document.getElementById('soundBtn');
    if (btn) btn.classList.toggle('sound-btn-active', state.soundEnabled);
}

function playExitSound(success) {
    try {
        const ctx = new (window.AudioContext || window.webkitAudioContext)();
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.connect(gain);
        gain.connect(ctx.destination);
        if (success) {
            osc.frequency.value = 880;
            osc.type = 'sine';
        } else {
            osc.frequency.value = 440;
            osc.type = 'square';
        }
        gain.gain.value = 0.1;
        gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
        osc.start(ctx.currentTime);
        osc.stop(ctx.currentTime + 0.5);
    } catch (e) { /* ignore — audio not supported */ }
}

// ─── Workspace Environments ──
// Environments are named sets of [panels, servers, commands] defined in
// the server config file ([[environments]]).  They allow the user to
// switch between predefined workspaces with a single click.
//
// On the CLI, environments can be specified in separate config files or
// inline in the main config.  The server exposes them via /api/environments.
// Environments with auto_start=true are pre-spawned when the server loads.

// Server-side environments fetched from /api/environments.
let _serverEnvironments = [];

/// Fetch workspace environments from the server.
async function fetchEnvironments() {
    try {
        const res = await fetch(apiUrl('/api/environments'), { headers: authHeaders() });
        if (!res.ok) return;
        const json = await res.json();
        if (json.status === 'ok' && Array.isArray(json.data)) {
            _serverEnvironments = json.data;
        }
    } catch (e) {
        // Not critical — environments are optional
    }
}

/// Render the environments list in the Envs tab.
function renderEnvironments() {
    const container = document.getElementById('envList');
    if (!container) return;

    // Merge server environments with any user-defined ones from localStorage
    const userEnvs = JSON.parse(localStorage.getItem('vrw_environments') || '[]');
    const allEnvs = [..._serverEnvironments, ...userEnvs];

    if (allEnvs.length === 0) {
        container.innerHTML = '<div style="padding:0.5rem;color:var(--text-muted);font-size:0.7rem;text-align:center;">No environments configured. Add [[environments]] to your config file or create user environments.</div>';
        return;
    }

    let html = '';
    for (const env of allEnvs) {
        const panelCount = (env.panels || []).length;
        const cmdCount = (env.panels || []).reduce((sum, p) => sum + (p.commands || []).length, 0);
        const autoBadge = env.auto_start
            ? '<span style="color:var(--green);font-size:0.6rem;">auto</span>'
            : '';
        const descHtml = env.description
            ? `<div style="font-size:0.6rem;color:var(--text-muted);margin-top:0.15rem;">${escHtml(env.description)}</div>`
            : '';
        const layoutHtml = env.layout
            ? `<span style="font-size:0.6rem;color:var(--text-muted);">${env.layout === 'vertical' ? 'stacked' : 'side-by-side'}</span>`
            : '';

        html += `<div class="template-card" onclick="activateEnvironment('${escHtml(env.name)}')" title="Click to activate this environment" style="cursor:pointer;">
            <div class="template-name">${escHtml(env.name)} ${autoBadge}</div>
            <div class="template-cmd">${panelCount} panel${panelCount !== 1 ? 's' : ''}, ${cmdCount} command${cmdCount !== 1 ? 's' : ''} ${layoutHtml}</div>
            ${descHtml}
        </div>`;
    }
    container.innerHTML = html;
}

/// Activate a workspace environment: create panels, connect servers, and spawn commands.
async function activateEnvironment(name) {
    const allEnvs = [..._serverEnvironments, ...JSON.parse(localStorage.getItem('vrw_environments') || '[]')];
    const env = allEnvs.find(e => e.name === name);
    if (!env) {
        console.error('[vrw] Environment not found:', name);
        return;
    }

    // Remove all existing panels
    const existingIds = state.panels.map(p => p.id);
    for (const id of existingIds) {
        disconnectPanelWs(id);
        stopPanelPoll(id);
    }
    state.panels = [];
    state._focusedPanelId = null;

    // Set layout direction
    if (env.layout === 'vertical') {
        state.panelLayout = 'column';
    } else if (env.layout === 'horizontal') {
        state.panelLayout = 'row';
    }
    localStorage.setItem('vrw_panel_layout', state.panelLayout);

    const defaultServer = env.default_server || getBaseUrl();
    const defaultToken = env.default_token || '';

    // Register all servers from the environment
    for (const panelDef of (env.panels || [])) {
        const serverUrl = panelDef.server || defaultServer;
        const serverToken = panelDef.token || defaultToken;
        const serverLabel = panelDef.server_label || '';
        addConnection(serverUrl, serverLabel, serverToken);
    }

    // Create panels and spawn commands
    for (let i = 0; i < (env.panels || []).length; i++) {
        const panelDef = env.panels[i];
        const panel = addPanelDirect();
        if (!panel) continue;

        const serverUrl = panelDef.server || defaultServer;
        panel.selectedInstUrl = serverUrl;

        // Focus the first panel
        if (i === 0) focusPanel(panel.id);

        // Spawn the first command in this panel (others can be spawned later)
        if (panelDef.commands && panelDef.commands.length > 0) {
            const cmdDef = panelDef.commands[0];
            try {
                const body = { cmd: cmdDef.cmd };
                if (cmdDef.args) body.args = cmdDef.args.split(' ');
                if (cmdDef.workdir) body.dir = cmdDef.workdir;
                if (cmdDef.certificate) body.certificate = cmdDef.certificate;
                if (cmdDef.rows) body.rows = cmdDef.rows;
                if (cmdDef.cols) body.cols = cmdDef.cols;
                if (cmdDef.retain_on_exit) body.retain_on_exit = true;

                const res = await fetch(apiUrl('/api/commands', { url: serverUrl }), {
                    method: 'POST',
                    headers: authHeadersForInstance({ url: serverUrl, token: serverUrl === defaultServer ? defaultToken : (panelDef.token || '') }),
                    body: JSON.stringify(body),
                });
                const json = await res.json();
                if (json.status === 'ok' && json.data && json.data.id) {
                    panel.selectedCmdId = json.data.id;
                }
            } catch (e) {
                console.error('[vrw] Failed to spawn command for panel:', e);
            }
        }
    }

    // Re-render panels
    _lastRenderedPanelCount = -1; // force rebuild
    renderPanels();

    // Reload commands list to show spawned commands in sidebar
    loadCommands();
    loadCertificates();

    // Switch to Servers tab to show the results
    const serversTab = document.querySelector('.sidebar-tab:first-child');
    if (serversTab) switchSidebarTab('servers', serversTab);

    console.log('[vrw] Environment activated:', name, '—', (env.panels || []).length, 'panels');
}

// ─── Command Groups ───
// Groups are stored in localStorage as vrw_cmd_groups = { "group-name": ["cmdName1", ...], ... }

/// Load command groups from localStorage.
function getCmdGroups() {
    try {
        const raw = localStorage.getItem('vrw_cmd_groups');
        if (!raw) return {};
        return JSON.parse(raw);
    } catch (e) {
        return {};
    }
}

/// Save command groups to localStorage.
function saveCmdGroups(groups) {
    try {
        localStorage.setItem('vrw_cmd_groups', JSON.stringify(groups));
    } catch (e) { /* quota exceeded */ }
}

/// Load collapsed state for group sections.
function getGroupCollapsedState() {
    try {
        const raw = localStorage.getItem('vrw_group_collapsed');
        if (!raw) return {};
        return JSON.parse(raw);
    } catch (e) {
        return {};
    }
}

function saveGroupCollapsedState(state) {
    try {
        localStorage.setItem('vrw_group_collapsed', JSON.stringify(state));
    } catch (e) { /* ignore */ }
}

/// Create a new command group.
function createCmdGroup() {
    const input = document.getElementById('newGroupName');
    if (!input) return;
    const name = input.value.trim();
    if (!name) return;
    const groups = getCmdGroups();
    if (groups[name]) {
        // Group already exists
        input.value = '';
        renderGroups();
        return;
    }
    groups[name] = [];
    saveCmdGroups(groups);
    input.value = '';
    renderGroups();
}

/// Delete a command group.
function deleteCmdGroup(groupName) {
    const groups = getCmdGroups();
    delete groups[groupName];
    saveCmdGroups(groups);
    renderGroups();
}

/// Rename a command group.
function renameCmdGroup(oldName) {
    const newName = prompt('Rename group "' + oldName + '" to:');
    if (!newName || !newName.trim()) return;
    const trimmed = newName.trim();
    if (trimmed === oldName) return;
    const groups = getCmdGroups();
    if (groups[trimmed]) {
        alert('A group named "' + trimmed + '" already exists.');
        return;
    }
    groups[trimmed] = groups[oldName] || [];
    delete groups[oldName];
    saveCmdGroups(groups);
    // Also update collapsed state
    const collapsed = getGroupCollapsedState();
    if (collapsed[oldName] !== undefined) {
        collapsed[trimmed] = collapsed[oldName];
        delete collapsed[oldName];
        saveGroupCollapsedState(collapsed);
    }
    renderGroups();
}

/// Toggle a command name in a group (add if absent, remove if present).
function toggleCmdInGroup(groupName, cmdName) {
    const groups = getCmdGroups();
    if (!groups[groupName]) groups[groupName] = [];
    const idx = groups[groupName].indexOf(cmdName);
    if (idx >= 0) {
        groups[groupName].splice(idx, 1);
    } else {
        groups[groupName].push(cmdName);
    }
    saveCmdGroups(groups);
    // Re-render groups if the tab is visible
    if (document.getElementById('tab-groups').style.display !== 'none') {
        renderGroups();
    }
}

/// Toggle collapsed state of a group section.
function toggleGroupCollapse(groupName) {
    const collapsed = getGroupCollapsedState();
    collapsed[groupName] = !collapsed[groupName];
    saveGroupCollapsedState(collapsed);
    renderGroups();
}

/// Render the groups tab content.
function renderGroups() {
    const container = document.getElementById('groupList');
    if (!container) return;
    const groups = getCmdGroups();
    const groupNames = Object.keys(groups);
    const collapsed = getGroupCollapsedState();

    if (groupNames.length === 0) {
        container.innerHTML = '<div style="padding:0.5rem;color:var(--text-muted);font-size:0.7rem;text-align:center;">No groups created yet. Right-click a command in the Servers tab to add it to a group.</div>';
        return;
    }

    // Build a lookup of all available commands: cmdName → { inst, cmd }
    const cmdMap = {};
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            const cmdName = cmd.name || cmd.id;
            if (!cmdMap[cmdName]) {
                cmdMap[cmdName] = { inst, cmd, cmdName };
            }
        }
    }

    let html = '';
    for (const gName of groupNames) {
        const isCollapsed = collapsed[gName] === true;
        const cmdNames = groups[gName] || [];
        html += '<div class="group-section">';
        html += '<div class="group-header" onclick="toggleGroupCollapse(\'' + escHtml(gName).replace(/'/g, "\\'") + '\')">';
        html += '<span class="group-caret">' + (isCollapsed ? '&#x25B6;' : '&#x25BC;') + '</span>';
        html += '<span class="group-name">' + escHtml(gName) + '</span>';
        html += '<span class="group-count">' + cmdNames.length + '</span>';
        html += '<span class="group-actions">';
        html += '<button class="btn btn-xs" onclick="event.stopPropagation();renameCmdGroup(\'' + escHtml(gName).replace(/'/g, "\\'") + '\')" title="Rename group">&#9998;</button>';
        html += '<button class="btn btn-xs btn-danger" onclick="event.stopPropagation();deleteCmdGroup(\'' + escHtml(gName).replace(/'/g, "\\'") + '\')" title="Delete group">&#x2715;</button>';
        html += '</span>';
        html += '</div>';
        if (!isCollapsed) {
            if (cmdNames.length === 0) {
                html += '<div style="padding:0.3rem 0.5rem 0.3rem 1.5rem;color:var(--text-muted);font-size:0.65rem;font-style:italic;">Empty — right-click a command to add it here</div>';
            } else {
                for (const cmdName of cmdNames) {
                    const entry = cmdMap[cmdName];
                    if (entry) {
                        const isAlive = entry.cmd.alive !== false;
                        const isFrozen = entry.cmd.frozen === true;
                        const isUnreachable = entry.inst.reachable === false;
                        const statusDot = isAlive ? '<span class="status-dot status-running"></span>' :
                            (isFrozen ? '<span class="status-dot status-frozen"></span>' : '<span class="status-dot status-exited"></span>');
                        const selected = (state.selectedInstUrl === entry.inst.url && state.selectedCmdId === entry.cmd.id) ? ' group-cmd-selected' : '';
                        html += '<div class="group-cmd-item' + selected + '"' +
                            ' data-inst-url="' + escHtml(entry.inst.url) + '"' +
                            ' data-cmd-id="' + escHtml(entry.cmd.id) + '"' +
                            ' data-cmd-name="' + escHtml(cmdName) + '"' +
                            (isUnreachable ? ' style="opacity:0.4;"' : '') +
                            ' onclick="selectCommand(this.dataset.instUrl, this.dataset.cmdId, this.dataset.cmdName)"' +
                            ' title="' + escHtml(entry.inst.label) + ' / ' + escHtml(cmdName) + '">' +
                            statusDot +
                            '<span class="group-cmd-name">' + escHtml(cmdName) + '</span>' +
                            '<button class="btn btn-xs" onclick="event.stopPropagation();toggleCmdInGroup(\'' + escHtml(gName).replace(/'/g, "\\'") + '\',\'' + escHtml(cmdName).replace(/'/g, "\\'") + '\');renderGroups()" title="Remove from group" style="margin-left:auto;padding:0 0.2rem;font-size:0.55rem;">&#x2715;</button>' +
                            '</div>';
                    } else {
                        html += '<div class="group-cmd-item" style="opacity:0.4;cursor:default;">' +
                            '<span class="group-cmd-name" style="text-decoration:line-through;">' + escHtml(cmdName) + '</span>' +
                            '<span style="font-size:0.55rem;color:var(--text-muted);margin-left:auto;">(not running)</span>' +
                            '<button class="btn btn-xs" onclick="event.stopPropagation();toggleCmdInGroup(\'' + escHtml(gName).replace(/'/g, "\\'") + '\',\'' + escHtml(cmdName).replace(/'/g, "\\'") + '\');renderGroups()" title="Remove from group" style="margin-left:auto;padding:0 0.2rem;font-size:0.55rem;">&#x2715;</button>' +
                            '</div>';
                    }
                }
            }
        }
        html += '</div>';
    }
    container.innerHTML = html;
}

// ─── Workspaces ───
// Workspaces save/restore the current panel configuration.
// Stored in localStorage as vrw_workspaces = { "name": { panels: [...], layout: "row", ... }, ... }

/// Load workspaces from localStorage.
function getWorkspaces() {
    try {
        const raw = localStorage.getItem('vrw_workspaces');
        if (!raw) return {};
        return JSON.parse(raw);
    } catch (e) {
        return {};
    }
}

/// Save workspaces to localStorage.
function saveWorkspaces(workspaces) {
    try {
        localStorage.setItem('vrw_workspaces', JSON.stringify(workspaces));
    } catch (e) { /* quota exceeded */ }
}

/// Toggle the workspace dropdown menu.
function toggleWorkspaceDropdown(e) {
    e.stopPropagation();
    const menu = document.getElementById('workspaceMenu');
    if (!menu) return;
    const isVisible = menu.style.display !== 'none';
    if (isVisible) {
        menu.style.display = 'none';
    } else {
        renderWorkspaceList();
        menu.style.display = '';
    }
}

/// Render the workspace list inside the dropdown.
function renderWorkspaceList() {
    const container = document.getElementById('workspaceList');
    if (!container) return;
    const workspaces = getWorkspaces();
    const names = Object.keys(workspaces);

    if (names.length === 0) {
        container.innerHTML = '<div style="padding:0.3rem 0.5rem;color:var(--text-muted);font-size:0.65rem;">No saved workspaces</div>';
        return;
    }

    let html = '';
    for (const name of names) {
        const panelCount = (workspaces[name].panels || []).length;
        html += '<div style="display:flex;align-items:center;gap:0.3rem;">';
        html += '<button class="ws-load-btn" onclick="loadWorkspace(\'' + escHtml(name).replace(/'/g, "\\'") + '\');toggleWorkspaceDropdown(event)" style="flex:1;text-align:left;">' +
            '<span style="color:var(--accent);">&#x1F4C2;</span> ' + escHtml(name) +
            ' <span style="color:var(--text-muted);font-size:0.55rem;">(' + panelCount + ' panels)</span></button>';
        html += '<button class="btn btn-xs" onclick="deleteWorkspace(\'' + escHtml(name).replace(/'/g, "\\'") + '\')" title="Delete" style="font-size:0.55rem;">&#x2715;</button>';
        html += '</div>';
    }
    container.innerHTML = html;
}

/// Save the current workspace configuration.
function saveCurrentWorkspace() {
    const name = prompt('Workspace name:');
    if (!name || !name.trim()) return;
    const trimmed = name.trim();

    // Capture current panel configuration
    const panels = state.panels.map(p => ({
        instUrl: p.selectedInstUrl || null,
        cmdId: p.selectedCmdId || null,
        cmdName: _getPanelCmdName(p),
        fontSize: p.fontSize,
        theme: p.theme || '',
        customTitle: p.customTitle || '',
    }));

    const workspaces = getWorkspaces();
    workspaces[trimmed] = {
        panels: panels,
        layout: state.panelLayout || 'row',
        timestamp: Date.now(),
    };
    saveWorkspaces(workspaces);
    renderWorkspaceList();

    // Close dropdown after a short delay so user sees the list update
    setTimeout(() => {
        const menu = document.getElementById('workspaceMenu');
        if (menu) menu.style.display = 'none';
    }, 600);
}

/// Get the command name for a panel from the current connections.
function _getPanelCmdName(panel) {
    if (!panel.selectedInstUrl || !panel.selectedCmdId) return null;
    const inst = state.connections.find(i => i.url === panel.selectedInstUrl);
    if (!inst || !inst._commands) return null;
    const cmd = inst._commands.find(c => c.id === panel.selectedCmdId);
    return cmd ? (cmd.name || cmd.id) : null;
}

/// Load a workspace and restore panel configuration.
function loadWorkspace(name) {
    const workspaces = getWorkspaces();
    const ws = workspaces[name];
    if (!ws) return;

    // Apply layout
    if (ws.layout) {
        state.panelLayout = ws.layout;
        localStorage.setItem('vrw_panel_layout', ws.layout);
    }

    // Clear existing panels (keep connections)
    // Disconnect WS for all existing panels first
    for (const p of state.panels) {
        if (p.ws) {
            try { p.ws.close(); } catch (e) { /* ignore */ }
            p.ws = null;
        }
        if (p.pollTimer) {
            clearInterval(p.pollTimer);
            p.pollTimer = null;
        }
    }
    state.panels = [];

    // Create panels from saved config
    const panelConfigs = ws.panels || [];
    if (panelConfigs.length === 0) {
        addPanelDirect();
    } else {
        for (const cfg of panelConfigs) {
            const panel = addPanelDirect();
            panel.fontSize = cfg.fontSize || state.fontSize;
            panel.theme = cfg.theme || '';
            panel.customTitle = cfg.customTitle || '';
            panel.selectedInstUrl = cfg.instUrl || null;
            panel.selectedCmdId = cfg.cmdId || null;
        }
    }

    // Force panel re-render
    _lastRenderedPanelCount = -1;
    renderPanels();

    // Focus the first panel
    if (state.panels.length > 0) {
        focusPanel(state.panels[0].id);
    }

    // Trigger command loading to populate sidebar and auto-select
    if (panelConfigs.length > 0) {
        loadCommands();
    }

    // Close the workspace menu
    const menu = document.getElementById('workspaceMenu');
    if (menu) menu.style.display = 'none';
}

/// Delete a workspace.
function deleteWorkspace(name) {
    const workspaces = getWorkspaces();
    delete workspaces[name];
    saveWorkspaces(workspaces);
    renderWorkspaceList();
}

/// Open the workspace management dialog.
function openWorkspaceManage() {
    // Close dropdown
    const menu = document.getElementById('workspaceMenu');
    if (menu) menu.style.display = 'none';

    const workspaces = getWorkspaces();
    const names = Object.keys(workspaces);

    // Create a simple management overlay
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.style.display = 'flex';
    overlay.id = 'workspaceManageOverlay';
    overlay.onclick = (e) => { if (e.target === overlay) { releaseCurrentFocusTrap(); overlay.remove(); } };

    let content = '<div class="modal">';
    content += '<h2>Manage Workspaces</h2>';
    if (names.length === 0) {
        content += '<p style="font-size:0.75rem;color:var(--text-muted);">No saved workspaces.</p>';
    } else {
        content += '<div style="max-height:300px;overflow-y:auto;">';
        for (const name of names) {
            const panelCount = (workspaces[name].panels || []).length;
            const ts = workspaces[name].timestamp ? new Date(workspaces[name].timestamp).toLocaleString() : 'unknown';
            content += '<div style="display:flex;align-items:center;gap:0.3rem;padding:0.3rem 0;border-bottom:1px solid var(--border);">';
            content += '<div style="flex:1;min-width:0;">';
            content += '<div style="font-size:0.75rem;color:var(--text-primary);font-weight:500;">' + escHtml(name) + '</div>';
            content += '<div style="font-size:0.6rem;color:var(--text-muted);">' + panelCount + ' panels &middot; saved ' + escHtml(ts) + '</div>';
            content += '</div>';
            content += '<button class="btn btn-xs" onclick="loadWorkspace(\'' + escHtml(name).replace(/'/g, "\\'") + '\');releaseCurrentFocusTrap();document.getElementById(\'workspaceManageOverlay\').remove()" title="Load">&#x25B6;</button>';
            content += '<button class="btn btn-xs" onclick="deleteWorkspace(\'' + escHtml(name).replace(/'/g, "\\'") + '\');openWorkspaceManage()" title="Delete">&#x2715;</button>';
            content += '</div>';
        }
        content += '</div>';
    }
    content += '<div class="actions" style="margin-top:1rem;">';
    content += '<button class="btn" onclick="releaseCurrentFocusTrap();document.getElementById(\'workspaceManageOverlay\').remove()">Close</button>';
    content += '</div>';
    content += '</div>';

    overlay.innerHTML = content;
    document.body.appendChild(overlay);
    trapFocus(overlay.querySelector('.modal'));
}

// Close workspace menu when clicking outside
document.addEventListener('click', (e) => {
    const dropdown = document.getElementById('workspaceDropdown');
    const menu = document.getElementById('workspaceMenu');
    if (dropdown && menu && menu.style.display !== 'none' && !dropdown.contains(e.target)) {
        menu.style.display = 'none';
    }
});



    // Logs
    window.connectLogWs = connectLogWs;
    window.disconnectLogWs = disconnectLogWs;
    window.loadLog = loadLog;
    window.searchLogs = searchLogs;
    window.clearLogSearch = clearLogSearch;
    window._updateLogTransportIndicator = _updateLogTransportIndicator;
    window._scheduleLogWsReconnect = _scheduleLogWsReconnect;
    // Docs
    window.showDocs = showDocs;
    // Refresh loop
    window.startRefresh = startRefresh;
    window.pollResources = pollResources;
    window.updateSidebarResourceText = updateSidebarResourceText;
    window.checkForExitedCommands = checkForExitedCommands;
    window.notifyCommandEnded = notifyCommandEnded;
    // Terminal search
    window.vttySearch = vttySearch;
    window.vttyApplyHighlights = vttyApplyHighlights;
    window.vttyRemoveHighlights = vttyRemoveHighlights;
    window.vttySearchClose = vttySearchClose;
    window.vttySearchNext = vttySearchNext;
    window.vttySearchPrev = vttySearchPrev;
    window.scrollTerminalBottom = scrollTerminalBottom;
    // Sound
    window.initSoundToggle = initSoundToggle;
    window.toggleSoundNotifications = toggleSoundNotifications;
    window.playExitSound = playExitSound;
    // Onboarding
    window.checkOnboarding = checkOnboarding;
    window.openOnboarding = openOnboarding;
    window.closeOnboarding = closeOnboarding;
    window.nextOnboardingStep = nextOnboardingStep;
    // Shortcuts
    window.showShortcuts = showShortcuts;
    window.closeShortcuts = closeShortcuts;
    // Global search
    window.openGlobalSearch = openGlobalSearch;
    window.closeGlobalSearch = closeGlobalSearch;
    window.executeGlobalSearch = executeGlobalSearch;
    window.onSearchResultClick = onSearchResultClick;
    window.updateFrozenIndicator = updateFrozenIndicator;
    window._toggleSearchFreezeCommands = _toggleSearchFreezeCommands;
    // Command manager
    window.openCmdManager = openCmdManager;
    window.closeCmdManager = closeCmdManager;
    window.renderCmdManagerList = renderCmdManagerList;
    window.cmdManagerKillAll = cmdManagerKillAll;
    // Templates
    window.fetchServerTemplates = fetchServerTemplates;
    window.getServerTemplates = getServerTemplates;
    window.getUserTemplates = getUserTemplates;
    window.saveUserTemplates = saveUserTemplates;
    window.renderTemplates = renderTemplates;
    window.spawnServerTemplate = spawnServerTemplate;
    window.spawnUserTemplate = spawnUserTemplate;
    window.deleteUserTemplate = deleteUserTemplate;
    window.showAddTemplateForm = showAddTemplateForm;
    window.hideAddTemplateForm = hideAddTemplateForm;
    window.saveTemplate = saveTemplate;
    // Workspaces
    window.getWorkspaces = getWorkspaces;
    window.saveWorkspaces = saveWorkspaces;
    window.toggleWorkspaceDropdown = toggleWorkspaceDropdown;
    window.renderWorkspaceList = renderWorkspaceList;
    window.saveCurrentWorkspace = saveCurrentWorkspace;
    window.loadWorkspace = loadWorkspace;
    window.deleteWorkspace = deleteWorkspace;
    window.openWorkspaceManage = openWorkspaceManage;
    // Environments
    window.fetchEnvironments = fetchEnvironments;
    window.renderEnvironments = renderEnvironments;
    window.activateEnvironment = activateEnvironment;
    // Groups
    window.getCmdGroups = getCmdGroups;
    window.saveCmdGroups = saveCmdGroups;
    window.getGroupCollapsedState = getGroupCollapsedState;
    window.saveGroupCollapsedState = saveGroupCollapsedState;
    window.createCmdGroup = createCmdGroup;
    window.deleteCmdGroup = deleteCmdGroup;
    window.renameCmdGroup = renameCmdGroup;
    window.toggleCmdInGroup = toggleCmdInGroup;
    window.toggleGroupCollapse = toggleGroupCollapse;
    window.renderGroups = renderGroups;
    // Sidebar drag
    window.onCmdDragStart = onCmdDragStart;
    window.getCmdOrder = getCmdOrder;
    window.setCmdOrder = setCmdOrder;
    window.getOrderedCmds = getOrderedCmds;
    window._openCommandInNewPane = _openCommandInNewPane;
    // Connections (also in commands.js — just exposing again is fine)
    window.handlePeerEvent = handlePeerEvent;
    window.fetchPeers = fetchPeers;
    window.addDiscoveredPeer = addDiscoveredPeer;
    window.savePeersToStorage = savePeersToStorage;
    window.addConnection = addConnection;
    window.removeConnection = removeConnection;
})();
