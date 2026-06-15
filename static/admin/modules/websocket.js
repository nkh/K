// ─── WebSocket Management ───
// Pure WS lifecycle: connect, disconnect, ping/pong, reconnect, poll.
// All VTTY display logic lives in vtty.js.
(function() {
    'use strict';

// ─── Shared Helpers ───

function _buildWsUrl(instUrl, cmdId) {
    const wsUrl = instUrl.replace(/^http/, 'ws');
    const token = state.authToken || (state.connections.find(i => i.url === instUrl) || {}).token || '';
    const sep = token ? '?' : '';
    return `${wsUrl}/api/commands/${cmdId}/ws${sep}${token ? 'token=' + encodeURIComponent(token) : ''}`;
}

/// Generic WS cleanup: clears timers, resets counters, closes socket, nulls properties.
function _cleanupWs(obj, prefix) {
    const t = obj[prefix + 'ReconnectTimer'];
    if (t) { clearTimeout(t); obj[prefix + 'ReconnectTimer'] = null; }
    clearInterval(obj[prefix + 'PingInterval']);
    obj[prefix + 'PingInterval'] = null;
    obj[prefix + 'PingSendTime'] = 0;
    obj[prefix + 'Latency'] = 0;
    obj[prefix + 'ReconnectCount'] = 0;
    const ws = obj[prefix];
    if (ws) {
        ws.onclose = null;
        ws.close();
        obj[prefix] = null;
        obj[prefix + 'InstUrl'] = null;
        obj[prefix + 'CmdId'] = null;
    }
}

/// Shared WS setup: creates WebSocket, sets up ping/pong, reconnect, and
/// message dispatch. Returns the WebSocket instance.
/// @param {object} obj      - The state object holding WS properties (panelObj or panelObj.split)
/// @param {string} prefix   - Property prefix ('ws' for primary, 'secondaryWs' for secondary)
/// @param {string} instUrl  - Server instance URL
/// @param {string} cmdId    - Command ID
/// @param {object} opts    - { onVtty, onEnded, onPong, onPeer, onError, isFocused, reconnectGuard }
function _setupWs(obj, prefix, instUrl, cmdId, opts) {
    _cleanupWs(obj, prefix);
    const url = _buildWsUrl(instUrl, cmdId);
    const ws = new WebSocket(url);
    obj[prefix] = ws;
    obj[prefix + 'InstUrl'] = instUrl;
    obj[prefix + 'CmdId'] = cmdId;

    ws.onopen = () => {
        clearInterval(obj[prefix + 'PingInterval']);
        obj[prefix + 'PingInterval'] = setInterval(() => {
            if (obj[prefix] && obj[prefix].readyState === WebSocket.OPEN) {
                obj[prefix + 'PingSendTime'] = Date.now();
                obj[prefix].send(JSON.stringify({ type: 'ping' }));
            }
        }, 10000);
        if (opts.isFocused) {
            document.getElementById('connStatus').textContent = 'WS Connected';
            updateWsQualityIndicator();
        }
    };

    ws.onmessage = (event) => {
        try {
            const msg = JSON.parse(event.data);
            switch (msg.type) {
                case 'vtty_dirty':
                    if (opts.onVtty) opts.onVtty(msg);
                    break;
                case 'vtty_close':
                    if (opts.onVtty) opts.onVtty(msg);
                    break;
                case 'command_ended':
                    if (opts.isFocused) {
                        document.getElementById('connStatus').textContent = 'Command ended';
                    }
                    _cleanupWs(obj, prefix);
                    if (opts.onEnded) opts.onEnded();
                    break;
                case 'pong':
                    if (obj[prefix + 'PingSendTime'] > 0) {
                        obj[prefix + 'Latency'] = Date.now() - obj[prefix + 'PingSendTime'];
                        obj[prefix + 'PingSendTime'] = 0;
                        if (opts.onPong) opts.onPong(obj[prefix + 'Latency']);
                    }
                    break;
                case 'peer_registered':
                case 'peer_unregistered':
                    if (opts.onPeer) opts.onPeer(msg);
                    break;
            }
        } catch (e) {
            console.error('WS message parse error:', e);
        }
    };

    ws.onclose = () => {
        if (obj[prefix] !== ws) return;
        obj[prefix] = null;
        clearInterval(obj[prefix + 'PingInterval']);
        obj[prefix + 'PingInterval'] = null;
        obj[prefix + 'PingSendTime'] = 0;
        obj[prefix + 'Latency'] = 0;
        if (opts.isFocused) {
            document.getElementById('connStatus').textContent = 'WS Disconnected';
            updateWsQualityIndicator();
        }
        // Schedule HTTP fallback
        if (opts.onDisconnect) opts.onDisconnect();
        // Auto-reconnect (max 5 attempts)
        if (instUrl && cmdId && !obj[prefix + 'ReconnectTimer']) {
            obj[prefix + 'ReconnectCount']++;
            if (obj[prefix + 'ReconnectCount'] <= 5) {
                obj[prefix + 'ReconnectTimer'] = setTimeout(() => {
                    obj[prefix + 'ReconnectTimer'] = null;
                    if (opts.reconnectGuard && opts.reconnectGuard()) {
                        const inst = state.connections.find(i => i.url === instUrl);
                        if (inst && inst.reachable !== false) {
                            _setupWs(obj, prefix, instUrl, cmdId, opts);
                        }
                    }
                }, 2000);
            }
        }
    };

    ws.onerror = (err) => {
        console.error('WebSocket error:', err);
        if (opts.onError) opts.onError(err);
    };

    return ws;
}

// ─── Per-Panel WebSocket Management ───

function connectPanelWs(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || !panelObj.selectedInstUrl || !panelObj.selectedCmdId) return;

    disconnectPanelWs(panelId);

    const instUrl = panelObj.selectedInstUrl;
    const cmdId = panelObj.selectedCmdId;
    const isFocused = panelObj.id === state._focusedPanelId;

    try {
        _setupWs(panelObj, 'ws', instUrl, cmdId, {
            isFocused,
            onVtty(msg) {
                if (msg.type === 'vtty_dirty') {
                    fetchVttyDiffForPanel(panelObj.id, instUrl, cmdId);
                } else if (msg.type === 'vtty_close') {
                    // Terminal closed — discard baseline so a reconnection starts fresh
                    delete state._diffBaselines[cmdId];
                }
            },
            onEnded() {
                delete state._diffBaselines[cmdId];
                notifyCommandEnded(cmdId);
            },
            onPong(latency) {
                if (isFocused) {
                    updateWsQualityIndicator();
                    const connEl = document.getElementById('connStatus');
                    if (connEl) connEl.textContent = 'Connected (' + latency + 'ms)';
                }
            },
            onPeer(msg) { handlePeerEvent(msg); },
            onDisconnect() {
                if (panelObj.selectedInstUrl && panelObj.selectedCmdId) {
                    fetchVttyDiffForPanel(panelObj.id, panelObj.selectedInstUrl, panelObj.selectedCmdId, 0);
                }
            },
            reconnectGuard() {
                return panelObj.selectedInstUrl && panelObj.selectedCmdId && state.updateMode === 'push';
            }
        });
    } catch (e) {
        console.error('WebSocket connect failed (panel ' + panelId + '):', e);
    }

    // Fetch initial terminal content via diff endpoint (no baseline yet).
    fetchVttyDiffForPanel(panelObj.id, instUrl, cmdId, 0);

    // Also connect secondary WS if panel is split
    if (panelObj.split && panelObj.split.secondaryCmdId && panelObj.split.secondaryInstUrl) {
        _connectSecondaryWs(panelObj);
    }
}

function disconnectPanelWs(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    _cleanupWs(panelObj, 'ws');
    if (panelObj.split) {
        _disconnectSecondaryWs(panelObj);
        if (panelObj.split.secondaryPollTimer) {
            clearInterval(panelObj.split.secondaryPollTimer);
            panelObj.split.secondaryPollTimer = null;
        }
    }
}

// ─── WebSocket Connection Quality Indicator ───
function updateWsQualityIndicator() {
    const el = document.getElementById('wsQuality');
    if (!el) return;

    const focusedPanel = state.panels.find(p => p.id === state._focusedPanelId);
    const latency = focusedPanel ? focusedPanel.wsLatency : 0;
    const reconnects = focusedPanel ? focusedPanel.wsReconnectCount : 0;
    const isConnected = focusedPanel && focusedPanel.ws && focusedPanel.ws.readyState === WebSocket.OPEN;

    if (!isConnected && latency === 0) {
        el.textContent = '--';
        el.style.color = 'var(--red)';
        el.title = 'Disconnected';
        return;
    }

    let color;
    if (latency === 0) color = 'var(--text-muted)';
    else if (latency < 50) color = 'var(--green)';
    else if (latency < 200) color = 'var(--yellow)';
    else color = 'var(--red)';

    el.textContent = latency > 0 ? latency + 'ms' : '...';
    el.style.color = color;
    el.title = 'Latency: ' + (latency > 0 ? latency + 'ms' : 'measuring...') + ' | Reconnects: ' + reconnects;
}

// ─── Poll Mode ───
function startPanelPoll(panelId) {
    stopPanelPoll(panelId);
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || !panelObj.selectedInstUrl || !panelObj.selectedCmdId) return;
    panelObj.pollTimer = setInterval(() => pollOncePanel(panelId), state.pollInterval);
    pollOncePanel(panelId);
}

function stopPanelPoll(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    if (panelObj.pollTimer) { clearInterval(panelObj.pollTimer); panelObj.pollTimer = null; }
}

async function pollOncePanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || !panelObj.selectedInstUrl || !panelObj.selectedCmdId) return;
    try {
        const json = await api.getVttyChanged(panelObj.selectedInstUrl, panelObj.selectedCmdId);
        if (json.status === 'ok' && json.data && json.data.changed) {
            loadVttyHttpForPanel(panelId, panelObj.selectedInstUrl, panelObj.selectedCmdId);
        }
    } catch (e) { /* next poll will retry */ }
}

// ─── Secondary WebSocket for Split Panels ───

function _disconnectSecondaryWs(panelObj) {
    if (!panelObj || !panelObj.split) return;
    _cleanupWs(panelObj.split, 'secondaryWs');
}

function _connectSecondaryWs(panelObj) {
    if (!panelObj || !panelObj.split) return;
    const s = panelObj.split;
    if (!s.secondaryCmdId || !s.secondaryInstUrl) return;
    _disconnectSecondaryWs(panelObj);

    try {
        _setupWs(s, 'secondaryWs', s.secondaryInstUrl, s.secondaryCmdId, {
            isFocused: false,
            onVtty(msg) {
                if (msg.type === 'vtty_dirty') {
                    fetchSecondaryVttyDiff(panelObj);
                }
            },
            onEnded() {
                delete state._diffBaselines[s.secondaryCmdId];
                notifyCommandEnded(s.secondaryCmdId);
            },
            onPeer(msg) { handlePeerEvent(msg); },
            onDisconnect() {
                if (s.secondaryInstUrl && s.secondaryCmdId) fetchSecondaryVttyDiff(panelObj, 0);
            },
            reconnectGuard() {
                return s.secondaryInstUrl && s.secondaryCmdId && state.updateMode === 'push';
            }
        });
    } catch (e) {
        console.error('Secondary WebSocket connect failed (panel ' + panelObj.id + '):', e);
    }

    // Fetch initial terminal content for secondary
    fetchSecondaryVttyDiff(panelObj, 0);
}

// ─── Diff fetch for push mode ───
// Debounce timer per panel to avoid hammering the server.
const _diffTimers = {};

function fetchVttyDiffForPanel(panelId, instUrl, cmdId, delayMs) {
    const timerKey = '_diffTimer_' + panelId;
    if (_diffTimers[timerKey]) clearTimeout(_diffTimers[timerKey]);
    _diffTimers[timerKey] = setTimeout(() => {
        _diffTimers[timerKey] = null;
        _doFetchVttyDiff(panelId, instUrl, cmdId, false);
    }, delayMs);
}

async function _doFetchVttyDiff(panelId, instUrl, cmdId, isSecondary) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;

    const baseline = state._diffBaselines[cmdId] || null;
    try {
        const json = await api.getVttyDiff(instUrl, cmdId, baseline);
        if (json.status === 'ok' && json.data) {
            // Store the baseline UUID for next request
            state._diffBaselines[cmdId] = json.data.baseline;

            if (json.data.full_sync_required) {
                // Too many cells changed — fetch full HTML instead
                const htmlJson = await api.getVttyHtml(instUrl, cmdId);
                if (htmlJson.status === 'ok' && htmlJson.data) {
                    // Reset baseline since we skipped the diff
                    state._diffBaselines[cmdId] = null;
                    const htmlResp = await api.getVttyDiff(instUrl, cmdId, null);
                    if (htmlResp.status === 'ok' && htmlResp.data) {
                        state._diffBaselines[cmdId] = htmlResp.data.baseline;
                        htmlResp.data.html = htmlJson.data.html;
                        if (isSecondary) {
                            const vttyEl = document.getElementById('vtty-' + panelId + '-secondary');
                            if (vttyEl) updateSecondaryVttyDisplay(panelObj, vttyEl, htmlResp.data);
                        } else {
                            updateVttyDisplayForPanel(panelObj, panelEl, htmlResp.data);
                        }
                    }
                }
            } else if (isSecondary) {
                const vttyEl = document.getElementById('vtty-' + panelId + '-secondary');
                if (vttyEl) applySecondaryVttyDiff(panelObj, vttyEl, json.data);
            } else {
                applyVttyDiffForPanel(panelObj, panelEl, json.data);
            }
        }
    } catch (e) {
        // Silently ignore — next dirty signal will retry
    }
}

function fetchSecondaryVttyDiff(panelObj, delayMs) {
    if (!panelObj || !panelObj.split) return;
    const s = panelObj.split;
    if (!s.secondaryCmdId || !s.secondaryInstUrl) return;
    const timerKey = '_secondaryDiffTimer_' + panelObj.id;
    if (_diffTimers[timerKey]) clearTimeout(_diffTimers[timerKey]);
    _diffTimers[timerKey] = setTimeout(() => {
        _diffTimers[timerKey] = null;
        _doFetchVttyDiff(panelObj.id, s.secondaryInstUrl, s.secondaryCmdId, true);
    }, delayMs || 0);
}

// ─── VTTY Update Mode Start/Stop ───
function startPanelUpdateMode(panelId) {
    stopPanelUpdateMode(panelId);
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || panelObj.selectedCmdId === null || state.bufferView !== 'current') return;
    if (state.updateMode === 'push') connectPanelWs(panelId);
    else startPanelPoll(panelId);
}

function stopPanelUpdateMode(panelId) {
    disconnectPanelWs(panelId);
    stopPanelPoll(panelId);
}

    // ─── Exports ───
    Object.assign(window, {
        connectPanelWs, disconnectPanelWs, updateWsQualityIndicator,
        startPanelPoll, stopPanelPoll, pollOncePanel,
        startPanelUpdateMode, stopPanelUpdateMode,
        _connectSecondaryWs, _disconnectSecondaryWs,
        fetchVttyDiffForPanel, fetchSecondaryVttyDiff,
    });
})();