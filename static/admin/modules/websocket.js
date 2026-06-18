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

    // Connect WS for all leaves in the split tree
    if (panelObj.split && typeof _getAllLeaves === 'function') {
        const leaves = _getAllLeaves(panelObj);
        for (const { leaf, side } of leaves) {
            if (!side) continue; // skip primary (panel itself, handled above)
            if (leaf.cmdId && leaf.instUrl) {
                if (state.updateMode === 'push') {
                    _connectLeafWs(leaf);
                } else {
                    leaf.pollTimer = setInterval(() => {
                        if (leaf.cmdId) _loadLeafVttyHttpDirect(leaf);
                    }, state.pollInterval);
                    _loadLeafVttyHttpDirect(leaf);
                }
            }
        }
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
    // Disconnect all leaves in the split tree
    if (panelObj.split && typeof _getAllLeaves === 'function') {
        const leaves = _getAllLeaves(panelObj);
        for (const { leaf, side } of leaves) {
            if (!side) continue;
            _disconnectSingleLeaf(leaf);
            if (leaf.cmdId) delete state._diffBaselines[leaf.id + '/' + leaf.cmdId];
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

// ─── Leaf-level VTTY HTTP load (for split tree leaves) ───
// Unlike loadVttyHttpForPanel (which requires a panel ID), this works
// with a leaf object directly and updates the correct DOM element.
async function _loadLeafVttyHttpDirect(leaf) {
    if (!leaf || !leaf.cmdId || !leaf.instUrl) return;
    const vttyEl = document.getElementById('vtty-' + leaf.id);
    if (!vttyEl) return;
    const pre = vttyEl.querySelector('pre');
    if (!pre) return;
    try {
        const json = await api.getVttyHtml(leaf.instUrl, leaf.cmdId);
        if (json.status === 'ok' && json.data) {
            const genKey = leaf.id + '/' + leaf.cmdId;
            if (leaf.cmdId && json.data.generation !== undefined) {
                state._lastGeneration[genKey] = json.data.generation;
            }
            if (json.data.html !== undefined && json.data.html !== null) {
                const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
                const oldScrollHeight = vttyEl.scrollHeight;
                pre.innerHTML = json.data.html;
                if (state._level3Enabled && json.data.dimensions) {
                    buildCellGrid(genKey, pre, json.data.dimensions.rows, json.data.dimensions.cols);
                }
                if (wasAtBottom) vttyEl.scrollTop = vttyEl.scrollHeight;
                else vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
            }
        }
    } catch (e) { /* ignore */ }
}

// ─── Diff fetch for push mode ───
// Debounce timer per panel to avoid hammering the server.
const _diffTimers = {};

function fetchVttyDiffForPanel(panelId, instUrl, cmdId, delayMs) {
    const timerKey = '_diffTimer_' + panelId;
    if (_diffTimers[timerKey]) clearTimeout(_diffTimers[timerKey]);
    _diffTimers[timerKey] = setTimeout(() => {
        _diffTimers[timerKey] = null;
        // Attach instUrl and cmdId to data so _doFetchVttyDiff can use them for fallback
        const data = { _instUrl: instUrl, _cmdId: cmdId };
        // Override genKey in _doFetchVttyDiff by storing these on state temporarily
        _doFetchVttyDiff(panelId, instUrl, cmdId, false);
    }, delayMs);
}

async function _doFetchVttyDiff(panelId, instUrl, cmdId, isSecondary) {
    // panelId may be a leaf ID (for split tree leaves) or a panel ID.
    // Find the actual panel and the target DOM element.
    let panelObj = state.panels.find(p => p.id === panelId);
    let targetEl = document.getElementById('vtty-' + panelId);
    let leaf = null;

    // If not found as panel, search for it as a leaf in the split tree
    if (!panelObj && targetEl) {
        const panelEl = targetEl.closest('.panel');
        if (panelEl) {
            panelObj = state.panels.find(p => p.id === panelEl.id);
        }
        if (panelObj && panelObj.split && typeof _findLeafState === 'function') {
            const found = _findLeafState(panelObj, panelId);
            if (found) leaf = found.leaf;
        }
    }

    if (!panelObj) return;
    if (!targetEl) return;

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
                        const pre = targetEl.querySelector('pre');
                        if (pre && htmlResp.data.html !== undefined) {
                            const wasAtBottom = targetEl.scrollHeight - targetEl.scrollTop - targetEl.clientHeight < 50;
                            const oldScrollHeight = targetEl.scrollHeight;
                            pre.innerHTML = htmlResp.data.html;
                            if (state._level3Enabled && htmlResp.data.dimensions) {
                                buildCellGrid(blKey, pre, htmlResp.data.dimensions.rows, htmlResp.data.dimensions.cols);
                            }
                            if (wasAtBottom) targetEl.scrollTop = targetEl.scrollHeight;
                            else targetEl.scrollTop += targetEl.scrollHeight - oldScrollHeight;
                            const genKey = panelId + '/' + cmdId;
                            if (htmlResp.data.generation !== undefined) state._lastGeneration[genKey] = htmlResp.data.generation;
                        }
                    }
                }
            } else {
                // Incremental diff — apply cell-level patches directly to the target element
                _applyLeafDiff(targetEl, panelId, json.data);
            }
        }
    } catch (e) {
        // Silently ignore — next dirty signal will retry
    }
}

// Apply incremental cell-level diff to a vtty-container element (works for ANY leaf)
function _applyLeafDiff(vttyEl, leafId, data) {
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;
    const genKey = leafId + '/' + (data._cmdId || '');
    if (data.generation !== undefined && state._lastGeneration[genKey] === data.generation) {
        if (data.cursor || data.dimensions) _updateLeafMetadata(vttyEl, data);
        return;
    }
    if (data.generation !== undefined) state._lastGeneration[genKey] = data.generation;

    if (data.html !== undefined) {
        const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
        const oldScrollHeight = vttyEl.scrollHeight;
        pre.innerHTML = data.html;
        if (state._level3Enabled && data.dimensions) {
            buildCellGrid(genKey, pre, data.dimensions.rows, data.dimensions.cols);
        }
        if (wasAtBottom) vttyEl.scrollTop = vttyEl.scrollHeight;
        else vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
        _updateLeafMetadata(vttyEl, data);
        return;
    }

    if (!state._level3Enabled || !data.cells || !data.cells.length) {
        // No cell grid or no cells — fall back to full HTTP fetch
        _loadLeafVttyHttpDirect({ id: leafId, instUrl: data._instUrl, cmdId: data._cmdId });
        return;
    }

    const cg = state._cellGrids[genKey];
    if (!cg) {
        _loadLeafVttyHttpDirect({ id: leafId, instUrl: data._instUrl, cmdId: data._cmdId });
        return;
    }

    const dims = data.dimensions || {};
    if (dims.rows !== cg.rows || dims.cols !== cg.cols) {
        delete state._cellGrids[genKey];
        _loadLeafVttyHttpDirect({ id: leafId, instUrl: data._instUrl, cmdId: data._cmdId });
        return;
    }

    const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
    const oldScrollHeight = vttyEl.scrollHeight;

    for (let i = 0; i < data.cells.length; i++) {
        const c = data.cells[i];
        if (c.row < cg.grid.length && c.col < cg.grid[c.row].length) {
            const entry = cg.grid[c.row][c.col];
            if (entry) {
                if (entry.len === 1) {
                    const cell = c.cell;
                    const ch = cell.width === 0 ? '\u200b' : (cell.ch === '\u0000' ? ' ' : cell.ch);
                    entry.span.textContent = _htmlEscapeChar(ch);
                    entry.span.setAttribute('style', _cellStyle(c));
                    const wCls = cell.width === 0 ? 'c w0' : cell.width === 2 ? 'c w2' : 'c w1';
                    entry.span.className = wCls;
                } else {
                    _splitAndUpdateCell(cg, c.row, c.col, c);
                }
            }
        }
    }

    if (wasAtBottom) vttyEl.scrollTop = vttyEl.scrollHeight;
    else vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;

    _updateLeafMetadata(vttyEl, data);
}

// Update cursor/dimensions metadata for a vtty-container (works for ANY leaf)
function _updateLeafMetadata(vttyEl, data) {
    const cursor = data.cursor || {};
    const cursorEl = vttyEl ? vttyEl.querySelector('.cursor-indicator') : null;
    const cursorHidden = data.cursor_visible === false;
    if (cursorEl && cursor.row !== undefined && !cursorHidden) {
        const charW = 10 * 0.6;
        const charH = 10 * 1.2;
        cursorEl.style.top = (cursor.row * charH) + 'px';
        cursorEl.style.left = (cursor.col * charW) + 'px';
        cursorEl.style.width = charW + 'px';
        cursorEl.style.height = charH + 'px';
        cursorEl.classList.remove('hidden');
    } else if (cursorEl) {
        cursorEl.classList.add('hidden');
    }
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

    // ─── Generic leaf WS connect (for recursive split tree) ───
function _connectLeafWs(leaf) {
    if (!leaf || !leaf.cmdId || !leaf.instUrl) return;
    if (leaf.ws) { try { leaf.ws.close(); } catch {} leaf.ws = null; }
    const url = _buildWsUrl(leaf.instUrl, leaf.cmdId);
    try {
        const ws = new WebSocket(url);
        leaf.ws = ws;
        leaf.wsInstUrl = leaf.instUrl;
        leaf.wsCmdId = leaf.cmdId;
        ws.onopen = () => {
            clearInterval(leaf.wsPingInterval);
            leaf.wsPingInterval = setInterval(() => {
                if (leaf.ws && leaf.ws.readyState === WebSocket.OPEN) {
                    leaf.wsPingSendTime = Date.now();
                    leaf.ws.send(JSON.stringify({ type: 'ping' }));
                }
            }, 10000);
        };
        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                if (msg.type === 'vtty_dirty') {
                    // Fetch diff for this leaf using its own ID
                    _fetchLeafDiff(leaf.id, leaf.instUrl, leaf.cmdId, 0);
                } else if (msg.type === 'vtty_close' || msg.type === 'command_ended') {
                    delete state._diffBaselines[leaf.id + '/' + leaf.cmdId];
                    if (msg.type === 'command_ended') { notifyCommandEnded(leaf.cmdId); }
                    if (leaf.ws === ws) { leaf.ws = null; clearInterval(leaf.wsPingInterval); }
                } else if (msg.type === 'peer_registered' || msg.type === 'peer_unregistered') {
                    handlePeerEvent(msg);
                }
            } catch (e) {}
        };
        ws.onclose = () => {
            if (leaf.ws !== ws) return;
            leaf.ws = null;
            clearInterval(leaf.wsPingInterval); leaf.wsPingInterval = null;
            leaf.wsPingSendTime = 0; leaf.wsLatency = 0;
            if (leaf.instUrl && leaf.cmdId) _fetchLeafDiff(leaf.id, leaf.instUrl, leaf.cmdId, 0);
            if (leaf.instUrl && leaf.cmdId && !leaf.wsReconnectTimer && state.updateMode === 'push') {
                leaf.wsReconnectCount = (leaf.wsReconnectCount || 0) + 1;
                if (leaf.wsReconnectCount <= 5) {
                    leaf.wsReconnectTimer = setTimeout(() => {
                        leaf.wsReconnectTimer = null;
                        const inst = state.connections.find(i => i.url === leaf.instUrl);
                        if (inst && inst.reachable !== false) _connectLeafWs(leaf);
                    }, 2000);
                }
            }
        };
        ws.onerror = () => {};
    } catch (e) {}
    // Fetch initial terminal content for this leaf
    _loadLeafVttyHttpDirect(leaf);
}

// Fetch VTTY diff for a leaf (by leaf ID, not panel ID).
// This uses _doFetchVttyDiff which now handles leaf IDs.
function _fetchLeafDiff(leafId, instUrl, cmdId, delayMs) {
    const timerKey = '_diffTimer_' + leafId;
    if (_diffTimers[timerKey]) clearTimeout(_diffTimers[timerKey]);
    _diffTimers[timerKey] = setTimeout(() => {
        _diffTimers[timerKey] = null;
        _doFetchVttyDiff(leafId, instUrl, cmdId, false);
    }, delayMs);
}

    // ─── Exports ───
    Object.assign(window, {
        connectPanelWs, disconnectPanelWs, updateWsQualityIndicator,
        startPanelPoll, stopPanelPoll, pollOncePanel,
        startPanelUpdateMode, stopPanelUpdateMode,
        fetchVttyDiffForPanel,
        _sharedSubs,
        _connectLeafWs,
        _loadLeafVttyHttpDirect,
        _fetchLeafDiff,
        _applyLeafDiff,
    });
})();