// ─── WebSocket Management ───
(function() {
    'use strict';
// ─── Push Mode: WebSocket ───
function connectVttyWs(instUrl, cmdId) {
    // Close existing connection if any
    disconnectVttyWs();

    const wsUrl = instUrl.replace(/^http/, 'ws');
    const token = state.authToken || (state.connections.find(i => i.url === instUrl) || {}).token || '';
    const sep = token ? '?' : '';
    const url = `${wsUrl}/api/commands/${cmdId}/ws${sep}${token ? 'token=' + encodeURIComponent(token) : ''}`;

    try {
        const ws = new WebSocket(url);
        state.vttyWs = ws;
        state.vttyWsUrl = instUrl;
        state.vttyWsCmdId = cmdId;

        ws.onopen = () => {
            document.getElementById('connStatus').textContent = 'WS Connected';
            // Start ping/pong latency measurement (every 10s)
            clearInterval(state._wsPingInterval);
            state._wsPingInterval = setInterval(() => {
                if (state.vttyWs && state.vttyWs.readyState === WebSocket.OPEN) {
                    state._wsPingSendTime = Date.now();
                    state.vttyWs.send(JSON.stringify({ type: 'ping' }));
                }
            }, 10000);
            updateWsQualityIndicator();
        };

        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                // Guard: discard messages for a command that is no longer selected.
                // This can happen if the WS was connected to command A and the user
                // switched to command B before the WS closed.
                if (msg.cmd_id && msg.cmd_id !== state.selectedCmdId) return;
                // Also guard on nested data.id — the server sends
                // {type:"vtty_full", data:{id:"...",...}} not top-level cmd_id.
                if (msg.data && msg.data.id && msg.data.id !== state.selectedCmdId) return;
                if (msg.type === 'vtty_full' && msg.data) {
                    // Initial full snapshot — buffer or apply
                    if (state.bufferView === 'current') {
                        if (_isTerminalVisible()) {
                            // Skip DOM update if refresh throttle is active —
                            // the throttle timer will fetch the latest state.
                            if (!_throttleRefresh()) {
                                updateVttyDisplay(msg.data);
                            }
                        } else {
                            state._pendingVttyData = msg.data;
                            state._pendingVttyDirty = true;
                        }
                    }
                    const selPanel = getSelectedPanel();
                    if (selPanel) {
                        const badge = document.getElementById('altScreenBadge-' + selPanel.id);
                        if (badge) badge.classList.toggle('visible', !!msg.data.alternate_screen);
                    }
                } else if (msg.type === 'vtty_diff' && msg.data) {
                    // Level 3: Incremental diff — buffer or apply
                    if (state.bufferView === 'current') {
                        if (_isTerminalVisible()) {
                            if (!_throttleRefresh()) {
                                applyVttyDiff(msg.data);
                            }
                        } else {
                            state._pendingVttyData = msg.data;
                            state._pendingVttyDirty = true;
                        }
                    }
                } else if (msg.type === 'vtty_dirty' && msg.data) {
                    // Legacy dirty signal (shouldn't arrive in Level 3 mode,
                    // but handled as fallback for older servers).
                    if (state.bufferView === 'current') {
                        if (_isTerminalVisible()) {
                            scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 50);
                        } else {
                            state._pendingVttyDirty = true;
                        }
                    }
                } else if (msg.type === 'command_ended') {
                    document.getElementById('connStatus').textContent = 'Command ended';
                    disconnectVttyWs();
                    // Browser notification on command exit
                    notifyCommandEnded(state.vttyWsCmdId);
                } else if (msg.type === 'pong') {
                    // Calculate RTT from ping/pong
                    if (state._wsPingSendTime > 0) {
                        state._wsLatency = Date.now() - state._wsPingSendTime;
                        state._wsPingSendTime = 0;
                        updateWsQualityIndicator();
                        // Also update connStatus to show latency
                        const connEl = document.getElementById('connStatus');
                        if (connEl) connEl.textContent = 'Connected (' + state._wsLatency + 'ms)';
                    }
                } else if (msg.type === 'connected') {
                    // Server confirms connection. A vtty_full follows immediately.
                } else if (msg.type === 'peer_registered' || msg.type === 'peer_unregistered') {
                    // Server-level peer notification — forward to handler
                    handlePeerEvent(msg);
                }
            } catch (e) {
                console.error('WS message parse error:', e);
            }
        };

        ws.onclose = () => {
            if (state.vttyWs === ws) {
                state.vttyWs = null;
                clearInterval(state._wsPingInterval);
                state._wsPingInterval = null;
                state._wsPingSendTime = 0;
                state._wsLatency = 0;
                document.getElementById('connStatus').textContent = 'WS Disconnected';
                updateWsQualityIndicator();
                // Mark instance as potentially unreachable when WS drops
                if (state.vttyWsUrl) {
                    const wsInst = state.connections.find(i => i.url === state.vttyWsUrl);
                    if (wsInst && wsInst.reachable) {
                        // Don't immediately mark unreachable — the server might just
                        // have closed this particular WS.  A failed /api/commands
                        // fetch in the next loadCommands() cycle will confirm it.
                        // But bump reconnect count so we stop retrying aggressively.
                    }
                }
                // When WebSocket disconnects, schedule an HTTP fetch to keep display alive
                if (state.selectedInstUrl && state.selectedCmdId) {
                    scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 0);
                }
                // Auto-reconnect after 2 seconds if the command is still selected and alive
                // Cap reconnect attempts to avoid hammering a dead server
                if (state.selectedInstUrl && state.selectedCmdId && !state._wsReconnectTimer) {
                    state._wsReconnectCount++;
                    if (state._wsReconnectCount <= 5) {
                        state._wsReconnectTimer = setTimeout(() => {
                            state._wsReconnectTimer = null;
                            if (state.selectedInstUrl && state.selectedCmdId && state.updateMode === 'push') {
                                // Only reconnect if the instance is still reachable
                                const inst = state.connections.find(i => i.url === state.selectedInstUrl);
                                if (inst && inst.reachable !== false) {
                                    connectVttyWs(state.selectedInstUrl, state.selectedCmdId);
                                }
                            }
                        }, 2000);
                    }
                }
            }
        };

        ws.onerror = (err) => {
            console.error('WebSocket error:', err);
            document.getElementById('connStatus').textContent = 'WS Error';
        };
    } catch (e) {
        console.error('WebSocket connect failed:', e);
    }
}

function disconnectVttyWs() {
    if (state._wsReconnectTimer) {
        clearTimeout(state._wsReconnectTimer);
        state._wsReconnectTimer = null;
    }
    clearInterval(state._wsPingInterval);
    state._wsPingInterval = null;
    state._wsPingSendTime = 0;
    state._wsLatency = 0;
    state._wsReconnectCount = 0;
    if (state.vttyWs) {
        state.vttyWs.onclose = null; // prevent re-entry
        state.vttyWs.close();
        state.vttyWs = null;
        state.vttyWsUrl = null;
        state.vttyWsCmdId = null;
    }
    updateWsQualityIndicator();
}

// ─── Per-Panel WebSocket Management ───
// Each panel has its own WebSocket connection to its selected command.
// This allows multiple panels to stream different commands simultaneously.

function connectPanelWs(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || !panelObj.selectedInstUrl || !panelObj.selectedCmdId) return;

    // Disconnect existing WS for this panel
    disconnectPanelWs(panelId);

    const instUrl = panelObj.selectedInstUrl;
    const cmdId = panelObj.selectedCmdId;
    const wsUrl = instUrl.replace(/^http/, 'ws');
    const token = state.authToken || (state.connections.find(i => i.url === instUrl) || {}).token || '';
    const sep = token ? '?' : '';
    const url = `${wsUrl}/api/commands/${cmdId}/ws${sep}${token ? 'token=' + encodeURIComponent(token) : ''}`;

    try {
        const ws = new WebSocket(url);
        panelObj.ws = ws;
        panelObj.wsInstUrl = instUrl;
        panelObj.wsCmdId = cmdId;

        ws.onopen = () => {
            // Update connStatus if this is the focused panel
            if (panelObj.id === state._focusedPanelId) {
                document.getElementById('connStatus').textContent = 'WS Connected';
            }
            // Start ping/pong latency measurement (every 10s)
            clearInterval(panelObj.wsPingInterval);
            panelObj.wsPingInterval = setInterval(() => {
                if (panelObj.ws && panelObj.ws.readyState === WebSocket.OPEN) {
                    panelObj.wsPingSendTime = Date.now();
                    panelObj.ws.send(JSON.stringify({ type: 'ping' }));
                }
            }, 10000);
            updateWsQualityIndicator();
        };

        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                // Guard: discard messages for a command that is no longer selected on this panel

                // Route VTTY updates to THIS panel's DOM
                const panelEl = document.getElementById(panelObj.id);
                if (!panelEl) return;

                if (msg.type === 'vtty_full' && msg.data) {
                    const throttled = _throttleRefresh();
                    if (!throttled) {
                        updateVttyDisplayForPanel(panelObj, panelEl, msg.data);
                    }
                    // Alt screen badge
                    const badge = panelEl.querySelector('.alt-screen-badge');
                    if (badge) badge.classList.toggle('visible', !!msg.data.alternate_screen);
                } else if (msg.type === 'vtty_diff' && msg.data) {
                    if (!_throttleRefresh()) {
                        applyVttyDiffForPanel(panelObj, panelEl, msg.data);
                    }
                } else if (msg.type === 'vtty_dirty' && msg.data) {
                    scheduleVttyHttpForPanel(panelObj.id, panelObj.selectedInstUrl, panelObj.selectedCmdId, 50);
                } else if (msg.type === 'command_ended') {
                    if (panelObj.id === state._focusedPanelId) {
                        document.getElementById('connStatus').textContent = 'Command ended';
                    }
                    disconnectPanelWs(panelObj.id);
                    notifyCommandEnded(panelObj.selectedCmdId);
                } else if (msg.type === 'pong') {
                    if (panelObj.wsPingSendTime > 0) {
                        panelObj.wsLatency = Date.now() - panelObj.wsPingSendTime;
                        panelObj.wsPingSendTime = 0;
                        if (panelObj.id === state._focusedPanelId) {
                            updateWsQualityIndicator();
                            const connEl = document.getElementById('connStatus');
                            if (connEl) connEl.textContent = 'Connected (' + panelObj.wsLatency + 'ms)';
                        }
                    }
                } else if (msg.type === 'connected') {
                    // Server confirms connection. A vtty_full follows immediately.
                } else if (msg.type === 'peer_registered' || msg.type === 'peer_unregistered') {
                    handlePeerEvent(msg);
                }
            } catch (e) {
                console.error('WS message parse error (panel ' + panelId + '):', e);
            }
        };

        ws.onclose = () => {
            if (panelObj.ws === ws) {
                panelObj.ws = null;
                clearInterval(panelObj.wsPingInterval);
                panelObj.wsPingInterval = null;
                panelObj.wsPingSendTime = 0;
                panelObj.wsLatency = 0;
                if (panelObj.id === state._focusedPanelId) {
                    document.getElementById('connStatus').textContent = 'WS Disconnected';
                    updateWsQualityIndicator();
                }
                // Schedule HTTP fallback to keep display alive
                if (panelObj.selectedInstUrl && panelObj.selectedCmdId) {
                    scheduleVttyHttpForPanel(panelObj.id, panelObj.selectedInstUrl, panelObj.selectedCmdId, 0);
                }
                // Auto-reconnect (max 5 attempts)
                if (panelObj.selectedInstUrl && panelObj.selectedCmdId && !panelObj.wsReconnectTimer) {
                    panelObj.wsReconnectCount++;
                    if (panelObj.wsReconnectCount <= 5) {
                        panelObj.wsReconnectTimer = setTimeout(() => {
                            panelObj.wsReconnectTimer = null;
                            if (panelObj.selectedInstUrl && panelObj.selectedCmdId && state.updateMode === 'push') {
                                const inst = state.connections.find(i => i.url === panelObj.selectedInstUrl);
                                if (inst && inst.reachable !== false) {
                                    connectPanelWs(panelObj.id);
                                }
                            }
                        }, 2000);
                    }
                }
            }
        };

        ws.onerror = (err) => {
            console.error('WebSocket error (panel ' + panelId + '):', err);
        };
    } catch (e) {
        console.error('WebSocket connect failed (panel ' + panelId + '):', e);
    }

    // Also connect secondary WS if panel is split and secondary has a command
    if (panelObj.split && panelObj.split.secondaryCmdId && panelObj.split.secondaryInstUrl) {
        _connectSecondaryWs(panelObj);
    }
}

function disconnectPanelWs(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    if (panelObj.wsReconnectTimer) {
        clearTimeout(panelObj.wsReconnectTimer);
        panelObj.wsReconnectTimer = null;
    }
    clearInterval(panelObj.wsPingInterval);
    panelObj.wsPingInterval = null;
    panelObj.wsPingSendTime = 0;
    panelObj.wsLatency = 0;
    panelObj.wsReconnectCount = 0;
    if (panelObj.ws) {
        panelObj.ws.onclose = null; // prevent re-entry
        panelObj.ws.close();
        panelObj.ws = null;
        panelObj.wsInstUrl = null;
        panelObj.wsCmdId = null;
    }
    // Also disconnect secondary WS if panel is split
    if (panelObj.split) {
        _disconnectSecondaryWs(panelObj);
        if (panelObj.split.secondaryPollTimer) {
            clearInterval(panelObj.split.secondaryPollTimer);
            panelObj.split.secondaryPollTimer = null;
        }
    }
}

/// Disconnect WS for ALL panels (e.g. on page unload).
function disconnectAllPanelWs() {
    for (const panel of state.panels) {
        disconnectPanelWs(panel.id);
    }
}

// ─── WebSocket Connection Quality Indicator ───
function updateWsQualityIndicator() {
    const el = document.getElementById('wsQuality');
    if (!el) return;

    // Use focused panel's WS state
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
    if (latency === 0) {
        // Connected but no measurement yet
        color = 'var(--text-muted)';
    } else if (latency < 50) {
        color = 'var(--green)';
    } else if (latency < 200) {
        color = 'var(--yellow)';
    } else {
        color = 'var(--red)';
    }

    el.textContent = latency > 0 ? latency + 'ms' : '...';
    el.style.color = color;
    el.title = 'Latency: ' + (latency > 0 ? latency + 'ms' : 'measuring...') + ' | Reconnects: ' + reconnects;
}

// ─── Poll Mode ───
function startPoll() {
    // Legacy wrapper for focused panel
    const panelId = getActivePanelId();
    if (panelId) startPanelPoll(panelId);
}

function startPanelPoll(panelId) {
    stopPanelPoll(panelId);
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || !panelObj.selectedInstUrl || !panelObj.selectedCmdId) return;
    panelObj.pollTimer = setInterval(() => pollOncePanel(panelId), state.pollInterval);
    pollOncePanel(panelId);
}

function stopPoll() {
    // Legacy: stop all panel polls
    for (const panel of state.panels) stopPanelPoll(panel.id);
}

function stopPanelPoll(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    if (panelObj.pollTimer) {
        clearInterval(panelObj.pollTimer);
        panelObj.pollTimer = null;
    }
}

async function pollOnce() {
    // Legacy: poll focused panel
    const panelId = getActivePanelId();
    if (panelId) await pollOncePanel(panelId);
}

async function pollOncePanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || !panelObj.selectedInstUrl || !panelObj.selectedCmdId) return;
    const cmdId = panelObj.selectedCmdId;
    const instUrl = panelObj.selectedInstUrl;
    try {
        const json = await api.getVttyChanged(instUrl, cmdId);
        if (json.status === 'ok' && json.data && json.data.changed) {
            loadVttyHttpForPanel(panelId, instUrl, cmdId);
        }
    } catch (e) {
        // Silently ignore — next poll will retry
    }
}

// NOTE: updateVttyDisplay is defined in vtty.js and exported via window.
// It is used here via the global scope (vtty.js loads before websocket.js).

/// Disconnect the secondary WebSocket for a split panel.
function _disconnectSecondaryWs(panelObj) {
    if (!panelObj || !panelObj.split) return;
    const s = panelObj.split;
    if (s.secondaryWsReconnectTimer) {
        clearTimeout(s.secondaryWsReconnectTimer);
        s.secondaryWsReconnectTimer = null;
    }
    clearInterval(s.secondaryWsPingInterval);
    s.secondaryWsPingInterval = null;
    s.secondaryWsPingSendTime = 0;
    s.secondaryWsLatency = 0;
    s.secondaryWsReconnectCount = 0;
    if (s.secondaryWs) {
        s.secondaryWs.onclose = null;
        s.secondaryWs.close();
        s.secondaryWs = null;
        s.secondaryWsCmdId = null;
        s.secondaryWsInstUrl = null;
    }
}

/// Connect a secondary WebSocket for a split panel's secondary pane.
function _connectSecondaryWs(panelObj) {
    if (!panelObj || !panelObj.split) return;
    const s = panelObj.split;
    if (!s.secondaryCmdId || !s.secondaryInstUrl) return;

    // Disconnect existing secondary WS
    _disconnectSecondaryWs(panelObj);

    const instUrl = s.secondaryInstUrl;
    const cmdId = s.secondaryCmdId;
    const wsUrl = instUrl.replace(/^http/, 'ws');
    const token = state.authToken || (state.connections.find(i => i.url === instUrl) || {}).token || '';
    const sep = token ? '?' : '';
    const url = `${wsUrl}/api/commands/${cmdId}/ws${sep}${token ? 'token=' + encodeURIComponent(token) : ''}`;

    try {
        const ws = new WebSocket(url);
        s.secondaryWs = ws;
        s.secondaryWsInstUrl = instUrl;
        s.secondaryWsCmdId = cmdId;

        ws.onopen = () => {
            clearInterval(s.secondaryWsPingInterval);
            s.secondaryWsPingInterval = setInterval(() => {
                if (s.secondaryWs && s.secondaryWs.readyState === WebSocket.OPEN) {
                    s.secondaryWsPingSendTime = Date.now();
                    s.secondaryWs.send(JSON.stringify({ type: 'ping' }));
                }
            }, 10000);
        };

        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                // Guard: discard messages for a command that is no longer selected on this pane
                if (msg.cmd_id && msg.cmd_id !== s.secondaryCmdId) return;
                if (msg.data && msg.data.id && msg.data.id !== s.secondaryCmdId) return;

                // Route VTTY updates to the secondary vtty-container
                const secondaryId = panelObj.id + '-secondary';
                const panelEl = document.getElementById(panelObj.id);
                if (!panelEl) return;
                const vttyEl = document.getElementById('vtty-' + secondaryId);
                if (!vttyEl) return;

                if (msg.type === 'vtty_full' && msg.data) {
                    if (!_throttleRefresh()) {
                        _updateSecondaryVttyDisplay(panelObj, vttyEl, msg.data);
                    }
                } else if (msg.type === 'vtty_diff' && msg.data) {
                    if (!_throttleRefresh()) {
                        _applySecondaryVttyDiff(panelObj, vttyEl, msg.data);
                    }
                } else if (msg.type === 'vtty_dirty' && msg.data) {
                    scheduleSecondaryVttyHttp(panelObj, 50);
                } else if (msg.type === 'command_ended') {
                    _disconnectSecondaryWs(panelObj);
                    notifyCommandEnded(s.secondaryCmdId);
                } else if (msg.type === 'pong') {
                    if (s.secondaryWsPingSendTime > 0) {
                        s.secondaryWsLatency = Date.now() - s.secondaryWsPingSendTime;
                        s.secondaryWsPingSendTime = 0;
                    }
                } else if (msg.type === 'connected') {
                    // Server confirms connection.
                } else if (msg.type === 'peer_registered' || msg.type === 'peer_unregistered') {
                    handlePeerEvent(msg);
                }
            } catch (e) {
                console.error('Secondary WS message parse error (panel ' + panelObj.id + '):', e);
            }
        };

        ws.onclose = () => {
            if (s.secondaryWs === ws) {
                s.secondaryWs = null;
                clearInterval(s.secondaryWsPingInterval);
                s.secondaryWsPingInterval = null;
                s.secondaryWsPingSendTime = 0;
                s.secondaryWsLatency = 0;
                // Schedule HTTP fallback to keep display alive
                if (s.secondaryInstUrl && s.secondaryCmdId) {
                    scheduleSecondaryVttyHttp(panelObj, 0);
                }
                // Auto-reconnect (max 5 attempts)
                if (s.secondaryInstUrl && s.secondaryCmdId && !s.secondaryWsReconnectTimer) {
                    s.secondaryWsReconnectCount++;
                    if (s.secondaryWsReconnectCount <= 5) {
                        s.secondaryWsReconnectTimer = setTimeout(() => {
                            s.secondaryWsReconnectTimer = null;
                            if (s.secondaryInstUrl && s.secondaryCmdId && state.updateMode === 'push') {
                                const inst = state.connections.find(i => i.url === s.secondaryInstUrl);
                                if (inst && inst.reachable !== false) {
                                    _connectSecondaryWs(panelObj);
                                }
                            }
                        }, 2000);
                    }
                }
            }
        };

        ws.onerror = (err) => {
            console.error('Secondary WebSocket error (panel ' + panelObj.id + '):', err);
        };
    } catch (e) {
        console.error('Secondary WebSocket connect failed (panel ' + panelObj.id + '):', e);
    }
}

/// Schedule an HTTP fetch for the secondary pane's VTTY content.
function scheduleSecondaryVttyHttp(panelObj, delayMs) {
    if (!panelObj || !panelObj.split) return;
    const s = panelObj.split;
    if (!s.secondaryCmdId || !s.secondaryInstUrl) return;
    const timerKey = '_secondaryVttyHttpTimer_' + panelObj.id;
    if (state[timerKey]) clearTimeout(state[timerKey]);
    state[timerKey] = setTimeout(() => {
        state[timerKey] = null;
        _loadSecondaryVttyHttp(panelObj);
    }, delayMs);
}

/// Load secondary pane's VTTY content via HTTP.
async function _loadSecondaryVttyHttp(panelObj) {
    if (!panelObj || !panelObj.split) return;
    const s = panelObj.split;
    const secondaryId = panelObj.id + '-secondary';
    const vttyEl = document.getElementById('vtty-' + secondaryId);
    if (!vttyEl) return;

    const cmdId = s.secondaryCmdId;
    const instUrl = s.secondaryInstUrl;

    try {
        const json = await api.getVttyHtml(instUrl, cmdId);
        if (json.status === 'ok' && json.data) {
            _updateSecondaryVttyDisplay(panelObj, vttyEl, json.data);
        }
    } catch (e) {
        // Silently ignore fetch errors
    }
}

/// Update secondary pane's VTTY display (similar to updateVttyDisplayForPanel).
function _updateSecondaryVttyDisplay(panelObj, vttyEl, data) {
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    const cmdId = panelObj.split.secondaryCmdId;
    // Use a separate generation cache key for secondary
    const genKey = '_secondaryGen_' + cmdId;
    if (cmdId && data.generation !== undefined) {
        if (state[genKey] === data.generation) {
            _updateSecondaryVttyMetadata(panelObj, vttyEl, data);
            return;
        }
        state[genKey] = data.generation;
    }

    if (data.html !== undefined && data.html !== null) {
        const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
        const oldScrollHeight = vttyEl.scrollHeight;
        pre.innerHTML = data.html;
        if (wasAtBottom) {
            vttyEl.scrollTop = vttyEl.scrollHeight;
        } else {
            vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
        }
    }

    _updateSecondaryVttyMetadata(panelObj, vttyEl, data);
}

/// Update secondary pane's VTTY metadata (cursor, dimensions, mouse state).
function _updateSecondaryVttyMetadata(panelObj, vttyEl, data) {
    const cursor = data.cursor || {};
    const dims = data.dimensions || {};
    const inScrollback = panelObj.split.secondaryScrollbackOffset > 0;
    const cursorHidden = data.cursor_visible === false;
    const cursorEl = vttyEl ? vttyEl.querySelector('.cursor-indicator') : null;
    if (cursorEl && cursor.row !== undefined && !inScrollback && !cursorHidden) {
        const charW = panelObj.fontSize * 0.6;
        const charH = panelObj.fontSize * 1.2;
        cursorEl.style.top = (cursor.row * charH) + 'px';
        cursorEl.style.left = (cursor.col * charW) + 'px';
        cursorEl.style.width = charW + 'px';
        cursorEl.style.height = charH + 'px';
        cursorEl.style.display = '';
    } else if (cursorEl) {
        cursorEl.style.display = 'none';
    }
    panelObj.split.secondaryMouseTracking = !!data.mouse_tracking;
    panelObj.split.secondaryMouseSgr = !!data.mouse_sgr;
    if (vttyEl) {
        const mt = panelObj.split.secondaryMouseTracking;
        vttyEl.classList.toggle('selectable', !mt);
        const pre = vttyEl.querySelector('pre');
        if (pre && dims.rows && dims.cols) {
            pre._vttyRows = dims.rows;
            pre._vttyCols = dims.cols;
        }
    }
}

/// Apply VTTY diff to secondary pane (similar to applyVttyDiffForPanel).
function _applySecondaryVttyDiff(panelObj, vttyEl, data) {
    const cmdId = panelObj.split.secondaryCmdId;
    if (!cmdId) return;
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    const genKey = '_secondaryGen_' + cmdId;
    if (data.generation !== undefined && state[genKey] === data.generation) {
        if (data.cursor || data.dimensions || data.mouse_tracking !== undefined) {
            _updateSecondaryVttyMetadata(panelObj, vttyEl, data);
        }
        return;
    }
    if (data.generation !== undefined) {
        state[genKey] = data.generation;
    }

    // If full HTML is embedded, use it directly
    if (data.html !== undefined) {
        const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
        const oldScrollHeight = vttyEl.scrollHeight;
        pre.innerHTML = data.html;
        if (wasAtBottom) {
            vttyEl.scrollTop = vttyEl.scrollHeight;
        } else {
            vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
        }
        _updateSecondaryVttyMetadata(panelObj, vttyEl, data);
        return;
    }

    // Level 3 cell-level incremental diff not supported for secondary pane — fall back to HTTP
    scheduleSecondaryVttyHttp(panelObj, 0);
}



// ─── VTTY Update Mode Start/Stop ───
function startUpdateMode() {
    // Legacy wrapper: start update for the focused panel
    const panelId = getActivePanelId();
    if (panelId) startPanelUpdateMode(panelId);
}

function startPanelUpdateMode(panelId) {
    stopPanelUpdateMode(panelId);
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || panelObj.selectedCmdId === null || state.bufferView !== 'current') return;
    if (state.updateMode === 'push') {
        connectPanelWs(panelId);
    } else {
        startPanelPoll(panelId);
    }
}

function stopPanelUpdateMode(panelId) {
    disconnectPanelWs(panelId);
    stopPanelPoll(panelId);
}

function stopUpdateMode() {
    const panelId = getActivePanelId();
    if (panelId) stopPanelUpdateMode(panelId);
}

    window.connectVttyWs = connectVttyWs;
    window.disconnectVttyWs = disconnectVttyWs;
    window.connectPanelWs = connectPanelWs;
    window.disconnectPanelWs = disconnectPanelWs;
    window.disconnectAllPanelWs = disconnectAllPanelWs;
    window.updateWsQualityIndicator = updateWsQualityIndicator;
    window.startPoll = startPoll;
    window.startPanelPoll = startPanelPoll;
    window.stopPoll = stopPoll;
    window.stopPanelPoll = stopPanelPoll;
    window.pollOnce = pollOnce;
    window.pollOncePanel = pollOncePanel;
    window.startUpdateMode = startUpdateMode;
    window.startPanelUpdateMode = startPanelUpdateMode;
    window.stopUpdateMode = stopUpdateMode;
    window.stopPanelUpdateMode = stopPanelUpdateMode;
    window._connectSecondaryWs = _connectSecondaryWs;
    window._disconnectSecondaryWs = _disconnectSecondaryWs;
    window.scheduleSecondaryVttyHttp = scheduleSecondaryVttyHttp;
    window._loadSecondaryVttyHttp = _loadSecondaryVttyHttp;
    window._updateSecondaryVttyDisplay = _updateSecondaryVttyDisplay;
    window._updateSecondaryVttyMetadata = _updateSecondaryVttyMetadata;
    window._applySecondaryVttyDiff = _applySecondaryVttyDiff;
})();
