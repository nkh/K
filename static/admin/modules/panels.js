// ─── Panels ───
(function() {
    'use strict';

// ─── Shared Helpers ───
function _findCmd(instUrl, cmdId) {
    const inst = instUrl ? state.connections.find(i => i.url === instUrl) : null;
    return inst && inst._commands ? inst._commands.find(c => c.id === cmdId) : null;
}

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

function _showCopyFeedback(pid) {
    const el = document.getElementById('copyFeedback-' + pid);
    if (el) { el.classList.add('visible'); setTimeout(() => el.classList.remove('visible'), 1200); }
}

function _getPanelLabel(panel) {
    if (panel.customTitle) return panel.customTitle;
    if (!panel.selectedCmdId) return 'Panel';
    for (const inst of state.connections) {
        if (inst._commands) {
            const cmd = inst._commands.find(c => c.id === panel.selectedCmdId);
            if (cmd) return cmd.name || cmd.id;
        }
    }
    return 'Panel';
}

function _findPanelVtty(pid) {
    return { panelObj: state.panels.find(p => p.id === pid), vttyEl: document.getElementById(pid)?.querySelector('.vtty-container') };
}

function _setToggleBtn(ids, active, offTitle, onTitle) {
    const btn = ids.map(id => document.getElementById(id)).find(Boolean);
    if (btn) { btn.classList.toggle('btn-primary', active); btn.title = active ? onTitle : offTitle; }
}

function _getResizeDims(pid) {
    const rv = id => parseInt(document.getElementById(id)?.value) || 0;
    return { rows: rv('stResizeRows') || rv('resizeRows-' + pid) || 24, cols: rv('stResizeCols') || rv('resizeCols-' + pid) || 80 };
}

function _addCtxSep(menu) {
    const s = document.createElement('div');
    s.className = 'ctx-menu-sep'; s.setAttribute('role', 'separator');
    menu.appendChild(s);
}

function _setText(id, text) { const el = document.getElementById(id); if (el) el.textContent = text; }

// ─── Panels (Multi-view) ───
function addPanelDirect() {
    const id = 'panel-' + Date.now() + '-' + Math.random().toString(36).slice(2, 6);
    const savedFontSize = parseInt(localStorage.getItem('vrw_panel_font_' + id));
    const fontSize = (savedFontSize >= 8 && savedFontSize <= 28) ? savedFontSize : state.fontSize;
    const savedSelMode = localStorage.getItem('vrw_panel_sel_' + id);
    const selectionMode = savedSelMode === 'true';
    const savedTheme = localStorage.getItem('vrw_panel_theme_' + id);
    const theme = (savedTheme === 'light' || savedTheme === 'dark') ? savedTheme : '';
    const customTitle = localStorage.getItem('vrw_panel_title_' + id) || '';
    const panel = { id, scrollbackOffset: 0, mouseTracking: false, mouseSgr: false, focused: false, fontSize, selectionMode, theme, customTitle, minimized: false, selectedCmdId: null, selectedInstUrl: null,
        ws: null, wsCmdId: null, wsInstUrl: null, wsReconnectCount: 0, wsReconnectTimer: null, wsPingInterval: null, wsPingSendTime: 0, wsLatency: 0,
        pollTimer: null,
        cmdHistory: [], cmdHistoryIdx: -1,
    };
    state.panels.push(panel);
    renderPanels();
    return panel;
}

function addPanel() {
    addPanelDirect();
    const newPanel = state.panels[state.panels.length - 1];
    if (newPanel) focusPanel(newPanel.id);
}

function closePanelModal() {
    releaseCurrentFocusTrap();
    document.getElementById('panelModal').classList.add('hidden');
}

function confirmAddPanel() {
    const url = document.getElementById('panelUrl').value.trim();
    if (!url) return;
    const token = document.getElementById('panelToken').value.trim();
    const splitDir = document.getElementById('panelSplitDir').value;
    let label = document.getElementById('panelLabel').value.trim();
    if (!label) { try { label = new URL(url).host; } catch (e) { label = url; } }
    try {
        addConnection(url, label, token);
        addPanelDirect();
        closePanelModal();
        if (splitDir === 'vertical') state.panelLayout = 'column';
        else if (splitDir === 'horizontal') state.panelLayout = 'row';
        localStorage.setItem('vrw_panel_layout', state.panelLayout);
        const newPanel = state.panels[state.panels.length - 1];
        if (newPanel) { newPanel.selectedInstUrl = url; state._pendingSelectId = null; }
        renderPanels();
        loadCommands();
        loadCertificates();
        fetchServerTemplates();
    } catch (e) {
        console.error('[vrw] confirmAddPanel failed:', e);
        closePanelModal();
    }
}

function removePanel(id) {
    disconnectPanelWs(id);
    stopPanelPoll(id);
    state.panels = state.panels.filter(p => p.id !== id);
    if (state.panels.length <= 1) {
        state.panelLayout = 'row';
        localStorage.setItem('vrw_panel_layout', state.panelLayout);
    } else if (state.panelLayout.startsWith('grid-')) {
        const needed = { 'grid-2x2': 4, 'grid-1-2': 3, 'grid-2-1': 3 };
        if (state.panels.length !== needed[state.panelLayout]) {
            state.panelLayout = 'row';
            localStorage.setItem('vrw_panel_layout', state.panelLayout);
        }
    }
    if (state._focusedPanelId === id) state._focusedPanelId = state.panels.length > 0 ? state.panels[0].id : null;
    renderPanels();
    updateSharedToolbar();
}

function toggleMinimizePanel(panelId) {
    const p = state.panels.find(pp => pp.id === panelId);
    if (!p) return;
    p.minimized = !p.minimized;
    if (p.minimized) {
        if (state._focusedPanelId === panelId) {
            const vis = state.panels.find(pp => !pp.minimized && pp.id !== panelId);
            if (vis) focusPanel(vis.id);
        }
    } else { focusPanel(panelId); }
    renderPanels();
}

function splitPanel(panelId, direction) {
    const p = state.panels.find(pp => pp.id === panelId);
    if (!p || p.split) return;
    p.split = {
        direction, splitRatio: 0.5, activeSide: 'primary',
        secondaryCmdId: null, secondaryInstUrl: null,
        secondaryWs: null, secondaryWsCmdId: null, secondaryWsInstUrl: null,
        secondaryWsReconnectCount: 0, secondaryWsReconnectTimer: null,
        secondaryWsPingInterval: null, secondaryWsPingSendTime: 0, secondaryWsLatency: 0,
        secondaryPollTimer: null, secondaryScrollbackOffset: 0,
        secondaryMouseTracking: false, secondaryMouseSgr: false,
    };
    renderPanels();
}

function unsplitPanel(panelId) {
    const p = state.panels.find(pp => pp.id === panelId);
    if (!p || !p.split) return;
    _disconnectSecondaryWs(p);
    if (p.split.secondaryPollTimer) clearInterval(p.split.secondaryPollTimer);
    p.split = null;
    renderPanels();
}

function _renderVttyContainer(panel) {
    const themeAttr = panel.theme ? 'data-panel-theme="' + panel.theme + '"' : '';
    return `<div class="vtty-container${panel.selectionMode ? ' selection-mode' : ''}" id="vtty-${panel.id}" ${themeAttr} style="font-size: ${panel.fontSize}px;">
    <div class="exited-banner hidden" id="exitedBanner-${panel.id}"></div>
    ${_renderSearchBar(panel.id)}
    <pre style="color:#484f58;">No command selected — select a command from the sidebar to view its output</pre>
    <div class="cursor-indicator hidden"></div>
    <div class="copy-feedback" id="copyFeedback-${panel.id}">Copied!</div>
    <button class="scroll-bottom-btn" id="scrollBtn-${panel.id}" data-action="ScrollTerminalBottom" data-panel="${panel.id}" title="Scroll to bottom">&#x25BC;</button>
</div>`;
}

function _getServerLabel(inst, instUrl) {
    if (inst?._serverName) return inst._serverName;
    if (inst?.label) return inst.label;
    if (!instUrl) return '';
    try {
        const u = new URL(instUrl);
        if (u.port) return u.port;
        const def = u.protocol === 'https:' ? 443 : u.protocol === 'http:' ? 80 : 0;
        return String(def || '');
    } catch { return instUrl; }
}

const _serverBg = ['var(--bg-tertiary)','#2d1f3d','#1f3d2d','#3d2d1f','#1f2d3d','#3d1f2d','#2d3d1f','#1f3d3d'];
const _serverFg = ['var(--text-primary)','#d4b8e8','#b8e8d4','#e8d4b8','#b8d4e8','#e8b8d4','#d4e8b8','#b8e8e8'];

function _getServerColor(inst) {
    if (!inst) return 'var(--bg-tertiary)';
    const idx = state.connections.indexOf(inst);
    if (idx <= 0) return 'var(--bg-tertiary)';
    if (state._serverPanelColors?.length) return state._serverPanelColors[(idx - 1) % state._serverPanelColors.length].background || 'var(--bg-tertiary)';
    return _serverBg[idx % _serverBg.length];
}

function _getServerTextColor(inst) {
    if (!inst) return 'var(--text-primary)';
    const idx = state.connections.indexOf(inst);
    if (idx <= 0) return 'var(--text-primary)';
    if (state._serverPanelColors?.length) return state._serverPanelColors[(idx - 1) % state._serverPanelColors.length].text || 'var(--text-primary)';
    return _serverFg[idx % _serverFg.length];
}

function _getPanelCmdLabel(cmdId, instUrl) {
    if (!cmdId) return 'No command';
    const cmd = _findCmd(instUrl, cmdId);
    return cmd ? (cmd.name || cmd.id) : cmdId;
}

function _updateSplitHeaders(panelObj) {
    if (!panelObj?.split) return;
    const el = document.getElementById(panelObj.id);
    if (!el) return;
    const sides = [
        { key: 'primary', instUrl: panelObj.selectedInstUrl, cmdId: panelObj.selectedCmdId },
        { key: 'secondary', instUrl: panelObj.split.secondaryInstUrl, cmdId: panelObj.split.secondaryCmdId },
    ];
    for (const { key, instUrl, cmdId } of sides) {
        const h = el.querySelector(`.split-header[data-split-side="${key}"]`);
        if (!h) continue;
        const inst = instUrl ? state.connections.find(i => i.url === instUrl) : null;
        h.style.background = _getServerColor(inst);
        h.style.color = _getServerTextColor(inst);
        const sl = h.querySelector('.split-server-label');
        if (sl) sl.textContent = _getServerLabel(inst, instUrl);
        const cl = h.querySelector('.split-cmd-label');
        if (cl) cl.textContent = _getPanelCmdLabel(cmdId, instUrl);
    }
}

function _renderSplitPane(panel, side, paneId, widthPct, serverLabel, color, textColor, cmdLabel, showSearch) {
    const selMode = panel.selectionMode ? ' selection-mode' : '';
    const themeAttr = panel.theme ? 'data-panel-theme="' + panel.theme + '"' : '';
    const searchHtml = showSearch ? _renderSearchBar(paneId) : '';
    const bannerStyle = side === 'secondary' ? ' style="display:none;"' : ' class="hidden"';
    const noCmdText = cmdLabel === 'No command' ? '<span style="color:#484f58;">No command selected — select a command from the sidebar</span>' : '';
    return `<div class="split-pane" data-split-side="${side}" data-panel="${panel.id}" style="flex: 0 0 ${widthPct}%; display:flex; flex-direction:column; min-width:0; min-height:0;">
<div class="split-header panel-header" data-panel-id="${panel.id}" data-split-side="${side}" style="background:${color};color:${textColor};">
    <span class="split-server-label" style="font-size:var(--ui-fs);opacity:0.8;">${escHtml(serverLabel)}</span>
    <span class="split-cmd-label" style="font-size:var(--ui-fs);font-family:var(--font-mono);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;min-width:0;">${escHtml(cmdLabel)}</span>
    <button class="btn btn-xs btn-danger" data-action="UnsplitPanel" data-panel="${panel.id}" title="Close split">&#x2715;</button>
</div>
<div class="vtty-container${selMode}" id="vtty-${paneId}" data-split-side="${side}" data-panel="${panel.id}" ${themeAttr} style="font-size: ${panel.fontSize}px; flex:1; min-height:0;">
    <div class="exited-banner"${bannerStyle} id="exitedBanner-${paneId}"></div>
    ${searchHtml}
    <pre>${noCmdText}</pre>
    <div class="cursor-indicator hidden"></div>
    ${showSearch ? `<div class="copy-feedback" id="copyFeedback-${paneId}">Copied!</div>` : ''}
    <button class="scroll-bottom-btn" id="scrollBtn-${paneId}" data-action="ScrollTerminalBottom" data-panel="${paneId}" title="Scroll to bottom">&#x25BC;</button>
</div></div>`;
}

function _renderSplitContainer(panel) {
    const s = panel.split, dir = s.direction;
    const secondaryId = panel.id + '-secondary';
    const pw = s.splitRatio ? (s.splitRatio * 100).toFixed(1) : '50';
    const sw = (100 - parseFloat(pw)).toFixed(1);
    const pi = panel.selectedInstUrl ? state.connections.find(i => i.url === panel.selectedInstUrl) : null;
    const si = s.secondaryInstUrl ? state.connections.find(i => i.url === s.secondaryInstUrl) : null;
    const ph = _renderSplitPane(panel, 'primary', panel.id, pw, _getServerLabel(pi, panel.selectedInstUrl), _getServerColor(pi), _getServerTextColor(pi), _getPanelCmdLabel(panel.selectedCmdId, panel.selectedInstUrl), true);
    const sh = _renderSplitPane(panel, 'secondary', secondaryId, sw, _getServerLabel(si, s.secondaryInstUrl), _getServerColor(si), _getServerTextColor(si), _getPanelCmdLabel(s.secondaryCmdId, s.secondaryInstUrl), false);
    return `<div class="split-container ${dir}" id="split-${panel.id}" data-panel="${panel.id}">${ph}<div class="split-divider" data-panel="${panel.id}"></div>${sh}</div>`;
}

function _updateSplitPanelHeader(panelObj) {
    if (!panelObj?.split) return;
    _updateSplitHeaders(panelObj);
    const el = document.getElementById(panelObj.id);
    const nameEl = el?.querySelector(':scope > .panel-header .cmd-fullname');
    if (!nameEl) return;
    const s = panelObj.split;
    const { cmdId, instUrl } = s.activeSide === 'secondary'
        ? { cmdId: s.secondaryCmdId, instUrl: s.secondaryInstUrl }
        : { cmdId: panelObj.selectedCmdId, instUrl: panelObj.selectedInstUrl };
    if (cmdId && instUrl) {
        const cmd = _findCmd(instUrl, cmdId);
        const fullName = cmd ? (cmd.name || cmd.id) : cmdId;
        nameEl.textContent = panelObj.customTitle || fullName;
        nameEl.title = fullName;
        const argsEl = el.querySelector(':scope > .panel-header .cmd-args');
        if (argsEl && cmd) argsEl.textContent = (cmd.args || []).join(' ');
    }
}

function _renderMinimizedPanels() {
    const minimized = state.panels.filter(p => p.minimized);
    if (!minimized.length) return '';
    let html = '<div class="minimized-panels" id="minimizedPanels">';
    for (const panel of minimized) {
        const label = _getPanelLabel(panel);
        html += `<div class="minimized-panel-item" data-action="ToggleMinimizePanel" data-panel="${panel.id}" title="Click to restore: ${escHtml(label)}">
            <span class="minimized-icon">&#x25A0;</span><span class="minimized-label">${escHtml(label)}</span></div>`;
    }
    return html + '</div>';
}

function togglePanelLayout() {
    state.panelLayout = state.panelLayout === 'row' ? 'column' : 'row';
    localStorage.setItem('vrw_panel_layout', state.panelLayout);
    renderPanels();
}

function toggleLayoutPresetMenu(event) {
    event.stopPropagation();
    const menu = document.getElementById('layoutPresetMenu');
    const isVisible = !menu.classList.contains('hidden');
    menu.classList.toggle('hidden', isVisible);
    if (!isVisible) {
        setTimeout(() => {
            document.addEventListener('click', function closeMenu(e) {
                document.removeEventListener('click', closeMenu);
                menu.classList.add('hidden');
            }, { once: true });
        }, 0);
    }
}

function applyLayoutPreset(preset) {
    const menu = document.getElementById('layoutPresetMenu');
    if (menu) menu.classList.add('hidden');
    const panelCounts = { 'row': null, 'column': null, 'grid-2x2': 4, 'grid-1-2': 3, 'grid-2-1': 3 };
    const neededCount = panelCounts[preset];
    if (neededCount !== null) {
        while (state.panels.length > neededCount) {
            const removed = state.panels.pop();
            disconnectPanelWs(removed.id); stopPanelPoll(removed.id);
        }
        while (state.panels.length < neededCount) addPanelDirect();
    }
    state.panelLayout = preset;
    localStorage.setItem('vrw_panel_layout', state.panelLayout);
    renderPanels();
    if (state.panels.length > 0) {
        if (!state.panels.find(p => p.id === state._focusedPanelId)) focusPanel(state.panels[0].id);
    }
}

function _applyPanelLayoutClass(container) {
    if (state._mobileTabbedLayout) {
        container.classList.remove('grid-2x2', 'grid-1-2', 'grid-2-1');
        container.style.flexDirection = 'column';
        return;
    }
    container.classList.remove('grid-2x2', 'grid-1-2', 'grid-2-1');
    if (state.panelLayout.startsWith('grid-')) {
        container.classList.add(state.panelLayout);
        container.style.flexDirection = '';
    } else {
        container.style.flexDirection = state.panelLayout;
    }
}

function _cacheVtty(id, vttyEl, cmdId, out) {
    if (!vttyEl) return;
    const pre = vttyEl.querySelector('pre');
    if (!pre || !pre.childNodes.length || !cmdId) return;
    const frag = document.createDocumentFragment();
    while (pre.firstChild) frag.appendChild(pre.firstChild);
    out[id] = { frag, scrollTop: vttyEl.scrollTop, cmdId };
}

function renderPanels() {
    const container = document.getElementById('view-vtty');
    const visible = state.panels.filter(p => !p.minimized);
    const multi = visible.length > 1;

    let hasCmds = state.connections.some(i => i._commands?.length > 0);
    const showWelcome = !hasCmds && !state.selectedCmdId && !state.serverReachable;
    if (showWelcome !== _showingWelcome) _showingWelcome = showWelcome;

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
        _showingWelcome = true;
        const tb = document.getElementById('sharedToolbar');
        if (tb) tb.classList.add('hidden');
        html = `<div class="welcome-panel"><div class="welcome-card">
            <img src="/favicon.png" alt="vrw" style="height:2rem;width:auto;margin-bottom:0.75rem;">
            <p class="welcome-not-running">vrw is not running</p>
            <p style="margin-top:0.25rem;">No vrw instance could be reached at <span class="welcome-url">${escHtml(getBaseUrl())}</span></p>
            <p>Start vrw and refresh this page to connect.</p></div></div>`;
    } else {
        _showingWelcome = false;
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
<div class="panel-header" data-panel-id="${panel.id}" oncontextmenu="showPanelContextMenu(event,'${panel.id}')" tabindex="0" role="button" aria-label="Panel: ${escHtml(panel.selectedInstUrl || 'empty')}" style="background:${color};color:${textColor};">
    ${multi ? `<span class="drag-handle" draggable="true" ondragstart="onPanelDragStart(event,'${panel.id}')" ondragend="onPanelDragEnd(event)" title="Drag to reorder">&#x2840;</span>` : ''}
    <button class="btn btn-xs btn-danger panel-close-btn" data-action="ClosePanelContent" data-panel="${panel.id}" title="Close panel">&#x2715;</button>
    <span class="panel-server-badge" style="font-size:var(--ui-fs);opacity:0.7;flex-shrink:0;">${escHtml(serverLabel)}</span>
    <button class="btn btn-xs cmd-history-btn hidden" id="histBack-${panel.id}" data-action="PanelHistoryBack" data-panel="${panel.id}" title="Back">&#x25C0;</button>
    <button class="btn btn-xs cmd-history-btn hidden" id="histFwd-${panel.id}" data-action="PanelHistoryForward" data-panel="${panel.id}" title="Forward">&#x25B6;</button>
    <div class="cmd-info" id="cmdInfo-${panel.id}">
        <span class="cmd-fullname" id="cmdName-${panel.id}" ondblclick="event.stopPropagation();startRenamePanel('${panel.id}')" title="Double-click to rename"></span>
        <span class="cmd-args" id="cmdArgs-${panel.id}"></span>
    </div>
    <span class="panel-header-label" id="panelLabel-${panel.id}"></span>
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
    if (!_showingWelcome) updateSharedToolbar();

    if (!_showingWelcome && state.bufferView === 'current') {
        for (const p of state.panels) {
            if (p.selectedCmdId && p.selectedInstUrl && (!p.ws || p.ws.readyState !== WebSocket.OPEN)) {
                startPanelUpdateMode(p.id);
            }
        }
    }
}

let _panelDelegated = false;
function _setupPanelDelegation() {
    if (_panelDelegated) return;
    _panelDelegated = true;
    const container = document.getElementById('view-vtty');
    if (!container) return;

    container.addEventListener('mousedown', (e) => {
        const divider = e.target.closest('.split-divider');
        if (divider) {
            e.preventDefault();
            const pid = divider.getAttribute('data-panel');
            const panelObj = state.panels.find(p => p.id === pid);
            if (!panelObj?.split) return;
            const splitContainer = divider.parentElement;
            const dir = panelObj.split.direction;
            divider.classList.add('active');
            const startPos = dir === 'horizontal' ? e.clientX : e.clientY;
            const cSize = dir === 'horizontal' ? splitContainer.offsetWidth : splitContainer.offsetHeight;
            const startRatio = panelObj.split.splitRatio || 0.5;
            const onMove = (ev) => {
                const pos = dir === 'horizontal' ? ev.clientX : ev.clientY;
                let ratio = Math.max(0.1, Math.min(0.9, startRatio + (pos - startPos) / cSize));
                panelObj.split.splitRatio = ratio;
                const panes = splitContainer.querySelectorAll('.split-pane');
                if (panes.length === 2) {
                    const p1 = (ratio * 100).toFixed(1), p2 = (100 - parseFloat(p1)).toFixed(1);
                    panes[0].style.flex = `0 0 ${p1}%`; panes[1].style.flex = `0 0 ${p2}%`;
                }
            };
            const onUp = () => { divider.classList.remove('active'); document.removeEventListener('mousemove', onMove); document.removeEventListener('mouseup', onUp); };
            document.addEventListener('mousemove', onMove);
            document.addEventListener('mouseup', onUp);
            return;
        }
        const panelEl = e.target.closest('.panel');
        if (!panelEl) return;
        const pid = panelEl.id;
        focusPanel(pid);
        const el = e.target.closest('.vtty-container') || e.target.closest('.split-header');
        if (el) {
            const side = el.getAttribute('data-split-side');
            if (side) { const p = state.panels.find(pp => pp.id === pid); if (p?.split) p.split.activeSide = side; }
        }
    });

    container.addEventListener('scroll', (e) => {
        const vtty = e.target.closest('.vtty-container');
        if (!vtty) return;
        const btn = vtty.querySelector('.scroll-bottom-btn');
        if (btn) btn.classList.toggle('visible', vtty.scrollHeight - vtty.scrollTop - vtty.clientHeight >= 50);
    }, true);
}

function _updatePanelMultiUI() {
    const multi = state.panels.length > 1, isGrid = state.panelLayout.startsWith('grid-');
    document.querySelectorAll('.drag-handle').forEach(el => el.classList.toggle('hidden', !multi));
    document.querySelectorAll('.panel-resize-handle').forEach(el => el.classList.toggle('hidden', !(multi && !isGrid)));
    const lb = document.getElementById('stLayoutBtn');
    if (lb) lb.classList.toggle('hidden', !multi);
    const pb = document.getElementById('stLayoutPresetBtn');
    if (pb) pb.classList.toggle('hidden', !multi);
}

function focusPanel(panelId) {
    if (state._focusedPanelId === panelId) return;
    state._focusedPanelId = panelId;
    document.querySelectorAll('.panel').forEach(el => el.classList.toggle('focused', el.id === panelId));
    if (state._mobileTabbedLayout) {
        document.querySelectorAll('.panel').forEach(el => el.classList.toggle('hidden', el.id !== panelId));
        document.querySelectorAll('.mobile-tab').forEach(el => el.classList.toggle('active', el.getAttribute('data-panel') === panelId));
    }
    const panelObj = state.panels.find(p => p.id === panelId);
    if (panelObj) { state.selectedInstUrl = panelObj.selectedInstUrl; state.selectedCmdId = panelObj.selectedCmdId; }
    updateSharedToolbar();
}

function updateSharedToolbar() {
    const pid = getActivePanelId();
    const panelObj = state.panels.find(p => p.id === pid);
    if (!panelObj) return;
    _setText('stFontSize', panelObj.fontSize + 'px');
    const themeBtn = document.getElementById('stPanelThemeBtn');
    if (themeBtn) {
        themeBtn.textContent = panelObj.theme === 'light' ? '\u263E' : panelObj.theme === 'dark' ? '\u2600' : '\u25D0';
        themeBtn.title = 'Panel theme: ' + (panelObj.theme || 'inherit') + ' (click to toggle)';
    }
    const selectBtn = document.getElementById('stSelectBtn');
    if (selectBtn) { selectBtn.classList.toggle('btn-primary', panelObj.selectionMode); selectBtn.textContent = panelObj.selectionMode ? '\u2713 Select' : 'Select'; }
    _setText('stInstanceUrl', (panelObj.selectedInstUrl || '').replace(/^https?:\/\//, ''));
    _setText('stRefreshVal', state.refreshMs || 'off');
    const bufferSel = document.getElementById('stBufferSelect');
    if (bufferSel) bufferSel.value = state.bufferView || 'current';
    const resourceBadge = document.getElementById('stResourceBadge');
    if (resourceBadge && panelObj.selectedCmdId) {
        const res = state._resourceCache[panelObj.selectedCmdId];
        if (state.showResources && res && (res.cpu_percent != null || res.memory_mb != null)) {
            resourceBadge.classList.remove('hidden');
            resourceBadge.textContent = (res.cpu_percent != null ? 'CPU ' + res.cpu_percent.toFixed(1) + '%' : '') + (res.memory_mb != null ? ' MEM ' + res.memory_mb.toFixed(1) + 'MB' : '');
        } else { resourceBadge.classList.add('hidden'); }
    }
    const restartBtn = document.getElementById('stRestartBtn');
    if (restartBtn) restartBtn.classList.toggle('hidden', !panelObj.selectedCmdId);
    const freezeBtn = document.getElementById('stFreezeBtn');
    if (freezeBtn) {
        if (panelObj.selectedCmdId) {
            const cmd = _findCmd(panelObj.selectedInstUrl, panelObj.selectedCmdId);
            const isAlive = cmd && cmd.alive !== false, isFrozen = cmd && cmd.frozen === true;
            freezeBtn.classList.toggle('hidden', !isAlive);
            freezeBtn.textContent = isFrozen ? '\u25B6' : '\u23F8';
            freezeBtn.title = isFrozen ? 'Thaw command' : 'Freeze command';
            freezeBtn.classList.toggle('btn-primary', isFrozen);
        } else { freezeBtn.classList.add('hidden'); }
    }
    _setToggleBtn(['stMaxFitBtn', 'maxFitBtn-' + pid], !!(_maxFitState[pid]?.active), 'Auto-fit terminal to panel', 'Restore previous size');
    _setToggleBtn(['stMaxFontBtn', 'maxFontBtn-' + pid], !!(_maxFontState[pid]?.active), 'Maximize font to fit', 'Restore previous font size');
}

async function sendKeysToPanel(panelId) {
    const panel = state.panels.find(p => p.id === panelId);
    if (!panel) return;
    const input = document.getElementById('stKeyInput') || document.getElementById('keyInput-' + panelId);
    if (!input || !input.value || !state.selectedCmdId) return;
    const keysValue = input.value;
    const cmdId = panel.selectedCmdId || state.selectedCmdId;
    const instUrl = panel.selectedInstUrl || state.selectedInstUrl;
    try {
        const json = await api.sendKeys(instUrl, cmdId, { keys: keysValue });
        input.value = '';
        if (json.status === 'ok') loadVttyHttpForPanel(panelId, instUrl, cmdId);
        else console.error('send_keys server error:', json.error);
    } catch (e) { console.error('send_keys error:', e); }
}

function showSpecialKeysHelp() {
    const old = document.getElementById('specialKeysModal');
    if (old) { old.remove(); return; }
    const overlay = document.createElement('div');
    overlay.id = 'specialKeysModal'; overlay.className = 'modal-overlay';
    overlay.onclick = (e) => { if (e.target === overlay) { releaseCurrentFocusTrap(); overlay.remove(); } };
    const rows = [
        ['Return / Enter', '<code>&lt;Enter&gt;</code> or <code>&lt;Return&gt;</code>', 'Send a newline (carriage return)'],
        ['Backspace', '<code>&lt;Backspace&gt;</code>', 'Delete character before cursor'],
        ['Tab', '<code>&lt;Tab&gt;</code>', 'Insert a tab character'],
        ['Escape', '<code>&lt;Esc&gt;</code>', 'Send the Escape character (0x1B)'],
        ['Space', '(space character)', 'Type a literal space'],
        ['Delete', '<code>&lt;Delete&gt;</code>', 'Delete character at cursor (forward delete)'],
        ['Insert', '<code>&lt;Insert&gt;</code>', 'Toggle insert/overwrite mode'],
        ['Home / End', '<code>&lt;Home&gt;</code> <code>&lt;End&gt;</code>', 'Jump to beginning / end of line'],
        ['Page Up / Down', '<code>&lt;PageUp&gt;</code> <code>&lt;PageDown&gt;</code>', 'Scroll up / down one page'],
        ['Arrow Keys', '<code>&lt;Up&gt;</code> <code>&lt;Down&gt;</code> <code>&lt;Left&gt;</code> <code>&lt;Right&gt;</code>', 'Cursor movement'],
        ['F1 – F12', '<code>&lt;F1&gt;</code> … <code>&lt;F12&gt;</code>', 'Function keys'],
        ['Ctrl + key', '<code>&lt;C-c&gt;</code> <code>&lt;C-a&gt;</code> …', 'Control modifier (lowercase). <code>&lt;C-c&gt;</code> = SIGINT'],
        ['Alt + key', '<code>&lt;A-x&gt;</code> <code>&lt;A-enter&gt;</code> …', 'Alt/Meta prefix (Escape + key)'],
    ];
    const p = 'padding:0.25rem 0.5rem;', th = p + 'color:var(--text-muted);font-weight:600;';
    const tbody = rows.map((r, i) => `<tr style="${i < rows.length - 1 ? 'border-bottom:1px solid var(--border);' : ''}"><td style="${p}">${r[0]}</td><td style="${p}">${r[1]}</td><td style="${p}color:var(--text-secondary);">${r[2]}</td></tr>`).join('');
    overlay.innerHTML = `<div class="modal" style="max-width:560px;max-height:80vh;overflow-y:auto;">
<h2 style="margin-bottom:0.5rem;">Special Keys Reference</h2>
<p style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.75rem;">Type special keys using <code style="background:var(--bg-tertiary);padding:0.1rem 0.3rem;border-radius:2px;">&lt;KeyName&gt;</code> syntax. Mix with text: <code style="background:var(--bg-tertiary);padding:0.1rem 0.3rem;border-radius:2px;">hello&lt;Enter&gt;world</code>.</p>
<table style="width:100%;font-size:0.75rem;border-collapse:collapse;">
<thead><tr style="border-bottom:1px solid var(--border);text-align:left;"><th style="${th}">Key</th><th style="${th}">Syntax</th><th style="${th}">Description</th></tr></thead>
<tbody>${tbody}</tbody></table>
<div style="margin-top:0.75rem;text-align:right;"><button class="btn btn-xs" data-action="CloseSpecialKeysModal">Close</button></div></div>`;
    document.body.appendChild(overlay);
    const modal = overlay.querySelector('.modal');
    if (modal) trapFocus(modal);
    overlay.querySelector('button')?.focus();
}

// ─── Panel Resize via Drag ───
(function() {
    let resizing = false, startX = 0, startWidth = 0, resizePanel = null;
    document.addEventListener('mousedown', (e) => {
        const handle = e.target.closest('.panel-resize-handle');
        if (!handle) return;
        e.preventDefault();
        resizePanel = handle.previousElementSibling;
        if (!resizePanel) return;
        startX = e.clientX; startWidth = resizePanel.getBoundingClientRect().width;
        handle.classList.add('active'); resizing = true;
    });
    document.addEventListener('mousemove', (e) => {
        if (!resizing || !resizePanel) return;
        const cw = resizePanel.parentElement.getBoundingClientRect().width;
        const pc = resizePanel.parentElement.children.length;
        const nw = Math.max(100, Math.min(cw - (pc - 1) * 100, startWidth + e.clientX - startX));
        resizePanel.style.flex = `0 0 ${(nw / cw) * 100}%`;
    });
    document.addEventListener('mouseup', () => {
        if (resizing) { document.querySelectorAll('.panel-resize-handle.active').forEach(h => h.classList.remove('active')); resizing = false; resizePanel = null; }
    });
})();

// ─── Export Terminal Output ───
function copyTerminalSelection(panelId) {
    let text = window.getSelection()?.toString().trim() || '';
    if (!text) { const pre = document.querySelector(`#${panelId} pre`); if (pre) text = pre.textContent || pre.innerText || ''; }
    if (!text) return;
    navigator.clipboard.writeText(text).then(() => _showCopyFeedback(panelId)).catch(() => {
        const ta = document.createElement('textarea');
        ta.value = text; ta.style.cssText = 'position:fixed;opacity:0;';
        document.body.appendChild(ta); ta.select();
        try { document.execCommand('copy'); } catch {}
        document.body.removeChild(ta); _showCopyFeedback(panelId);
    });
}

function exportTerminal(panelId) {
    const pre = document.querySelector(`#${panelId} pre`);
    if (!pre) return;
    const text = pre.textContent || pre.innerText || '';
    const cmd = _findCmd(state.selectedInstUrl, state.selectedCmdId);
    const cmdName = cmd ? (cmd.name || cmd.id).replace(/\//g, '_') : 'terminal';
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([text], { type: 'text/plain' }));
    a.download = cmdName + '.txt'; a.click(); URL.revokeObjectURL(a.href);
}

async function screenshotPanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const instUrl = panelObj.selectedInstUrl, cmdId = panelObj.selectedCmdId;
    if (!instUrl || !cmdId) { alert('No command selected to screenshot.'); return; }
    const fontSize = state.serverScreenshotFontSize || 12;
    const fontName = state.serverScreenshotFontName || 'monospace';
    const params = new URLSearchParams({ font_size: fontSize });
    if (fontName !== 'monospace') params.set('font_name', fontName);
    try {
        const blob = await api.getVttyPng(instUrl, cmdId, Object.fromEntries(params));
        const cmd = _findCmd(instUrl, cmdId);
        const parts = cmd ? [cmd.name || 'unknown', ...(cmd.args || [])] : ['vrw'];
        const cmdInfo = parts.join(' ').replace(/[^a-zA-Z0-9_\-\.]/g, '_').substring(0, 120);
        const now = new Date(), pad = n => String(n).padStart(2, '0');
        const ts = `${now.getFullYear()}${pad(now.getMonth()+1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
        const pre = document.querySelector(`#vtty-${panelId} pre`);
        const dims = (pre && pre._vttyRows) ? pre._vttyRows + 'x' + pre._vttyCols : '';
        const a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = `vrw_${ts}${dims ? '_' + dims : ''}_${cmdInfo}.png`;
        a.click(); URL.revokeObjectURL(a.href);
    } catch (e) { alert('Screenshot failed: ' + e.message); }
}

// ─── Right-click Context Menu ───
let _ctxMenuFocusedIndex = -1;

function closeContextMenu() {
    const el = document.getElementById('ctxMenu');
    if (el) el.remove();
    _ctxMenuFocusedIndex = -1;
}

function _createCtxMenuItem(label, onClick, isDanger) {
    const div = document.createElement('div');
    div.className = 'ctx-menu-item' + (isDanger ? ' danger' : '');
    div.setAttribute('role', 'menuitem'); div.setAttribute('tabindex', '-1');
    div.textContent = label;
    div.addEventListener('click', () => { onClick(); closeContextMenu(); });
    return div;
}

function _positionCtxMenu(menu, x, y) {
    menu.style.left = x + 'px'; menu.style.top = y + 'px';
    document.body.appendChild(menu);
    const rect = menu.getBoundingClientRect();
    if (rect.right > window.innerWidth) menu.style.left = (window.innerWidth - rect.width - 4) + 'px';
    if (rect.bottom > window.innerHeight) menu.style.top = (window.innerHeight - rect.height - 4) + 'px';
}

function _setupCtxMenuListeners(menu) {
    setTimeout(() => { document.addEventListener('click', closeContextMenu, { once: true }); }, 0);
    menu.addEventListener('keydown', (e) => {
        const items = menu.querySelectorAll('.ctx-menu-item');
        if (!items.length) return;
        if (e.key === 'ArrowDown') { e.preventDefault(); _ctxMenuFocusedIndex = (_ctxMenuFocusedIndex + 1) % items.length; }
        else if (e.key === 'ArrowUp') { e.preventDefault(); _ctxMenuFocusedIndex = (_ctxMenuFocusedIndex - 1 + items.length) % items.length; }
        else if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); if (_ctxMenuFocusedIndex >= 0) items[_ctxMenuFocusedIndex].click(); return; }
        else if (e.key === 'Escape') { e.preventDefault(); closeContextMenu(); return; }
        else if (e.key === 'Tab') { e.preventDefault(); closeContextMenu(); return; }
        else return;
        _focusCtxMenuItem(items);
    });
    _ctxMenuFocusedIndex = 0;
    const firstItem = menu.querySelector('.ctx-menu-item');
    if (firstItem) firstItem.focus();
}

function _focusCtxMenuItem(items) {
    items.forEach((item, i) => { item.classList.toggle('ctx-menu-focused', i === _ctxMenuFocusedIndex); if (i === _ctxMenuFocusedIndex) item.focus(); });
}

function showCmdContextMenu(e, instUrl, cmdId, cmdName, isAlive, isRetained) {
    e.preventDefault(); closeContextMenu();
    const menu = document.createElement('div');
    menu.id = 'ctxMenu'; menu.className = 'ctx-menu'; menu.setAttribute('role', 'menu');
    menu.appendChild(_createCtxMenuItem('View Terminal', () => selectCommand(instUrl, cmdId, cmdName)));
    menu.appendChild(_createCtxMenuItem('Copy URL', () => copyCommandUrl(instUrl, cmdId, cmdName)));
    const groups = getCmdGroups(), groupNames = Object.keys(groups);
    if (groupNames.length > 0) {
        _addCtxSep(menu);
        for (const gName of groupNames) {
            const inGroup = groups[gName].includes(cmdName);
            menu.appendChild(_createCtxMenuItem((inGroup ? '✓ ' : '') + escHtml(gName), () => toggleCmdInGroup(gName, cmdName)));
        }
    }
    _addCtxSep(menu);
    if (isAlive) {
        menu.appendChild(_createCtxMenuItem(isRetained ? 'Unkeep' : 'Keep', () => toggleKeepCmd(instUrl, cmdId)));
        menu.appendChild(_createCtxMenuItem('Pause/Resume', () => togglePauseCmd(instUrl, cmdId)));
        menu.appendChild(_createCtxMenuItem('Restart', () => restartCommandById(instUrl, cmdId)));
        menu.appendChild(_createCtxMenuItem('Kill', () => killCommand(instUrl, cmdId), true));
    } else {
        menu.appendChild(_createCtxMenuItem('Purge', () => purgeCommand(instUrl, cmdId, cmdName), true));
    }
    _positionCtxMenu(menu, e.clientX, e.clientY);
    _setupCtxMenuListeners(menu);
}

function showPanelContextMenu(e, panelId) {
    e.preventDefault(); closeContextMenu();
    const panel = state.panels.find(p => p.id === panelId);
    if (!panel) return;
    const instUrl = panel.selectedInstUrl, cmdId = panel.selectedCmdId;
    const menu = document.createElement('div');
    menu.id = 'ctxMenu'; menu.className = 'ctx-menu'; menu.setAttribute('role', 'menu');
    menu.appendChild(_createCtxMenuItem('Copy URL', () => {
        if (cmdId) { const cmd = _findCmd(instUrl, cmdId); copyCommandUrl(instUrl, cmdId, cmd ? (cmd.name || cmd.id) : cmdId); }
        else navigator.clipboard.writeText(instUrl).catch(() => {});
    }));
    if (cmdId) {
        menu.appendChild(_createCtxMenuItem('Pause/Resume', () => togglePauseCmd(instUrl, cmdId)));
        menu.appendChild(_createCtxMenuItem('Restart', () => restartCommandById(instUrl, cmdId)));
        menu.appendChild(_createCtxMenuItem('Kill', () => killCommand(instUrl, cmdId), true));
    }
    menu.appendChild(_createCtxMenuItem('Rename Panel', () => startRenamePanel(panelId)));
    if (state.panels.length > 1) {
        menu.appendChild(_createCtxMenuItem(panel.minimized ? 'Restore Panel' : 'Minimize Panel', () => toggleMinimizePanel(panelId)));
    }
    _addCtxSep(menu);
    if (!panel.split) {
        menu.appendChild(_createCtxMenuItem('Split Horizontal', () => splitPanel(panelId, 'horizontal')));
        menu.appendChild(_createCtxMenuItem('Split Vertical', () => splitPanel(panelId, 'vertical')));
    } else {
        menu.appendChild(_createCtxMenuItem('Unsplit', () => unsplitPanel(panelId)));
    }
    _addCtxSep(menu);
    if (state.panels.length > 1) menu.appendChild(_createCtxMenuItem('Remove Panel', () => removePanel(panelId), true));
    _positionCtxMenu(menu, e.clientX, e.clientY);
    _setupCtxMenuListeners(menu);
}

// ─── Panel title rename ───
function startRenamePanel(panelId) {
    const panelEl = document.getElementById(panelId), nameEl = panelEl?.querySelector('.cmd-fullname');
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!nameEl || !panelObj || nameEl.getAttribute('contenteditable') === 'true') return;
    nameEl.contentEditable = 'true'; nameEl.classList.add('panel-title-editing');
    nameEl.textContent = panelObj.customTitle || ''; nameEl.focus();
    const range = document.createRange(); range.selectNodeContents(nameEl);
    const sel = window.getSelection(); sel.removeAllRanges(); sel.addRange(range);
    nameEl._renameOriginal = panelObj.customTitle || '';
    const onKeydown = (e) => { e.preventDefault(); finishRenamePanel(panelId, e.key === 'Enter'); };
    const onBlur = () => setTimeout(() => finishRenamePanel(panelId, true), 100);
    const onInput = () => { nameEl.textContent = nameEl.textContent.replace(/\n/g, ' '); };
    nameEl.addEventListener('keydown', onKeydown);
    nameEl.addEventListener('blur', onBlur);
    nameEl.addEventListener('input', onInput);
    nameEl._renameHandlers = { keydown: onKeydown, blur: onBlur, input: onInput };
}

function finishRenamePanel(panelId, save) {
    const panelEl = document.getElementById(panelId), nameEl = panelEl?.querySelector('.cmd-fullname');
    if (!nameEl || nameEl.getAttribute('contenteditable') !== 'true') return;
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    if (nameEl._renameHandlers) {
        nameEl.removeEventListener('keydown', nameEl._renameHandlers.keydown);
        nameEl.removeEventListener('blur', nameEl._renameHandlers.blur);
        nameEl.removeEventListener('input', nameEl._renameHandlers.input);
        delete nameEl._renameHandlers;
    }
    nameEl.contentEditable = 'false'; nameEl.classList.remove('panel-title-editing');
    if (save) {
        const t = nameEl.textContent.trim();
        panelObj.customTitle = t;
        if (t) localStorage.setItem('vrw_panel_title_' + panelId, t);
        else localStorage.removeItem('vrw_panel_title_' + panelId);
    }
    updatePanelCommandInfo();
}

function copyCommandUrl(instUrl, cmdId, cmdName) {
    const base = cmdName.replace(/.*\//, '');
    navigator.clipboard.writeText(instUrl.replace(/^http/, 'http') + '/' + encodeURIComponent(base)).catch(() => {});
}

async function togglePauseCmd(instUrl, cmdId) {
    const prevInstUrl = state.selectedInstUrl, prevCmdId = state.selectedCmdId;
    state.selectedInstUrl = instUrl; state.selectedCmdId = cmdId;
    await togglePauseRun();
    if (prevInstUrl !== instUrl || prevCmdId !== cmdId) { state.selectedInstUrl = prevInstUrl; state.selectedCmdId = prevCmdId; }
}

// ─── Auto-fit Terminal on Window Resize ───
function autoFitActiveTerminal() {
    if (!state.selectedInstUrl || !state.selectedCmdId) return;
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    if (!vttyEl) return;
    const rect = vttyEl.getBoundingClientRect();
    if (rect.width < 10 || rect.height < 10) return;
    const charW = state.fontSize * 0.6, charH = state.fontSize * 1.2;
    const cols = Math.max(20, Math.min(500, Math.floor(rect.width / charW)));
    const rows = Math.max(5, Math.min(200, Math.floor(rect.height / charH)));
    if (rows !== state._termRows || cols !== state._termCols) api.resize(state.selectedInstUrl, state.selectedCmdId, { rows, cols }).catch(() => {});
}

async function _resizePanelTo(panelId, rows, cols) {
    const p = state.panels.find(pp => pp.id === panelId);
    if (!p || !p.selectedCmdId) return false;
    const cmd = _findCmd(p.selectedInstUrl, p.selectedCmdId);
    if (cmd && cmd.status === 'exited') return false;
    try { await api.resize(p.selectedInstUrl, p.selectedCmdId, { rows, cols }); return true; } catch { return false; }
}

const _maxFitState = {};

async function toggleMaxFit(panelId) {
    const { panelObj, vttyEl } = _findPanelVtty(panelId);
    if (!panelObj || !vttyEl) return;
    const st = _maxFitState[panelId];
    const btnIds = ['stMaxFitBtn', 'maxFitBtn-' + panelId];
    if (st?.active) {
        st.active = false;
        _setToggleBtn(btnIds, false, 'Auto-fit terminal to panel', 'Restore previous size');
        if (!(await _resizePanelTo(panelId, st.prevRows, st.prevCols))) delete _maxFitState[panelId];
    } else {
        const rect = vttyEl.getBoundingClientRect();
        if (rect.width < 10 || rect.height < 10) return;
        const cmd = _findCmd(panelObj.selectedInstUrl, panelObj.selectedCmdId);
        if (panelObj.selectedCmdId && cmd?.status === 'exited') return;
        const fs = panelObj.fontSize || state.fontSize;
        const maxCols = Math.max(20, Math.min(500, Math.floor(rect.width / (fs * 0.6))));
        const maxRows = Math.max(5, Math.min(200, Math.floor(rect.height / (fs * 1.2))));
        const { rows: curRows, cols: curCols } = _getResizeDims(panelId);
        _maxFitState[panelId] = { prevRows: curRows, prevCols: curCols, active: true };
        _setToggleBtn(btnIds, true, 'Auto-fit terminal to panel', 'Restore previous size');
        if (!(await _resizePanelTo(panelId, maxRows, maxCols))) delete _maxFitState[panelId];
    }
}

const _maxFontState = {};

async function toggleMaxFont(panelId) {
    const { panelObj, vttyEl } = _findPanelVtty(panelId);
    if (!panelObj || !vttyEl) return;
    const st = _maxFontState[panelId];
    const btnIds = ['stMaxFontBtn', 'maxFontBtn-' + panelId];
    const { rows: curRows, cols: curCols } = _getResizeDims(panelId);
    if (st?.active) {
        st.active = false;
        _setToggleBtn(btnIds, false, 'Maximize font to fit', 'Restore previous font size');
        panelObj.fontSize = st.prevFontSize;
        localStorage.setItem('vrw_panel_font_' + panelId, String(panelObj.fontSize));
        vttyEl.style.fontSize = panelObj.fontSize + 'px';
        delete _maxFontState[panelId];
    } else {
        const rect = vttyEl.getBoundingClientRect();
        if (rect.width < 10 || rect.height < 10) return;
        const maxFont = Math.max(8, Math.min(28, Math.min(Math.floor(rect.width / (curCols * 0.6)), Math.floor(rect.height / (curRows * 1.2)))));
        _maxFontState[panelId] = { prevFontSize: panelObj.fontSize, active: true };
        _setToggleBtn(btnIds, true, 'Maximize font to fit', 'Restore previous font size');
        panelObj.fontSize = maxFont;
        localStorage.setItem('vrw_panel_font_' + panelId, String(panelObj.fontSize));
        vttyEl.style.fontSize = panelObj.fontSize + 'px';
    }
}

// ─── Drag-and-Drop Panel Reorder ───
let _draggedPanelId = null;

function onPanelDragStart(e, panelId) {
    _draggedPanelId = panelId;
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', panelId);
    setTimeout(() => { const el = document.getElementById(panelId); if (el) el.classList.add('dragging'); }, 0);
}

function onPanelDragOver(e) {
    e.preventDefault();
    e.dataTransfer.dropEffect = _draggedPanelId ? 'move' : 'copy';
    const panel = e.target.closest('.panel');
    if (!panel || panel.id === _draggedPanelId) return;
    const rect = panel.getBoundingClientRect(), midX = rect.left + rect.width / 2;
    panel.classList.remove('drag-over-left', 'drag-over-right');
    panel.classList.add(e.clientX < midX ? 'drag-over-left' : 'drag-over-right');
}

function onPanelDragLeave(e) {
    const panel = e.target.closest('.panel');
    if (panel) panel.classList.remove('drag-over-left', 'drag-over-right');
}

function onPanelDrop(e, targetPanelId) {
    e.preventDefault(); if (e.stopPropagation) e.stopPropagation();
    if (!_draggedPanelId) {
        try {
            const cmdData = JSON.parse(e.dataTransfer.getData('application/x-cmd'));
            if (cmdData?.cmdId) { document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left', 'drag-over-right')); _openCommandInNewPane(cmdData.instUrl, cmdData.cmdId, cmdData.cmdName); return; }
        } catch {}
        onPanelDragEnd(e); return;
    }
    if (_draggedPanelId === targetPanelId) { onPanelDragEnd(e); return; }
    const container = document.getElementById('view-vtty');
    const draggedEl = document.getElementById(_draggedPanelId), targetEl = document.getElementById(targetPanelId);
    if (!draggedEl || !targetEl || !container) { onPanelDragEnd(e); return; }
    const rect = targetEl.getBoundingClientRect(), midX = rect.left + rect.width / 2;
    container.insertBefore(draggedEl, e.clientX < midX ? targetEl : targetEl.nextSibling);
    const handle = draggedEl.nextElementSibling;
    if (handle?.classList.contains('panel-resize-handle')) {
        container.removeChild(handle);
        container.insertBefore(handle, draggedEl.nextElementSibling);
    }
    const newOrder = [];
    container.querySelectorAll('.panel').forEach(el => { const p = state.panels.find(pp => pp.id === el.id); if (p) newOrder.push(p); });
    state.panels = newOrder;
    localStorage.setItem('vrw_panel_order', JSON.stringify(newOrder.map(p => p.id)));
    onPanelDragEnd(e);
}

function onPanelAreaDragOver(e) { e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; }

function onPanelAreaDrop(e) {
    e.preventDefault();
    try { const d = JSON.parse(e.dataTransfer.getData('application/x-cmd')); if (d?.cmdId) _openCommandInNewPane(d.instUrl, d.cmdId, d.cmdName); } catch {}
}

function onPanelDragEnd() {
    _draggedPanelId = null;
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('dragging', 'drag-over-left', 'drag-over-right'));
}

function closePanelContent(panelId) { removePanel(panelId); }

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

// ─── Drag-and-Drop (Sidebar Commands) ───
let _draggedCmd = null;

function onCmdDragStart(e, instUrl, cmdId, cmdName) {
    _draggedCmd = { instUrl, cmdId, cmdName };
    e.dataTransfer.effectAllowed = 'copy';
    e.dataTransfer.setData('text/plain', cmdId);
    e.dataTransfer.setData('application/x-cmd', JSON.stringify({ instUrl, cmdId, cmdName }));
    if (e.target?.style) { e.target.style.opacity = '0.5'; setTimeout(() => { if (e.target?.style) e.target.style.opacity = ''; }, 0); }
}

// ─── Sidebar Command Reorder (mousedown-based) ───
function getCmdOrder() { try { return JSON.parse(localStorage.getItem('vrw_cmd_order') || '{}'); } catch { return {}; } }
function setCmdOrder(order) { localStorage.setItem('vrw_cmd_order', JSON.stringify(order)); }
function getOrderedCmds(instUrl, items) {
    const instOrder = getCmdOrder()[instUrl];
    if (!instOrder) return items;
    const ordered = [], remaining = [];
    for (const item of items) {
        const idx = instOrder.indexOf(item.cmd.id);
        (idx >= 0 ? ordered : remaining).push(idx >= 0 ? { item, idx } : item);
    }
    ordered.sort((a, b) => a.idx - b.idx);
    return [...ordered.map(x => x.item), ...remaining];
}

let _reorderState = null;

function _cmdReorderMouseDown(e, instUrl, cmdId, cmdName) {
    if (e.button !== 0) return;
    e.preventDefault(); e.stopPropagation();
    const srcEl = e.target.closest('.cmd-item');
    if (!srcEl) return;
    const rect = srcEl.getBoundingClientRect();
    _reorderState = { instUrl, cmdId, cmdName: cmdName || cmdId, srcEl, startY: e.clientY, startRect: rect, placeholder: null, offsetY: e.clientY - rect.top, overPane: false };
    document.addEventListener('mousemove', _cmdReorderMouseMove);
    document.addEventListener('mouseup', _cmdReorderMouseUp);
}

function _cmdReorderMouseMove(e) {
    if (!_reorderState) return;
    const s = _reorderState;
    if (Math.abs(e.clientY - s.startY) < 4 && !s.placeholder) return;
    const container = document.getElementById('commandList');
    if (!container) return;
    if (!s.placeholder) {
        const ph = document.createElement('div');
        ph.className = 'cmd-reorder-placeholder';
        ph.style.cssText = 'border-top:2px solid var(--accent);margin:0;pointer-events:none;';
        s.placeholder = ph;
        s.srcEl.parentNode.insertBefore(ph, s.srcEl);
        Object.assign(s.srcEl.style, { position: 'fixed', left: s.startRect.left + 'px', width: s.startRect.width + 'px', zIndex: '1000', opacity: '0.85', pointerEvents: 'none' });
        s.srcEl.classList.add('cmd-dragging');
    }
    s.srcEl.style.top = (e.clientY - s.offsetY) + 'px';
    s.srcEl.classList.add('hidden');
    const underEl = document.elementFromPoint(e.clientX, e.clientY);
    s.srcEl.classList.remove('hidden');
    const overPanel = underEl?.closest('.panel');
    const overArea = underEl?.closest('#view-vtty') && !underEl.closest('#sidebar');
    const wasOverPane = s.overPane;
    s.overPane = !!(overPanel || overArea);
    if (s.overPane !== wasOverPane) {
        document.querySelectorAll('.panel').forEach(p => p.classList.toggle('drag-over-left', s.overPane));
        if (!s.overPane) container.querySelectorAll('.cmd-item').forEach(el => el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom'));
    }
    if (s.overPane) return;
    container.querySelectorAll('.cmd-item').forEach(el => el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom'));
    const target = underEl?.closest('.cmd-item');
    if (!target || target === s.srcEl) return;
    const midY = target.getBoundingClientRect().top + target.getBoundingClientRect().height / 2;
    target.classList.add(e.clientY < midY ? 'cmd-drag-over-top' : 'cmd-drag-over-bottom');
    target.parentNode.insertBefore(s.placeholder, e.clientY < midY ? target : target.nextElementSibling);
}

function _cmdReorderMouseUp() {
    document.removeEventListener('mousemove', _cmdReorderMouseMove);
    document.removeEventListener('mouseup', _cmdReorderMouseUp);
    if (!_reorderState) return;
    const { srcEl, placeholder, instUrl, cmdId, cmdName, overPane } = _reorderState;
    if (srcEl) {
        ['position','left','top','width','zIndex','opacity','pointerEvents'].forEach(p => srcEl.style[p] = '');
        srcEl.classList.remove('cmd-dragging');
    }
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left'));
    const container = document.getElementById('commandList');
    if (container) container.querySelectorAll('.cmd-item').forEach(el => el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom'));
    if (overPane && placeholder) { placeholder.remove(); _openCommandInNewPane(instUrl, cmdId, cmdName); _reorderState = null; return; }
    if (placeholder && container) {
        const nextEl = placeholder.nextElementSibling;
        const targetCmdId = nextEl?.classList.contains('cmd-item') ? nextEl.dataset.cmdId : null;
        placeholder.remove();
        if (targetCmdId && targetCmdId !== cmdId) {
            const order = getCmdOrder();
            let instOrder = order[instUrl] || [];
            instOrder = instOrder.filter(id => id !== cmdId);
            instOrder.splice((instOrder.indexOf(targetCmdId) + 1 || instOrder.length + 1) - 1, 0, cmdId);
            order[instUrl] = instOrder;
            setCmdOrder(order); loadCommands();
        }
    }
    _reorderState = null;
}

function _openCommandInNewPane(instUrl, cmdId, cmdName) {
    const p = addPanelDirect();
    if (p) _selectCommandForPanel(p, instUrl, cmdId);
}

    // ── Exports ──
    Object.assign(window, {
        addPanelDirect, addPanel, closePanelModal, confirmAddPanel,
        removePanel, closePanelContent, toggleMinimizePanel, splitPanel, unsplitPanel,
        renderPanels, focusPanel, updateSharedToolbar, sendKeysToPanel, showSpecialKeysHelp,
        closeSpecialKeysModal: function() { releaseCurrentFocusTrap(); const m = document.getElementById('specialKeysModal'); if (m) m.remove(); },
        togglePanelLayout, toggleLayoutPresetMenu, applyLayoutPreset,
        copyTerminalSelection, exportTerminal, screenshotPanel,
        closeContextMenu, showCmdContextMenu, showPanelContextMenu,
        startRenamePanel, finishRenamePanel, copyCommandUrl, togglePauseCmd,
        autoFitActiveTerminal, toggleMaxFit, toggleMaxFont,
        onPanelDragStart, onPanelDragOver, onPanelDragLeave, onPanelDrop, onPanelDragEnd,
        onPanelAreaDragOver, onPanelAreaDrop,
        _renderVttyContainer, _getServerLabel, _getServerColor, _getServerTextColor,
        _getPanelCmdLabel, _updateSplitHeaders, _renderSplitContainer,
        _updateSplitPanelHeader, _renderMinimizedPanels, _applyPanelLayoutClass,
        _updatePanelMultiUI, _isTerminalVisible, updateSidebarSelection,
        _cacheTerminalForSwitch, _restoreCachedDom,
        _pushPanelHistory, _updatePanelHistoryBtns, panelHistoryBack, panelHistoryForward,
        _selectCommandForPanel, selectCommand,
        onCmdDragStart, getCmdOrder, setCmdOrder, getOrderedCmds,
        _openCommandInNewPane, _findCmd, _renderSearchBar, _showCopyFeedback,
        _getPanelLabel, _renderSplitPane,
    });
})();