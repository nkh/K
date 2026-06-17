// ─── Panels: Layout ───
(function() {
    'use strict';

// ─── Shared Helpers ───
function _findCmd(instUrl, cmdId) {
    const inst = instUrl ? state.connections.find(i => i.url === instUrl) : null;
    return inst && inst._commands ? inst._commands.find(c => c.id === cmdId) : null;
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

function _getServerLabel(inst, instUrl) {
    if (inst?._serverName) return inst._serverName;
    if (inst?.label) {
        try {
            const u = new URL(inst.url);
            if (u.hostname === 'localhost' || u.hostname === '127.0.0.1') return u.port || inst.label;
        } catch {}
        return inst.label;
    }
    if (!instUrl) return '';
    try {
        const u = new URL(instUrl);
        if (u.hostname === 'localhost' || u.hostname === '127.0.0.1') return u.port || '';
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

// ─── Window Management ───
// Windows are tmux-like tabs that each contain their own set of panels.
function _initWindows() {
    if (state.windows.length > 0) return;
    const winId = 'win-0';
    state.windows = [{ id: winId, name: '1', panelIds: [] }];
    state.activeWindowId = winId;
}

function _getActiveWindow() {
    _initWindows();
    return state.windows.find(w => w.id === state.activeWindowId) || state.windows[0];
}

function _getVisiblePanels() {
    const win = _getActiveWindow();
    const ids = new Set(win.panelIds || []);
    return state.panels.filter(p => ids.has(p.id));
}

function switchWindow(winId) {
    if (state.activeWindowId === winId) return;
    // Don't disconnect WS — panels in other windows keep their shared subscriptions.
    // The shared WS pool broadcasts to all subscribed panels regardless of visibility.
    state.activeWindowId = winId;
    const visible = _getVisiblePanels();
    if (visible.length > 0) {
        focusPanel(visible[0].id);
    }
    // Mark visible panels for content re-fetch: their DOM is about to be rebuilt
    // and they won't have cached VTTY content since they weren't in the previous render.
    if (!state._panelsNeedingFetch) state._panelsNeedingFetch = new Set();
    for (const p of visible) {
        if (p.selectedCmdId && p.selectedInstUrl) {
            state._panelsNeedingFetch.add(p.id);
        }
    }
    renderPanels();
}

function createWindow() {
    _initWindows();
    const id = 'win-' + Date.now() + '-' + Math.random().toString(36).slice(2, 6);
    const name = String(state.windows.length + 1);
    state.windows.push({ id, name, panelIds: [] });
    switchWindow(id);
    addPanel();
}

function closeWindow(winId) {
    if (state.windows.length <= 1) return;
    const idx = state.windows.findIndex(w => w.id === winId);
    if (idx < 0) return;
    const win = state.windows[idx];
    for (const pid of (win.panelIds || [])) {
        disconnectPanelWs(pid);
        stopPanelPoll(pid);
        state.panels = state.panels.filter(p => p.id !== pid);
    }
    state.windows.splice(idx, 1);
    if (state.activeWindowId === winId) {
        const newIdx = Math.min(idx, state.windows.length - 1);
        state.activeWindowId = state.windows[newIdx].id;
    }
    renderPanels();
    updateSharedToolbar();
}

function _renderWindowBar() {
    _initWindows();
    if (state.windows.length <= 1) return '';
    let html = '<div class="window-bar" id="windowBar">';
    for (const w of state.windows) {
        const active = w.id === state.activeWindowId;
        const closeBtn = state.windows.length > 1
            ? `<button class="window-tab-close" data-action="CloseWindow" data-value="${w.id}" title="Close window">&#x2715;</button>`
            : '';
        html += `<div class="window-tab${active ? ' active' : ''}" data-action="SwitchWindow" data-value="${w.id}" title="Window ${escHtml(w.name)}"><span class="window-tab-label">${escHtml(w.name)}</span>${closeBtn}</div>`;
    }
    html += `<button class="window-tab-add" data-action="CreateWindow" title="New window">+</button>`;
    html += '</div>';
    return html;
}

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
    // Register in active window BEFORE rendering so _getVisiblePanels includes it
    _initWindows();
    const win = _getActiveWindow();
    if (!win.panelIds) win.panelIds = [];
    win.panelIds.push(panel.id);
    renderPanels();
    return panel;
}

function addPanel() {
    addPanelDirect();
    const newPanel = state.panels[state.panels.length - 1];
    if (newPanel) focusPanel(newPanel.id);
}

function removePanel(id) {
    disconnectPanelWs(id);
    stopPanelPoll(id);
    state.panels = state.panels.filter(p => p.id !== id);
    // Remove from all windows' panelIds
    for (const w of state.windows) {
        if (w.panelIds) w.panelIds = w.panelIds.filter(pid => pid !== id);
    }
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
        // Independent pane — no command inherited; user selects one via drag-drop or sidebar
        secondaryCmdId: null,
        secondaryInstUrl: null,
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
    ${_renderSearchBar(panel.id)}
    <pre style="color:var(--text-muted);">No command selected — select a command from the sidebar to view its output</pre>
    <div class="cursor-indicator hidden"></div>
    <div class="copy-feedback" id="copyFeedback-${panel.id}">Copied!</div>
    <button class="scroll-bottom-btn" id="scrollBtn-${panel.id}" data-action="ScrollTerminalBottom" data-panel="${panel.id}" title="Scroll to bottom">&#x25BC;</button>
</div>`;
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
    const noCmdText = cmdLabel === 'No command' ? '<span style="color:var(--text-muted);">No command selected — select a command from the sidebar</span>' : '';
    return `<div class="split-pane" data-split-side="${side}" data-panel="${panel.id}" style="flex: 0 0 ${widthPct}%; display:flex; flex-direction:column; min-width:0; min-height:0;">
<div class="split-header panel-header" data-panel-id="${panel.id}" data-split-side="${side}" style="--ph-bg:${color};--ph-fg:${textColor};background:var(--ph-bg);color:var(--ph-fg);">
    <span class="split-server-label" style="font-size:var(--ui-fs);opacity:0.8;">${escHtml(serverLabel)}</span>
    <span class="split-cmd-label" style="font-size:var(--ui-fs);font-family:var(--font-mono);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;min-width:0;">${escHtml(cmdLabel)}</span>
    <button class="panel-close-btn" data-action="UnsplitPanel" data-panel="${panel.id}" title="Close split">&#x2715;</button>
</div>
<div class="vtty-container${selMode}" id="vtty-${paneId}" data-split-side="${side}" data-panel="${panel.id}" ${themeAttr} style="font-size: ${panel.fontSize}px; flex:1; min-height:0;">
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
        if (argsEl && cmd) { const a = (cmd.args || []).join(' '); argsEl.textContent = a ? ' ' + a : ''; }
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
    // Target ONLY #panelArea for layout direction.
    // #view-vtty (container) must ALWAYS stay flex-direction: column
    // (window-bar stacked above panel-area). Never fall back to container.
    const area = document.getElementById('panelArea');
    if (!area) return;
    if (state._mobileTabbedLayout) {
        area.classList.remove('grid-2x2', 'grid-1-2', 'grid-2-1');
        area.style.flexDirection = 'column';
        return;
    }
    area.classList.remove('grid-2x2', 'grid-1-2', 'grid-2-1');
    if (state.panelLayout.startsWith('grid-')) {
        area.classList.add(state.panelLayout);
        area.style.flexDirection = '';
    } else {
        area.style.flexDirection = state.panelLayout;
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

function _updatePanelMultiUI() {
    const multi = state.panels.length > 1, isGrid = state.panelLayout.startsWith('grid-');
    document.querySelectorAll('.drag-handle').forEach(el => el.classList.toggle('hidden', !multi));
    document.querySelectorAll('.panel-resize-handle').forEach(el => el.classList.toggle('hidden', !(multi && !isGrid)));
    const lb = document.getElementById('stLayoutBtn');
    if (lb) lb.classList.toggle('hidden', !multi);
    const pb = document.getElementById('stLayoutPresetBtn');
    if (pb) pb.classList.toggle('hidden', !multi);
}

function closePanelContent(panelId) { removePanel(panelId); }

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

    // ── Exports ──
    Object.assign(window, {
        addPanelDirect, addPanel,
        removePanel, closePanelContent, toggleMinimizePanel, splitPanel, unsplitPanel,
        togglePanelLayout, toggleLayoutPresetMenu, applyLayoutPreset,
        startRenamePanel, finishRenamePanel,
        _renderVttyContainer, _getServerLabel, _getServerColor, _getServerTextColor,
        _getPanelCmdLabel, _updateSplitHeaders, _renderSplitContainer,
        _updateSplitPanelHeader, _renderMinimizedPanels, _applyPanelLayoutClass,
        _updatePanelMultiUI, _getPanelLabel, _renderSplitPane,
        _findCmd, _findPanelVtty, _cacheVtty,
        switchWindow, createWindow, closeWindow, _renderWindowBar,
        _getActiveWindow, _getVisiblePanels,
    });
})();