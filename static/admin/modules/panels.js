// ─── Panels: Render ───
(function() {
    'use strict';

function _renderSearchBar(pid) {
    return `<div class="search-bar" id="searchBar-${pid}">
  <input type="text" id="searchInput-${pid}" data-search-panel="${pid}" placeholder="Search terminal...">
  <span class="search-count" id="searchCount-${pid}" title="Click to jump: Shift+Click to reverse"></span>
  <div class="search-progress-bar hidden" id="searchProgress-${pid}"></div>
  <button data-action="VttySearchNext" data-panel="${pid}" title="Next (Enter)">&#x25BC;</button>
  <button data-action="VttySearchPrev" data-panel="${pid}" title="Prev (Shift+Enter)">&#x25B2;</button>
  <button data-action="VttySearchClose" data-panel="${pid}" title="Close">&#x2715;</button>
</div>`;
}

function renderPanels() {
    const container = document.getElementById('view-vtty');
    const visiblePanels = _getVisiblePanels();
    const visible = visiblePanels.filter(p => !p.minimized);
    const multi = visible.length > 1;

    let hasCmds = state.connections.some(i => i._commands?.length > 0);
    const showWelcome = !hasCmds && !state.selectedCmdId && !state.serverReachable;
    if (showWelcome !== state._showingWelcome) state._showingWelcome = showWelcome;

    const cached = {};
    // Cache VTTY content from ALL panels whose DOM still exists (not just visible).
    // Walk the split tree for each panel to find all leaves.
    if (!state._lastGeneration) state._lastGeneration = {};
    for (const panel of state.panels) {
        const el = document.getElementById(panel.id);
        if (!el) {
            // DOM was destroyed — clear generation cache for all leaves
            const allLeaves = (typeof _getAllLeaves === 'function') ? _getAllLeaves(panel) : [{ leaf: panel }];
            for (const { leaf, side } of allLeaves) {
                const isPanelLeaf = !side;
                const cmdId = isPanelLeaf ? panel.selectedCmdId : leaf.cmdId;
                if (cmdId) {
                    const gk = (isPanelLeaf ? panel.id : leaf.id) + '/' + cmdId;
                    delete state._lastGeneration[gk];
                }
            }
            continue;
        }
        _cacheVtty(panel.id, document.getElementById('vtty-' + panel.id), panel.selectedCmdId, cached);
        if ((panel.split || panel._rootSplit) && typeof _getAllLeaves === 'function') {
            const allLeaves = _getAllLeaves(panel);
            for (const { leaf, side } of allLeaves) {
                if (!side) continue;
                _cacheVtty(leaf.id, document.getElementById('vtty-' + leaf.id), leaf.cmdId, cached);
            }
        }
    }
    let html = '';

    if (showWelcome) {
        state._showingWelcome = true;
        const tb = document.getElementById('sharedToolbar');
        if (tb) tb.classList.add('hidden');
        html = `<div class="welcome-panel"><div class="welcome-card">
            <img src="/favicon.png" alt="vrw" style="height:2rem;width:auto;margin-bottom:0.75rem;">
            <p class="welcome-not-running">vrw is not running</p>
            <p style="margin-top:0.25rem;">No vrw instance could be reached at <span class="welcome-url">${escHtml(getBaseUrl())}</span></p>
            <p>Start vrw and refresh this page to connect.</p></div></div>`;
    } else {
        state._showingWelcome = false;
        const tb = document.getElementById('sharedToolbar');
        if (tb) tb.classList.remove('hidden');

        // Render window tab bar (only if >1 window) — outside panel-area so it's always on top
        if (typeof _renderWindowBar === 'function') html += _renderWindowBar();

        const isMobile = state._mobileTabbedLayout;
        if (isMobile && visible.length > 1) {
            html += '<div class="mobile-tab-bar" id="mobileTabBar">';
            for (const panel of visible) {
                const focused = panel.id === state._focusedPanelId;
                const label = _getPanelLabel(panel);
                html += `<div class="mobile-tab${focused ? ' active' : ''}" data-action="FocusPanel" data-panel="${panel.id}" title="${escHtml(label)}">
                    <span class="mobile-tab-label">${escHtml(label)}</span>
                    ${multi ? `<button class="mobile-tab-close" data-action="ClosePanelContent" data-panel="${panel.id}" title="Remove">&#x2715;</button>` : ''}
                </div>`;
            }
            html += '</div>';
        }

        // Open panel-area wrapper — panels go inside here
        html += '<div class="panel-area" id="panelArea">';

        for (const panel of visible) {
            if (panel.minimized) continue;
            const conn = panel.selectedInstUrl ? state.connections.find(i => i.url === panel.selectedInstUrl) : null;
            const serverLabel = _getServerLabel(conn, panel.selectedInstUrl);
            const color = _getServerColor(conn);
            const textColor = _getServerTextColor(conn);
            const focused = panel.id === state._focusedPanelId;
            const mHide = isMobile && multi && !focused ? ' hidden' : '';
            html += `<div class="panel${focused ? ' focused' : ''}" id="${panel.id}"${mHide}>
${panel.split ? _renderSplitContainer(panel) : `<div class="panel-header" data-panel-id="${panel.id}" data-leaf-id="${panel.id}" data-ctxmenu="panel" data-panel="${panel.id}" data-leaf="${panel.id}" tabindex="0" role="button" aria-label="Panel: ${escHtml(panel.selectedInstUrl || 'empty')}" style="--ph-bg:${color};--ph-fg:${textColor}">
    <button class="btn btn-xs cmd-history-btn hidden" id="histBack-${panel.id}" data-action="PanelHistoryBack" data-panel="${panel.id}" data-leaf="${panel.id}" title="Back">&#x25C0;</button>
    <button class="btn btn-xs cmd-history-btn hidden" id="histFwd-${panel.id}" data-action="PanelHistoryForward" data-panel="${panel.id}" data-leaf="${panel.id}" title="Forward">&#x25B6;</button>
    <div class="cmd-info" id="cmdInfo-${panel.id}">
        <span class="cmd-fullname" id="cmdName-${panel.id}" data-leaf-id="${panel.id}" title="Double-click to rename"></span>
        <span class="cmd-args" id="cmdArgs-${panel.id}"></span>
    </div>
    <span class="panel-exit-banner hidden" id="exitedBanner-${panel.id}"></span>
    <span class="panel-reach-dot unknown" id="panelReachDot-${panel.id}" title="Server state"></span>
    <span class="panel-header-meta" id="panelMeta-${panel.id}"></span>
    <button class="cmd-freeze-btn panel-freeze-btn hidden" id="panelFreezeBtn-${panel.id}" data-action="TogglePauseRunLeaf" data-panel="${panel.id}" data-leaf="${panel.id}" title="Freeze/Thaw command">&#8545;</button>
    <button class="panel-close-btn" data-action="ClosePanelContent" data-panel="${panel.id}" title="Close panel">&#x2715;</button>
</div>` + _renderVttyContainer(panel)}
</div>
${multi ? `<div class="panel-resize-handle" data-panel="${panel.id}"></div>` : ''}`;
        }

        // Close panel-area wrapper
        html += '</div>';
    }

    html += _renderMinimizedPanels();
    container.innerHTML = html;
    // Apply layout class AFTER innerHTML so #panelArea exists in the DOM.
    // Never fall back to the container itself — #view-vtty must stay flex-column.
    _applyPanelLayoutClass(container);
    setupPanelHeaderDrag();

    for (const [id, c] of Object.entries(cached)) {
        const el = document.getElementById(id);
        if (!el) continue;
        const pre = el.querySelector('pre');
        if (pre) { pre.innerHTML = ''; pre.appendChild(c.frag); }
        const vtty = el.querySelector('.vtty-container');
        if (vtty) vtty.scrollTop = c.scrollTop;
    }

    _setupPanelDelegation();
    localStorage.setItem('vrw_panel_count', String(state.panels.length));
    _updatePanelMultiUI();
    if (!state._showingWelcome) updateSharedToolbar();

    if (!state._showingWelcome && state.bufferView === 'current') {
        const pending = state._panelsNeedingFetch;
        state._panelsNeedingFetch = null;
        for (const p of visiblePanels) {
            // Fetch for panel root leaf
            if (p.selectedCmdId && p.selectedInstUrl) {
                const mustFetch = pending && pending.has(p.id);
                if (mustFetch) {
                    loadVttyHttpForPanel(p.id, p.selectedInstUrl, p.selectedCmdId);
                }
                // Check if this panel is already subscribed to a shared WS for its command.
                // p.ws is a borrowed ref from _sharedSubs and may be null even when connected,
                // so check the shared pool directly.
                if (state.updateMode === 'push') {
                    const key = p.selectedInstUrl + '/' + p.selectedCmdId;
                    const sub = _sharedSubs ? _sharedSubs[key] : null;
                    if (!sub || !sub.panels || !sub.panels.has(p.id)) {
                        connectPanelWs(p.id);
                    }
                } else {
                    startPanelUpdateMode(p.id);
                }
            }
            // Fetch for all branch/deeper leaves in the split tree
            if ((p.split || p._rootSplit) && typeof _getAllLeaves === 'function') {
                const leaves = _getAllLeaves(p);
                for (const { leaf, side } of leaves) {
                    if (!side || !leaf.cmdId || !leaf.instUrl) continue;
                    // Only fetch/reconnect if the leaf's DOM was destroyed (mustFetch)
                    // or if the leaf has no active WS/poll connection.
                    const leafMustFetch = pending && pending.has(leaf.id);
                    const hasWs = leaf.ws && leaf.ws.readyState === WebSocket.OPEN;
                    const hasPoll = leaf.pollTimer;
                    if (leafMustFetch || (!hasWs && !hasPoll)) {
                        if (typeof _loadLeafVttyHttpDirect === 'function') {
                            _loadLeafVttyHttpDirect(leaf);
                        }
                        if (state.updateMode === 'push' && typeof _connectLeafWs === 'function') {
                            _connectLeafWs(leaf);
                        } else if (leaf.cmdId && typeof _loadLeafVttyHttpDirect === 'function') {
                            leaf.pollTimer = setInterval(() => { _loadLeafVttyHttpDirect(leaf); }, state.pollInterval);
                        }
                    }
                }
            }
        }
    }
}

    // ── Exports ──
    Object.assign(window, {
        renderPanels, _renderSearchBar,
    });
})();