// ─── Miscellaneous ───
'use strict';

function saveToken() {
    const val = document.getElementById('authToken').value.trim();
    state.authToken = val;
    val ? localStorage.setItem('vrw_auth_token', val) : localStorage.removeItem('vrw_auth_token');
}

function changeFontSize(delta) {
    state.fontSize = Math.max(2, Math.min(28, state.fontSize + delta));
    applyFontSize();
}

function applyFontSize() {
    document.documentElement.style.setProperty('--font-size', state.fontSize + 'px');
    const label = document.getElementById('fontSizeLabel');
    if (label) label.textContent = state.fontSize + 'px';
    localStorage.setItem('vrw_font_size', state.fontSize.toString());
}

function changePanelFontSize(panelId, delta) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    panelObj.fontSize = Math.max(2, Math.min(28, panelObj.fontSize + delta));
    localStorage.setItem('vrw_panel_font_' + panelId, panelObj.fontSize.toString());
    const vttyEl = document.getElementById('vtty-' + panelId);
    if (vttyEl) {
        vttyEl.style.fontSize = panelObj.fontSize + 'px';
        vttyEl.classList.toggle('thin-scrollbar', panelObj.fontSize < 10);
    }
    const stFontSize = document.getElementById('stFontSize');
    if (stFontSize && panelId === getActivePanelId()) stFontSize.textContent = panelObj.fontSize + 'px';
    const label = document.querySelector(`#${panelId} .panel-font-size`);
    if (label) label.textContent = panelObj.fontSize + 'px';
}

// ─── Refresh throttle (0 = off, 100–2000 = ms interval) ───
function _snapRefreshMs() {
    if (state.refreshMs > 0 && state.refreshMs % 100 !== 0)
        state.refreshMs = Math.round(state.refreshMs / 100) * 100;
}

function changeRefreshMs(delta) {
    state.refreshMs = Math.max(0, Math.min(2000, state.refreshMs + delta));
    _snapRefreshMs();
    localStorage.setItem('vrw_refresh_ms', state.refreshMs.toString());
    _syncRefreshMsUI();
}

function applyRefreshMs() {
    state.refreshMs = Math.max(0, Math.min(2000, parseInt(document.getElementById('refreshMs').value) || 0));
    _snapRefreshMs();
    localStorage.setItem('vrw_refresh_ms', state.refreshMs.toString());
    document.getElementById('refreshMs').value = state.refreshMs;
    _syncRefreshMsUI();
}

function _syncRefreshMsUI() {
    const input = document.getElementById('refreshMs');
    if (input) input.value = state.refreshMs;
    document.querySelectorAll('.refresh-val').forEach(el => { el.textContent = state.refreshMs || 'off'; });
}

function _throttleRefresh() {
    if (state.refreshMs <= 0) return false;
    if (state._refreshThrottleTimer) return true;
    state._refreshThrottleTimer = setTimeout(() => {
        state._refreshThrottleTimer = null;
        _flushThrottledRefresh();
    }, state.refreshMs);
    return true;
}

function _flushThrottledRefresh() {
    for (const p of state.panels)
        if (p.selectedInstUrl && p.selectedCmdId)
            scheduleVttyHttpForPanel(p.id, p.selectedInstUrl, p.selectedCmdId, 0);
}

function _updateSelectBtn(btn, active) {
    if (!btn) return;
    btn.classList.toggle('btn-primary', active);
    btn.textContent = active ? '\u2713 Select' : 'Select';
}

function toggleSelectionMode(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    panelObj.selectionMode = !panelObj.selectionMode;
    localStorage.setItem('vrw_panel_sel_' + panelId, panelObj.selectionMode.toString());
    const vttyEl = document.getElementById('vtty-' + panelId);
    if (vttyEl) vttyEl.classList.toggle('selection-mode', panelObj.selectionMode);
    _updateSelectBtn(document.getElementById('selectBtn-' + panelId), panelObj.selectionMode);
    if (panelObj.selectionMode) stopPanelUpdateMode(panelId);
    else if (panelObj.selectedInstUrl && panelObj.selectedCmdId) startPanelUpdateMode(panelId);
    if (panelId === getActivePanelId()) _updateSelectBtn(document.getElementById('stSelectBtn'), panelObj.selectionMode);
}

// ─── Refresh Loop ───
function startRefresh() {
    if (state._serverConfigInterval) clearInterval(state._serverConfigInterval);
    state._serverConfigInterval = setInterval(fetchServerConfig, 5000);
    if (state._resourceInterval) clearInterval(state._resourceInterval);
    pollResources();
    state._resourceInterval = setInterval(pollResources, 2000);
    loadSnapshot().then(() => {
        if (state.refreshInterval) clearInterval(state.refreshInterval);
        state.refreshInterval = setInterval(() => { loadCommands(); checkForExitedCommands(); }, 1000);
    });
}

const _notifiedExits = new Set();

function notifyCommandEnded(cmdId) {
    if (!cmdId || _notifiedExits.has(cmdId)) return;
    _notifiedExits.add(cmdId);
    let cmdName = cmdId, exitCode = null;
    for (const inst of state.connections) {
        if (inst._commands) {
            const cmd = inst._commands.find(c => c.id === cmdId);
            if (cmd) { cmdName = cmd.name || cmdId; exitCode = cmd.exit_code; break; }
        }
    }
    if (state.soundEnabled) playExitSound(exitCode === 0);
    if ('Notification' in window) {
        const opts = { body: cmdName, icon: '/favicon.ico' };
        if (Notification.permission === 'granted') {
            new Notification('vrw: Command exited', opts);
        } else if (Notification.permission !== 'denied') {
            Notification.requestPermission().then(p => { if (p === 'granted') new Notification('vrw: Command exited', opts); });
        }
    }
}

const _autoRestartDebounce = new Map();

function checkForExitedCommands() {
    const pinnedNames = getPinnedNames();
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false && !_notifiedExits.has(cmd.id)) {
                notifyCommandEnded(cmd.id);
                const cmdName = cmd.name || cmd.id;
                if (pinnedNames.includes(cmdName)) _autoRestartCommand(inst.url, cmd, cmdName);
            }
        }
    }
}

function _autoRestartCommand(instUrl, cmd, cmdName) {
    if (_autoRestartDebounce.has(cmdName)) return;
    _autoRestartDebounce.set(cmdName, setTimeout(() => _autoRestartDebounce.delete(cmdName), 10000));
    restartCommandById(instUrl, cmd.id).then(() => {
        const el = document.getElementById('autoRestartIndicator');
        if (el) { el.textContent = 'Auto-restarted: ' + cmdName; el.classList.remove('hidden'); setTimeout(() => el.classList.add('hidden'), 3000); }
    }).catch(() => { const t = _autoRestartDebounce.get(cmdName); if (t) { clearTimeout(t); _autoRestartDebounce.delete(cmdName); } });
}

async function pollResources() {
    const promises = [];
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false) continue;
            promises.push((async () => {
                try {
                    const json = await api.getCommandResources(inst.url, cmd.id);
                    if (json.status === 'ok' && json.data) state._resourceCache[cmd.id] = json.data;
                } catch (e) { /* silent */ }
            })());
        }
    }
    await Promise.all(promises);
    updateSidebarResourceText();
}

function updateSidebarResourceText() {
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false) continue;
            const res = state._resourceCache[cmd.id];
            const item = document.querySelector(`.cmd-item[data-cmd-id="${cmd.id}"]`);
            if (!item) continue;
            const parts = [];
            if (cmd.runtime_secs > 0) parts.push(formatRuntime(cmd.runtime_secs));
            if (cmd.frozen === true) parts.push('PAUSED');
            if (res && res.cpu_percent != null) parts.push(res.cpu_percent.toFixed(1) + '%');
            if (res && res.memory_mb != null) {
                const mb = res.memory_mb;
                parts.push(mb >= 1024 ? (mb / 1024).toFixed(1) + 'G' : mb.toFixed(1) + 'M');
            }
            let detailRow = item.querySelector('.cmd-detail-row');
            if (!parts.length) { if (detailRow) detailRow.remove(); }
            else {
                if (!detailRow) { detailRow = document.createElement('div'); detailRow.className = 'cmd-detail-row'; item.appendChild(detailRow); }
                detailRow.innerHTML = parts.join(' · ');
            }
        }
    }
}

function initSoundToggle() { const b = document.getElementById('soundBtn'); if (b && state.soundEnabled) b.classList.add('sound-btn-active'); }

function toggleSoundNotifications() {
    state.soundEnabled = !state.soundEnabled;
    localStorage.setItem('vrw_sound', state.soundEnabled.toString());
    const b = document.getElementById('soundBtn');
    if (b) b.classList.toggle('sound-btn-active', state.soundEnabled);
}

function playExitSound(success) {
    try {
        const ctx = new (window.AudioContext || window.webkitAudioContext)();
        const osc = ctx.createOscillator(), gain = ctx.createGain();
        osc.connect(gain); gain.connect(ctx.destination);
        osc.frequency.value = success ? 880 : 440;
        osc.type = success ? 'sine' : 'square';
        gain.gain.value = 0.1;
        gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
        osc.start(ctx.currentTime); osc.stop(ctx.currentTime + 0.5);
    } catch (e) { /* ignore */ }
}

// ─── Peer Instances ───
async function fetchPeers() {
    try {
        const json = await api.getPeers();
        if (json.status !== 'ok' || !Array.isArray(json.data)) return;
        for (const peer of json.data) {
            if (!state.connections.some(i => i.url === peer.url))
                addDiscoveredPeer(peer.url, peer.label || peer.url, peer.token || '');
        }
        savePeersToStorage();
        if (json.data.length) loadCommands();
    } catch (e) { /* WS push fallback */ }
}

function addDiscoveredPeer(url, label, token) { addConnection(url, label, token); console.log('[vrw] Peer discovered:', label, '(' + url + ')'); }

function handlePeerEvent(msg) {
    if (msg.type === 'peer_registered' && msg.data) {
        const { url, label, token } = msg.data;
        addDiscoveredPeer(url, label, token); savePeersToStorage();
    } else if (msg.type === 'peer_unregistered' && msg.data) {
        removeConnection(msg.data.url); loadCommands(); savePeersToStorage();
    }
}

function savePeersToStorage() {
    const peers = state.connections.filter(i => i.url !== window.location.origin);
    if (peers.length) {
        try { localStorage.setItem('vrw_peers', JSON.stringify(peers.map(p => ({ url: p.url, label: p.label, token: p.token })))); } catch (e) { /* quota */ }
    } else localStorage.removeItem('vrw_peers');
}

// ─── Command Templates ───
let _serverTemplates = [];
function getServerTemplates() { return _serverTemplates; }

async function fetchServerTemplates() {
    try { const j = await api.getTemplates(); if (j.status === 'ok') _serverTemplates = j.data || []; } catch { /* cached */ }
}

function getUserTemplates() { try { return JSON.parse(localStorage.getItem('vrw_templates') || '[]'); } catch { return []; } }
function saveUserTemplates(t) { localStorage.setItem('vrw_templates', JSON.stringify(t)); }

function _spawnFromTemplate(t, extraBody) {
    const instUrl = (document.getElementById('spawnInstance') || {}).value || window._userSpawnInstUrl || getBaseUrl();
    const body = Object.assign({ cmd: t.cmd, args: t.args ? t.args.split(/\s+/) : [] }, extraBody);
    api.spawnCommand(instUrl, body).then(json => {
        if (json.status === 'ok') {
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) { state.selectedInstUrl = instUrl; _cacheTerminalForSwitch(); state._pendingSelectId = newId; }
            loadCommands();
            const cmdTab = document.querySelector('.sidebar-tab');
            if (cmdTab) switchSidebarTab('commands', cmdTab);
        } else alert('Spawn failed: ' + (json.error || 'unknown'));
    }).catch(e => alert('Spawn failed: ' + e.message));
}

function spawnServerTemplate(index) {
    const t = getServerTemplates()[index];
    if (!t) return;
    const extra = {};
    if (t.env && t.env.length) {
        const envObj = {};
        for (const entry of t.env) { const eq = entry.indexOf('='); if (eq > 0) envObj[entry.substring(0, eq)] = entry.substring(eq + 1); }
        extra.env = envObj;
    }
    if (t.workdir) extra.workdir = t.workdir;
    if (t.certificate) extra.certificate = t.certificate;
    if (t.rows) extra.rows = t.rows;
    if (t.cols) extra.cols = t.cols;
    _spawnFromTemplate(t, extra);
}

function spawnUserTemplate(index) { const t = getUserTemplates()[index]; if (t) _spawnFromTemplate(t, {}); }

function deleteUserTemplate(index) { const t = getUserTemplates(); t.splice(index, 1); saveUserTemplates(t); renderTemplates(); }
function showAddTemplateForm() { const f = document.getElementById('templateAddForm'); if (f) f.classList.remove('hidden'); }

function hideAddTemplateForm() {
    const f = document.getElementById('templateAddForm');
    if (f) f.classList.add('hidden');
    document.getElementById('templateName').value = '';
    document.getElementById('templateCmd').value = '';
    document.getElementById('templateArgs').value = '';
}

function saveTemplate() {
    const name = document.getElementById('templateName').value.trim();
    const cmd = document.getElementById('templateCmd').value.trim();
    const args = document.getElementById('templateArgs').value.trim();
    if (!name || !cmd) { alert('Name and command are required'); return; }
    const t = getUserTemplates(); t.push({ name, cmd, args }); saveUserTemplates(t); hideAddTemplateForm(); renderTemplates();
}

function renderTemplates() {
    const container = document.getElementById('templateList');
    if (!container) return;
    const server = getServerTemplates(), user = getUserTemplates();
    if (!server.length && !user.length) {
        container.innerHTML = '<div style="padding:0.5rem;color:var(--text-muted);font-size:0.7rem;text-align:center;">No templates configured. Add templates in your config file under [[templates]].</div>';
        return;
    }
    let html = '';
    if (server.length) {
        html += '<div style="font-size:0.6rem;color:var(--text-muted);padding:0.2rem 0.3rem;text-transform:uppercase;letter-spacing:0.05em;">From config</div>';
        html += server.map((t, i) => {
            const extras = [];
            if (t.workdir) extras.push('dir: ' + t.workdir);
            if (t.certificate) extras.push('cert: ' + t.certificate);
            if (t.rows || t.cols) extras.push((t.rows || '?') + 'x' + (t.cols || '?'));
            return `<div class="template-card" data-action="SpawnServerTemplate" data-index="${i}" title="Click to spawn"><div style="display:flex;align-items:center;gap:0.3rem;"><div class="template-name">${escHtml(t.name)}</div><span style="font-size:0.5rem;background:var(--accent);color:var(--bg-primary);padding:0 0.25rem;border-radius:2px;">config</span></div><div class="template-cmd">${escHtml([t.cmd, t.args].filter(Boolean).join(' '))}</div>${extras.length ? `<div style="font-size:0.6rem;color:var(--text-muted);padding-left:0.2rem;">${escHtml(extras.join(' | '))}</div>` : ''}</div>`;
        }).join('');
    }
    if (user.length) {
        html += '<div style="font-size:0.6rem;color:var(--text-muted);padding:0.3rem 0.3rem 0.1rem;text-transform:uppercase;letter-spacing:0.05em;">Custom</div>';
        html += user.map((t, i) => `<div class="template-card" data-action="SpawnUserTemplate" data-index="${i}" title="Click to spawn"><div class="template-name">${escHtml(t.name)}</div><div class="template-cmd">${escHtml(t.cmd)}${t.args ? ' ' + escHtml(t.args) : ''}</div><div class="template-actions"><button class="btn btn-xs btn-danger" data-action="DeleteUserTemplate" data-index="${i}" title="Delete">&#x2715;</button></div></div>`).join('');
    }
    container.innerHTML = html;
}

// ─── Log Viewer ───
function _updateLogTransportIndicator(mode) {
    const el = document.getElementById('logTransportIndicator');
    if (el) { el.textContent = mode.toUpperCase(); el.dataset.mode = mode; }
}

function _cleanupLogWs(ws) {
    _updateLogTransportIndicator('http');
    clearInterval(state._logWsPingTimer); state._logWsPingTimer = null;
    if (state.logWs === ws) state.logWs = null;
    _scheduleLogWsReconnect();
}

function connectLogWs() {
    if (state.logWs && state.logWs.readyState === WebSocket.OPEN) return;
    disconnectLogWs();
    const wsUrl = getBaseUrl().replace(/^http/, 'ws');
    const token = state.authToken || (state.connections[0] || {}).token || '';
    const url = `${wsUrl}/api/ws/logs${token ? '?token=' + encodeURIComponent(token) : ''}`;
    try {
        const ws = new WebSocket(url);
        state.logWs = ws;
        ws.onopen = () => {
            state._logWsReconnectAttempts = 0;
            _updateLogTransportIndicator('ws');
            const c = document.getElementById('logContent');
            if (c && c.querySelector('.log-line')) {
                const d = document.createElement('div');
                d.className = 'log-line log-ws-indicator';
                d.innerHTML = '<span class="timestamp">[' + new Date().toISOString().replace('T', ' ').replace(/\.\d+Z$/, '') + ']</span> <span class="details" style="color:var(--green);">Connected to log stream</span>';
                c.appendChild(d); _autoScrollLog(c);
            }
            clearInterval(state._logWsPingTimer);
            state._logWsPingTimer = setInterval(() => { if (state.logWs && state.logWs.readyState === WebSocket.OPEN) state.logWs.send(JSON.stringify({ type: 'ping' })); }, 15000);
        };
        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                if (msg.type === 'log_entry' && msg.data) {
                    const c = document.getElementById('logContent');
                    if (!c) return;
                    const ph = c.querySelector('[style*="text-align:center"]');
                    if (ph && !ph.classList.contains('log-line')) ph.remove();
                    const div = document.createElement('div');
                    div.className = 'log-line';
                    div.innerHTML = formatLogLine(parseLogLine(msg.data), msg.data);
                    c.appendChild(div); _autoScrollLog(c);
                    const countEl = document.getElementById('logCount');
                    if (countEl) countEl.textContent = `${c.querySelectorAll('.log-line').length} lines (streaming)`;
                }
            } catch (e) { console.error('Log WS parse error:', e); }
        };
        ws.onclose = () => _cleanupLogWs(ws);
        ws.onerror = () => _cleanupLogWs(ws);
    } catch (e) { console.error('Log WS connect failed:', e); _updateLogTransportIndicator('http'); _scheduleLogWsReconnect(); }
}

function _scheduleLogWsReconnect() {
    if (state.logWsReconnectTimer || state.currentView !== 'log') return;
    const delay = Math.min(1000 * Math.pow(2, state._logWsReconnectAttempts), 30000);
    state._logWsReconnectAttempts++;
    state.logWsReconnectTimer = setTimeout(() => { state.logWsReconnectTimer = null; if (state.currentView === 'log') connectLogWs(); }, delay);
}

function disconnectLogWs() {
    if (state.logWsReconnectTimer) { clearTimeout(state.logWsReconnectTimer); state.logWsReconnectTimer = null; }
    clearInterval(state._logWsPingTimer); state._logWsPingTimer = null;
    if (state.logWs) { state.logWs.onclose = null; state.logWs.onerror = null; state.logWs.close(); state.logWs = null; }
    _updateLogTransportIndicator('http');
}

function _autoScrollLog(c) { if (c.scrollHeight - c.scrollTop - c.clientHeight < 50) c.scrollTop = c.scrollHeight; }

async function loadLog() {
    _updateLogTransportIndicator('http');
    try {
        const search = document.getElementById('logSearch').value;
        const params = new URLSearchParams();
        if (search) params.set('search', search);
        params.set('limit', '500');
        const json = await api.getLog(undefined, Object.fromEntries(params));
        if (json.status === 'ok' && json.data) {
            const c = document.getElementById('logContent');
            const lines = json.data.lines || [];
            document.getElementById('logCount').textContent = `${json.data.filtered_lines}/${json.data.total_lines} lines`;
            if (!lines.length) {
                c.innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;">No log entries found.' + (json.data.message ? ' ' + json.data.message : '') + '</div>';
            } else {
                c.innerHTML = lines.map(line => {
                    const cls = search && line.toLowerCase().includes(search.toLowerCase()) ? ' highlight' : '';
                    return `<div class="log-line${cls}">${formatLogLine(parseLogLine(line), line)}</div>`;
                }).join('');
                c.scrollTop = c.scrollHeight;
            }
            if (!search) connectLogWs();
        }
    } catch (e) {
        document.getElementById('logContent').innerHTML = `<div style="padding:1rem;color:var(--red);">Failed to load log: ${escHtml(e.message)}</div>`;
    }
}

function parseLogLine(line) {
    const m = line.match(/^\[([^\]]+)\]\s+(\w+):\s+(.*)$/);
    return m ? { timestamp: m[1], command: m[2], details: m[3], raw: line } : { timestamp: '', command: '', details: line, raw: line };
}

function formatLogLine(p, raw) {
    return p.timestamp ? `<span class="timestamp">[${escHtml(p.timestamp)}]</span> <span class="cmd-type">${escHtml(p.command)}</span> <span class="details">${escHtml(p.details)}</span>` : escHtml(raw);
}

function searchLogs() { disconnectLogWs(); state._logWsReconnectAttempts = 0; loadLog(); }

function clearLogSearch() {
    document.getElementById('logSearch').value = '';
    loadLog();
    clearTimeout(state._logSearchReconnectTimer);
    state._logSearchReconnectTimer = setTimeout(() => { state._logSearchReconnectTimer = null; if (state.currentView === 'log') connectLogWs(); }, 500);
}

// ─── Keyboard Shortcuts Panel ───
// Must be defined before keyboard.js loads (keyboard.js references
// window.showShortcuts and window.closeShortcuts in _defaultShortcuts).
function showShortcuts() {
    closeContextMenu();
    window.closeShortcuts();
    const overlay = document.createElement('div');
    overlay.className = 'shortcuts-overlay';
    overlay.id = 'shortcutsOverlay';

    const shortcuts = (typeof _defaultShortcuts !== 'undefined') ? _defaultShortcuts : [];
    let rows = '';
    for (const s of shortcuts) {
        let keys = [];
        if (s.prefix) keys.push('Ctrl+A');
        if (s.ctrl) keys.push('Ctrl');
        if (s.shift && typeof s.shift !== 'string') keys.push('Shift');
        if (s.alt) keys.push('Alt');
        if (s.meta) keys.push('Meta');
        const keyLabel = (s.shift && typeof s.shift === 'string') ? s.shift : s.key;
        keys.push(keyLabel);
        rows += '<tr><td>' + escHtml(keys.join('+')) + '</td><td>' + escHtml(s.label || '') + '</td></tr>';
    }

    overlay.innerHTML = '<div class="shortcuts-panel"><h2>Keyboard Shortcuts</h2><div class="shortcuts-scroll"><table>' + rows + '</table></div><div class="shortcuts-footer"><button class="btn" data-action="CloseShortcuts">Close</button></div></div>';
    overlay.addEventListener('click', (e) => { if (e.target === overlay) closeShortcuts(); });
    document.body.appendChild(overlay);
}

function closeShortcuts() {
    const overlay = document.getElementById('shortcutsOverlay');
    if (overlay) overlay.remove();
}

Object.assign(window, {
    _syncRefreshMsUI, saveToken, changeFontSize, applyFontSize, changePanelFontSize,
    changeRefreshMs, applyRefreshMs, toggleSelectionMode, startRefresh,
    pollResources, updateSidebarResourceText, checkForExitedCommands,
    notifyCommandEnded, initSoundToggle, toggleSoundNotifications, playExitSound,
    fetchPeers, addDiscoveredPeer, savePeersToStorage, handlePeerEvent,
    fetchServerTemplates, getServerTemplates, getUserTemplates, saveUserTemplates,
    renderTemplates, spawnServerTemplate, spawnUserTemplate, deleteUserTemplate,
    showAddTemplateForm, hideAddTemplateForm, saveTemplate,
    connectLogWs, disconnectLogWs, loadLog, searchLogs, clearLogSearch,
    _updateLogTransportIndicator, _scheduleLogWsReconnect,
    showShortcuts, closeShortcuts,
});