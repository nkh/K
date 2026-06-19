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
            ? `<button class="window-tab-close" data-action="CloseWindow" data-window="${w.id}" title="Close window">&#x2715;</button>`
            : '';
        html += `<div class="window-tab${active ? ' active' : ''}" data-action="SwitchWindow" data-window="${w.id}" title="Window ${escHtml(w.name)}"><span class="window-tab-label" ondblclick="event.stopPropagation();startRenameWindow('${w.id}')">${escHtml(w.name)}</span>${closeBtn}</div>`;
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
    const panel = { id, scrollbackOffset: 0, mouseTracking: false, mouseSgr: false, focused: false, fontSize, selectionMode, theme, customTitle, minimized: false, selectedCmdId: null, selectedInstUrl: null, _focusedLeafId: null,
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

// ─── Pane Tree (recursive splitting) ───
// Each panel can have a .split tree. The panel itself is the root leaf.
// panel.split.branch is a leaf object that can itself be split.
// This allows unlimited recursive splitting.

function _newLeafState(id) {
    return {
        id: id,
        cmdId: null, instUrl: null,
        ws: null, wsCmdId: null, wsInstUrl: null,
        wsReconnectCount: 0, wsReconnectTimer: null,
        wsPingInterval: null, wsPingSendTime: 0, wsLatency: 0,
        pollTimer: null, scrollbackOffset: 0,
        mouseTracking: false, mouseSgr: false,
        split: null,  // recursive!
    };
}

let _leafCounter = 0;
function _nextLeafId(panelId) {
    return panelId + '-L' + (++_leafCounter);
}

// Find a leaf state object by its ID, walking the split tree.
// Returns { leaf, parentSplit, side } where side is 'branch' or null (panel itself).
function _findLeafState(panel, leafId) {
    if (!leafId || panel.id === leafId) return { leaf: panel, parentSplit: null, side: null };
    if (!panel.split) return null;
    return _findLeafInNode(panel.split, leafId);
}

function _findLeafInNode(splitNode, leafId) {
    // Check branch leaf
    if (splitNode.branch) {
        if (splitNode.branch.id === leafId) return { leaf: splitNode.branch, parentSplit: splitNode, side: 'branch' };
        // Recurse into branch's own split tree
        if (splitNode.branch.split) {
            const found = _findLeafInNode(splitNode.branch.split, leafId);
            if (found) return found;
        }
    }
    return null;
}

// Get all leaf nodes in the split tree (for caching, WS management, etc.)
function _getAllLeaves(panel) {
    const leaves = [{ leaf: panel, side: null }];
    if (panel.split) _collectLeaves(panel.split, leaves);
    return leaves;
}

function _collectLeaves(splitNode, leaves) {
    if (splitNode.branch) {
        leaves.push({ leaf: splitNode.branch, side: 'branch' });
        if (splitNode.branch.split) _collectLeaves(splitNode.branch.split, leaves);
    }
}

// Find the parent split node that contains a leaf with the given ID
function _findParentSplit(panel, leafId) {
    if (!panel.split) return null;
    return _findParentSplitInNode(panel.split, leafId);
}

function _findParentSplitInNode(splitNode, leafId) {
    // The branch of THIS node?
    if (splitNode.branch && splitNode.branch.id === leafId) return splitNode;
    // Recurse into branch's split tree
    if (splitNode.branch && splitNode.branch.split) {
        return _findParentSplitInNode(splitNode.branch.split, leafId);
    }
    return null;
}

function splitPanel(panelId, direction, leafId) {
    const p = state.panels.find(pp => pp.id === panelId);
    if (!p) return;

    // If no leafId specified, split the panel itself if not already split
    // or split the active side if already split
    if (!leafId) {
        if (!p.split) {
            leafId = p.id; // split the panel itself
        } else {
            // Default: split the currently active side
            leafId = _getFocusedLeafId(p);
        }
    }

    // Find the leaf to split
    if (leafId === p.id) {
        // Splitting the panel itself
        if (p.split) return; // already has top-level split
        const sid = _nextLeafId(p.id);
        p.split = {
            direction, splitRatio: 0.5, activeSide: 'panel',
            branch: _newLeafState(sid),
        };
    } else {
        // Splitting a branch (or deeper) leaf
        const found = _findLeafState(p, leafId);
        if (!found || !found.leaf || found.leaf.split) return; // already split
        const sid = _nextLeafId(p.id);
        found.leaf.split = {
            direction, splitRatio: 0.5, activeSide: 'panel',
            branch: _newLeafState(sid),
        };
    }
    renderPanels();
}

function unsplitPanel(panelId, leafId) {
    const p = state.panels.find(pp => pp.id === panelId);
    if (!p || !p.split) return;

    if (!leafId || leafId === p.id) {
        // Remove the entire top-level split
        // First disconnect all branch WS in the tree
        _disconnectLeafTree(p.split);
        p.split = null;
        renderPanels();
        return;
    }

    // Find the leaf and its parent split
    const parentSplit = _findParentSplit(p, leafId);
    if (!parentSplit) return;

    // Disconnect the leaf being closed
    const closingLeaf = parentSplit.branch;
    if (closingLeaf) _disconnectSingleLeaf(closingLeaf);

    // If the closing leaf itself was split, disconnect its whole subtree
    if (closingLeaf && closingLeaf.split) _disconnectLeafTree(closingLeaf.split);

    // Remove the split — the other side (panel or ancestor) remains unchanged.
    parentSplit.branch = null;
    parentSplit.direction = null;
    parentSplit.activeSide = 'panel';

    // If this was the top-level split, clear it
    if (p.split === parentSplit && !p.split.branch) {
        p.split = null;
    }
    // Walk up and clean any split nodes that lost their branch
    _cleanEmptySplits(p);

    renderPanels();
}

function _cleanEmptySplits(panel) {
    if (!panel.split) return;
    if (!panel.split.branch) { panel.split = null; return; }
    _cleanEmptySplitsInNode(panel.split);
}

function _cleanEmptySplitsInNode(splitNode) {
    if (!splitNode.branch) return;
    if (splitNode.branch.split && !splitNode.branch.split.branch) {
        splitNode.branch.split = null;
    }
    if (splitNode.branch.split) _cleanEmptySplitsInNode(splitNode.branch.split);
}

// Disconnect all WS in a split tree
function _disconnectLeafTree(splitNode) {
    if (!splitNode) return;
    if (splitNode.branch) {
        _disconnectSingleLeaf(splitNode.branch);
        if (splitNode.branch.split) _disconnectLeafTree(splitNode.branch.split);
    }
}

function _disconnectSingleLeaf(leaf) {
    if (!leaf) return;
    if (leaf.ws) { try { leaf.ws.close(); } catch {} leaf.ws = null; }
    if (leaf.wsPingInterval) { clearInterval(leaf.wsPingInterval); leaf.wsPingInterval = null; }
    if (leaf.wsReconnectTimer) { clearTimeout(leaf.wsReconnectTimer); leaf.wsReconnectTimer = null; }
    if (leaf.pollTimer) { clearInterval(leaf.pollTimer); leaf.pollTimer = null; }
    leaf.wsPingSendTime = 0;
    leaf.wsLatency = 0;
    leaf.wsReconnectCount = 0;
    leaf.wsInstUrl = null;
    leaf.wsCmdId = null;
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

// ─── Recursive pane tree rendering ───
// All leaves are equal.
// The panel object itself acts as the "root leaf" and stores its cmdId/instUrl
// in selectedCmdId/selectedInstUrl like a non-split panel.

function _getLeafCmdState(leaf, leafId, panel) {
    // For the panel itself (leafId === panel.id), use panel's own fields.
    // For any other leaf, use the leaf's cmdId/instUrl.
    if (leafId === panel.id) {
        return { cmdId: panel.selectedCmdId, instUrl: panel.selectedInstUrl };
    }
    return { cmdId: leaf.cmdId, instUrl: leaf.instUrl };
}

// Render a FULL panel header for ANY leaf — same as non-split panels.
// This includes history buttons, cmd-info, exit banner, reach dot, meta, freeze, close.
// Context menu is triggered via oncontextmenu.
function _renderLeafHeader(panel, leaf, leafId) {
    const cs = _getLeafCmdState(leaf, leafId, panel);
    const inst = cs.instUrl ? state.connections.find(i => i.url === cs.instUrl) : null;
    const color = _getServerColor(inst);
    const textColor = _getServerTextColor(inst);
    const isTopLevel = (leafId === panel.id);
    // For the root leaf in a split, close removes the split.
    // For deeper leaves, close removes just that leaf.
    const closeAction = isTopLevel ? 'UnsplitPanel' : 'UnsplitLeaf';
    const closeData = isTopLevel
        ? `data-panel="${panel.id}"`
        : `data-panel="${panel.id}" data-leaf="${leafId}"`;
    return `<div class="panel-header" data-panel-id="${panel.id}" data-leaf-id="${leafId}" oncontextmenu="showPanelContextMenu(event,'${panel.id}','${leafId}')" tabindex="0" role="button" style="--ph-bg:${color};--ph-fg:${textColor};background:var(--ph-bg);color:var(--ph-fg);">
    <button class="btn btn-xs cmd-history-btn hidden" data-action="PanelHistoryBack" data-panel="${panel.id}" data-leaf="${leafId}" title="Back">&#x25C0;</button>
    <button class="btn btn-xs cmd-history-btn hidden" data-action="PanelHistoryForward" data-panel="${panel.id}" data-leaf="${leafId}" title="Forward">&#x25B6;</button>
    <div class="cmd-info">
        <span class="cmd-fullname" data-leaf-id="${leafId}" ondblclick="event.stopPropagation();startRenamePanel('${panel.id}')" title="Double-click to rename"></span>
        <span class="cmd-args"></span>
    </div>
    <span class="panel-exit-banner hidden"></span>
    <span class="panel-reach-dot unknown" title="Server state"></span>
    <span class="panel-header-meta"></span>
    <button class="cmd-freeze-btn panel-freeze-btn hidden" data-action="TogglePauseRunPanel" data-panel="${panel.id}" data-leaf="${leafId}" title="Freeze/Thaw command">&#8545;</button>
    <button class="panel-close-btn" data-action="${closeAction}" ${closeData} title="Close pane">&#x2715;</button>
</div>`;
}

function _renderLeafVtty(panel, leaf, leafId) {
    const cs = _getLeafCmdState(leaf, leafId, panel);
    const selMode = panel.selectionMode ? ' selection-mode' : '';
    const themeAttr = panel.theme ? 'data-panel-theme="' + panel.theme + '"' : '';
    const noCmdText = !cs.cmdId ? '<span style="color:var(--text-muted);">No command selected — select a command from the sidebar</span>' : '';
    return `<div class="vtty-container${selMode}" id="vtty-${leafId}" data-leaf-id="${leafId}" data-panel="${panel.id}" ${themeAttr} style="font-size: ${panel.fontSize}px; flex:1; min-height:0;">
    ${_renderSearchBar(leafId)}
    <pre>${noCmdText}</pre>
    <div class="cursor-indicator hidden"></div>
    <div class="copy-feedback" id="copyFeedback-${leafId}">Copied!</div>
    <button class="scroll-bottom-btn" id="scrollBtn-${leafId}" data-action="ScrollTerminalBottom" data-panel="${leafId}" title="Scroll to bottom">&#x25BC;</button>
</div>`;
}

function _renderSplitContainer(panel) {
    if (!panel.split) return '';
    return _renderSplitNode(panel, panel.split, panel.id, true);
}

// Recursively render a split node. panelLeafId is either the panel (for top-level) or a leaf object.
function _renderSplitNode(panel, splitNode, panelLeafId, isTopLevel) {
    const dir = splitNode.direction || 'horizontal';
    const pw = splitNode.splitRatio ? (splitNode.splitRatio * 100).toFixed(1) : '50';
    const sw = (100 - parseFloat(pw)).toFixed(1);

    // Render panel side
    const panelIsLeaf = (panelLeafId === panel.id);
    let panelSideHtml;
    if (panelIsLeaf && panel.split === splitNode) {
        // Top-level: panel is the root leaf
        // (panel itself can't have a deeper split at this level since we just entered from panel.split)
        panelSideHtml = _renderLeafPane(panel, panel, panel.id);
    } else {
        // At deeper levels, find the actual leaf object.
        // panelLeafId is the branch of the parent split that was itself split.
        const found = _findLeafState(panel, panelLeafId);
        const panelLeaf = found ? found.leaf : panel;
        panelSideHtml = _renderLeafPane(panel, panelLeaf, panelLeafId);
    }

    // Render branch side
    const branch = splitNode.branch;
    let branchHtml;
    if (branch.split) {
        // Branch is itself split — recurse
        branchHtml = _renderSplitNode(panel, branch.split, branch.id, false);
    } else {
        branchHtml = _renderLeafPane(panel, branch, branch.id);
    }

    const containerId = isTopLevel ? 'split-' + panel.id : '';
    const containerIdAttr = containerId ? ' id="' + containerId + '"' : '';
    return `<div class="split-container ${dir}"${containerIdAttr} data-panel="${panel.id}" style="display:flex;flex:1;min-width:0;min-height:0;${dir === 'vertical' ? 'flex-direction:column;' : ''}">${panelSideHtml}<div class="split-divider" data-panel="${panel.id}"></div>${branchHtml}</div>`;
}

function _renderLeafPane(panel, leaf, leafId) {
    const isFocused = panel._focusedLeafId === leafId;
    return `<div class="split-pane${isFocused ? ' focused' : ''}" data-leaf-id="${leafId}" data-panel="${panel.id}" style="flex: 0 0 50%; display:flex; flex-direction:column; min-width:0; min-height:0;">
${_renderLeafHeader(panel, leaf, leafId)}
${_renderLeafVtty(panel, leaf, leafId)}
</div>`;
}

// ─── Header updates ───

function _updateSplitHeaders(panelObj) {
    if (!panelObj?.split) return;
    const el = document.getElementById(panelObj.id);
    if (!el) return;
    // Update panel root leaf
    _updateOneSplitHeader(el, panelObj.id, panelObj.selectedInstUrl, panelObj.selectedCmdId);
    // Walk tree for all branch leaves
    _updateTreeHeaders(el, panelObj.split);
}

function _updateTreeHeaders(container, splitNode) {
    if (!splitNode || !splitNode.branch) return;
    _updateOneSplitHeader(container, splitNode.branch.id, splitNode.branch.instUrl, splitNode.branch.cmdId);
    if (splitNode.branch.split) _updateTreeHeaders(container, splitNode.branch.split);
}

function _updateOneSplitHeader(container, leafId, instUrl, cmdId) {
    const h = container.querySelector(`.panel-header[data-leaf-id="${leafId}"]`);
    if (!h) return;
    const inst = instUrl ? state.connections.find(i => i.url === instUrl) : null;
    h.style.background = _getServerColor(inst);
    h.style.color = _getServerTextColor(inst);
    // Update the command name (same as non-split panels)
    const nameEl = h.querySelector('.cmd-fullname');
    if (nameEl) {
        const cmd = _findCmd(instUrl, cmdId);
        const fullName = cmd ? (cmd.name || cmd.id) : (cmdId || 'Panel');
        nameEl.textContent = fullName;
        nameEl.title = fullName;
    }
    // Update args
    const argsEl = h.querySelector('.cmd-args');
    if (argsEl) {
        const cmd = _findCmd(instUrl, cmdId);
        const a = cmd && cmd.args ? (cmd.args || []).join(' ') : '';
        argsEl.textContent = a ? ' ' + a : '';
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
    // Use visible panels (current window) instead of all panels across windows
    const visibleCount = (typeof _getVisiblePanels === 'function') ? _getVisiblePanels() : state.panels;
    const multi = visibleCount.length > 1;
    const isGrid = state.panelLayout.startsWith('grid-');
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

function startRenameWindow(winId) {
    const win = state.windows.find(w => w.id === winId);
    if (!win) return;
    const tab = document.querySelector(`.window-tab[data-window="${winId}"]`);
    if (!tab) return;
    const label = tab.querySelector('.window-tab-label');
    if (!label || label.getAttribute('contenteditable') === 'true') return;
    label.contentEditable = 'true';
    label.classList.add('editing');
    label.textContent = win.name;
    label.focus();
    const range = document.createRange();
    range.selectNodeContents(label);
    const sel = window.getSelection(); sel.removeAllRanges(); sel.addRange(range);
    const finish = (save) => {
        label.removeEventListener('keydown', onKey);
        label.removeEventListener('blur', onBlur);
        label.contentEditable = 'false';
        label.classList.remove('editing');
        if (save) {
            const t = label.textContent.trim();
            win.name = t || win.name;
        }
        // Update only this tab's label text — do NOT re-render the whole window bar
        // (which would destroy any other in-progress operations).
        const allTabs = document.querySelectorAll('.window-tab[data-window="' + winId + '"]');
        for (const tab of allTabs) {
            const lbl = tab.querySelector('.window-tab-label');
            if (lbl) lbl.textContent = win.name;
        }
    };
    const onKey = (e) => {
        if (e.key === 'Enter') { e.preventDefault(); finish(true); }
        if (e.key === 'Escape') { e.preventDefault(); finish(false); }
        e.stopPropagation();
    };
    const onBlur = () => setTimeout(() => finish(true), 100);
    label.addEventListener('keydown', onKey);
    label.addEventListener('blur', onBlur);
}

    // ── Exports ──
    Object.assign(window, {
        addPanelDirect, addPanel,
        removePanel, closePanelContent, toggleMinimizePanel, splitPanel, unsplitPanel,
        togglePanelLayout, toggleLayoutPresetMenu, applyLayoutPreset,
        startRenamePanel, finishRenamePanel,
        _renderVttyContainer, _getServerLabel, _getServerColor, _getServerTextColor,
        _getPanelCmdLabel, _updateSplitHeaders, _renderSplitContainer,
        _renderMinimizedPanels, _applyPanelLayoutClass,
        _updatePanelMultiUI, _getPanelLabel,
        _renderLeafHeader, _renderLeafPane, _findCmd, _findPanelVtty, _cacheVtty,
        switchWindow, createWindow, closeWindow, _renderWindowBar, startRenameWindow,
        _getActiveWindow, _getVisiblePanels,
        // Recursive split tree
        _findLeafState, _getAllLeaves, _findParentSplit,
        _getLeafCmdState, _disconnectLeafTree, _disconnectSingleLeaf,
        _newLeafState, _nextLeafId,
    });

    // UnsplitLeaf action — close a specific leaf (from close button in split header)
    window.unsplitLeaf = function(panelId, leafId) { unsplitPanel(panelId, leafId); };
})();