// ─── WebSocket Management ───
// Uses a shared WS pool: one WebSocket per instUrl/cmdId, broadcast to
// all panels subscribed to that command.  This avoids the server dropping
// older connections when multiple panels view the same command.
(function() {
    'use strict';

// ─── Shared Helpers ───

function _buildWsUrl(instUrl, cmdId) {
    const wsUrl = instUrl.replace(/^http/, 'ws');
    const token = state.authToken || (state.connections.find(i => i.url === instUrl) || {}).token || '';
    const sep = token ? '?' : '';
    return `${wsUrl}/api/commands/${cmdId}/ws${sep}${token ? 'token=' + encodeURIComponent(token) : ''}`;
}

// ─── Shared WS Subscription Pool ───
// Key: "instUrl/cmdId" → { ws, instUrl, cmdId, panels: Set<panelId>,
//                           reconnectTimer, reconnectCount, pingInterval, pingSendTime, latency, closed }
const _sharedSubs = {};

function _subKey(instUrl, cmdId) { return instUrl + '/' + cmdId; }

function _connectSharedSub(sub) {
    if (sub.ws) return;
    const url = _buildWsUrl(sub.instUrl, sub.cmdId);
    sub.closed = false;
    try {
        const ws = new WebSocket(url);
        sub.ws = ws;

        ws.onopen = () => {
            clearInterval(sub.pingInterval);
            sub.pingInterval = setInterval(() => {
                if (sub.ws && sub.ws.readyState === WebSocket.OPEN) {
                    sub.pingSendTime = Date.now();
                    sub.ws.send(JSON.stringify({ type: 'ping' }));
                }
            }, 10000);
            // Update conn status if any subscribed panel is focused
            _updateConnStatus(sub);
            updateWsQualityIndicator();
        };

        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                switch (msg.type) {
                    case 'vtty_dirty':
                        // Broadcast to ALL panels subscribed to this command
                        for (const pid of sub.panels) {
                            fetchVttyDiffForPanel(pid, sub.instUrl, sub.cmdId);
                        }
                        break;
                    case 'vtty_close':
                        for (const pid of sub.panels) {
                            delete state._diffBaselines[pid + '/' + sub.cmdId];
                        }
                        break;
                    case 'command_ended':
                        for (const pid of sub.panels) {
                            delete state._diffBaselines[pid + '/' + sub.cmdId];
                        }
                        notifyCommandEnded(sub.cmdId);
                        _closeSharedSub(sub);
                        break;
                    case 'pong':
                        if (sub.pingSendTime > 0) {
                            sub.latency = Date.now() - sub.pingSendTime;
                            sub.pingSendTime = 0;
                            updateWsQualityIndicator();
                            _updateConnStatus(sub);
                        }
                        break;
                    case 'peer_registered':
                    case 'peer_unregistered':
                        handlePeerEvent(msg);
                        break;
                }
            } catch (e) {
                console.error('WS message parse error:', e);
            }
        };

        ws.onclose = () => {
            if (sub.ws !== ws) return;
            sub.ws = null;
            clearInterval(sub.pingInterval);
            sub.pingInterval = null;
            sub.pingSendTime = 0;
            sub.latency = 0;
            sub.closed = true;
            _updateConnStatus(sub);
            updateWsQualityIndicator();
            // HTTP fallback fetch for all subscribed panels
            for (const pid of sub.panels) {
                const p = state.panels.find(pp => pp.id === pid);
                if (p && p.selectedInstUrl && p.selectedCmdId) {
                    fetchVttyDiffForPanel(pid, p.selectedInstUrl, p.selectedCmdId, 0);
                }
            }
            // Auto-reconnect (max 5 attempts) if still has subscribers
            if (sub.panels.size > 0 && !sub.reconnectTimer) {
                sub.reconnectCount++;
                if (sub.reconnectCount <= 5) {
                    sub.reconnectTimer = setTimeout(() => {
                        sub.reconnectTimer = null;
                        if (sub.panels.size > 0 && state.updateMode === 'push') {
                            const inst = state.connections.find(i => i.url === sub.instUrl);
                            if (inst && inst.reachable !== false) {
                                _connectSharedSub(sub);
                            }
                        }
                    }, 2000);
                }
            }
        };

        ws.onerror = (err) => {
            console.error('WebSocket error:', err);
        };
    } catch (e) {
        console.error('WebSocket connect failed:', e);
    }
}

function _closeSharedSub(sub) {
    if (sub.reconnectTimer) { clearTimeout(sub.reconnectTimer); sub.reconnectTimer = null; }
    clearInterval(sub.pingInterval);
    sub.pingInterval = null;
    sub.pingSendTime = 0;
    sub.latency = 0;
    sub.reconnectCount = 0;
    if (sub.ws) {
        sub.ws.onclose = null;
        sub.ws.close();
        sub.ws = null;
    }
    sub.closed = true;
}

function _updateConnStatus(sub) {
    // Only update if a subscribed panel is focused
    const focusedId = state._focusedPanelId;
    if (!sub.panels.has(focusedId)) return;
    const connEl = document.getElementById('connStatus');
    if (!connEl) return;
    if (sub.ws && sub.ws.readyState === WebSocket.OPEN) {
        if (sub.latency > 0) connEl.textContent = 'Connected (' + sub.latency + 'ms)';
        else connEl.textContent = 'WS Connected';
    } else {
        connEl.textContent = 'WS Disconnected';
    }
}

// Subscribe a panel to a command's shared WS
function _subscribePanel(panelId, instUrl, cmdId) {
    const key = _subKey(instUrl, cmdId);
    let sub = _sharedSubs[key];
    if (!sub) {
        sub = { instUrl, cmdId, panels: new Set(), ws: null, reconnectTimer: null, reconnectCount: 0, pingInterval: null, pingSendTime: 0, latency: 0, closed: false };
        _sharedSubs[key] = sub;
    }
    const wasEmpty = sub.panels.size === 0;
    sub.panels.add(panelId);
    if (wasEmpty || !sub.ws || sub.ws.readyState !== WebSocket.OPEN) {
        if (sub.ws) { sub.ws.onclose = null; sub.ws.close(); sub.ws = null; }
        sub.reconnectCount = 0;
        _connectSharedSub(sub);
    }
    // Fetch initial content for this panel
    fetchVttyDiffForPanel(panelId, instUrl, cmdId, 0);
}

// Unsubscribe a panel from a command's shared WS
function _unsubscribePanel(panelId, instUrl, cmdId) {
    const key = _subKey(instUrl, cmdId);
    const sub = _sharedSubs[key];
    if (!sub) return;
    sub.panels.delete(panelId);
    if (sub.panels.size === 0) {
        _closeSharedSub(sub);
        delete _sharedSubs[key];
    }
}

// ─── Per-Panel WebSocket Management ───

function connectPanelWs(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || !panelObj.selectedInstUrl || !panelObj.selectedCmdId) return;

    disconnectPanelWs(panelId);

    const instUrl = panelObj.selectedInstUrl;
    const cmdId = panelObj.selectedCmdId;

    // Track subscription on panel so disconnectPanelWs can unsubscribe
    panelObj.wsInstUrl = instUrl;
    panelObj.wsCmdId = cmdId;

    if (state.updateMode === 'push') {
        _subscribePanel(panelId, instUrl, cmdId);
        // Store WS ref on panel for backward compat (tests, quality indicator)
        const key = _subKey(instUrl, cmdId);
        panelObj.ws = _sharedSubs[key] ? _sharedSubs[key].ws : null;
    }

    // Also connect secondary WS if panel is split
    if (panelObj.split && panelObj.split.secondaryCmdId && panelObj.split.secondaryInstUrl) {
        _connectSecondaryWs(panelObj);
    }
}

function disconnectPanelWs(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    // Clear diff baselines so reconnection gets a full refresh (not a stale empty diff)
    if (panelObj.wsInstUrl && panelObj.wsCmdId) {
        delete state._diffBaselines[panelId + '/' + panelObj.wsCmdId];
        _unsubscribePanel(panelId, panelObj.wsInstUrl, panelObj.wsCmdId);
    }
    panelObj.wsInstUrl = null;
    panelObj.wsCmdId = null;
    panelObj.ws = null;
    panelObj.wsReconnectCount = 0;
    panelObj.wsPingSendTime = 0;
    panelObj.wsLatency = 0;
    if (panelObj.split) {
        // Clear secondary diff baseline too
        if (panelObj.split.secondaryCmdId) {
            delete state._diffBaselines[panelId + '/' + panelObj.split.secondaryCmdId];
        }
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

    const focusedId = state._focusedPanelId;
    const focusedPanel = state.panels.find(p => p.id === focusedId);
    if (!focusedPanel || !focusedPanel.selectedInstUrl || !focusedPanel.selectedCmdId) {
        el.textContent = '--';
        el.style.color = 'var(--red)';
        el.title = 'Disconnected';
        return;
    }

    const key = _subKey(focusedPanel.selectedInstUrl, focusedPanel.selectedCmdId);
    const sub = _sharedSubs[key];
    const isConnected = sub && sub.ws && sub.ws.readyState === WebSocket.OPEN;
    const latency = sub ? sub.latency : 0;

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
    el.title = 'Latency: ' + (latency > 0 ? latency + 'ms' : 'measuring...');
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
// Secondary panes are always unique (different command), so keep per-object WS.

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

function _disconnectSecondaryWs(panelObj) {
    if (!panelObj || !panelObj.split) return;
    _cleanupWs(panelObj.split, 'secondaryWs');
}

function _connectSecondaryWs(panelObj) {
    if (!panelObj || !panelObj.split) return;
    const s = panelObj.split;
    if (!s.secondaryCmdId || !s.secondaryInstUrl) return;
    _disconnectSecondaryWs(panelObj);

    const url = _buildWsUrl(s.secondaryInstUrl, s.secondaryCmdId);
    try {
        const ws = new WebSocket(url);
        s.secondaryWs = ws;
        s.secondaryWsInstUrl = s.secondaryInstUrl;
        s.secondaryWsCmdId = s.secondaryCmdId;

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
                if (msg.type === 'vtty_dirty') {
                    fetchSecondaryVttyDiff(panelObj);
                } else if (msg.type === 'vtty_close') {
                    delete state._diffBaselines[panelObj.id + '/' + s.secondaryCmdId];
                } else if (msg.type === 'command_ended') {
                    delete state._diffBaselines[panelObj.id + '/' + s.secondaryCmdId];
                    notifyCommandEnded(s.secondaryCmdId);
                    _disconnectSecondaryWs(panelObj);
                } else if (msg.type === 'peer_registered' || msg.type === 'peer_unregistered') {
                    handlePeerEvent(msg);
                }
            } catch (e) {
                console.error('Secondary WS message parse error:', e);
            }
        };

        ws.onclose = () => {
            if (s.secondaryWs !== ws) return;
            s.secondaryWs = null;
            clearInterval(s.secondaryWsPingInterval);
            s.secondaryWsPingInterval = null;
            s.secondaryWsPingSendTime = 0;
            s.secondaryWsLatency = 0;
            if (s.secondaryInstUrl && s.secondaryCmdId) fetchSecondaryVttyDiff(panelObj, 0);
            // Reconnect
            if (s.secondaryInstUrl && s.secondaryCmdId && !s.secondaryWsReconnectTimer && state.updateMode === 'push') {
                s.secondaryWsReconnectCount = (s.secondaryWsReconnectCount || 0) + 1;
                if (s.secondaryWsReconnectCount <= 5) {
                    s.secondaryWsReconnectTimer = setTimeout(() => {
                        s.secondaryWsReconnectTimer = null;
                        const inst = state.connections.find(i => i.url === s.secondaryInstUrl);
                        if (inst && inst.reachable !== false) _connectSecondaryWs(panelObj);
                    }, 2000);
                }
            }
        };

        ws.onerror = (err) => {
            console.error('Secondary WebSocket error:', err);
        };
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

    const blKey = panelId + '/' + cmdId;
    const baseline = state._diffBaselines[blKey] || null;
    try {
        const json = await api.getVttyDiff(instUrl, cmdId, baseline);
        if (json.status === 'ok' && json.data) {
            // Store the baseline UUID for next request
            state._diffBaselines[blKey] = json.data.baseline;

            if (json.data.full_sync_required) {
                // Too many cells changed — fetch full HTML instead
                const htmlJson = await api.getVttyHtml(instUrl, cmdId);
                if (htmlJson.status === 'ok' && htmlJson.data) {
                    // Reset baseline since we skipped the diff
                    state._diffBaselines[blKey] = null;
                    const htmlResp = await api.getVttyDiff(instUrl, cmdId, null);
                    if (htmlResp.status === 'ok' && htmlResp.data) {
                        state._diffBaselines[blKey] = htmlResp.data.baseline;
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
        _sharedSubs,
    });
})();