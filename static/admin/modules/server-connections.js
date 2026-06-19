// ─── Server Connections: config, update modes, certs, CRUD, modal, restart, spawn ───
(function() {
    'use strict';

// ─── Pause/Run Toggle ───
function _doFreezeThaw(instUrl, cmdId) {
    const inst = state.connections.find(i => i.url === instUrl);
    if (!inst || !inst._commands) return Promise.resolve();
    const cmd = inst._commands.find(c => c.id === cmdId);
    if (!cmd) return Promise.resolve();
    return cmd.frozen ? api.thaw(instUrl, cmdId) : api.freeze(instUrl, cmdId);
}

async function togglePauseRun() {
    if (state._focusedPanelId) { togglePauseRunPanel(state._focusedPanelId); return; }
    if (!state.selectedCmdId) return;
    try { await _doFreezeThaw(state.selectedInstUrl, state.selectedCmdId); loadCommands(); } catch (e) {}
}

async function togglePauseRunPanel(panelId) {
    const p = state.panels.find(p => p.id === panelId);
    if (!p || !p.selectedInstUrl || !p.selectedCmdId) return;
    try { await _doFreezeThaw(p.selectedInstUrl, p.selectedCmdId); loadCommands(); } catch (e) {}
}

// ─── VTTY Update Modes (Push / Poll) ───
async function fetchServerConfig() {
    try {
        const json = await api.getInfo();
        const wasReachable = state.serverReachable;
        state.serverReachable = !!json.status;
        if (!wasReachable && state.serverReachable) loadCommands();
        if (wasReachable !== state.serverReachable) { renderPanels(); updateSidebarTabsVisibility(); }
        const d = json.data;
        if (json.status === 'ok' && d && d.web) {
            state.serverUpdateMode = d.web.update_mode;
            state.serverPollMs = d.web.default_poll_ms;
            state.serverDirtyMs = d.web.dirty_check_ms;
            if (!localStorage.getItem('vrw_update_mode')) state.updateMode = state.serverUpdateMode || 'push';
            if (!localStorage.getItem('vrw_poll_interval')) state.pollInterval = state.serverPollMs || 500;
            if (d.web.panel_colors && d.web.panel_colors.length) state._serverPanelColors = d.web.panel_colors;
        }
        if (json.status === 'ok' && d) {
            const pc = state.connections.find(c => c.url === getBaseUrl());
            if (pc && d.server_name) pc._serverName = d.server_name;
            if (d.vtty) {
                state.serverScreenshotFontSize = d.vtty.screenshot_font_size || 12;
                state.serverScreenshotFontName = d.vtty.screenshot_font_name || 'monospace';
            }
        }
    } catch (e) {
        const wasReachable = state.serverReachable;
        state.serverReachable = false;
        if (wasReachable !== state.serverReachable) { renderPanels(); updateSidebarTabsVisibility(); }
    }
}

function applyUpdateModeUI() {
    document.getElementById('updateMode').value = state.updateMode;
    document.getElementById('pollInterval').value = String(state.pollInterval);
    document.getElementById('pollIntervalWrap').classList.toggle('hidden', state.updateMode !== 'poll');
}

function switchUpdateMode(mode) {
    state.updateMode = mode;
    localStorage.setItem('vrw_update_mode', mode);
    applyUpdateModeUI();
    stopPanelUpdateMode(state._focusedPanelId);
    if (state.selectedInstUrl && state.selectedCmdId) startPanelUpdateMode(state._focusedPanelId);
}

function applyPollInterval() {
    const val = parseInt(document.getElementById('pollInterval').value) || 500;
    state.pollInterval = Math.max(50, Math.min(5000, val));
    localStorage.setItem('vrw_poll_interval', String(state.pollInterval));
    document.getElementById('pollInterval').value = String(state.pollInterval);
    if (state.updateMode === 'poll') {
        for (const p of state.panels) stopPanelPoll(p.id);
        if (state._focusedPanelId) startPanelPoll(state._focusedPanelId);
    }
}

// ─── Certificates ───
async function loadCertificates() {
    for (const inst of state.connections) {
        try { const r = await api.getCertificates(inst.url); inst._certs = (r.status === 'ok' && Array.isArray(r.data)) ? r.data : []; }
        catch (e) { inst._certs = []; }
    }
    let html = '';
    for (const inst of state.connections) {
        html += `<div style="font-size:0.7rem;color:var(--text-muted);padding:0.3rem 0;margin-top:0.3rem;">${escHtml(inst.label)}</div>`;
        const certs = inst._certs || [];
        if (!certs.length) html += '<div style="padding:0.3rem;font-size:0.8rem;color:var(--text-muted);">No certificates</div>';
        for (const cert of certs) {
            html += `<div style="padding:0.3rem 0.5rem;border-bottom:1px solid var(--border);font-size:0.8rem;">
                <span class="cert-badge">${escHtml(cert.name)}</span>
                <span style="color:var(--text-muted);font-size:0.7rem;margin-left:0.5rem;font-family:var(--font-mono);">${escHtml(cert.token_preview || '')}...</span>
            </div>`;
        }
    }
    document.getElementById('certList').innerHTML = html;
    updateCertDropdown();
}

function updateCertDropdown() {
    let html = '<option value="">None</option>';
    for (const inst of state.connections)
        for (const cert of (inst._certs || []))
            html += `<option value="${escHtml(cert.name)}">${escHtml(inst.label)}: ${escHtml(cert.name)}</option>`;
    document.getElementById('spawnCert').innerHTML = html;
}

// window._userSpawnInstUrl tracks user's explicit spawn instance choice (not the focused panel's).
// Must be a window property because sidebar/spawn dialog set it via inline handlers.
function updateInstanceDropdown() {
    const select = document.getElementById('spawnInstance');
    const current = select.value;
    let html = '';
    for (const inst of state.connections)
        html += `<option value="${escHtml(inst.url)}">${escHtml(inst.label)} (${escHtml(inst.url.replace(/^https?:\/\//, ''))})</option>`;
    select.innerHTML = html;
    // Priority: user explicit choice > previous value still valid > first connection
    const userUrl = window._userSpawnInstUrl;
    if (userUrl && state.connections.some(i => i.url === userUrl)) {
        select.value = userUrl;
    } else if (current && state.connections.some(i => i.url === current)) {
        select.value = current;
        window._userSpawnInstUrl = current;
    } else if (state.connections.length) {
        select.value = state.connections[0].url;
        window._userSpawnInstUrl = state.connections[0].url;
    }
}

// ─── Server Connection Management ───
function addConnection(url, label, token) {
    const existing = state.connections.find(c => c.url === url);
    if (existing) return existing;
    if (!label) {
        try {
            const u = new URL(url);
            const scheme = u.protocol.replace(':', '');
            const port = u.port || (scheme === 'https' ? 443 : scheme === 'http' ? 80 : 0);
            label = u.hostname + ':' + port;
        } catch (e) { label = url; }
    }
    const conn = { url, label: label || url, token: token || '', reachable: undefined, _lastError: null, _commands: null, _certs: null, _serverName: null };
    state.connections.push(conn);
    _saveConnections();
    return conn;
}

function removeConnection(url) {
    state.connections = state.connections.filter(c => c.url !== url);
    _saveConnections();
    loadCommands();
    updateDisconnectedUI();
}

function _saveConnections() {
    localStorage.setItem('vrw_connections', JSON.stringify(
        state.connections.map(c => ({ url: c.url, label: c.label, token: c.token }))
    ));
}

function _restoreConnections() {
    try {
        const data = JSON.parse(localStorage.getItem('vrw_connections') || '[]');
        if (!Array.isArray(data) || !data.length) return null;
        const originUrl = window.location.origin;
        const restored = [];
        for (const item of data) {
            if (item.url && item.url !== originUrl) {
                addConnection(item.url, item.label || '', item.token || '');
                restored.push(item.url);
            }
        }
        return restored.length ? restored : null;
    } catch (e) { return null; }
}

function healthCheckConnections(restoredUrls) {
    if (!restoredUrls || !restoredUrls.length) return;
    const MAX = 5, INTERVAL = 500, counts = {};
    restoredUrls.forEach(u => counts[u] = 0);

    function tick() {
        let pending = false;
        for (const url of restoredUrls) {
            const conn = state.connections.find(c => c.url === url);
            if (!conn || conn.reachable === true) continue;
            if (++counts[url] > MAX) { removeConnection(url); continue; }
            pending = true;
        }
        if (pending) setTimeout(tick, INTERVAL);
    }
    setTimeout(tick, INTERVAL);
}

async function _fetchServerName(conn) {
    try {
        const json = await api.getInfo(conn.url);
        if (json.status === 'ok' && json.data && json.data.server_name) conn._serverName = json.data.server_name;
    } catch (e) {}
}

function disconnectServer(url) {
    const inst = state.connections.find(c => c.url === url);
    if (!inst) return;
    if (url === window.location.origin) { alert('Cannot disconnect from the default server.'); return; }
    const active = state.panels.filter(p => p.selectedInstUrl === url && p.selectedCmdId);
    const msg = active.length
        ? `Disconnect from "${inst.label}"? ${active.length} panel(s) will keep their last state.`
        : `Disconnect from "${inst.label}"?`;
    if (!confirm(msg)) return;
    for (const p of active) { disconnectPanelWs(p.id); stopPanelPoll(p.id); }
    removeConnection(url);
}

// ─── Add Server Modal ───
function showAddServerModal() {
    const modal = document.getElementById('addServerModal');
    modal.classList.remove('hidden');
    document.getElementById('addServerUrl').value = 'http://localhost:9090';
    document.getElementById('addServerLabel').value = '';
    document.getElementById('addServerToken').value = '';
    document.getElementById('addServerOpenPane').checked = true;
    const inner = modal.querySelector('.modal');
    if (inner) trapFocus(inner);
    document.getElementById('addServerUrl').focus();
}

function closeAddServerModal() {
    releaseCurrentFocusTrap();
    document.getElementById('addServerModal').classList.add('hidden');
}

async function confirmAddServer() {
    const url = document.getElementById('addServerUrl').value.trim();
    if (!url) return;
    const token = document.getElementById('addServerToken').value.trim();
    let label = document.getElementById('addServerLabel').value.trim();
    if (!label) { try { label = new URL(url).host; } catch (e) { label = url; } }
    const openPane = document.getElementById('addServerOpenPane').checked;
    const conn = addConnection(url, label, token);
    closeAddServerModal();
    _fetchServerName(conn);
    loadCertificates();
    fetchServerTemplates();
    if (!openPane) { loadCommands(); return; }

    await loadCommands();
    const cmds = conn._commands || [];
    if (cmds.length === 0) { renderPanels(); return; }
    const targetCmd = cmds.find(c => c.spawn_order === 0) || cmds[0];
    const panelObj = addPanelDirect();
    panelObj.selectedInstUrl = url;
    focusPanel(panelObj.id);
    _selectCommandForPanel(panelObj, url, targetCmd.id);
}

// ─── Restart Command ───
async function restartCommand(targetId) {
    // targetId may be a panel ID or a leaf ID (from _withPanel in split context)
    const p = state.panels.find(pp => pp.id === targetId);
    if (!p) return;
    let instUrl, cmdId;
    if (!p.split || targetId === p.id) {
        instUrl = p.selectedInstUrl; cmdId = p.selectedCmdId;
    } else {
        const found = typeof _findLeafState === 'function' ? _findLeafState(p, targetId) : null;
        if (found && found.leaf) { instUrl = found.leaf.instUrl; cmdId = found.leaf.cmdId; }
        else { instUrl = p.selectedInstUrl; cmdId = p.selectedCmdId; }
    }
    if (!instUrl || !cmdId) return;
    const inst = state.connections.find(i => i.url === instUrl);
    if (!inst || !inst._commands) return;
    await restartCommandById(instUrl, cmdId);
}

async function restartCommandById(instUrl, cmdId) {
    // Atomic restart: server spawns new command first, then kills old one.
    try {
        const json = await api.restart(instUrl, cmdId);
        if (json.status === 'ok' && json.data && json.data.id) {
            const newId = json.data.id;
            state.selectedInstUrl = instUrl;
            state.selectedCmdId = newId;
            await loadCommands();
            const inst = state.connections.find(i => i.url === instUrl);
            const newCmd = inst && inst._commands && inst._commands.find(c => c.id === newId);
            selectCommand(instUrl, newId, newCmd ? (newCmd.name || newId) : newId);
        }
    } catch (e) {}
}

// ─── Welcome Panel Spawn ───
async function spawnFromWelcome() {
    const input = document.getElementById('welcomeCmd');
    if (!input || !input.value.trim()) return;
    const cmd = input.value.trim();
    const instUrl = getBaseUrl();
    try {
        const json = await api.spawnCommand(instUrl, { cmd });
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

    Object.assign(window, {
        _doFreezeThaw, togglePauseRun, togglePauseRunPanel, fetchServerConfig, applyUpdateModeUI,
        switchUpdateMode, applyPollInterval, loadCertificates, updateCertDropdown,
        updateInstanceDropdown, addConnection, _saveConnections, _restoreConnections,
        healthCheckConnections, _fetchServerName, removeConnection, disconnectServer,
        showAddServerModal, closeAddServerModal, confirmAddServer, restartCommand,
        restartCommandById, spawnFromWelcome
    });
})();