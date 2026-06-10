// ─── Server Connections: config fetch, update modes, certs, connection CRUD, modal, restart, spawn ───
(function() {
    'use strict';

// ─── Pause/Run Toggle ───
async function togglePauseRun() {
    if (!state.selectedCmdId) return;
    const inst = state.connections.find(i => i.url === state.selectedInstUrl);
    const cmd = inst && inst._commands ? inst._commands.find(c => c.id === state.selectedCmdId) : null;
    const isFrozen = cmd && cmd.frozen;
    const endpoint = isFrozen ? 'thaw' : 'freeze';
    try {
        await fetch(apiUrl(`/api/commands/${state.selectedCmdId}/${endpoint}`, { url: state.selectedInstUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: state.selectedInstUrl }),
            body: JSON.stringify({}),
        });
        loadCommands();
    } catch (e) { /* ignore */ }
}

async function togglePauseRunPanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || !panelObj.selectedInstUrl || !panelObj.selectedCmdId) return;
    const inst = state.connections.find(i => i.url === panelObj.selectedInstUrl);
    if (!inst || !inst._commands) return;
    const cmdId = panelObj.selectedCmdId;
    const cmd = inst._commands.find(c => c.id === cmdId);
    const isFrozen = cmd && cmd.frozen;
    const endpoint = isFrozen ? 'thaw' : 'freeze';
    try {
        await fetch(apiUrl(`/api/commands/${cmdId}/${endpoint}`, { url: panelObj.selectedInstUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: panelObj.selectedInstUrl }),
            body: JSON.stringify({}),
        });
        loadCommands();
    } catch (e) { /* ignore */ }
}


// ─── VTTY Update Modes (Push / Poll) ───
// The web UI supports two modes for detecting VTTY buffer changes:
//
// PUSH MODE (default): The server monitors the buffer and sends lightweight
//   "vtty_dirty" signals over the WebSocket whenever the buffer changes.
//   On receiving a dirty signal, the client does a debounced HTTP fetch to
//   get the latest HTML.  This is the most efficient mode.
//
// POLL MODE: The client periodically calls GET /api/commands/:id/vtty/changed
//   to ask "has the buffer changed?".  If yes, it fetches the full HTML.
//   This mode is useful when WebSocket connections are unreliable.

/// Fetch server-side web config (update_mode, poll defaults) from /api/info.
/// Also tracks whether the server is reachable at all.
async function fetchServerConfig() {
    try {
        const res = await fetch(apiUrl('/api/info'), { headers: authHeaders() });
        const json = await res.json();
        const wasReachable = state.serverReachable;
        state.serverReachable = !!json.status;
        // When server transitions from unreachable → reachable, immediately
        // load commands so the auto-select logic in loadCommands fires.
        // Without this, the next loadCommands interval tick (up to 1s delay)
        // is the earliest the panel gets a command.
        if (!wasReachable && state.serverReachable) {
            loadCommands();
        }
        // Re-render panels if reachability changed (e.g. "not running" -> welcome)
        if (wasReachable !== state.serverReachable) {
            renderPanels();
            updateSidebarTabsVisibility();
        }
        if (json.status === 'ok' && json.data && json.data.web) {
            state.serverUpdateMode = json.data.web.update_mode;
            state.serverPollMs = json.data.web.default_poll_ms;
            state.serverDirtyMs = json.data.web.dirty_check_ms;
            // If no user preference is set, use the server default
            if (!localStorage.getItem('vrw_update_mode')) {
                state.updateMode = state.serverUpdateMode || 'push';
            }
            if (!localStorage.getItem('vrw_poll_interval')) {
                state.pollInterval = state.serverPollMs || 500;
            }
            // Apply server-configured panel colors if available
            if (json.data.web.panel_colors && json.data.web.panel_colors.length > 0) {
                state._serverPanelColors = json.data.web.panel_colors;
            }
        }
        // Capture server name from API for display in panel headers
        if (json.status === 'ok' && json.data && json.data.server_name) {
            const primaryConn = state.connections.find(c => c.url === getBaseUrl());
            if (primaryConn) {
                primaryConn._serverName = json.data.server_name;
            }
        }
        if (json.status === 'ok' && json.data && json.data.vtty) {
            state.serverScreenshotFontSize = json.data.vtty.screenshot_font_size || 12;
            state.serverScreenshotFontName = json.data.vtty.screenshot_font_name || 'monospace';
        }
    } catch (e) {
        const wasReachable = state.serverReachable;
        state.serverReachable = false;
        if (wasReachable !== state.serverReachable) {
            renderPanels();
            updateSidebarTabsVisibility();
        }
    }
}

/// Apply the current updateMode to the UI controls.
function applyUpdateModeUI() {
    document.getElementById('updateMode').value = state.updateMode;
    document.getElementById('pollInterval').value = String(state.pollInterval);
    document.getElementById('pollIntervalWrap').style.display = state.updateMode === 'poll' ? '' : 'none';
}

/// Switch update mode (called from the dropdown).
function switchUpdateMode(mode) {
    state.updateMode = mode;
    localStorage.setItem('vrw_update_mode', mode);
    applyUpdateModeUI();
    // Stop existing update mechanism and restart with new mode
    stopUpdateMode();
    if (state.selectedInstUrl && state.selectedCmdId) {
        startUpdateMode();
    }
}

/// Apply the poll interval from the input.
function applyPollInterval() {
    const val = parseInt(document.getElementById('pollInterval').value) || 500;
    state.pollInterval = Math.max(50, Math.min(5000, val));
    localStorage.setItem('vrw_poll_interval', state.pollInterval.toString());
    document.getElementById('pollInterval').value = String(state.pollInterval);
    // If currently polling, restart the timer with new interval
    if (state.updateMode === 'poll' && state._pollTimer) {
        stopPoll();
        startPoll();
    }
}
// ─── Certificates ───
async function loadCertificates() {
    for (const inst of state.connections) {
        try {
            const res = await fetch(apiUrl('/api/certificates', inst), { headers: authHeadersForInstance(inst) });
            const json = await res.json();
            inst._certs = json.status === 'ok' ? json.data : [];
        } catch (e) {
            inst._certs = [];
        }
    }

    const container = document.getElementById('certList');
    let html = '';
    for (const inst of state.connections) {
        html += `<div style="font-size:0.7rem;color:var(--text-muted);padding:0.3rem 0;margin-top:0.3rem;">${escHtml(inst.label)}</div>`;
        const certs = inst._certs || [];
        if (certs.length === 0) {
            html += '<div style="padding:0.3rem;font-size:0.8rem;color:var(--text-muted);">No certificates</div>';
        }
        for (const cert of certs) {
            html += `<div style="padding:0.3rem 0.5rem;border-bottom:1px solid var(--border);font-size:0.8rem;">
                <span class="cert-badge">${escHtml(cert.name)}</span>
                <span style="color:var(--text-muted);font-size:0.7rem;margin-left:0.5rem;font-family:var(--font-mono);">${escHtml(cert.token_preview || '')}...</span>
            </div>`;
        }
    }
    container.innerHTML = html;

    // Update spawn cert dropdown
    updateCertDropdown();
}

function updateCertDropdown() {
    const select = document.getElementById('spawnCert');
    let html = '<option value="">None</option>';
    for (const inst of state.connections) {
        for (const cert of (inst._certs || [])) {
            html += `<option value="${escHtml(cert.name)}">${escHtml(inst.label)}: ${escHtml(cert.name)}</option>`;
        }
    }
    select.innerHTML = html;
}

// Track the user's explicit spawn instance choice separately from
// state.selectedInstUrl.  Without this, updateInstanceDropdown() would
// reset the dropdown to whatever panel is focused, overwriting the user's
// choice every time the sidebar rebuilds.  Once set (either by the user
// manually changing the dropdown or by spawning a command), it persists
// for the lifetime of the session — it is never silently overridden by
// the focused panel's instance.
// IMPORTANT: This MUST be a window property (not a local let/var) because
// the sidebar sort bar and spawn dialog set window._userSpawnInstUrl
// directly via inline onclick/onchange handlers.  A local variable in this
// IIFE would shadow the window property, causing the dropdown to ignore
// the user's explicit server selection.

function updateInstanceDropdown() {
    const select = document.getElementById('spawnInstance');
    const current = select.value;
    let html = '';
    for (const inst of state.connections) {
        html += `<option value="${escHtml(inst.url)}">${escHtml(inst.label)} (${escHtml(inst.url.replace(/^https?:\/\//, ''))})</option>`;
    }
    select.innerHTML = html;

    // The spawn instance dropdown is fully decoupled from the focused panel.
    // It only changes when the user explicitly selects a different instance.
    // Priority:
    // 1. The user's explicit spawn-instance choice (set when the user
    //    manually changes the dropdown or when a command is spawned).
    // 2. The previous dropdown value, if it still exists in the list.
    // 3. Fall back to the first connection (never to the focused panel,
    //    since that would re-introduce the coupling bug).
    const userUrl = window._userSpawnInstUrl;
    if (userUrl && state.connections.some(i => i.url === userUrl)) {
        select.value = userUrl;
    } else if (current && state.connections.some(i => i.url === current)) {
        select.value = current;
        window._userSpawnInstUrl = current;  // remember the restored value
    } else if (state.connections.length > 0) {
        select.value = state.connections[0].url;
        window._userSpawnInstUrl = state.connections[0].url;
    }
}

// ─── Server Connection Management ───
// Connections are separate from panels. Adding a connection makes its
// commands available in the sidebar. Removing a connection removes its
// commands from the sidebar but does NOT close any panels (they keep
// their last VTTY state).
function addConnection(url, label, token) {
    // Idempotent: if connection already exists, return it unchanged.
    // This prevents accidental overwrites of user-set labels/tokens.
    const existing = state.connections.find(c => c.url === url);
    if (existing) {
        return existing;
    }
    // Derive label from URL if not provided: show host:port
    if (!label) {
        try {
            const u = new URL(url);
            if (u.port) {
                label = u.host;
            } else {
                const scheme = u.protocol.replace(':', '');
                const defaultPort = scheme === 'https' ? 443 : scheme === 'http' ? 80 : 0;
                const actualPort = parseInt(u.port || '0') || defaultPort;
                label = u.hostname + ':' + actualPort;
            }
        } catch (e) { label = url; }
    }
    const conn = { url, label: label || url, token: token || '', reachable: undefined, _lastError: null, _commands: null, _certs: null, _serverName: null };
    state.connections.push(conn);
    // Persist connections to localStorage
    _saveConnections();
    return conn;
}

function removeConnection(url) {
    state.connections = state.connections.filter(c => c.url !== url);
    _lastCommandState = ''; // force sidebar rebuild
    _saveConnections();
    loadCommands();
    updateDisconnectedUI();
}

/// Save connection list to localStorage for persistence across page reloads.
function _saveConnections() {
    const data = state.connections.map(c => ({ url: c.url, label: c.label, token: c.token }));
    localStorage.setItem('vrw_connections', JSON.stringify(data));
}

/// Restore connections from localStorage.
function _restoreConnections() {
    try {
        const data = JSON.parse(localStorage.getItem('vrw_connections') || '[]');
        if (!Array.isArray(data) || data.length === 0) return null;
        // First connection is always the origin server; skip saved ones that match
        const originUrl = window.location.origin;
        let restored = [];
        for (const item of data) {
            if (item.url && item.url !== originUrl) {
                addConnection(item.url, item.label || '', item.token || '');
                restored.push(item.url);
            }
        }
        return restored.length > 0 ? restored : null;
    } catch (e) { return null; }
}

/// Health-check restored connections: try each URL up to 5 times at 500ms intervals.
/// If a connection never responds, remove it from state.connections and localStorage.
/// This prevents stale connections from persisting across page reloads when the
/// remote server is no longer running.
function healthCheckConnections(restoredUrls) {
    if (!restoredUrls || restoredUrls.length === 0) return;
    const MAX_RETRIES = 5;
    const RETRY_INTERVAL_MS = 500;
    const retryCounts = {};
    for (const url of restoredUrls) {
        retryCounts[url] = 0;
    }

    function attemptCheck() {
        let anyPending = false;
        for (const url of restoredUrls) {
            // Already removed or already reachable (loadCommands will set that)
            const conn = state.connections.find(c => c.url === url);
            if (!conn) continue;
            if (conn.reachable === true) continue; // successful — keep it

            retryCounts[url] = (retryCounts[url] || 0) + 1;
            if (retryCounts[url] > MAX_RETRIES) {
                // Give up — remove this connection
                removeConnection(url);
                continue;
            }
            anyPending = true;
        }
        if (anyPending) {
            // loadCommands() already runs every 1s and sets conn.reachable.
            // We just need to wait and check again.
            setTimeout(attemptCheck, RETRY_INTERVAL_MS);
        }
    }

    // Start checking after loadCommands has had a chance to run once
    setTimeout(attemptCheck, RETRY_INTERVAL_MS);
}

/// Fetch server_name from /api/info for a non-primary connection.
async function _fetchServerName(conn) {
    try {
        const res = await fetch(apiUrl('/api/info', conn), { headers: authHeadersForInstance(conn) });
        const json = await res.json();
        if (json.status === 'ok' && json.data && json.data.server_name) {
            conn._serverName = json.data.server_name;
        }
    } catch (e) { /* ignore */ }
}

function disconnectServer(url) {
    const inst = state.connections.find(c => c.url === url);
    if (!inst) return;
    // Check if any panels are connected to commands on this server
    const activePanels = state.panels.filter(p => p.selectedInstUrl === url && p.selectedCmdId);
    if (activePanels.length > 0) {
        if (!confirm(`Disconnect from "${inst.label}"? ${activePanels.length} panel(s) showing commands from this server will keep their last state.`)) return;
    } else {
        if (!confirm(`Disconnect from "${inst.label}"?`)) return;
    }
    // Disconnect WS and poll for panels on this server
    for (const panel of activePanels) {
        disconnectPanelWs(panel.id);
        stopPanelPoll(panel.id);
    }
    removeConnection(url);
}

// ─── Add Server Modal (sidebar only, no panel) ───
function showAddServerModal() {
    const modal = document.getElementById('addServerModal');
    modal.style.display = '';
    document.getElementById('addServerUrl').value = 'http://localhost:9090';
    document.getElementById('addServerLabel').value = '';
    document.getElementById('addServerToken').value = '';
    document.getElementById('addServerOpenPane').checked = true;
    const modalInner = modal.querySelector('.modal');
    if (modalInner) trapFocus(modalInner);
    document.getElementById('addServerUrl').focus();
}

function closeAddServerModal() {
    releaseCurrentFocusTrap();
    document.getElementById('addServerModal').style.display = 'none';
}

async function confirmAddServer() {
    const url = document.getElementById('addServerUrl').value.trim();
    if (!url) return;
    const token = document.getElementById('addServerToken').value.trim();
    let label = document.getElementById('addServerLabel').value.trim();
    if (!label) {
        try { label = new URL(url).host; } catch (e) { label = url; }
    }
    const openPane = document.getElementById('addServerOpenPane').checked;
    const isNew = !state.connections.some(c => c.url === url);
    const conn = addConnection(url, label, token);
    closeAddServerModal();
    // Fetch server_name for this connection
    _fetchServerName(conn);
    loadCommands();
    loadCertificates();
    fetchServerTemplates();

    if (openPane) {
        // Wait for commands to load, then open a pane connected to the server's
        // main command (first spawned, i.e. spawn_order 0) or the first command.
        await loadCommands();
        const targetCmd = (conn._commands || []).find(c => c.spawn_order === 0) ||
                         (conn._commands || [])[0];
        if (targetCmd) {
            _cacheTerminalForSwitch();
            // Create a new panel and connect it to the server's main/first command
            const panelObj = addPanelDirect();
            panelObj.selectedInstUrl = url;
            panelObj.selectedCmdId = targetCmd.id;
            focusPanel(panelObj.id);
            state.selectedInstUrl = url;
            state.selectedCmdId = targetCmd.id;
            state._pendingVttyData = null;
            state._pendingVttyDirty = false;
            state.bufferView = 'current';
            _restoreCachedDom(targetCmd.id);
            updatePanelCommandInfo();
            updateTerminalDisconnectedOverlay();
            updateSidebarSelection();
            loadVttyHttpForPanel(panelObj.id, url, targetCmd.id);
            startPanelUpdateMode(panelObj.id);
        } else {
            // No commands yet — create an empty panel focused on this server
            const panelObj = addPanelDirect();
            panelObj.selectedInstUrl = url;
            focusPanel(panelObj.id);
        }
        renderPanels();
    }
}


// ─── Restart Command ───
async function restartCommand(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const inst = panelObj.selectedInstUrl ? state.connections.find(i => i.url === panelObj.selectedInstUrl) : null;
    if (!inst || !inst._commands) return;
    const cmdId = panelObj.selectedCmdId;
    if (!cmdId) return;
    await restartCommandById(panelObj.selectedInstUrl, cmdId);
}

async function restartCommandById(instUrl, cmdId) {
    // Use the atomic restart endpoint: the server spawns the new command
    // FIRST, then kills the old one.  This prevents the server from
    // shutting down when the old command was the last one running.
    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}/restart`, { url: instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: instUrl }),
            body: JSON.stringify({}),
        });
        const json = await res.json();
        if (json.status === 'ok' && json.data && json.data.id) {
            const newId = json.data.id;
            state.selectedInstUrl = instUrl;
            state.selectedCmdId = newId;
            _lastCommandState = '';
            // Reload command list so the sidebar contains the new command.
            await loadCommands();
            // Find the new command's name from the refreshed list.
            const inst = state.connections.find(i => i.url === instUrl);
            let newName = newId;
            if (inst && inst._commands) {
                const newCmd = inst._commands.find(c => c.id === newId);
                if (newCmd) newName = newCmd.name || newCmd.id;
            }
            // Stop the old WS/poll (connected to the now-dead old command)
            // and start fresh with the new command.
            selectCommand(instUrl, newId, newName);
        }
    } catch (e) { /* ignore */ }
}


// ─── Welcome Panel Spawn ───
async function spawnFromWelcome() {
    const input = document.getElementById('welcomeCmd');
    if (!input || !input.value.trim()) return;
    const cmd = input.value.trim();
    const instUrl = getBaseUrl();
    try {
        const res = await fetch(apiUrl('/api/commands', { url: instUrl }), {
            method: 'POST',
            headers: authHeaders(),
            body: JSON.stringify({ cmd }),
        });
        const json = await res.json();
        if (json.status === 'ok') {
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                _cacheTerminalForSwitch();
                state._pendingSelectId = newId;
            }
            loadCommands();
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    } catch (e) {
        alert('Spawn failed: ' + e.message);
    }
}


    window.togglePauseRun = togglePauseRun;
    window.togglePauseRunPanel = togglePauseRunPanel;
    window.fetchServerConfig = fetchServerConfig;
    window.applyUpdateModeUI = applyUpdateModeUI;
    window.switchUpdateMode = switchUpdateMode;
    window.applyPollInterval = applyPollInterval;
    window.loadCertificates = loadCertificates;
    window.updateCertDropdown = updateCertDropdown;
    window.updateInstanceDropdown = updateInstanceDropdown;
    window.addConnection = addConnection;
    window._saveConnections = _saveConnections;
    window._restoreConnections = _restoreConnections;
    window.healthCheckConnections = healthCheckConnections;
    window._fetchServerName = _fetchServerName;
    window.removeConnection = removeConnection;
    window.disconnectServer = disconnectServer;
    window.showAddServerModal = showAddServerModal;
    window.closeAddServerModal = closeAddServerModal;
    window.confirmAddServer = confirmAddServer;
    window.restartCommand = restartCommand;
    window.restartCommandById = restartCommandById;
    window.spawnFromWelcome = spawnFromWelcome;
})();
