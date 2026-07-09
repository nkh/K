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

const _serverBg = ['var(--bg-tertiary)','var(--server-tint-1-bg)','var(--server-tint-2-bg)','var(--server-tint-3-bg)','var(--server-tint-4-bg)','var(--server-tint-5-bg)','var(--server-tint-6-bg)','var(--server-tint-7-bg)'];
const _serverFg = ['var(--text-primary)','var(--server-tint-1-fg)','var(--server-tint-2-fg)','var(--server-tint-3-fg)','var(--server-tint-4-fg)','var(--server-tint-5-fg)','var(--server-tint-6-fg)','var(--server-tint-7-fg)'];

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
    let html = '<div class="window-bar" id="windowBar">';
    // Action buttons (always visible)
    html += '<div class="window-bar-actions">';
    html += `<button class="window-bar-btn" data-action="CreateWindow" title="New window (Ctrl+A w)">+ Win</button>`;
    const pid = getActivePanelId();
    const p = pid ? state.panels.find(x => x.id === pid) : null;
    const canSplit = p && !p.minimized;
    html += `<button class="window-bar-btn${canSplit ? '' : ' disabled'}" data-action="SplitPaneVertical" data-panel="${pid || ''}" title="Split vertically — side by side (Ctrl+A |)">| Split V</button>`;
    html += `<button class="window-bar-btn${canSplit ? '' : ' disabled'}" data-action="SplitPaneHorizontal" data-panel="${pid || ''}" title="Split horizontally — top/bottom (Ctrl+A -)">— Split H</button>`;
    const canClose = p && (p.split || p._rootSplit);
    html += `<button class="window-bar-btn${canClose ? '' : ' disabled'}" data-action="UnsplitPane" data-panel="${pid || ''}" title="Close pane (Ctrl+A Ctrl+D)">&#x2715; Close</button>`;
    html += '</div>';
    // Window tabs (only if >1 window)
    if (state.windows.length > 1) {
        for (const w of state.windows) {
            const active = w.id === state.activeWindowId;
            const closeBtn = state.windows.length > 1
                ? `<button class="window-tab-close" data-action="CloseWindow" data-window="${w.id}" title="Close window">&#x2715;</button>`
                : '';
            html += `<div class="window-tab${active ? ' active' : ''}" data-action="SwitchWindow" data-window="${w.id}" title="Window ${escHtml(w.name)}"><span class="window-tab-label" data-action-placeholder="StartRenameWindow" data-window-id="${w.id}">${escHtml(w.name)}</span>${closeBtn}</div>`;
        }
    }
    html += '</div>';
    return html;
}

// ─── Panels (Multi-view) ───
function addPanelDirect() {
    const id = 'panel-' + Date.now() + '-' + Math.random().toString(36).slice(2, 6);
    const fontSize = state.fontSize;
    const selectionMode = false;
    const theme = '';
    const customTitle = '';
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
    // Check the root pane's own split (_rootSplit) first
    if (panel._rootSplit) {
        const found = _findLeafInNode(panel._rootSplit, leafId);
        if (found) return found;
    }
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
    if (panel._rootSplit) _collectLeaves(panel._rootSplit, leaves);
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
    if (panel._rootSplit) {
        const found = _findParentSplitInNode(panel._rootSplit, leafId);
        if (found) return found;
    }
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
    // or split the focused leaf if already split
    if (!leafId) {
        if (!p.split) {
            leafId = p.id; // split the panel itself
        } else {
            // Default: split the currently focused leaf
            leafId = _getFocusedLeafId(p);
        }
    }

    // Find the leaf to split
    if (leafId === p.id) {
        if (!p.split) {
            // First split — create top-level split
            const sid = _nextLeafId(p.id);
            p.split = {
                direction, splitRatio: 0.5, activeSide: 'panel',
                branch: _newLeafState(sid),
            };
            // Focus the NEW pane so the user can immediately select a command.
            p._focusedLeafId = sid;
        } else {
            // Splitting the root pane when top-level split already exists.
            // Use _rootSplit — a separate split node for the root pane itself.
            // This is parallel to branch.split for branch leaves.
            if (!p._rootSplit) {
                const sid = _nextLeafId(p.id);
                p._rootSplit = {
                    direction, splitRatio: 0.5, activeSide: 'panel',
                    branch: _newLeafState(sid),
                };
            } else {
                // Root already has its own split — find a non-split target within it
                let target = p;
                // Walk into the deepest non-split leaf of the root's split tree
                if (target._rootSplit && target._rootSplit.branch) {
                    const subId = p._focusedLeafId && p._focusedLeafId !== p.id
                        ? p._focusedLeafId : target._rootSplit.branch.id;
                    const subFound = _findLeafState(p, subId);
                    if (subFound && subFound.leaf && !subFound.leaf.split && subFound.leaf.id !== p.id) {
                        target = subFound.leaf;
                    } else {
                        target = target._rootSplit.branch;
                        while (target.split && target.split.branch) {
                            target = target.split.branch;
                        }
                    }
                }
                if (target.id === p.id && !p._rootSplit) return;
                if (target.split) return; // all leaves in root split are already split
                const sid = _nextLeafId(p.id);
                target.split = {
                    direction, splitRatio: 0.5, activeSide: 'panel',
                    branch: _newLeafState(sid),
                };
            }
        }
        // Focus the NEW pane — find it by walking the root split tree.
        if (p._rootSplit && p._rootSplit.branch) {
            // Find the deepest branch in _rootSplit to get the newly created leaf
            let branch = p._rootSplit.branch;
            while (branch.split && branch.split.branch) branch = branch.split.branch;
            p._focusedLeafId = branch.id;
        }
    } else {
        // Splitting a branch (or deeper) leaf
        const found = _findLeafState(p, leafId);
        if (!found || !found.leaf) return;
        // If leaf already split, find a non-split target leaf
        let target = found.leaf;
        while (target.split && target.split.branch) {
            const subId = target._focusedLeafId && target._focusedLeafId !== target.id
                ? target._focusedLeafId : target.split.branch.id;
            const subFound = _findLeafState(p, subId);
            if (!subFound || !subFound.leaf) { target = target.split.branch; if (!target.split) break; continue; }
            target = subFound.leaf;
            if (!target.split) break;
        }
        if (target.split) return;
        const sid = _nextLeafId(p.id);
        target.split = {
            direction, splitRatio: 0.5, activeSide: 'panel',
            branch: _newLeafState(sid),
        };
        // Focus the NEW pane so the user can immediately select a command.
        p._focusedLeafId = sid;
    }
    renderPanels();
    // After render, ensure the panel is focused and split-pane focus classes are applied.
    // renderPanels() rebuilds the DOM so we must re-apply focus on the new elements.
    focusPanel(panelId);
    const panelEl = document.getElementById(panelId);
    if (panelEl && p._focusedLeafId && (p.split || p._rootSplit)) {
        panelEl.querySelectorAll('.split-pane').forEach(function(sp) {
            sp.classList.toggle('focused', sp.getAttribute('data-leaf-id') === p._focusedLeafId);
        });
    }
}

function unsplitPanel(panelId, leafId) {
    const p = state.panels.find(pp => pp.id === panelId);
    if (!p) return;
    if (!p.split && !p._rootSplit) return;

    // Closing the ROOT leaf (panel.id): promote the branch to become the new root.
    // This preserves all other panes in the split tree.
    if (leafId === p.id && p.split && p.split.branch) {
        const branch = p.split.branch;
        // Transfer branch's command data to the panel root
        if (branch.cmdId) p.selectedCmdId = branch.cmdId;
        if (branch.instUrl) p.selectedInstUrl = branch.instUrl;
        // If the branch itself has children (was split), those become the new top-level split.
        // If no children, the split is fully removed.
        _disconnectSingleLeaf(branch); // disconnect branch's own ws/poll
        if (branch.split && branch.split.branch) {
            // Branch has children — the children become the new top-level split
            p.split = branch.split;
        } else {
            p.split = null;
        }
        // Also handle _rootSplit: if root was split, its tree becomes the only remaining split
        if (p._rootSplit) {
            // Keep _rootSplit as-is (it was the root's own split tree)
        }
        p._focusedLeafId = p.id;
        renderPanels();
        return;
    }

    // No leafId or clearing all splits (used by explicit "remove all" action)
    if (!leafId) {
        _disconnectLeafTree(p.split);
        _disconnectLeafTree(p._rootSplit);
        p.split = null;
        p._rootSplit = null;
        p._focusedLeafId = p.id;
        renderPanels();
        return;
    }

    // Check if the leaf is in _rootSplit
    let parentSplit = null;
    let isRootSplit = false;
    if (p._rootSplit) {
        parentSplit = _findParentSplitInNode(p._rootSplit, leafId);
        if (parentSplit) isRootSplit = true;
    }
    // If not found in _rootSplit, check the main split tree
    if (!parentSplit && p.split) {
        parentSplit = _findParentSplitInNode(p.split, leafId);
    }
    if (!parentSplit) return;

    // The leaf being closed
    const closingLeaf = parentSplit.branch;

    // CRITICAL: If the focused leaf was the one being closed, refocus.
    // If the closing leaf has children, try to focus the first child instead of root.
    if (p._focusedLeafId && p._focusedLeafId !== p.id) {
        if (p._focusedLeafId === leafId) {
            // Focused leaf is being closed. If it has a child split, focus that child.
            if (closingLeaf && closingLeaf.split && closingLeaf.split.branch) {
                p._focusedLeafId = closingLeaf.split.branch.id;
            } else {
                p._focusedLeafId = p.id;
            }
        } else if (closingLeaf && _leafIdInSubtree(closingLeaf, p._focusedLeafId)) {
            // Focused leaf is in the closing leaf's subtree — check if it's in a child split
            // that will be promoted. If not, reset to root.
            if (closingLeaf.split && _leafIdInSubtree(closingLeaf.split.branch || {}, p._focusedLeafId)) {
                // The focused leaf is in a child that will survive — keep focus
            } else {
                p._focusedLeafId = p.id;
            }
        }
    }

    // Disconnect only the closing leaf itself (not its children).
    // If the leaf has a child split, the children will be promoted to replace it.
    if (closingLeaf) _disconnectSingleLeaf(closingLeaf);

    if (closingLeaf && closingLeaf.split && closingLeaf.split.branch) {
        // The closing leaf has children — promote the first child (branch) to take its place.
        // This preserves sibling panes that were created by splitting this leaf.
        parentSplit.branch = closingLeaf.split.branch;
        // The promoted branch may have inherited splitRatio from the child split;
        // keep the parent's splitRatio and direction unchanged.
    } else {
        // No children — simply remove the branch.
        parentSplit.branch = null;
        parentSplit.direction = null;
        parentSplit.activeSide = 'panel';
    }

    // Clean empty splits
    if (isRootSplit && p._rootSplit) {
        if (!p._rootSplit.branch) p._rootSplit = null;
        else _cleanEmptySplitsInNode(p._rootSplit);
    }
    if (p.split && !p.split.branch) p.split = null;
    else if (p.split) _cleanEmptySplitsInNode(p.split);

    renderPanels();
}

function _cleanEmptySplits(panel) {
    if (panel._rootSplit) {
        if (!panel._rootSplit.branch) panel._rootSplit = null;
        else _cleanEmptySplitsInNode(panel._rootSplit);
    }
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

// Check if a leafId exists anywhere in a leaf's subtree (for _focusedLeafId validation)
function _leafIdInSubtree(leaf, targetId) {
    if (!leaf || !targetId) return false;
    if (leaf.id === targetId) return true;
    if (leaf.split && leaf.split.branch) {
        if (leaf.split.branch.id === targetId) return true;
        return _leafIdInSubtree(leaf.split.branch, targetId);
    }
    return false;
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
    // For ALL leaves (including root), close removes just that leaf.
    // If the root is closed, the branch is promoted to become the new root.
    const closeAction = 'UnsplitLeaf';
    const closeData = `data-panel="${panel.id}" data-leaf="${leafId}"`;
    return `<div class="panel-header" data-panel-id="${panel.id}" data-leaf-id="${leafId}" data-ctxmenu="panel" data-panel="${panel.id}" data-leaf="${leafId}" tabindex="0" role="button" style="--ph-bg:${color};--ph-fg:${textColor}">
    <button class="btn btn-xs cmd-history-btn hidden" data-action="PanelHistoryBack" data-panel="${panel.id}" data-leaf="${leafId}" title="Back">&#x25C0;</button>
    <button class="btn btn-xs cmd-history-btn hidden" data-action="PanelHistoryForward" data-panel="${panel.id}" data-leaf="${leafId}" title="Forward">&#x25B6;</button>
    <div class="cmd-info">
        <span class="cmd-fullname" data-leaf-id="${leafId}" title="Double-click to rename"></span>
        <span class="cmd-args"></span>
    </div>
    <span class="panel-exit-banner hidden"></span>
    <span class="panel-reach-dot unknown" title="Server state"></span>
    <span class="panel-header-meta"></span>
    <button class="cmd-freeze-btn panel-freeze-btn hidden" data-action="TogglePauseRunLeaf" data-panel="${panel.id}" data-leaf="${leafId}" title="Freeze/Thaw command">&#8545;</button>
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
        // Top-level: panel is the root leaf.
        // If the root pane itself is split (_rootSplit), render that split tree.
        if (panel._rootSplit) {
            panelSideHtml = _renderSplitNode(panel, panel._rootSplit, panel.id, false);
        } else {
            panelSideHtml = _renderLeafPane(panel, panel, panel.id);
        }
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
    // Only the top-level split container gets flex:1 to fill the panel.
    // Nested split containers must NOT have flex:1 — they are children of their
    // parent split and should respect the parent's splitRatio (same as leaf panes).
    const flexStyle = isTopLevel ? 'flex:1;' : '';
    return `<div class="split-container ${dir}"${containerIdAttr} data-panel="${panel.id}" style="display:flex;${flexStyle}min-width:0;min-height:0;${dir === 'vertical' ? 'flex-direction:column;' : ''}">${panelSideHtml}<div class="split-divider" data-panel="${panel.id}"></div>${branchHtml}</div>`;
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
    if (!panelObj?.split && !panelObj?._rootSplit) return;
    const el = document.getElementById(panelObj.id);
    if (!el) return;
    // Update panel root leaf
    _updateOneSplitHeader(el, panelObj.id, panelObj.selectedInstUrl, panelObj.selectedCmdId);
    // Walk root's own split tree (_rootSplit)
    if (panelObj._rootSplit) _updateTreeHeaders(el, panelObj._rootSplit);
    // Walk top-level split tree
    if (panelObj.split) _updateTreeHeaders(el, panelObj.split);
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
    h.style.setProperty('--ph-bg', _getServerColor(inst));
    h.style.setProperty('--ph-fg', _getServerTextColor(inst));
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
            // Remove from all windows' panelIds to prevent stale references
            for (const w of state.windows) {
                if (w.panelIds) w.panelIds = w.panelIds.filter(pid => pid !== removed.id);
            }
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
        _newLeafState, _nextLeafId, _leafIdInSubtree,
    });

    // UnsplitLeaf action — close a specific leaf (from close button in split header)
    window.unsplitLeaf = function(panelId, leafId) { unsplitPanel(panelId, leafId); };

    // Toolbar action handlers — always use current active panel, not stale data-panel
    window.splitPaneVertical = function(panelId) {
        const id = getActivePanelId() || panelId;
        if (!id) return;
        const p = state.panels.find(x => x.id === id);
        if (p) { const leafId = (typeof _getFocusedLeafId === 'function') ? _getFocusedLeafId(p) : p.id; splitPanel(id, 'horizontal', leafId); }
    };
    window.splitPaneHorizontal = function(panelId) {
        const id = getActivePanelId() || panelId;
        if (!id) return;
        const p = state.panels.find(x => x.id === id);
        if (p) { const leafId = (typeof _getFocusedLeafId === 'function') ? _getFocusedLeafId(p) : p.id; splitPanel(id, 'vertical', leafId); }
    };
    window.unsplitPaneAction = function(panelId) {
        const id = getActivePanelId() || panelId;
        if (!id) return;
        const p = state.panels.find(x => x.id === id);
        if (p && (p.split || p._rootSplit)) {
            const leafId = (typeof _getFocusedLeafId === 'function') ? _getFocusedLeafId(p) : p.id;
            unsplitPanel(id, leafId);
        }
    };
})();