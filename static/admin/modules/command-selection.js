// ─── Command Selection: terminal cache, sidebar highlight, panel history, command switching ───
(function() {
    'use strict';

function _isTerminalVisible() {
    if (state.currentView !== 'vtty') return false;
    if (!state.selectedCmdId) return false;
    return true;
}

function _flushPendingVttyUpdate() {
    if (!state._pendingVttyDirty) return;
    state._pendingVttyDirty = false;
    if (state._pendingVttyData) {
        const data = state._pendingVttyData;
        state._pendingVttyData = null;
        if (data.cells && data.cells.length > 0) {
            applyVttyDiff(data);
        } else {
            updateVttyDisplay(data);
        }
    } else {
        if (state.selectedInstUrl && state.selectedCmdId) {
            loadVttyHttp(state.selectedInstUrl, state.selectedCmdId);
        }
    }
}

/// Cache the terminal display DOM for the currently selected command.
/// Called before switching to a different command.  Moves the <pre> children
/// into a detached DocumentFragment so they can be re-attached instantly on
/// switch-back, avoiding a full HTML fetch when the command hasn't changed.
function _cacheTerminalForSwitch() {
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    const cmdId = state.selectedCmdId;
    if (!pre || !cmdId) return;

    // Detach all children into a DocumentFragment (preserves DOM nodes)
    const frag = document.createDocumentFragment();
    while (pre.firstChild) {
        frag.appendChild(pre.firstChild);
    }
    state._cachedDomPre[cmdId] = frag;
    // Save scroll position for this command
    if (vttyEl) {
        state._cachedScrollPos[cmdId] = vttyEl.scrollTop;
    }
    // Keep _cellGrids and _lastGeneration — they are still valid for the cached DOM.
}

/// Restore a previously cached DOM tree into the <pre> element for instant display.
/// Called from selectCommand() when switching to a command that was viewed before.
/// The cached DOM is moved (not cloned) back into the document, and scroll position
/// is restored.  After this, loadVttyHttp() checks generation — if unchanged, the
/// cached DOM stays; if changed, the full HTML fetch replaces it.
function _restoreCachedDom(cmdId) {
    const frag = state._cachedDomPre[cmdId];
    if (!frag) return;
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;
    // Move the cached DocumentFragment into the <pre> (O(1), no parsing)
    pre.appendChild(frag);
    delete state._cachedDomPre[cmdId];
    // Restore scroll position
    const savedScroll = state._cachedScrollPos[cmdId];
    if (savedScroll !== undefined) {
        vttyEl.scrollTop = savedScroll;
        delete state._cachedScrollPos[cmdId];
    }
}

/// Lightweight DOM-only update: toggle the .selected class on sidebar items
/// without re-fetching /api/commands. Used by selectCommand() to avoid
/// a redundant HTTP roundtrip that would delay the initial VTTY load.
function updateSidebarSelection() {
    document.querySelectorAll('#commandList .cmd-item').forEach(el => {
        const matchInst = el.dataset.instUrl === state.selectedInstUrl;
        const matchCmd = el.dataset.cmdId === state.selectedCmdId;
        el.classList.toggle('selected', matchInst && matchCmd);
    });
}

/// Push current command selection to panel's history before switching.
/// Truncates forward history (like browser back/forward).
function _pushPanelHistory(panelObj) {
    if (!panelObj || !panelObj.selectedCmdId) return;
    // If we're not at the end of history, truncate forward entries
    if (panelObj.cmdHistoryIdx < panelObj.cmdHistory.length - 1) {
        panelObj.cmdHistory = panelObj.cmdHistory.slice(0, panelObj.cmdHistoryIdx + 1);
    }
    // Don't push duplicate of current
    const last = panelObj.cmdHistory[panelObj.cmdHistory.length - 1];
    if (last && last.instUrl === panelObj.selectedInstUrl && last.cmdId === panelObj.selectedCmdId) return;
    panelObj.cmdHistory.push({
        instUrl: panelObj.selectedInstUrl,
        cmdId: panelObj.selectedCmdId,
    });
    panelObj.cmdHistoryIdx = panelObj.cmdHistory.length - 1;
    // Cap history at 50 entries per panel
    if (panelObj.cmdHistory.length > 50) {
        panelObj.cmdHistory.shift();
        panelObj.cmdHistoryIdx--;
    }
}

/// Update back/forward button visibility for a panel.
function _updatePanelHistoryBtns(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    const backBtn = document.getElementById('histBack-' + panelId);
    const fwdBtn = document.getElementById('histFwd-' + panelId);
    if (backBtn) backBtn.style.display = (panelObj && panelObj.cmdHistoryIdx > 0) ? '' : 'none';
    if (fwdBtn) fwdBtn.style.display = (panelObj && panelObj.cmdHistoryIdx < panelObj.cmdHistory.length - 1) ? '' : 'none';
}

/// Navigate back in panel's command history.
function panelHistoryBack(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || panelObj.cmdHistoryIdx <= 0) return;
    panelObj.cmdHistoryIdx--;
    const entry = panelObj.cmdHistory[panelObj.cmdHistoryIdx];
    // Apply selection without pushing to history (we're navigating)
    _selectCommandForPanel(panelObj, entry.instUrl, entry.cmdId);
    _updatePanelHistoryBtns(panelId);
}

/// Navigate forward in panel's command history.
function panelHistoryForward(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || panelObj.cmdHistoryIdx >= panelObj.cmdHistory.length - 1) return;
    panelObj.cmdHistoryIdx++;
    const entry = panelObj.cmdHistory[panelObj.cmdHistoryIdx];
    _selectCommandForPanel(panelObj, entry.instUrl, entry.cmdId);
    _updatePanelHistoryBtns(panelId);
}


/// Internal: switch a panel to a command without recording history.
function _selectCommandForPanel(panelObj, instUrl, cmdId) {
    disconnectPanelWs(panelObj.id);
    panelObj.selectedInstUrl = instUrl;
    panelObj.selectedCmdId = cmdId;
    focusPanel(panelObj.id);
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    state._pendingVttyData = null;
    state._pendingVttyDirty = false;
    state.bufferView = 'current';
    _restoreCachedDom(cmdId);
    const globalBufferSel = document.getElementById('bufferSelect');
    if (globalBufferSel) globalBufferSel.value = 'current';
    updatePanelCommandInfo();
    updateTerminalDisconnectedOverlay();
    updateSidebarSelection();
    loadVttyHttpForPanel(panelObj.id, instUrl, cmdId);
    startPanelUpdateMode(panelObj.id);
}

function selectCommand(instUrl, cmdId, name) {
    // Determine which panel to apply the selection to.
    // If the user clicked in a specific panel, use that; otherwise use the focused panel.
    let panelObj = state.panels.find(p => p.id === state._focusedPanelId);
    if (!panelObj) panelObj = state.panels[0];
    if (!panelObj) return;

    // Check if panel is split and the active side is secondary
    const isSecondary = panelObj.split && panelObj.split.activeSide === 'secondary';

    // Record current command in history before switching
    _pushPanelHistory(panelObj);

    // Ensure this panel is visually focused
    focusPanel(panelObj.id);

    if (isSecondary) {
        // ── Secondary pane command selection ──
        // Disconnect existing secondary WS
        _disconnectSecondaryWs(panelObj);
        if (panelObj.split.secondaryPollTimer) {
            clearInterval(panelObj.split.secondaryPollTimer);
            panelObj.split.secondaryPollTimer = null;
        }

        // Update secondary pane selection
        panelObj.split.secondaryInstUrl = instUrl;
        panelObj.split.secondaryCmdId = cmdId;
        panelObj.split.secondaryScrollbackOffset = 0;

        // Also sync global state so bottom bar etc. work
        state.selectedInstUrl = instUrl;
        state.selectedCmdId = cmdId;

        // Clear any buffered update
        state._pendingVttyData = null;
        state._pendingVttyDirty = false;
        state.bufferView = 'current';

        // Fetch VTTY content for secondary pane
        _loadSecondaryVttyHttp(panelObj);

        // Start secondary WS/poll
        if (state.updateMode === 'push') {
            _connectSecondaryWs(panelObj);
        } else {
            panelObj.split.secondaryPollTimer = setInterval(() => {
                if (panelObj.split && panelObj.split.secondaryCmdId) {
                    _loadSecondaryVttyHttp(panelObj);
                }
            }, state.pollInterval);
        }

        // Update panel header to show secondary command info
        _updateSplitPanelHeader(panelObj);
        updateSidebarSelection();
        return;
    }

    // ── Primary pane command selection (existing behavior) ──
    // Cache the current command's terminal DOM before switching away.
    disconnectPanelWs(panelObj.id);
    _cacheTerminalForSwitch();

    // Update per-panel selection
    panelObj.selectedInstUrl = instUrl;
    panelObj.selectedCmdId = cmdId;
    // Sync global state
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    // Clear any buffered update — we fetch fresh data below
    state._pendingVttyData = null;
    state._pendingVttyDirty = false;
    // Restore cached DOM from previous visit if available (instant display).
    // Then loadVttyHttp will check generation — if unchanged, the cached
    // DOM is kept; if changed, a full HTML fetch replaces it.
    _restoreCachedDom(cmdId);
    state.bufferView = 'current';
    const globalBufferSel = document.getElementById('bufferSelect');
    if (globalBufferSel) globalBufferSel.value = 'current';
    // Reset panel-scoped buffer selects too
    state.panels.forEach(p => {
        const sel = document.getElementById('bufferSelect-' + p.id);
        if (sel) sel.value = 'current';
    });

    // Restore scrollback offset from sessionStorage for the new command
    const savedOffset = sessionStorage.getItem('vrw_scrollback_' + cmdId);
    const restoredOffset = savedOffset !== null ? parseInt(savedOffset, 10) : 0;
    state.panels.forEach(p => p.scrollbackOffset = restoredOffset);

    updatePanelCommandInfo();
    updateTerminalDisconnectedOverlay();
    updateSidebarSelection();
    // Fetch VTTY content — will skip DOM write if generation unchanged
    loadVttyHttpForPanel(panelObj.id, instUrl, cmdId);
    // Start per-panel WS for push mode (or poll)
    startPanelUpdateMode(panelObj.id);
    // Update history button visibility
    _updatePanelHistoryBtns(panelObj.id);
}

    window._isTerminalVisible = _isTerminalVisible;
    window._flushPendingVttyUpdate = _flushPendingVttyUpdate;
    window.updateSidebarSelection = updateSidebarSelection;
    window._cacheTerminalForSwitch = _cacheTerminalForSwitch;
    window._restoreCachedDom = _restoreCachedDom;
    window._pushPanelHistory = _pushPanelHistory;
    window._updatePanelHistoryBtns = _updatePanelHistoryBtns;
    window.panelHistoryBack = panelHistoryBack;
    window.panelHistoryForward = panelHistoryForward;
    window._selectCommandForPanel = _selectCommandForPanel;
    window.selectCommand = selectCommand;
})();
