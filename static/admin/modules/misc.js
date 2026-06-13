// ─── Miscellaneous ───
// UI Controls, Refresh Loop, Snapshot loading, Shared Toolbar.
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
/// When refreshMs > 0, the first call sets a timer; subsequent calls within
/// the window are coalesced.  When the timer fires, ALL panels with active
/// WS connections are flushed via HTTP to pick up the latest state.
function _throttleRefresh() {
    if (state.refreshMs <= 0) return false; // no throttle
    if (state._refreshThrottleTimer) return true; // already pending
    state._refreshThrottleTimer = setTimeout(() => {
        state._refreshThrottleTimer = null;
        _flushThrottledRefresh();
    }, state.refreshMs);
    return true;
}

/// Called when the throttle timer fires: fetch the latest VTTY state for
/// ALL panels that have an active command selection.  Uses per-panel
/// scheduleVttyHttpForPanel to avoid clobbering updates across panels.
function _flushThrottledRefresh() {
    for (const panelObj of state.panels) {
        if (panelObj.selectedInstUrl && panelObj.selectedCmdId) {
            scheduleVttyHttpForPanel(panelObj.id, panelObj.selectedInstUrl, panelObj.selectedCmdId, 0);
        }
    }
}

// ─── Selection Mode ───
// When active, mouse events are NOT forwarded to PTY, enabling native text selection.
// Also freezes VTTY DOM updates so text doesn't shift under the cursor.
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
        btn.textContent = panelObj.selectionMode ? '\u2713 Select' : 'Select';
    }
    // Freeze/thaw VTTY updates so text stays stable for selection
    if (panelObj.selectionMode) {
        stopPanelUpdateMode(panelId);
    } else if (panelObj.selectedInstUrl && panelObj.selectedCmdId) {
        startPanelUpdateMode(panelId);
    }
    // Update shared toolbar button if this is the active panel
    if (panelId === getActivePanelId()) {
        const stBtn = document.getElementById('stSelectBtn');
        if (stBtn) {
            stBtn.classList.toggle('btn-primary', panelObj.selectionMode);
            stBtn.textContent = panelObj.selectionMode ? '\u2713 Select' : 'Select';
        }
    }
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

    // Periodically re-check server reachability via /api/info so that
    // state.serverReachable stays accurate even when the server starts
    // after the page loaded.  Runs every 5 seconds (no need to be aggressive).
    if (state._serverConfigInterval) clearInterval(state._serverConfigInterval);
    state._serverConfigInterval = setInterval(() => {
        fetchServerConfig();
    }, 5000);

    // Start resource polling (every 2 seconds) — first poll fires immediately
    if (state._resourceInterval) clearInterval(state._resourceInterval);
    pollResources(); // immediate first poll
    state._resourceInterval = setInterval(pollResources, 2000);
}

    // ─── Notifications, Sound, Auto-Restart, Resource Polling ───
const _notifiedExits = new Set();

function notifyCommandEnded(cmdId) {
    if (!cmdId || _notifiedExits.has(cmdId)) return;
    _notifiedExits.add(cmdId);
    let cmdName = cmdId;
    let exitCode = null;
    for (const inst of state.connections) {
        if (inst._commands) {
            const cmd = inst._commands.find(c => c.id === cmdId);
            if (cmd) { cmdName = cmd.name || cmdId; exitCode = cmd.exit_code; break; }
        }
    }
    if (state.soundEnabled) playExitSound(exitCode === 0);
    if ('Notification' in window) {
        if (Notification.permission === 'granted') {
            new Notification('vrw: Command exited', { body: cmdName, icon: '/favicon.ico' });
        } else if (Notification.permission !== 'denied') {
            Notification.requestPermission().then(perm => {
                if (perm === 'granted') new Notification('vrw: Command exited', { body: cmdName, icon: '/favicon.ico' });
            });
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
    _autoRestartDebounce.set(cmdName, setTimeout(() => { _autoRestartDebounce.delete(cmdName); }, 10000));
    restartCommandById(instUrl, cmd.id).then(() => {
        const indicator = document.getElementById('autoRestartIndicator');
        if (indicator) {
            indicator.textContent = 'Auto-restarted: ' + cmdName;
            indicator.classList.remove('hidden');
            setTimeout(() => { indicator.classList.add('hidden'); }, 3000);
        }
    }).catch(() => {
        const t = _autoRestartDebounce.get(cmdName);
        if (t) { clearTimeout(t); _autoRestartDebounce.delete(cmdName); }
    });
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
            const isFrozen = cmd.frozen === true;
            const runtimeStr = cmd.runtime_secs > 0
                ? (cmd.runtime_secs < 60 ? Math.floor(cmd.runtime_secs) + 's'
                   : cmd.runtime_secs < 3600 ? Math.floor(cmd.runtime_secs / 60) + 'm ' + Math.floor(cmd.runtime_secs % 60) + 's'
                   : Math.floor(cmd.runtime_secs / 3600) + 'h ' + Math.floor((cmd.runtime_secs % 3600) / 60) + 'm')
                : '';
            const frozenBadge = isFrozen ? 'PAUSED' : '';
            const detailParts = [];
            if (runtimeStr) detailParts.push(runtimeStr);
            if (frozenBadge) detailParts.push(frozenBadge);
            if (res && res.cpu_percent != null) detailParts.push(res.cpu_percent.toFixed(1) + '%');
            if (res && res.memory_mb != null) {
                const mb = res.memory_mb;
                detailParts.push(mb >= 1024 ? (mb / 1024).toFixed(1) + 'G' : mb.toFixed(1) + 'M');
            }
            if (cmd.pid) detailParts.push(String(cmd.pid));
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
        if (success) { osc.frequency.value = 880; osc.type = 'sine'; }
        else { osc.frequency.value = 440; osc.type = 'square'; }
        gain.gain.value = 0.1;
        gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
        osc.start(ctx.currentTime);
        osc.stop(ctx.currentTime + 0.5);
    } catch (e) { /* ignore */ }
}

    // UI Controls
    window._syncRefreshMsUI = _syncRefreshMsUI;
    window.saveToken = saveToken;
    window.changeFontSize = changeFontSize;
    window.applyFontSize = applyFontSize;
    window.changePanelFontSize = changePanelFontSize;
    window.changeRefreshMs = changeRefreshMs;
    window.applyRefreshMs = applyRefreshMs;
    window.toggleSelectionMode = toggleSelectionMode;
    // Refresh loop
    window.startRefresh = startRefresh;
    // Notifications & Resources
    window.pollResources = pollResources;
    window.updateSidebarResourceText = updateSidebarResourceText;
    window.checkForExitedCommands = checkForExitedCommands;
    window.notifyCommandEnded = notifyCommandEnded;
    window.initSoundToggle = initSoundToggle;
    window.toggleSoundNotifications = toggleSoundNotifications;
    window.playExitSound = playExitSound;
    // Peer discovery
    window.fetchPeers = fetchPeers;
    window.addDiscoveredPeer = addDiscoveredPeer;
    window.savePeersToStorage = savePeersToStorage;
    window.handlePeerEvent = handlePeerEvent;
    // addConnection and removeConnection are exported by commands.js
})();

// ─── Peer Instances (registration & failover) ───
async function fetchPeers() {
    try {
        const json = await api.getPeers();
        if (json.status !== 'ok' || !Array.isArray(json.data)) return;

        for (const peer of json.data) {
            if (state.connections.some(i => i.url === peer.url)) continue;
            addDiscoveredPeer(peer.url, peer.label || peer.url, peer.token || '');
        }

        savePeersToStorage();

        if (json.data.length > 0) {
            loadCommands();
        }
    } catch (e) {
        // Not critical — peers can also be discovered via WS push
    }
}

function addDiscoveredPeer(url, label, token) {
    addConnection(url, label, token);
    console.log('[vrw] Peer discovered:', label, '(' + url + ')');
}

function handlePeerEvent(msg) {
    if (msg.type === 'peer_registered' && msg.data) {
        const { url, label, token } = msg.data;
        addDiscoveredPeer(url, label, token);
        savePeersToStorage();
    } else if (msg.type === 'peer_unregistered' && msg.data) {
        const { url } = msg.data;
        removeConnection(url);
        loadCommands();
        savePeersToStorage();
    }
}

function savePeersToStorage() {
    const peers = state.connections.filter(i => i.url !== window.location.origin);
    if (peers.length > 0) {
        try {
            localStorage.setItem('vrw_peers', JSON.stringify(
                peers.map(p => ({ url: p.url, label: p.label, token: p.token }))
            ));
        } catch (e) { /* quota exceeded — not critical */ }
    } else {
        localStorage.removeItem('vrw_peers');
    }
}
