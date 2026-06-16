// ─── Panels: Render ───
(function() {
    'use strict';

function _renderSearchBar(pid) {
    return `<div class="search-bar" id="searchBar-${pid}">
  <input type="text" id="searchInput-${pid}" placeholder="Search terminal..." oninput="vttySearch('${pid}')" onkeydown="if(event.key==='Enter'){event.shiftKey?vttySearchPrev('${pid}'):vttySearchNext('${pid}')}">
  <span class="search-count" id="searchCount-${pid}" title="Click to jump: Shift+Click to reverse"></span>
  <div class="search-progress-bar hidden" id="searchProgress-${pid}"></div>
  <button data-action="VttySearchNext" data-panel="${pid}" title="Next (Enter)">&#x25BC;</button>
  <button data-action="VttySearchPrev" data-panel="${pid}" title="Prev (Shift+Enter)">&#x25B2;</button>
  <button data-action="VttySearchClose" data-panel="${pid}" title="Close">&#x2715;</button>
</div>`;
}

function renderPanels() {
    const container = document.getElementById('view-vtty');
    const visible = state.panels.filter(p => !p.minimized);
    const multi = visible.length > 1;

    let hasCmds = state.connections.some(i => i._commands?.length > 0);
    const showWelcome = !hasCmds && !state.selectedCmdId && !state.serverReachable;
    if (showWelcome !== state._showingWelcome) state._showingWelcome = showWelcome;

    const cached = {};
    for (const panel of state.panels) {
        const el = document.getElementById(panel.id);
        if (!el) continue;
        _cacheVtty(panel.id, document.getElementById('vtty-' + panel.id), panel.selectedCmdId, cached);
        if (panel.split?.secondaryCmdId) {
            const sid = panel.id + '-secondary';
            _cacheVtty(sid, document.getElementById('vtty-' + sid), panel.split.secondaryCmdId, cached);
        }
    }

    _applyPanelLayoutClass(container);
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

        const isMobile = state._mobileTabbedLayout;
        if (isMobile && state.panels.length > 1) {
            html += '<div class="mobile-tab-bar" id="mobileTabBar">';
            for (const panel of state.panels) {
                const focused = panel.id === state._focusedPanelId;
                const label = _getPanelLabel(panel);
                html += `<div class="mobile-tab${focused ? ' active' : ''}" data-action="FocusPanel" data-panel="${panel.id}" title="${escHtml(label)}">
                    <span class="mobile-tab-label">${escHtml(label)}</span>
                    ${multi ? `<button class="mobile-tab-close" data-action="ClosePanelContent" data-panel="${panel.id}" title="Remove">&#x2715;</button>` : ''}
                </div>`;
            }
            html += '</div>';
        }

        for (const panel of state.panels) {
            if (panel.minimized) continue;
            const conn = panel.selectedInstUrl ? state.connections.find(i => i.url === panel.selectedInstUrl) : null;
            const serverLabel = _getServerLabel(conn, panel.selectedInstUrl);
            const color = _getServerColor(conn);
            const textColor = _getServerTextColor(conn);
            const focused = panel.id === state._focusedPanelId;
            const mHide = isMobile && multi && !focused ? ' hidden' : '';
            html += `<div class="panel${focused ? ' focused' : ''}" id="${panel.id}" draggable="false" ondragover="onPanelDragOver(event)" ondrop="onPanelDrop(event,'${panel.id}')" ondragleave="onPanelDragLeave(event)"${mHide}>
<div class="panel-header" data-panel-id="${panel.id}" oncontextmenu="showPanelContextMenu(event,'${panel.id}')" tabindex="0" role="button" aria-label="Panel: ${escHtml(panel.selectedInstUrl || 'empty')}" style="--ph-bg:${color};--ph-fg:${textColor};background:var(--ph-bg);color:var(--ph-fg);">
    ${multi ? `<span class="drag-handle" draggable="true" ondragstart="onPanelDragStart(event,'${panel.id}')" ondragend="onPanelDragEnd(event)" title="Drag to reorder">&#x2840;</span>` : ''}
    <button class="panel-close-btn" data-action="ClosePanelContent" data-panel="${panel.id}" title="Close panel">&#x2715;</button>
    <button class="btn btn-xs cmd-history-btn hidden" id="histBack-${panel.id}" data-action="PanelHistoryBack" data-panel="${panel.id}" title="Back">&#x25C0;</button>
    <button class="btn btn-xs cmd-history-btn hidden" id="histFwd-${panel.id}" data-action="PanelHistoryForward" data-panel="${panel.id}" title="Forward">&#x25B6;</button>
    <div class="cmd-info" id="cmdInfo-${panel.id}">
        <span class="cmd-fullname" id="cmdName-${panel.id}" ondblclick="event.stopPropagation();startRenamePanel('${panel.id}')" title="Double-click to rename"></span>
        <span class="cmd-args" id="cmdArgs-${panel.id}"></span>
    </div>
    <span class="panel-reach-dot unknown" id="panelReachDot-${panel.id}" title="Server state"></span>
    <span class="panel-header-meta" id="panelMeta-${panel.id}"></span>
    <button class="cmd-freeze-btn panel-freeze-btn hidden" id="panelFreezeBtn-${panel.id}" data-action="TogglePauseRunPanel" data-panel="${panel.id}" title="Freeze/Thaw command">&#8545;</button>
</div>
${panel.split ? _renderSplitContainer(panel) : _renderVttyContainer(panel)}
</div>
${multi ? `<div class="panel-resize-handle" data-panel="${panel.id}"></div>` : ''}`;
        }
    }

    html += _renderMinimizedPanels();
    container.innerHTML = html;

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
        for (const p of state.panels) {
            if (p.selectedCmdId && p.selectedInstUrl && (!p.ws || p.ws.readyState !== WebSocket.OPEN)) {
                startPanelUpdateMode(p.id);
            }
        }
    }
}

    // ── Exports ──
    Object.assign(window, {
        renderPanels, _renderSearchBar,
    });
})();