// ─── Panels: Command Selection ───
(function() {
    'use strict';

// ─── Command Selection ───
function _isTerminalVisible() { return state.currentView === 'vtty' && !!state.selectedCmdId; }

function _cacheTerminalForSwitch() {
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl?.querySelector('pre');
    if (!pre || !state.selectedCmdId) return;
    const frag = document.createDocumentFragment();
    while (pre.firstChild) frag.appendChild(pre.firstChild);
    state._cachedDomPre[state.selectedCmdId] = frag;
    if (vttyEl) state._cachedScrollPos[state.selectedCmdId] = vttyEl.scrollTop;
}

function _restoreCachedDom(cmdId) {
    const frag = state._cachedDomPre[cmdId];
    if (!frag) return;
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl?.querySelector('pre');
    if (!pre) return;
    pre.appendChild(frag);
    delete state._cachedDomPre[cmdId];
    const savedScroll = state._cachedScrollPos[cmdId];
    if (savedScroll !== undefined) { vttyEl.scrollTop = savedScroll; delete state._cachedScrollPos[cmdId]; }
}

function updateSidebarSelection() {
    document.querySelectorAll('#commandList .cmd-item').forEach(el => {
        el.classList.toggle('selected', el.dataset.instUrl === state.selectedInstUrl && el.dataset.cmdId === state.selectedCmdId);
    });
}

function _pushPanelHistory(panelObj) {
    if (!panelObj || !panelObj.selectedCmdId) return;
    if (panelObj.cmdHistoryIdx < panelObj.cmdHistory.length - 1) panelObj.cmdHistory = panelObj.cmdHistory.slice(0, panelObj.cmdHistoryIdx + 1);
    const last = panelObj.cmdHistory[panelObj.cmdHistory.length - 1];
    if (last?.instUrl === panelObj.selectedInstUrl && last.cmdId === panelObj.selectedCmdId) return;
    panelObj.cmdHistory.push({ instUrl: panelObj.selectedInstUrl, cmdId: panelObj.selectedCmdId });
    panelObj.cmdHistoryIdx = panelObj.cmdHistory.length - 1;
    if (panelObj.cmdHistory.length > 50) { panelObj.cmdHistory.shift(); panelObj.cmdHistoryIdx--; }
}

function _updatePanelHistoryBtns(panelId) {
    const p = state.panels.find(pp => pp.id === panelId);
    const backBtn = document.getElementById('histBack-' + panelId), fwdBtn = document.getElementById('histFwd-' + panelId);
    if (backBtn) backBtn.classList.toggle('hidden', !(p && p.cmdHistoryIdx > 0));
    if (fwdBtn) fwdBtn.classList.toggle('hidden', !(p && p.cmdHistoryIdx < p.cmdHistory.length - 1));
}

function panelHistoryBack(panelId) {
    const p = state.panels.find(pp => pp.id === panelId);
    if (!p || p.cmdHistoryIdx <= 0) return;
    p.cmdHistoryIdx--;
    const entry = p.cmdHistory[p.cmdHistoryIdx];
    _selectCommandForPanel(p, entry.instUrl, entry.cmdId);
    _updatePanelHistoryBtns(panelId);
}

function panelHistoryForward(panelId) {
    const p = state.panels.find(pp => pp.id === panelId);
    if (!p || p.cmdHistoryIdx >= p.cmdHistory.length - 1) return;
    p.cmdHistoryIdx++;
    const entry = p.cmdHistory[p.cmdHistoryIdx];
    _selectCommandForPanel(p, entry.instUrl, entry.cmdId);
    _updatePanelHistoryBtns(panelId);
}

function _selectCommandForPanel(panelObj, instUrl, cmdId, { cache = false, resetBuffers = false, scrollback = false } = {}) {
    disconnectPanelWs(panelObj.id);
    if (cache) _cacheTerminalForSwitch();
    panelObj.selectedInstUrl = instUrl;
    panelObj.selectedCmdId = cmdId;
    focusPanel(panelObj.id);
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    state.bufferView = 'current';
    _restoreCachedDom(cmdId);
    if (resetBuffers) {
        const gbs = document.getElementById('bufferSelect');
        if (gbs) gbs.value = 'current';
        state.panels.forEach(p => { const s = document.getElementById('bufferSelect-' + p.id); if (s) s.value = 'current'; });
    }
    if (scrollback) {
        const off = sessionStorage.getItem('vrw_scrollback_' + cmdId);
        state.panels.forEach(p => p.scrollbackOffset = off != null ? parseInt(off, 10) : 0);
    }
    updatePanelCommandInfo();
    updateTerminalDisconnectedOverlay();
    updateSidebarSelection();
    loadVttyHttpForPanel(panelObj.id, instUrl, cmdId);
    startPanelUpdateMode(panelObj.id);
}

function _selectLeafCommand(panelObj, leaf, instUrl, cmdId) {
    // Select a command into a specific leaf (secondary or deeper in the tree)
    _disconnectSingleLeaf(leaf);
    leaf.instUrl = instUrl;
    leaf.cmdId = cmdId;
    leaf.scrollbackOffset = 0;
    state.selectedInstUrl = instUrl; state.selectedCmdId = cmdId; state.bufferView = 'current';
    // Load VTTY content for this leaf using the leaf-aware function
    if (typeof _loadLeafVttyHttpDirect === 'function') {
        _loadLeafVttyHttpDirect(leaf);
    }
    // Connect WS or start polling for this leaf
    if (state.updateMode === 'push') _connectLeafWs(leaf);
    else leaf.pollTimer = setInterval(() => { if (leaf.cmdId && typeof _loadLeafVttyHttpDirect === 'function') _loadLeafVttyHttpDirect(leaf); }, state.pollInterval);
    if (panelObj.split) _updateSplitHeaders(panelObj);
    updateSidebarSelection();
}

function _selectActiveLeafCommand(panelObj, instUrl, cmdId) {
    // Select a command into the currently active leaf of the panel
    if (!panelObj.split) {
        _selectCommandForPanel(panelObj, instUrl, cmdId, { cache: true, resetBuffers: true, scrollback: true });
        return;
    }
    const leafId = (typeof _getFocusedLeafId === 'function') ? _getFocusedLeafId(panelObj) : panelObj.id;
    if (leafId === panelObj.id) {
        // Primary leaf
        _selectCommandForPanel(panelObj, instUrl, cmdId, { cache: true, resetBuffers: true, scrollback: true });
    } else {
        const found = (typeof _findLeafState === 'function') ? _findLeafState(panelObj, leafId) : null;
        if (found && found.leaf) {
            _selectLeafCommand(panelObj, found.leaf, instUrl, cmdId);
        }
    }
}

function selectCommand(instUrl, cmdId, name) {
    let panelObj = state.panels.find(p => p.id === state._focusedPanelId) || state.panels[0];
    if (!panelObj) return;
    _pushPanelHistory(panelObj);
    focusPanel(panelObj.id);
    _selectActiveLeafCommand(panelObj, instUrl, cmdId);
    _updatePanelHistoryBtns(panelObj.id);
}

function _openCommandInNewPane(instUrl, cmdId, cmdName) {
    // Don't open a pane for commands from unreachable servers
    const inst = state.connections.find(i => i.url === instUrl);
    if (inst && inst.reachable === false) return;
    const p = addPanelDirect();
    if (p) _selectCommandForPanel(p, instUrl, cmdId);
}

    // ── Exports ──
    Object.assign(window, {
        _isTerminalVisible, updateSidebarSelection,
        _cacheTerminalForSwitch, _restoreCachedDom,
        _pushPanelHistory, _updatePanelHistoryBtns,
        panelHistoryBack, panelHistoryForward,
        _selectCommandForPanel, selectCommand,
        _openCommandInNewPane,
        _selectLeafCommand, _selectActiveLeafCommand,
    });
})();