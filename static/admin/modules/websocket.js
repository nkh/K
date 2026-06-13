// ─── WebSocket Management ───
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
/// Works for both primary (prefix='ws') and secondary (prefix='secondaryWs').
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

/// Scroll-aware innerHTML replacement shared by secondary VTTY display/diff.
function _applyScrollHtml(vttyEl, pre, html) {
    const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
    const oldScrollHeight = vttyEl.scrollHeight;
    pre.innerHTML = html;
    if (wasAtBottom) {
        vttyEl.scrollTop = vttyEl.scrollHeight;
    } else {
        vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
    }
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
    const url = _buildWsUrl(instUrl, cmdId);

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
                const panelEl = document.getElementById(panelObj.id);
                if (!panelEl) return;

                if (msg.type === 'vtty_full' && msg.data) {
                    if (!_throttleRefresh()) updateVttyDisplayForPanel(panelObj, panelEl, msg.data);
                    // Alt screen badge
                    const badge = panelEl.querySelector('.alt-screen-badge');
                    if (badge) badge.classList.toggle('visible', !!msg.data.alternate_screen);
                } else if (msg.type === 'vtty_diff' && msg.data) {
                    if (!_throttleRefresh()) applyVttyDiffForPanel(panelObj, panelEl, msg.data);
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
                } else if (msg.type === 'peer_registered' || msg.type === 'peer_unregistered') {
                    handlePeerEvent(msg);
                }
            } catch (e) {
                console.error('WS message parse error (panel ' + panelId + '):', e);
            }
        };

        ws.onclose = () => {
            if (panelObj.ws !== ws) return;
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
                            if (inst && inst.reachable !== false) connectPanelWs(panelObj.id);
                        }
                    }, 2000);
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
    _cleanupWs(panelObj, 'ws');
    // Also disconnect secondary WS if panel is split
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
    if (panelObj.pollTimer) {
        clearInterval(panelObj.pollTimer);
        panelObj.pollTimer = null;
    }
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

// ─── Secondary WebSocket for Split Panels ───

function _disconnectSecondaryWs(panelObj) {
    if (!panelObj || !panelObj.split) return;
    _cleanupWs(panelObj.split, 'secondaryWs');
}

function _connectSecondaryWs(panelObj) {
    if (!panelObj || !panelObj.split) return;
    const s = panelObj.split;
    if (!s.secondaryCmdId || !s.secondaryInstUrl) return;

    // Disconnect existing secondary WS
    _disconnectSecondaryWs(panelObj);

    const instUrl = s.secondaryInstUrl;
    const cmdId = s.secondaryCmdId;
    const url = _buildWsUrl(instUrl, cmdId);

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
                if (msg.cmd_id && msg.cmd_id !== s.secondaryCmdId) return;
                if (msg.data && msg.data.id && msg.data.id !== s.secondaryCmdId) return;

                const secondaryId = panelObj.id + '-secondary';
                const panelEl = document.getElementById(panelObj.id);
                if (!panelEl) return;
                const vttyEl = document.getElementById('vtty-' + secondaryId);
                if (!vttyEl) return;

                if (msg.type === 'vtty_full' && msg.data) {
                    if (!_throttleRefresh()) _updateSecondaryVttyDisplay(panelObj, vttyEl, msg.data);
                } else if (msg.type === 'vtty_diff' && msg.data) {
                    if (!_throttleRefresh()) _applySecondaryVttyDiff(panelObj, vttyEl, msg.data);
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
                } else if (msg.type === 'peer_registered' || msg.type === 'peer_unregistered') {
                    handlePeerEvent(msg);
                }
            } catch (e) {
                console.error('Secondary WS message parse error (panel ' + panelObj.id + '):', e);
            }
        };

        ws.onclose = () => {
            if (s.secondaryWs !== ws) return;
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
                            if (inst && inst.reachable !== false) _connectSecondaryWs(panelObj);
                        }
                    }, 2000);
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

// ─── Secondary Pane VTTY Display ───
// These mirror the primary functions in vtty.js but operate on the secondary
// split pane. They use a separate generation cache key (_secondaryGen_) and
// write to split-specific state properties (secondaryMouseTracking, etc.)
// instead of the panel-level properties and global toolbar/bottombar elements.

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

async function _loadSecondaryVttyHttp(panelObj) {
    if (!panelObj || !panelObj.split) return;
    const s = panelObj.split;
    const vttyEl = document.getElementById('vtty-' + panelObj.id + '-secondary');
    if (!vttyEl) return;
    try {
        const json = await api.getVttyHtml(s.secondaryInstUrl, s.secondaryCmdId);
        if (json.status === 'ok' && json.data) _updateSecondaryVttyDisplay(panelObj, vttyEl, json.data);
    } catch (e) { /* ignore fetch errors */ }
}

function _updateSecondaryVttyDisplay(panelObj, vttyEl, data) {
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;
    const cmdId = panelObj.split.secondaryCmdId;
    const genKey = '_secondaryGen_' + cmdId;
    if (cmdId && data.generation !== undefined) {
        if (state[genKey] === data.generation) {
            _updateSecondaryVttyMetadata(panelObj, vttyEl, data);
            return;
        }
        state[genKey] = data.generation;
    }
    if (data.html !== undefined && data.html !== null) {
        _applyScrollHtml(vttyEl, pre, data.html);
    }
    _updateSecondaryVttyMetadata(panelObj, vttyEl, data);
}

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
        cursorEl.classList.remove('hidden');
    } else if (cursorEl) {
        cursorEl.classList.add('hidden');
    }
    panelObj.split.secondaryMouseTracking = !!data.mouse_tracking;
    panelObj.split.secondaryMouseSgr = !!data.mouse_sgr;
    if (vttyEl) {
        vttyEl.classList.toggle('selectable', !panelObj.split.secondaryMouseTracking);
        const pre = vttyEl.querySelector('pre');
        if (pre && dims.rows && dims.cols) {
            pre._vttyRows = dims.rows;
            pre._vttyCols = dims.cols;
        }
    }
}

function _applySecondaryVttyDiff(panelObj, vttyEl, data) {
    const cmdId = panelObj.split.secondaryCmdId;
    if (!cmdId) return;
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;
    const genKey = '_secondaryGen_' + cmdId;
    if (data.generation !== undefined && state[genKey] === data.generation) {
        if (data.cursor || data.dimensions || data.mouse_tracking !== undefined)
            _updateSecondaryVttyMetadata(panelObj, vttyEl, data);
        return;
    }
    if (data.generation !== undefined) state[genKey] = data.generation;
    if (data.html !== undefined) {
        _applyScrollHtml(vttyEl, pre, data.html);
        _updateSecondaryVttyMetadata(panelObj, vttyEl, data);
        return;
    }
    // Level 3 cell-level incremental diff not supported for secondary pane — fall back to HTTP
    scheduleSecondaryVttyHttp(panelObj, 0);
}

// ─── VTTY Update Mode Start/Stop ───
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

    // ─── Exports ───
    Object.assign(window, {
        connectPanelWs, disconnectPanelWs, updateWsQualityIndicator,
        startPanelPoll, stopPanelPoll, pollOncePanel,
        startPanelUpdateMode, stopPanelUpdateMode,
        _connectSecondaryWs, _disconnectSecondaryWs,
        scheduleSecondaryVttyHttp, _loadSecondaryVttyHttp,
        _updateSecondaryVttyDisplay, _updateSecondaryVttyMetadata,
        _applySecondaryVttyDiff,
    });
})();