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

    // UI Controls
    window.saveToken = saveToken;
    window.changeFontSize = changeFontSize;
    window.applyFontSize = applyFontSize;
    window.changePanelFontSize = changePanelFontSize;
    window.changeRefreshMs = changeRefreshMs;
    window.applyRefreshMs = applyRefreshMs;
    window.toggleSelectionMode = toggleSelectionMode;
    // Refresh loop
    window.startRefresh = startRefresh;
    // Peer discovery
    window.fetchPeers = fetchPeers;
    window.addDiscoveredPeer = addDiscoveredPeer;
    window.savePeersToStorage = savePeersToStorage;
    window.handlePeerEvent = handlePeerEvent;
    window.addConnection = addConnection;
    window.removeConnection = removeConnection;
})();

// ─── Peer Instances (registration & failover) ───
async function fetchPeers() {
    try {
        const res = await fetch(apiUrl('/api/peers'), { headers: authHeaders() });
        if (!res.ok) return;
        const json = await res.json();
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
