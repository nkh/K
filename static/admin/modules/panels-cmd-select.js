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

function _handleSecondarySelect(panelObj, instUrl, cmdId) {
    _disconnectSecondaryWs(panelObj);
    if (panelObj.split.secondaryPollTimer) { clearInterval(panelObj.split.secondaryPollTimer); panelObj.split.secondaryPollTimer = null; }
    panelObj.split.secondaryInstUrl = instUrl;
    panelObj.split.secondaryCmdId = cmdId;
    panelObj.split.secondaryScrollbackOffset = 0;
    state.selectedInstUrl = instUrl; state.selectedCmdId = cmdId; state.bufferView = 'current';
    _loadSecondaryVttyHttp(panelObj);
    if (state.updateMode === 'push') _connectSecondaryWs(panelObj);
    else panelObj.split.secondaryPollTimer = setInterval(() => { if (panelObj.split?.secondaryCmdId) _loadSecondaryVttyHttp(panelObj); }, state.pollInterval);
    _updateSplitPanelHeader(panelObj);
    updateSidebarSelection();
}

function selectCommand(instUrl, cmdId, name) {
    let panelObj = state.panels.find(p => p.id === state._focusedPanelId) || state.panels[0];
    if (!panelObj) return;
    _pushPanelHistory(panelObj);
    focusPanel(panelObj.id);
    if (panelObj.split?.activeSide === 'secondary') { _handleSecondarySelect(panelObj, instUrl, cmdId); return; }
    _selectCommandForPanel(panelObj, instUrl, cmdId, { cache: true, resetBuffers: true, scrollback: true });
    _updatePanelHistoryBtns(panelObj.id);
}

function _openCommandInNewPane(instUrl, cmdId, cmdName) {
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
    });
})();