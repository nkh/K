// ─── Panels ───
(function() {
    'use strict';
// ─── Panels (Multi-view) ───
// Panels are pure display containers — decoupled from server connections.
// A panel can display any command's VTTY from any server connection.
function addPanelDirect() {
    const id = 'panel-' + Date.now() + '-' + Math.random().toString(36).slice(2, 6);
    const savedFontSize = parseInt(localStorage.getItem('vrw_panel_font_' + id));
    const fontSize = (savedFontSize >= 8 && savedFontSize <= 28) ? savedFontSize : state.fontSize;
    const savedSelMode = localStorage.getItem('vrw_panel_sel_' + id);
    const selectionMode = savedSelMode === 'true';
    const savedTheme = localStorage.getItem('vrw_panel_theme_' + id);
    // Per-panel theme: 'light', 'dark', or '' (inherit global). Default is inherit.
    const theme = (savedTheme === 'light' || savedTheme === 'dark') ? savedTheme : '';
    // Per-panel custom title (user-editable via double-click or context menu)
    const customTitle = localStorage.getItem('vrw_panel_title_' + id) || '';
    const panel = { id, scrollbackOffset: 0, mouseTracking: false, mouseSgr: false, focused: false, fontSize, selectionMode, theme, customTitle, minimized: false, selectedCmdId: null, selectedInstUrl: null,
        // Per-panel WebSocket connection
        ws: null, wsCmdId: null, wsInstUrl: null, wsReconnectCount: 0, wsReconnectTimer: null, wsPingInterval: null, wsPingSendTime: 0, wsLatency: 0,
        // Per-panel poll timer
        pollTimer: null,
        // Per-panel command history (browser-like back/forward navigation)
        cmdHistory: [],  // array of { instUrl, cmdId, cmdName }
        cmdHistoryIdx: -1,  // -1 = no history; 0+ = index in cmdHistory
    };
    state.panels.push(panel);
    renderPanels();
    return panel;
}

function addPanel() {
    // Create an empty panel directly (no server URL required).
    // Users can connect a command from the sidebar later.
    addPanelDirect();
    // Focus the new panel
    const newPanel = state.panels[state.panels.length - 1];
    if (newPanel) focusPanel(newPanel.id);
}

function closePanelModal() {
    releaseCurrentFocusTrap();
    document.getElementById('panelModal').style.display = 'none';
}

function confirmAddPanel() {
    const url = document.getElementById('panelUrl').value.trim();
    if (!url) return;

    const token = document.getElementById('panelToken').value.trim();
    const splitDir = document.getElementById('panelSplitDir').value;
    let label = document.getElementById('panelLabel').value.trim();
    if (!label) {
        try { label = new URL(url).host; } catch (e) { label = url; }
    }

    try {
        // Ensure server connection exists (addConnection is idempotent)
        addConnection(url, label, token);
        // Create a new panel
        addPanelDirect();
        closePanelModal();

        // Apply layout direction
        if (splitDir === 'vertical') {
            state.panelLayout = 'column';
        } else if (splitDir === 'horizontal') {
            state.panelLayout = 'row';
        }
        // 'auto' doesn't change the layout
        localStorage.setItem('vrw_panel_layout', state.panelLayout);

        // The new panel will auto-select the first command from this server
        // after loadCommands() runs and _buildSidebar() selects it.
        const newPanel = state.panels[state.panels.length - 1];
        if (newPanel) {
            newPanel.selectedInstUrl = url;
            // Set _pendingSelectId to null so _buildSidebar picks the first command
            state._pendingSelectId = null;
        }

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
    // Disconnect panel's WS and poll before removing
    disconnectPanelWs(id);
    stopPanelPoll(id);
    state.panels = state.panels.filter(p => p.id !== id);
    // If only one panel left, reset layout to row
    if (state.panels.length <= 1) {
        state.panelLayout = 'row';
        localStorage.setItem('vrw_panel_layout', state.panelLayout);
    } else if (state.panelLayout.startsWith('grid-')) {
        // Grid preset panel count no longer valid — fall back to row
        const needed = { 'grid-2x2': 4, 'grid-1-2': 3, 'grid-2-1': 3 };
        if (state.panels.length !== needed[state.panelLayout]) {
            state.panelLayout = 'row';
            localStorage.setItem('vrw_panel_layout', state.panelLayout);
        }
    }
    // If the removed panel was focused, focus the first remaining
    if (state._focusedPanelId === id) {
        state._focusedPanelId = state.panels.length > 0 ? state.panels[0].id : null;
    }
    renderPanels();
    // Update shared toolbar to reflect new focused panel
    updateSharedToolbar();
}

// ─── Panel Minimize / Restore ───
function toggleMinimizePanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    panelObj.minimized = !panelObj.minimized;
    if (panelObj.minimized) {
        // If we're minimizing the focused panel, focus another
        if (state._focusedPanelId === panelId) {
            const visible = state.panels.find(p => !p.minimized && p.id !== panelId);
            if (visible) focusPanel(visible.id);
        }
    } else {
        // Restoring — focus it
        focusPanel(panelId);
    }
    renderPanels();
}


// ─── Split Panel ───
function splitPanel(panelId, direction) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || panelObj.split) return;
    panelObj.split = {
        direction: direction, // 'horizontal' or 'vertical'
        splitRatio: 0.5,
        // Which sub-pane is active for command selection from sidebar
        activeSide: 'primary',
        // Secondary pane command state
        secondaryCmdId: null,
        secondaryInstUrl: null,
        // Secondary pane WebSocket connection
        secondaryWs: null,
        secondaryWsCmdId: null,
        secondaryWsInstUrl: null,
        secondaryWsReconnectCount: 0,
        secondaryWsReconnectTimer: null,
        secondaryWsPingInterval: null,
        secondaryWsPingSendTime: 0,
        secondaryWsLatency: 0,
        // Secondary pane poll timer
        secondaryPollTimer: null,
        // Secondary pane scrollback
        secondaryScrollbackOffset: 0,
        // Secondary pane mouse tracking
        secondaryMouseTracking: false,
        secondaryMouseSgr: false,
    };
    renderPanels();
}

function unsplitPanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || !panelObj.split) return;
    // Disconnect secondary WS and poll
    _disconnectSecondaryWs(panelObj);
    if (panelObj.split.secondaryPollTimer) {
        clearInterval(panelObj.split.secondaryPollTimer);
    }
    panelObj.split = null;
    renderPanels();
}

/// Render a single vtty-container (non-split panel).
function _renderVttyContainer(panel) {
    return `<div class="vtty-container${panel.selectionMode ? ' selection-mode' : ''}" id="vtty-${panel.id}" ${panel.theme ? 'data-panel-theme="' + panel.theme + '"' : ''} style="font-size: ${panel.fontSize}px;">
                        <div class="exited-banner" id="exitedBanner-${panel.id}" style="display:none;"></div>
                        <div class="search-bar" id="searchBar-${panel.id}">
                            <input type="text" id="searchInput-${panel.id}" placeholder="Search terminal..." oninput="vttySearch('${panel.id}')" onkeydown="if(event.key==='Enter'){event.shiftKey?vttySearchPrev('${panel.id}'):vttySearchNext('${panel.id}')}">
                            <span class="search-count" id="searchCount-${panel.id}" title="Click to jump: Shift+Click to reverse"></span>
                            <div class="search-progress-bar" id="searchProgress-${panel.id}"></div>
                            <button data-action="VttySearchNext" data-panel="${panel.id}" title="Next match (Enter)">&#x25BC;</button>
                            <button data-action="VttySearchPrev" data-panel="${panel.id}" title="Previous match (Shift+Enter)">&#x25B2;</button>
                            <button data-action="VttySearchClose" data-panel="${panel.id}" title="Close search">&#x2715;</button>
                        </div>
                        <pre style="color:#484f58;">No command selected — select a command from the sidebar to view its output</pre>
                        <div class="cursor-indicator" style="display:none;"></div>
                        <div class="copy-feedback" id="copyFeedback-${panel.id}">Copied!</div>
                        <button class="scroll-bottom-btn" id="scrollBtn-${panel.id}" data-action="ScrollTerminalBottom" data-panel="${panel.id}" title="Scroll to bottom">&#x25BC;</button>
                    </div>`;
}

/// Get a human-readable server label: name if available, otherwise host:port.
/// Never returns '?' — always shows the port or a meaningful fallback.
function _getServerLabel(inst, instUrl) {
    if (inst && inst._serverName) return inst._serverName;
    if (inst && inst.label) {
        // Use label which is typically set by user or derived from URL
        return inst.label;
    }
    if (instUrl) {
        try {
            const u = new URL(instUrl);
            // Show only the port number for compactness
            if (u.port) {
                return u.port;
            }
            const scheme = u.protocol.replace(':', '');
            const defaultPort = scheme === 'https' ? 443 : scheme === 'http' ? 80 : 0;
            return String(parseInt(u.port || '0') || defaultPort);
        } catch (e) { return instUrl; }
    }
    return '';
}

/// Get a distinct background color for a server connection.
/// Colors are assigned per-connection index to ensure consistency.
const _serverColorPalette = [
    'var(--bg-tertiary)',  // default (no special color)
    '#2d1f3d',  // purple
    '#1f3d2d',  // green
    '#3d2d1f',  // brown
    '#1f2d3d',  // blue
    '#3d1f2d',  // red
    '#2d3d1f',  // olive
    '#1f3d3d',  // teal
];

/// Text colors paired with _serverColorPalette for readability.
const _serverTextColorPalette = [
    'var(--text-primary)',
    '#d4b8e8',  // purple
    '#b8e8d4',  // green
    '#e8d4b8',  // brown
    '#b8d4e8',  // blue
    '#e8b8d4',  // red
    '#d4e8b8',  // olive
    '#b8e8e8',  // teal
];

function _getServerColor(inst, instUrl) {
    if (!inst) return 'var(--bg-tertiary)';
    const idx = state.connections.indexOf(inst);
    if (idx <= 0) return 'var(--bg-tertiary)';
    // Use server-configured panel colors if available
    if (state._serverPanelColors && state._serverPanelColors.length > 0) {
        const colorIdx = (idx - 1) % state._serverPanelColors.length;
        return state._serverPanelColors[colorIdx].background || 'var(--bg-tertiary)';
    }
    return _serverColorPalette[idx % _serverColorPalette.length];
}

function _getServerTextColor(inst, instUrl) {
    if (!inst) return 'var(--text-primary)';
    const idx = state.connections.indexOf(inst);
    if (idx <= 0) return 'var(--text-primary)';
    // Use server-configured panel colors if available
    if (state._serverPanelColors && state._serverPanelColors.length > 0) {
        const colorIdx = (idx - 1) % state._serverPanelColors.length;
        return state._serverPanelColors[colorIdx].text || 'var(--text-primary)';
    }
    return _serverTextColorPalette[idx % _serverTextColorPalette.length];
}

/// Get the command name label for a panel/sub-pane.
function _getPanelCmdLabel(cmdId, instUrl) {
    if (!cmdId) return 'No command';
    const inst = instUrl ? state.connections.find(i => i.url === instUrl) : null;
    const cmd = inst && inst._commands ? inst._commands.find(c => c.id === cmdId) : null;
    return cmd ? (cmd.name || cmd.id) : cmdId;
}

/// Update the split header labels (called after command selection changes).
function _updateSplitHeaders(panelObj) {
    if (!panelObj || !panelObj.split) return;
    const panelEl = document.getElementById(panelObj.id);
    if (!panelEl) return;

    // Update primary split header
    const primaryHeader = panelEl.querySelector('.split-header[data-split-side="primary"]');
    if (primaryHeader) {
        const primaryInst = panelObj.selectedInstUrl ? state.connections.find(i => i.url === panelObj.selectedInstUrl) : null;
        primaryHeader.style.background = _getServerColor(primaryInst, panelObj.selectedInstUrl);
        primaryHeader.style.color = _getServerTextColor(primaryInst, panelObj.selectedInstUrl);
        const serverLabel = primaryHeader.querySelector('.split-server-label');
        if (serverLabel) serverLabel.textContent = _getServerLabel(primaryInst, panelObj.selectedInstUrl);
        const cmdLabel = primaryHeader.querySelector('.split-cmd-label');
        if (cmdLabel) cmdLabel.textContent = _getPanelCmdLabel(panelObj.selectedCmdId, panelObj.selectedInstUrl);
    }

    // Update secondary split header
    const secondaryHeader = panelEl.querySelector('.split-header[data-split-side="secondary"]');
    if (secondaryHeader) {
        const secondaryInst = panelObj.split.secondaryInstUrl ? state.connections.find(i => i.url === panelObj.split.secondaryInstUrl) : null;
        secondaryHeader.style.background = _getServerColor(secondaryInst, panelObj.split.secondaryInstUrl);
        secondaryHeader.style.color = _getServerTextColor(secondaryInst, panelObj.split.secondaryInstUrl);
        const serverLabel = secondaryHeader.querySelector('.split-server-label');
        if (serverLabel) serverLabel.textContent = _getServerLabel(secondaryInst, panelObj.split.secondaryInstUrl);
        const cmdLabel = secondaryHeader.querySelector('.split-cmd-label');
        if (cmdLabel) cmdLabel.textContent = _getPanelCmdLabel(panelObj.split.secondaryCmdId, panelObj.split.secondaryInstUrl);
    }
}

/// Render a split container with two vtty-panes and a draggable divider.
/// Each sub-pane has its own header showing the server label and command name.
function _renderSplitContainer(panel) {
    const split = panel.split;
    const dir = split.direction; // 'horizontal' or 'vertical'
    const secondaryId = panel.id + '-secondary';
    const primaryWidth = split.splitRatio ? (split.splitRatio * 100).toFixed(1) : '50';
    const secondaryWidth = (100 - parseFloat(primaryWidth)).toFixed(1);

    // Get server info for primary pane
    const primaryInst = panel.selectedInstUrl ? state.connections.find(i => i.url === panel.selectedInstUrl) : null;
    const primaryServerLabel = _getServerLabel(primaryInst, panel.selectedInstUrl);
    const primaryColor = _getServerColor(primaryInst, panel.selectedInstUrl);
    const primaryTextColor = _getServerTextColor(primaryInst, panel.selectedInstUrl);
    const primaryCmdLabel = _getPanelCmdLabel(panel.selectedCmdId, panel.selectedInstUrl);

    // Get server info for secondary pane
    const secondaryInst = split.secondaryInstUrl ? state.connections.find(i => i.url === split.secondaryInstUrl) : null;
    const secondaryServerLabel = _getServerLabel(secondaryInst, split.secondaryInstUrl);
    const secondaryColor = _getServerColor(secondaryInst, split.secondaryInstUrl);
    const secondaryTextColor = _getServerTextColor(secondaryInst, split.secondaryInstUrl);
    const secondaryCmdLabel = _getPanelCmdLabel(split.secondaryCmdId, split.secondaryInstUrl);

    // Primary sub-pane
    const primaryHtml = `<div class="split-pane" data-split-side="primary" data-panel="${panel.id}" style="flex: 0 0 ${primaryWidth}%; display:flex; flex-direction:column; min-width:0; min-height:0;">
            <div class="split-header panel-header" data-panel-id="${panel.id}" data-split-side="primary" style="background:${primaryColor};color:${primaryTextColor};">
                <span class="split-server-label" style="font-size:var(--ui-fs);opacity:0.8;">${escHtml(primaryServerLabel)}</span>
                <span class="split-cmd-label" style="font-size:var(--ui-fs);font-family:var(--font-mono);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;min-width:0;">${escHtml(primaryCmdLabel)}</span>
                <button class="btn btn-xs btn-danger" data-action="UnsplitPanel" data-panel="${panel.id}" title="Close split">&#x2715;</button>
            </div>
            <div class="vtty-container${panel.selectionMode ? ' selection-mode' : ''}" id="vtty-${panel.id}" data-split-side="primary" data-panel="${panel.id}" ${panel.theme ? 'data-panel-theme="' + panel.theme + '"' : ''} style="font-size: ${panel.fontSize}px; flex:1; min-height:0;">
                <div class="exited-banner" id="exitedBanner-${panel.id}" style="display:none;"></div>
                <div class="search-bar" id="searchBar-${panel.id}">
                    <input type="text" id="searchInput-${panel.id}" placeholder="Search terminal..." oninput="vttySearch('${panel.id}')" onkeydown="if(event.key==='Enter'){event.shiftKey?vttySearchPrev('${panel.id}'):vttySearchNext('${panel.id}')}">
                    <span class="search-count" id="searchCount-${panel.id}" title="Click to jump: Shift+Click to reverse"></span>
                    <div class="search-progress-bar" id="searchProgress-${panel.id}"></div>
                    <button data-action="VttySearchNext" data-panel="${panel.id}" title="Next match (Enter)">&#x25BC;</button>
                    <button data-action="VttySearchPrev" data-panel="${panel.id}" title="Previous match (Shift+Enter)">&#x25B2;</button>
                    <button data-action="VttySearchClose" data-panel="${panel.id}" title="Close search">&#x2715;</button>
                </div>
                <pre>${panel.selectedCmdId ? '' : '<span style="color:#484f58;">No command selected — select a command from the sidebar</span>'}</pre>
                <div class="cursor-indicator" style="display:none;"></div>
                <div class="copy-feedback" id="copyFeedback-${panel.id}">Copied!</div>
                <button class="scroll-bottom-btn" id="scrollBtn-${panel.id}" data-action="ScrollTerminalBottom" data-panel="${panel.id}" title="Scroll to bottom">&#x25BC;</button>
            </div>
        </div>`;

    // Secondary sub-pane
    const secondaryHtml = `<div class="split-pane" data-split-side="secondary" data-panel="${panel.id}" style="flex: 0 0 ${secondaryWidth}%; display:flex; flex-direction:column; min-width:0; min-height:0;">
            <div class="split-header panel-header" data-panel-id="${panel.id}" data-split-side="secondary" style="background:${secondaryColor};color:${secondaryTextColor};">
                <span class="split-server-label" style="font-size:var(--ui-fs);opacity:0.8;">${escHtml(secondaryServerLabel)}</span>
                <span class="split-cmd-label" style="font-size:var(--ui-fs);font-family:var(--font-mono);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;min-width:0;">${escHtml(secondaryCmdLabel)}</span>
                <button class="btn btn-xs btn-danger" data-action="UnsplitPanel" data-panel="${panel.id}" title="Close split">&#x2715;</button>
            </div>
            <div class="vtty-container" id="vtty-${secondaryId}" data-split-side="secondary" data-panel="${panel.id}" ${panel.theme ? 'data-panel-theme="' + panel.theme + '"' : ''} style="font-size: ${panel.fontSize}px; flex:1; min-height:0;">
                <div class="exited-banner" id="exitedBanner-${secondaryId}" style="display:none;"></div>
                <pre>${split.secondaryCmdId ? '' : '<span style="color:#484f58;">No command selected — select a command from the sidebar</span>'}</pre>
                <div class="cursor-indicator" style="display:none;"></div>
                <button class="scroll-bottom-btn" id="scrollBtn-${secondaryId}" data-action="ScrollTerminalBottom" data-panel="${secondaryId}" title="Scroll to bottom">&#x25BC;</button>
            </div>
        </div>`;

    return `<div class="split-container ${dir}" id="split-${panel.id}" data-panel="${panel.id}">
                    ${primaryHtml}
                    <div class="split-divider" data-panel="${panel.id}"></div>
                    ${secondaryHtml}
                </div>`;
}

/// Update the panel header for a split panel to indicate both commands.
function _updateSplitPanelHeader(panelObj) {
    if (!panelObj || !panelObj.split) return;
    // Also update the sub-pane headers (server labels and command names)
    _updateSplitHeaders(panelObj);
    const panelEl = document.getElementById(panelObj.id);
    if (!panelEl) return;
    const nameEl = panelEl.querySelector(':scope > .panel-header .cmd-fullname');
    const argsEl = panelEl.querySelector(':scope > .panel-header .cmd-args');
    if (!nameEl) return;

    // Show the active side's command name
    const s = panelObj.split;
    let cmdId, instUrl;
    if (s.activeSide === 'secondary') {
        cmdId = s.secondaryCmdId;
        instUrl = s.secondaryInstUrl;
    } else {
        cmdId = panelObj.selectedCmdId;
        instUrl = panelObj.selectedInstUrl;
    }

    if (cmdId && instUrl) {
        const inst = state.connections.find(i => i.url === instUrl);
        const cmd = inst && inst._commands ? inst._commands.find(c => c.id === cmdId) : null;
        const fullName = cmd ? (cmd.name || cmd.id) : cmdId;
        const displayTitle = panelObj.customTitle || fullName;
        nameEl.textContent = displayTitle;
        nameEl.title = fullName;
        if (argsEl && cmd) {
            const argsStr = (cmd.args || []).join(' ');
            argsEl.textContent = argsStr;
        }
    }
}


function _renderMinimizedPanels() {
    const minimized = state.panels.filter(p => p.minimized);
    if (minimized.length === 0) return '';
    let html = '<div class="minimized-panels" id="minimizedPanels">';
    for (const panel of minimized) {
        let label = 'Panel';
        if (panel.customTitle) {
            label = panel.customTitle;
        } else if (panel.selectedCmdId) {
            for (const inst of state.connections) {
                if (inst._commands) {
                    const cmd = inst._commands.find(c => c.id === panel.selectedCmdId);
                    if (cmd) { label = cmd.name || cmd.id; break; }
                }
            }
        }
        html += `<div class="minimized-panel-item" data-action="ToggleMinimizePanel" data-panel="${panel.id}" title="Click to restore: ${escHtml(label)}">
            <span class="minimized-icon">&#x25A0;</span>
            <span class="minimized-label">${escHtml(label)}</span>
        </div>`;
    }
    html += '</div>';
    return html;
}

/// Toggle panel layout between horizontal (row) and vertical (column).
function togglePanelLayout() {
    state.panelLayout = state.panelLayout === 'row' ? 'column' : 'row';
    localStorage.setItem('vrw_panel_layout', state.panelLayout);
    renderPanels();
}

/// Toggle the layout preset dropdown menu.
function toggleLayoutPresetMenu(event) {
    event.stopPropagation();
    const menu = document.getElementById('layoutPresetMenu');
    const isVisible = menu.style.display !== 'none';
    menu.style.display = isVisible ? 'none' : 'block';
    // Close on outside click
    if (!isVisible) {
        setTimeout(() => {
            document.addEventListener('click', function closeMenu(e) {
                document.removeEventListener('click', closeMenu);
                menu.style.display = 'none';
            }, { once: true });
        }, 0);
    }
}

/// Apply a layout preset. Creates/removes panels as needed and sets the layout.
function applyLayoutPreset(preset) {
    // Close the menu
    const menu = document.getElementById('layoutPresetMenu');
    if (menu) menu.style.display = 'none';

    // Determine how many panels this preset needs
    const panelCounts = { 'row': null, 'column': null, 'grid-2x2': 4, 'grid-1-2': 3, 'grid-2-1': 3 };
    const neededCount = panelCounts[preset];
    const isGrid = preset.startsWith('grid-');

    // Adjust panel count for grid presets
    if (neededCount !== null) {
        // Remove excess panels (from the end)
        while (state.panels.length > neededCount) {
            const removed = state.panels.pop();
            disconnectPanelWs(removed.id);
            stopPanelPoll(removed.id);
        }
        // Add missing panels
        while (state.panels.length < neededCount) {
            addPanelDirect();
        }
    }

    state.panelLayout = preset;
    localStorage.setItem('vrw_panel_layout', state.panelLayout);
    renderPanels();

    // Focus the first panel if none is focused
    if (state.panels.length > 0) {
        const focused = state.panels.find(p => p.id === state._focusedPanelId);
        if (!focused) focusPanel(state.panels[0].id);
    }
}

/// Apply the panel layout class to the container element.
/// Grid presets use CSS grid (class-based), row/column use flexbox (inline style).
function _applyPanelLayoutClass(container) {
    // On mobile tabbed layout, force column direction and clear grid
    if (state._mobileTabbedLayout) {
        container.classList.remove('grid-2x2', 'grid-1-2', 'grid-2-1');
        container.style.flexDirection = 'column';
        return;
    }
    // Remove all layout classes first
    container.classList.remove('grid-2x2', 'grid-1-2', 'grid-2-1');
    if (state.panelLayout.startsWith('grid-')) {
        // CSS grid mode: add the grid class, clear flexDirection
        container.classList.add(state.panelLayout);
        container.style.flexDirection = '';
    } else {
        // Flexbox mode: set direction, no grid class
        container.style.flexDirection = state.panelLayout;
    }
}

function renderPanels() {
    const container = document.getElementById('view-vtty');
    const visiblePanels = state.panels.filter(p => !p.minimized);
    const hasMultiplePanels = visiblePanels.length > 1;

    // Recalculate welcome state BEFORE the fast-path check.
    // This ensures that when commands arrive (or the server becomes reachable)
    // after the welcome screen was shown, the fast path is invalidated and
    // the welcome screen is properly dismissed.
    let hasAnyCommands = false;
    for (const inst of state.connections) {
        if (inst._commands && inst._commands.length > 0) {
            hasAnyCommands = true;
            break;
        }
    }
    const shouldShowWelcome = (!hasAnyCommands && !state.selectedCmdId && !state.serverReachable);
    if (shouldShowWelcome !== _showingWelcome) {
        _showingWelcome = shouldShowWelcome;
    }

    // ── Cache all terminal DOM before rebuild ──
    const cachedVtty = {};
    for (const panel of state.panels) {
        const el = document.getElementById(panel.id);
        if (!el) continue;
        // Use ID-based lookup to get the correct vtty-container (primary or non-split)
        const vttyEl = document.getElementById('vtty-' + panel.id);
        const pre = vttyEl ? vttyEl.querySelector('pre') : null;
        if (pre && pre.childNodes.length > 0 && panel.selectedCmdId) {
            const frag = document.createDocumentFragment();
            while (pre.firstChild) frag.appendChild(pre.firstChild);
            cachedVtty[panel.id] = {
                frag,
                scrollTop: vttyEl ? vttyEl.scrollTop : 0,
                cmdId: panel.selectedCmdId,
            };
        }
        // Also cache secondary pane if panel is split
        if (panel.split && panel.split.secondaryCmdId) {
            const secondaryId = panel.id + '-secondary';
            const secondaryVtty = document.getElementById('vtty-' + secondaryId);
            const secondaryPre = secondaryVtty ? secondaryVtty.querySelector('pre') : null;
            if (secondaryPre && secondaryPre.childNodes.length > 0) {
                const secFrag = document.createDocumentFragment();
                while (secondaryPre.firstChild) secFrag.appendChild(secondaryPre.firstChild);
                cachedVtty[secondaryId] = {
                    frag: secFrag,
                    scrollTop: secondaryVtty ? secondaryVtty.scrollTop : 0,
                    cmdId: panel.split.secondaryCmdId,
                };
            }
        }
    }

    let html = '';

    // Apply panel layout direction
    _applyPanelLayoutClass(container);

    if (!hasAnyCommands && !state.selectedCmdId && !state.serverReachable) {
        _showingWelcome = true;
        // Hide shared toolbar in welcome state
        const toolbar = document.getElementById('sharedToolbar');
        if (toolbar) toolbar.style.display = 'none';
        // Server is unreachable — vrw is not running
        html += `
            <div class="welcome-panel">
                <div class="welcome-card">
                    <img src="/favicon.png" alt="vrw" style="height:2rem;width:auto;margin-bottom:0.75rem;">
                    <p class="welcome-not-running">vrw is not running</p>
                    <p style="margin-top:0.25rem;">No vrw instance could be reached at <span class="welcome-url">${escHtml(getBaseUrl())}</span></p>
                    <p>Start vrw and refresh this page to connect.</p>
                </div>
            </div>`;
    } else {
        _showingWelcome = false;
        // Show the shared toolbar when panels are visible
        const toolbar = document.getElementById('sharedToolbar');
        if (toolbar) toolbar.style.display = '';

        // On mobile: render tab bar for multiple panels
        const isMobile = state._mobileTabbedLayout;
        if (isMobile && state.panels.length > 1) {
            html += '<div class="mobile-tab-bar" id="mobileTabBar">';
            for (const panel of state.panels) {
                const isFocused = panel.id === state._focusedPanelId;
                // Find command name for tab label
                let tabLabel = 'Panel';
                if (panel.customTitle) {
                    tabLabel = panel.customTitle;
                } else if (panel.selectedCmdId) {
                    for (const inst of state.connections) {
                        if (inst._commands) {
                            const cmd = inst._commands.find(c => c.id === panel.selectedCmdId);
                            if (cmd) { tabLabel = cmd.name || cmd.id; break; }
                        }
                    }
                }
                html += `<div class="mobile-tab${isFocused ? ' active' : ''}" data-action="FocusPanel" data-panel="${panel.id}" title="${escHtml(tabLabel)}">
                    <span class="mobile-tab-label">${escHtml(tabLabel)}</span>
                    ${state.panels.length > 1 ? `<button class="mobile-tab-close" data-action="ClosePanelContent" data-panel="${panel.id}" title="Remove">&#x2715;</button>` : ''}
                </div>`;
            }
            html += '</div>';
        }

        for (const panel of state.panels) {
            // Skip minimized panels — they're shown in the minimized strip
            if (panel.minimized) continue;
            const conn = panel.selectedInstUrl ? state.connections.find(i => i.url === panel.selectedInstUrl) : null;
            const serverLabel = _getServerLabel(conn, panel.selectedInstUrl);
            const serverColor = _getServerColor(conn, panel.selectedInstUrl);
            const serverTextColor = _getServerTextColor(conn, panel.selectedInstUrl);
            const resizeHandle = hasMultiplePanels ? `<div class="panel-resize-handle" data-panel="${panel.id}"></div>` : '';
            const dragHandle = hasMultiplePanels ? `<span class="drag-handle" draggable="true" ondragstart="onPanelDragStart(event,'${panel.id}')" ondragend="onPanelDragEnd(event)" title="Drag to reorder">&#x2840;</span>` : '';
            const isFocused = panel.id === state._focusedPanelId;
            const mobileHidden = isMobile && hasMultiplePanels && !isFocused ? ' style="display:none;"' : '';
            html += `
                <div class="panel${isFocused ? ' focused' : ''}" id="${panel.id}" draggable="false" ondragover="onPanelDragOver(event)" ondrop="onPanelDrop(event,'${panel.id}')" ondragleave="onPanelDragLeave(event)"${mobileHidden}>
                    <div class="panel-header" data-panel-id="${panel.id}" oncontextmenu="showPanelContextMenu(event,'${panel.id}')" tabindex="0" role="button" aria-label="Panel: ${escHtml(panel.selectedInstUrl || 'empty')}" style="background:${serverColor};color:${serverTextColor};">
                        ${dragHandle}
                        <button class="btn btn-xs btn-danger panel-close-btn" data-action="ClosePanelContent" data-panel="${panel.id}" title="Close panel">&#x2715;</button>
                        <span class="panel-server-badge" style="font-size:var(--ui-fs);opacity:0.7;flex-shrink:0;">${escHtml(serverLabel)}</span>
                        <button class="btn btn-xs cmd-history-btn" id="histBack-${panel.id}" data-action="PanelHistoryBack" data-panel="${panel.id}" title="Back in command history" style="display:none;">&#x25C0;</button>
                        <button class="btn btn-xs cmd-history-btn" id="histFwd-${panel.id}" data-action="PanelHistoryForward" data-panel="${panel.id}" title="Forward in command history" style="display:none;">&#x25B6;</button>
                        <div class="cmd-info" id="cmdInfo-${panel.id}">
                            <span class="cmd-fullname" id="cmdName-${panel.id}" ondblclick="event.stopPropagation();startRenamePanel('${panel.id}')" title="Double-click to rename"></span>
                            <span class="cmd-args" id="cmdArgs-${panel.id}"></span>
                        </div>
                        <span class="panel-header-label" id="panelLabel-${panel.id}"></span>
                    </div>
                    ${panel.split ? _renderSplitContainer(panel) : _renderVttyContainer(panel)}
                </div>
                ${resizeHandle}`;
        }
    }
    // Append minimized panels strip
    html += _renderMinimizedPanels();
    container.innerHTML = html;

    // ── Restore cached terminal DOM after rebuild ──
    for (const [panelId, cached] of Object.entries(cachedVtty)) {
        const el = document.getElementById(panelId);
        if (!el) continue;
        const pre = el.querySelector('pre');
        const vttyEl = el.querySelector('.vtty-container');
        if (pre) {
            pre.innerHTML = '';
            pre.appendChild(cached.frag);
        }
        if (vttyEl) {
            vttyEl.scrollTop = cached.scrollTop;
        }
    }

    // ── Attach event listeners ──
    // Panel focus: clicking in a vtty-container or panel-header sets it as focused.
    document.querySelectorAll('.panel').forEach(panelEl => {
        const panelId = panelEl.id;
        // Click on terminal area → focus this panel (and set activeSide for split panels)
        const vttyContainers = panelEl.querySelectorAll('.vtty-container');
        vttyContainers.forEach(vttyEl => {
            vttyEl.addEventListener('mousedown', () => {
                focusPanel(panelId);
                // Track which sub-pane is active in split panels
                const panelObj = state.panels.find(p => p.id === panelId);
                if (panelObj && panelObj.split) {
                    const side = vttyEl.getAttribute('data-split-side');
                    if (side) panelObj.split.activeSide = side;
                }
            });
        });
        // Click on panel header → focus this panel
        const headerEl = panelEl.querySelector('.panel-header');
        if (headerEl) {
            headerEl.addEventListener('mousedown', () => {
                focusPanel(panelId);
            });
        }
        // Click on split headers → set activeSide
        const splitHeaders = panelEl.querySelectorAll('.split-header');
        splitHeaders.forEach(splitHeader => {
            splitHeader.addEventListener('mousedown', () => {
                focusPanel(panelId);
                const panelObj = state.panels.find(p => p.id === panelId);
                if (panelObj && panelObj.split) {
                    const side = splitHeader.getAttribute('data-split-side');
                    if (side) panelObj.split.activeSide = side;
                }
            });
        });
    });

    // Scroll-to-bottom button visibility
    document.querySelectorAll('.vtty-container').forEach(vtty => {
        vtty.addEventListener('scroll', () => {
            // Find the scroll-bottom-btn INSIDE this specific vtty-container
            const btn = vtty.querySelector('.scroll-bottom-btn');
            if (!btn) return;
            const isNearBottom = vtty.scrollHeight - vtty.scrollTop - vtty.clientHeight < 50;
            btn.classList.toggle('visible', !isNearBottom);
        });
    });

    // Split divider drag handler
    document.querySelectorAll('.split-divider').forEach(divider => {
        const panelId = divider.getAttribute('data-panel');
        const panelObj = state.panels.find(p => p.id === panelId);
        if (!panelObj || !panelObj.split) return;
        const splitContainer = divider.parentElement;
        const dir = panelObj.split.direction;

        divider.addEventListener('mousedown', (e) => {
            e.preventDefault();
            divider.classList.add('active');
            const startPos = dir === 'horizontal' ? e.clientX : e.clientY;
            const containerSize = dir === 'horizontal' ? splitContainer.offsetWidth : splitContainer.offsetHeight;
            const startRatio = panelObj.split.splitRatio || 0.5;

            const onMouseMove = (e) => {
                const currentPos = dir === 'horizontal' ? e.clientX : e.clientY;
                const delta = currentPos - startPos;
                let newRatio = startRatio + (delta / containerSize);
                newRatio = Math.max(0.1, Math.min(0.9, newRatio));
                panelObj.split.splitRatio = newRatio;

                // Update flex basis for both split-panes
                const splitPanes = splitContainer.querySelectorAll('.split-pane');
                if (splitPanes.length === 2) {
                    const pct1 = (newRatio * 100).toFixed(1);
                    const pct2 = (100 - parseFloat(pct1)).toFixed(1);
                    splitPanes[0].style.flex = `0 0 ${pct1}%`;
                    splitPanes[1].style.flex = `0 0 ${pct2}%`;
                }
            };

            const onMouseUp = () => {
                divider.classList.remove('active');
                document.removeEventListener('mousemove', onMouseMove);
                document.removeEventListener('mouseup', onMouseUp);
            };

            document.addEventListener('mousemove', onMouseMove);
            document.addEventListener('mouseup', onMouseUp);
        });
    });

    // Persist panel count for reload
    localStorage.setItem('vrw_panel_count', state.panels.length.toString());
    _updatePanelMultiUI();
    // Sync shared toolbar with current state
    if (!_showingWelcome) updateSharedToolbar();

    // ── Reconnect per-panel WS/poll after full DOM rebuild ──
    // A full rebuild destroys and recreates all DOM elements.  The existing
    // WS objects on panelObj are still alive (their onmessage closures use
    // document.getElementById which finds the new elements), but if the WS
    // silently died during the rebuild or the browser closed it when the
    // old DOM was GC'd, we need to re-establish the connection.  Calling
    // startPanelUpdateMode for each panel with an active command ensures
    // a live WS/poll is running after the rebuild.
    if (!_showingWelcome && state.bufferView === 'current') {
        for (const panelObj of state.panels) {
            if (panelObj.selectedCmdId && panelObj.selectedInstUrl) {
                // Only reconnect if there's no live WS already
                if (!panelObj.ws || panelObj.ws.readyState !== WebSocket.OPEN) {
                    startPanelUpdateMode(panelObj.id);
                }
            }
        }
    }
    // NOTE: initPanelDropTargets() is intentionally NOT called here.
    // Command drag-and-drop from sidebar is handled by inline ondragover/ondrop
    // handlers on each .panel element (onPanelDragOver/onPanelDrop), which already
    // detect command drops via the 'application/x-cmd' dataTransfer type.
    // Calling initPanelDropTargets() would add duplicate addEventListener listeners.
}

/// Update multi-panel UI elements (drag handles, remove buttons, layout toggle)
/// without rebuilding the entire panel DOM.
function _updatePanelMultiUI() {
    const hasMultiplePanels = state.panels.length > 1;
    const isGrid = state.panelLayout.startsWith('grid-');
    document.querySelectorAll('.drag-handle').forEach(el => el.style.display = hasMultiplePanels ? '' : 'none');
    document.querySelectorAll('.panel-resize-handle').forEach(el => el.style.display = (hasMultiplePanels && !isGrid) ? '' : 'none');
    const layoutBtn = document.getElementById('stLayoutBtn');
    if (layoutBtn) layoutBtn.style.display = hasMultiplePanels ? '' : 'none';
    const presetBtn = document.getElementById('stLayoutPresetBtn');
    if (presetBtn) presetBtn.style.display = hasMultiplePanels ? '' : 'none';
}


/// Focus a panel: update focused state, visual indicator, and shared toolbar.
function focusPanel(panelId) {
    if (state._focusedPanelId === panelId) return;
    state._focusedPanelId = panelId;
    // Update visual indicator
    document.querySelectorAll('.panel').forEach(el => {
        el.classList.toggle('focused', el.id === panelId);
    });
    // Mobile: show focused panel, hide others
    if (state._mobileTabbedLayout) {
        document.querySelectorAll('.panel').forEach(el => {
            el.style.display = el.id === panelId ? '' : 'none';
        });
        // Update mobile tab bar
        document.querySelectorAll('.mobile-tab').forEach(el => {
            el.classList.toggle('active', el.getAttribute('data-panel') === panelId);
        });
    }
    // Sync global state from the focused panel
    const panelObj = state.panels.find(p => p.id === panelId);
    if (panelObj) {
        state.selectedInstUrl = panelObj.selectedInstUrl;
        state.selectedCmdId = panelObj.selectedCmdId;
    }
    // Update shared toolbar to reflect focused panel's state
    updateSharedToolbar();
}

/// Update the shared toolbar to reflect the focused panel's state.
/// Called when focus changes, command selection changes, or font/theme changes.
function updateSharedToolbar() {
    const panelId = getActivePanelId();
    if (!panelId) return;
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;

    // Font size
    const fontSizeEl = document.getElementById('stFontSize');
    if (fontSizeEl) fontSizeEl.textContent = panelObj.fontSize + 'px';

    // Theme button
    const themeBtn = document.getElementById('stPanelThemeBtn');
    if (themeBtn) {
        themeBtn.textContent = panelObj.theme === 'light' ? '\u263E' : panelObj.theme === 'dark' ? '\u2600' : '\u25D0';
        themeBtn.title = panelObj.theme === 'light' ? 'Panel theme: light (click to toggle)' : panelObj.theme === 'dark' ? 'Panel theme: dark (click to toggle)' : 'Panel theme: inherit (click to toggle)';
    }

    // Selection mode button
    const selectBtn = document.getElementById('stSelectBtn');
    if (selectBtn) {
        selectBtn.classList.toggle('btn-primary', panelObj.selectionMode);
        selectBtn.textContent = panelObj.selectionMode ? '\u2713 Select' : 'Select';
    }

    // Instance URL
    const instUrlEl = document.getElementById('stInstanceUrl');
    if (instUrlEl) instUrlEl.textContent = (panelObj.selectedInstUrl || '').replace(/^https?:\/\//, '');

    // Refresh throttle
    const refreshVal = document.getElementById('stRefreshVal');
    if (refreshVal) refreshVal.textContent = state.refreshMs || 'off';

    // Buffer select
    const bufferSel = document.getElementById('stBufferSelect');
    if (bufferSel) bufferSel.value = state.bufferView || 'current';

    // Resource badge
    const resourceBadge = document.getElementById('stResourceBadge');
    if (resourceBadge && panelObj.selectedCmdId) {
        const res = state._resourceCache[panelObj.selectedCmdId];
        if (state.showResources && res && (res.cpu_percent != null || res.memory_mb != null)) {
            resourceBadge.style.display = '';
            resourceBadge.textContent = (res.cpu_percent != null ? 'CPU ' + res.cpu_percent.toFixed(1) + '%' : '') +
                (res.memory_mb != null ? ' MEM ' + res.memory_mb.toFixed(1) + 'MB' : '');
        } else {
            resourceBadge.style.display = 'none';
        }
    }

    // Restart button visibility
    const restartBtn = document.getElementById('stRestartBtn');
    if (restartBtn) {
        restartBtn.style.display = panelObj.selectedCmdId ? '' : 'none';
    }

    // Freeze/thaw button
    const freezeBtn = document.getElementById('stFreezeBtn');
    if (freezeBtn) {
        if (panelObj.selectedCmdId) {
            const inst = state.connections.find(i => i.url === panelObj.selectedInstUrl);
            const cmd = inst && inst._commands ? inst._commands.find(c => c.id === panelObj.selectedCmdId) : null;
            const isAlive = cmd && cmd.alive !== false;
            const isFrozen = cmd && cmd.frozen === true;
            freezeBtn.style.display = isAlive ? '' : 'none';
            freezeBtn.textContent = isFrozen ? '\u25B6' : '\u23F8';
            freezeBtn.title = isFrozen ? 'Thaw command' : 'Freeze command';
            freezeBtn.classList.toggle('btn-primary', isFrozen);
        } else {
            freezeBtn.style.display = 'none';
        }
    }

    // Max Fit button state
    const maxFitBtn = document.getElementById('stMaxFitBtn');
    if (maxFitBtn) {
        const fitState = _maxFitState[panelId];
        maxFitBtn.classList.toggle('btn-primary', !!(fitState && fitState.active));
    }

    // Max Font button state
    const maxFontBtn = document.getElementById('stMaxFontBtn');
    if (maxFontBtn) {
        const fontState = _maxFontState[panelId];
        maxFontBtn.classList.toggle('btn-primary', !!(fontState && fontState.active));
    }
}

async function sendKeysToPanel(panelId) {
    const panel = state.panels.find(p => p.id === panelId);
    if (!panel) return;
    // Try the shared toolbar input first, fall back to per-panel input
    const input = document.getElementById('stKeyInput') || document.getElementById('keyInput-' + panelId);
    if (!input || !input.value || !state.selectedCmdId) return;

    const keysValue = input.value;
    const cmdId = panel.selectedCmdId || state.selectedCmdId;
    const instUrl = panel.selectedInstUrl || state.selectedInstUrl;

    try {
        const json = await api.sendKeys(instUrl, cmdId, { keys: keysValue });
        if (json.status === 'ok') {
            input.value = '';
            loadVttyHttp(instUrl, cmdId);
        } else {
            console.error('send_keys server error:', json.error);
            input.value = '';
        }
    } catch (e) {
        console.error('send_keys error:', e);
    }
}

// ─── Special Keys Help ───
function showSpecialKeysHelp() {
    // Remove existing modal if present
    const old = document.getElementById('specialKeysModal');
    if (old) { old.remove(); return; }

    const overlay = document.createElement('div');
    overlay.id = 'specialKeysModal';
    overlay.className = 'modal-overlay';
    overlay.style.display = 'flex';
    overlay.onclick = (e) => { if (e.target === overlay) { releaseCurrentFocusTrap(); overlay.remove(); } };

    overlay.innerHTML = `<div class="modal" style="max-width:560px;max-height:80vh;overflow-y:auto;">
        <h2 style="margin-bottom:0.5rem;">Special Keys Reference</h2>
        <p style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.75rem;">
            Type special keys using <code style="background:var(--bg-tertiary);padding:0.1rem 0.3rem;border-radius:2px;">&lt;KeyName&gt;</code> syntax in the send-keys input.
            You can mix plain text with special keys, e.g. <code style="background:var(--bg-tertiary);padding:0.1rem 0.3rem;border-radius:2px;">hello&lt;Enter&gt;world</code>.
        </p>
        <table style="width:100%;font-size:0.75rem;border-collapse:collapse;">
            <thead>
                <tr style="border-bottom:1px solid var(--border);text-align:left;">
                    <th style="padding:0.3rem 0.5rem;color:var(--text-muted);font-weight:600;">Key</th>
                    <th style="padding:0.3rem 0.5rem;color:var(--text-muted);font-weight:600;">Syntax</th>
                    <th style="padding:0.3rem 0.5rem;color:var(--text-muted);font-weight:600;">Description</th>
                </tr>
            </thead>
            <tbody>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Return / Enter</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Enter&gt;</code> or <code>&lt;Return&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Send a newline (carriage return)</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Backspace</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Backspace&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Delete character before cursor</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Tab</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Tab&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Insert a tab character</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Escape</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Esc&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Send the Escape character (0x1B)</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Space</td><td style="padding:0.25rem 0.5rem;">(space character)</td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Type a literal space in the input</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Delete</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Delete&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Delete character at cursor (forward delete)</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Insert</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Insert&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Toggle insert/overwrite mode</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Home / End</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Home&gt;</code> <code>&lt;End&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Jump to beginning / end of line</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Page Up / Down</td><td style="padding:0.25rem 0.5rem;"><code>&lt;PageUp&gt;</code> <code>&lt;PageDown&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Scroll up / down one page</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Arrow Keys</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Up&gt;</code> <code>&lt;Down&gt;</code> <code>&lt;Left&gt;</code> <code>&lt;Right&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Cursor movement</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">F1 &ndash; F12</td><td style="padding:0.25rem 0.5rem;"><code>&lt;F1&gt;</code> &hellip; <code>&lt;F12&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Function keys</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Ctrl + key</td><td style="padding:0.25rem 0.5rem;"><code>&lt;C-c&gt;</code> <code>&lt;C-a&gt;</code> <code>&lt;C-d&gt;</code> &hellip;</td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Control modifier (use lowercase letter). <code>&lt;C-c&gt;</code> = SIGINT (interrupt)</td></tr>
                <tr><td style="padding:0.25rem 0.5rem;">Alt + key</td><td style="padding:0.25rem 0.5rem;"><code>&lt;A-x&gt;</code> <code>&lt;A-enter&gt;</code> &hellip;</td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Alt/Meta modifier prefix (Escape + key)</td></tr>
            </tbody>
        </table>
        <div style="margin-top:0.75rem;text-align:right;">
            <button class="btn btn-xs" data-action="CloseSpecialKeysModal">Close</button>
        </div>
    </div>`;

    document.body.appendChild(overlay);
    const panel = overlay.querySelector('.modal');
    if (panel) trapFocus(panel);
    const closeBtn = overlay.querySelector('button');
    if (closeBtn) closeBtn.focus();
}


// ─── Panel Resize via Drag ───
(function() {
    let resizing = false;
    let startX = 0;
    let startWidth = 0;
    let resizePanel = null;

    document.addEventListener('mousedown', (e) => {
        const handle = e.target.closest('.panel-resize-handle');
        if (!handle) return;
        e.preventDefault();
        resizePanel = handle.previousElementSibling;
        if (!resizePanel) return;
        startX = e.clientX;
        startWidth = resizePanel.getBoundingClientRect().width;
        handle.classList.add('active');
        resizing = true;
    });

    document.addEventListener('mousemove', (e) => {
        if (!resizing || !resizePanel) return;
        const delta = e.clientX - startX;
        const containerWidth = resizePanel.parentElement.getBoundingClientRect().width;
        const panelCount = resizePanel.parentElement.children.length;
        const minW = 100;
        const newWidth = Math.max(minW, Math.min(containerWidth - (panelCount - 1) * minW, startWidth + delta));
        const pct = (newWidth / containerWidth) * 100;
        resizePanel.style.flex = `0 0 ${pct}%`;
    });

    document.addEventListener('mouseup', () => {
        if (resizing) {
            document.querySelectorAll('.panel-resize-handle.active').forEach(h => h.classList.remove('active'));
            resizing = false;
            resizePanel = null;
        }
    });
})();

// ─── Export Terminal Output ───
/// Copy terminal text to the clipboard.
/// If the user has selected text in the VTTY, copy that selection.
/// Otherwise, fall back to the full VTTY content.
function copyTerminalSelection(panelId) {
    const panel = document.getElementById(panelId);
    if (!panel) return;

    // First try the browser text selection
    const selection = window.getSelection();
    let text = selection ? selection.toString().trim() : '';

    // Fallback: copy full VTTY content
    if (!text) {
        const pre = panel.querySelector('pre');
        if (pre) {
            text = pre.textContent || pre.innerText || '';
        }
    }

    if (!text) return;

    navigator.clipboard.writeText(text).then(() => {
        // Show "Copied!" feedback
        const feedback = document.getElementById('copyFeedback-' + panelId);
        if (feedback) {
            feedback.classList.add('visible');
            setTimeout(() => feedback.classList.remove('visible'), 1200);
        }
    }).catch(() => {
        // Clipboard API may fail (e.g. non-HTTPS); fall back to execCommand
        const ta = document.createElement('textarea');
        ta.value = text;
        ta.style.cssText = 'position:fixed;opacity:0;';
        document.body.appendChild(ta);
        ta.select();
        try { document.execCommand('copy'); } catch (_) { /* ignore */ }
        document.body.removeChild(ta);
        const feedback = document.getElementById('copyFeedback-' + panelId);
        if (feedback) {
            feedback.classList.add('visible');
            setTimeout(() => feedback.classList.remove('visible'), 1200);
        }
    });
}

function exportTerminal(panelId) {
    const panel = document.getElementById(panelId);
    if (!panel) return;
    const pre = panel.querySelector('pre');
    if (!pre) return;
    const text = pre.textContent || pre.innerText || '';
    const blob = new Blob([text], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    // Use command name for the filename
    let cmdName = 'terminal';
    for (const inst of state.connections) {
        if (inst._commands) {
            const cmd = inst._commands.find(c => c.id === state.selectedCmdId);
            if (cmd) { cmdName = (cmd.name || cmd.id).replace(/\//g, '_'); break; }
        }
    }
    a.href = url;
    a.download = cmdName + '.txt';
    a.click();
    URL.revokeObjectURL(url);
}

/// Download a PNG screenshot of the currently selected command's VTTY buffer.
/// Uses server-configured default font size and font name.
async function screenshotPanel(panelId) {
    // Determine which command is shown in this panel
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const instUrl = panelObj.selectedInstUrl || state.selectedInstUrl;
    const isSelectedPanel = (instUrl === state.selectedInstUrl);
    const cmdId = isSelectedPanel ? state.selectedCmdId : null;
    if (!cmdId) {
        alert('No command selected to screenshot.');
        return;
    }

    // Use server-configured defaults for font
    const fontSize = state.serverScreenshotFontSize || 12;
    const fontName = state.serverScreenshotFontName || 'monospace';

    // Build PNG screenshot parameters
    const params = new URLSearchParams({ font_size: fontSize });
    if (fontName && fontName !== 'monospace') {
        params.set('font_name', fontName);
    }
    try {
        const blob = await api.getVttyPng(instUrl, cmdId, Object.fromEntries(params));
        const blobUrl = URL.createObjectURL(blob);
        const a = document.createElement('a');

        // Build filename: vrw_YYYYMMDD_HHMMSS_rowsxcols_command_args.png
        let cmdInfo = 'vrw';
        for (const inst of state.connections) {
            if (inst._commands) {
                const cmd = inst._commands.find(c => c.id === cmdId);
                if (cmd) {
                    const parts = [cmd.name || 'unknown'];
                    if (cmd.args && cmd.args.length > 0) parts.push(...cmd.args);
                    cmdInfo = parts.join(' ').replace(/[^a-zA-Z0-9_\-\.]/g, '_');
                    break;
                }
            }
        }
        const now = new Date();
        const pad = (n) => String(n).padStart(2, '0');
        const ts = `${now.getFullYear()}${pad(now.getMonth()+1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;

        // Include terminal dimensions if known from VTTY metadata
        let dims = '';
        const pre = document.querySelector(`#vtty-${panelId} pre`);
        if (pre && pre._vttyRows && pre._vttyCols) {
            dims = pre._vttyRows + 'x' + pre._vttyCols;
        }

        const truncated = cmdInfo.length > 120 ? cmdInfo.substring(0, 117) + '...' : cmdInfo;
        const filename = dims
            ? `vrw_${ts}_${dims}_${truncated}.png`
            : `vrw_${ts}_${truncated}.png`;

        a.href = blobUrl;
        a.download = filename;
        a.click();
        URL.revokeObjectURL(blobUrl);
    } catch (e) {
        alert('Screenshot failed: ' + e.message);
    }
}

// ─── Right-click Context Menu ───
// Tracks the currently focused menu item index for keyboard navigation.
let _ctxMenuFocusedIndex = -1;

function closeContextMenu() {
    const el = document.getElementById('ctxMenu');
    if (el) el.remove();
    _ctxMenuFocusedIndex = -1;
}

// Helper: create a single context menu item div with safe textContent + addEventListener.
function _createCtxMenuItem(label, onClick, isDanger) {
    const div = document.createElement('div');
    div.className = 'ctx-menu-item' + (isDanger ? ' danger' : '');
    div.setAttribute('role', 'menuitem');
    div.setAttribute('tabindex', '-1');
    div.textContent = label;
    div.addEventListener('click', () => {
        onClick();
        closeContextMenu();
    });
    return div;
}

// Helper: position menu at (x, y), ensuring it stays within the viewport.
function _positionCtxMenu(menu, x, y) {
    menu.style.left = x + 'px';
    menu.style.top = y + 'px';
    document.body.appendChild(menu);
    const rect = menu.getBoundingClientRect();
    if (rect.right > window.innerWidth) menu.style.left = (window.innerWidth - rect.width - 4) + 'px';
    if (rect.bottom > window.innerHeight) menu.style.top = (window.innerHeight - rect.height - 4) + 'px';
}

// Helper: set up close-on-click-outside and keyboard navigation for a context menu.
function _setupCtxMenuListeners(menu) {
    // Close on click outside
    setTimeout(() => {
        document.addEventListener('click', closeContextMenu, { once: true });
    }, 0);

    // Keyboard navigation inside the context menu
    menu.addEventListener('keydown', (e) => {
        const items = menu.querySelectorAll('.ctx-menu-item');
        if (items.length === 0) return;

        if (e.key === 'ArrowDown') {
            e.preventDefault();
            _ctxMenuFocusedIndex = (_ctxMenuFocusedIndex + 1) % items.length;
            _focusCtxMenuItem(items);
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            _ctxMenuFocusedIndex = (_ctxMenuFocusedIndex - 1 + items.length) % items.length;
            _focusCtxMenuItem(items);
        } else if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            if (_ctxMenuFocusedIndex >= 0 && _ctxMenuFocusedIndex < items.length) {
                items[_ctxMenuFocusedIndex].click();
            }
        } else if (e.key === 'Escape') {
            e.preventDefault();
            closeContextMenu();
        } else if (e.key === 'Tab') {
            // Prevent tab from leaving the menu
            e.preventDefault();
            closeContextMenu();
        }
    });

    // Focus the first item for keyboard users
    _ctxMenuFocusedIndex = 0;
    const firstItem = menu.querySelector('.ctx-menu-item');
    if (firstItem) firstItem.focus();
}

function _focusCtxMenuItem(items) {
    items.forEach((item, i) => {
        item.classList.toggle('ctx-menu-focused', i === _ctxMenuFocusedIndex);
        if (i === _ctxMenuFocusedIndex) {
            item.focus();
        }
    });
}

function showCmdContextMenu(e, instUrl, cmdId, cmdName, isAlive, isRetained) {
    e.preventDefault();
    closeContextMenu();
    const menu = document.createElement('div');
    menu.id = 'ctxMenu';
    menu.className = 'ctx-menu';
    menu.setAttribute('role', 'menu');

    // View Terminal
    menu.appendChild(_createCtxMenuItem('View Terminal', () => selectCommand(instUrl, cmdId, cmdName), false));
    // Copy URL
    menu.appendChild(_createCtxMenuItem('Copy URL', () => copyCommandUrl(instUrl, cmdId, cmdName), false));

    // Add to Group submenu
    const groups = getCmdGroups();
    const groupNames = Object.keys(groups);
    if (groupNames.length > 0) {
        // Separator
        const sepGroup = document.createElement('div');
        sepGroup.className = 'ctx-menu-sep';
        sepGroup.setAttribute('role', 'separator');
        menu.appendChild(sepGroup);

        // "Add to Group" — show sub-pickers for each group
        for (const gName of groupNames) {
            const inGroup = groups[gName].includes(cmdName);
            const label = (inGroup ? '✓ ' : '') + escHtml(gName);
            menu.appendChild(_createCtxMenuItem(label, () => {
                toggleCmdInGroup(gName, cmdName);
            }, false));
        }
    }

    if (isAlive) {
        // Separator
        const sep1 = document.createElement('div');
        sep1.className = 'ctx-menu-sep';
        sep1.setAttribute('role', 'separator');
        menu.appendChild(sep1);
        // Keep/Unkeep
        const keepLabel = isRetained ? 'Unkeep' : 'Keep';
        menu.appendChild(_createCtxMenuItem(keepLabel, () => toggleKeepCmd(instUrl, cmdId), false));
        // Pause/Resume
        menu.appendChild(_createCtxMenuItem('Pause/Resume', () => togglePauseCmd(instUrl, cmdId), false));
        // Restart
        menu.appendChild(_createCtxMenuItem('Restart', () => restartCommandById(instUrl, cmdId), false));
        // Kill
        menu.appendChild(_createCtxMenuItem('Kill', () => killCommand(instUrl, cmdId), true));
    } else {
        // Separator
        const sep1 = document.createElement('div');
        sep1.className = 'ctx-menu-sep';
        sep1.setAttribute('role', 'separator');
        menu.appendChild(sep1);
        // Purge
        menu.appendChild(_createCtxMenuItem('Purge', () => purgeCommand(instUrl, cmdId, cmdName), true));
    }

    _positionCtxMenu(menu, e.clientX, e.clientY);
    _setupCtxMenuListeners(menu);
}

function showPanelContextMenu(e, panelId) {
    e.preventDefault();
    closeContextMenu();
    const panel = state.panels.find(p => p.id === panelId);
    if (!panel) return;

    const instUrl = panel.selectedInstUrl;
    const cmdId = panel.selectedCmdId;

    const menu = document.createElement('div');
    menu.id = 'ctxMenu';
    menu.className = 'ctx-menu';
    menu.setAttribute('role', 'menu');

    // Copy URL
    menu.appendChild(_createCtxMenuItem('Copy URL', () => {
        if (cmdId) {
            // Find the command name from instance data
            const inst = state.connections.find(i => i.url === instUrl);
            const cmd = inst && inst._commands ? inst._commands.find(c => c.id === cmdId) : null;
            const cmdName = cmd ? (cmd.name || cmd.id) : cmdId;
            copyCommandUrl(instUrl, cmdId, cmdName);
        } else {
            // Just copy the instance URL
            navigator.clipboard.writeText(instUrl).catch(() => {});
        }
    }, false));

    if (cmdId) {
        // Pause/Resume
        menu.appendChild(_createCtxMenuItem('Pause/Resume', () => togglePauseCmd(instUrl, cmdId), false));
        // Restart
        menu.appendChild(_createCtxMenuItem('Restart', () => restartCommandById(instUrl, cmdId), false));
        // Kill
        menu.appendChild(_createCtxMenuItem('Kill', () => killCommand(instUrl, cmdId), true));
    }

    // Rename Panel
    menu.appendChild(_createCtxMenuItem('Rename Panel', () => startRenamePanel(panelId), false));

    // Minimize / Restore Panel
    if (state.panels.length > 1) {
        const isMin = panel.minimized;
        menu.appendChild(_createCtxMenuItem(isMin ? 'Restore Panel' : 'Minimize Panel', () => toggleMinimizePanel(panelId), false));
    }

    // Split / Unsplit
    const sepSplit = document.createElement('div');
    sepSplit.className = 'ctx-menu-sep';
    sepSplit.setAttribute('role', 'separator');
    menu.appendChild(sepSplit);

    if (!panel.split) {
        menu.appendChild(_createCtxMenuItem('Split Horizontal', () => splitPanel(panelId, 'horizontal'), false));
        menu.appendChild(_createCtxMenuItem('Split Vertical', () => splitPanel(panelId, 'vertical'), false));
    } else {
        menu.appendChild(_createCtxMenuItem('Unsplit', () => unsplitPanel(panelId), false));
    }

    // Separator
    const sep = document.createElement('div');
    sep.className = 'ctx-menu-sep';
    sep.setAttribute('role', 'separator');
    menu.appendChild(sep);

    // Remove Panel (only if more than one panel)
    if (state.panels.length > 1) {
        menu.appendChild(_createCtxMenuItem('Remove Panel', () => removePanel(panelId), true));
    }

    _positionCtxMenu(menu, e.clientX, e.clientY);
    _setupCtxMenuListeners(menu);
}

// ─── Panel title rename ───
function startRenamePanel(panelId) {
    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;
    const nameEl = panelEl.querySelector('.cmd-fullname');
    if (!nameEl) return;
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;

    // Already editing? Do nothing
    if (nameEl.getAttribute('contenteditable') === 'true') return;

    const currentText = panelObj.customTitle || '';
    nameEl.contentEditable = 'true';
    nameEl.classList.add('panel-title-editing');
    nameEl.textContent = currentText;
    nameEl.focus();

    // Select all text for easy replacement
    const range = document.createRange();
    range.selectNodeContents(nameEl);
    const sel = window.getSelection();
    sel.removeAllRanges();
    sel.addRange(range);

    // Store original value for cancel
    nameEl._renameOriginal = currentText;

    const onKeydown = (e) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            finishRenamePanel(panelId, true);
        } else if (e.key === 'Escape') {
            e.preventDefault();
            finishRenamePanel(panelId, false);
        }
    };
    const onBlur = () => {
        // Small delay to allow click on context menu "Rename" to not conflict
        setTimeout(() => finishRenamePanel(panelId, true), 100);
    };
    const onInput = () => {
        // Prevent multi-line
        nameEl.textContent = nameEl.textContent.replace(/\n/g, ' ');
    };

    nameEl.addEventListener('keydown', onKeydown);
    nameEl.addEventListener('blur', onBlur);
    nameEl.addEventListener('input', onInput);
    nameEl._renameHandlers = { keydown: onKeydown, blur: onBlur, input: onInput };
}

function finishRenamePanel(panelId, save) {
    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;
    const nameEl = panelEl.querySelector('.cmd-fullname');
    if (!nameEl || nameEl.getAttribute('contenteditable') !== 'true') return;

    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;

    // Remove event listeners
    if (nameEl._renameHandlers) {
        nameEl.removeEventListener('keydown', nameEl._renameHandlers.keydown);
        nameEl.removeEventListener('blur', nameEl._renameHandlers.blur);
        nameEl.removeEventListener('input', nameEl._renameHandlers.input);
        delete nameEl._renameHandlers;
    }

    nameEl.contentEditable = 'false';
    nameEl.classList.remove('panel-title-editing');

    if (save) {
        const newTitle = nameEl.textContent.trim();
        panelObj.customTitle = newTitle;
        if (newTitle) {
            localStorage.setItem('vrw_panel_title_' + panelId, newTitle);
        } else {
            localStorage.removeItem('vrw_panel_title_' + panelId);
        }
    }

    // Refresh display from current command data
    updatePanelCommandInfo();
}

function copyCommandUrl(instUrl, cmdId, cmdName) {
    const base = cmdName.replace(/.*\//, ''); // basename
    const url = instUrl.replace(/^http/, 'http') + '/' + encodeURIComponent(base);
    navigator.clipboard.writeText(url).catch(() => {});
}

async function togglePauseCmd(instUrl, cmdId) {
    // Temporarily set the selected command so togglePauseRun targets the right one
    const prevInstUrl = state.selectedInstUrl;
    const prevCmdId = state.selectedCmdId;
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    await togglePauseRun();
    // Restore previous selection if the panel context menu was for a non-selected panel
    if (prevInstUrl !== instUrl || prevCmdId !== cmdId) {
        state.selectedInstUrl = prevInstUrl;
        state.selectedCmdId = prevCmdId;
    }
}

// ─── Auto-fit Terminal on Window Resize ───
function autoFitActiveTerminal() {
    if (!state.selectedInstUrl || !state.selectedCmdId) return;
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    if (!vttyEl) return;
    const rect = vttyEl.getBoundingClientRect();
    if (rect.width < 10 || rect.height < 10) return; // too small or hidden
    const charW = state.fontSize * 0.6;
    const charH = state.fontSize * 1.2;
    const cols = Math.max(20, Math.min(500, Math.floor(rect.width / charW)));
    const rows = Math.max(5, Math.min(200, Math.floor(rect.height / charH)));
    // Only resize if dimensions actually changed
    if (rows !== state._termRows || cols !== state._termCols) {
        api.resize(state.selectedInstUrl, state.selectedCmdId, { rows, cols }).catch(() => {});
    }
}

// ─── Panel Resize Helper ───
/// Send a resize request for a specific panel's command with exact rows/cols.
/// Returns true on success, false on failure (no command, exited, or network error).
async function _resizePanelTo(panelId, rows, cols) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || !panelObj.selectedCmdId) return false;
    const inst = panelObj.selectedInstUrl ? state.connections.find(i => i.url === panelObj.selectedInstUrl) : null;
    const cmd = inst && inst._commands ? inst._commands.find(c => c.id === panelObj.selectedCmdId) : null;
    if (cmd && cmd.status === 'exited') return false;
    try {
        await api.resize(panelObj.selectedInstUrl, panelObj.selectedCmdId, { rows, cols });
        return true;
    } catch {
        return false;
    }
}

// ─── Max Fit Toggle ───
// Per-panel state: stores the previous rows/cols before max-fit was applied,
// so toggling back restores them.
const _maxFitState = {};  // panelId → { prevRows, prevCols, active }

/// Toggle "max fit" mode: resize the terminal rows/cols to the maximum that
/// fits in the panel container at the current font size.  Toggle back to
/// restore the previous dimensions.
async function toggleMaxFit(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;

    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;
    const vttyEl = panelEl.querySelector('.vtty-container');
    if (!vttyEl) return;

    const st = _maxFitState[panelId];
    const btn = document.getElementById('stMaxFitBtn') || document.getElementById('maxFitBtn-' + panelId);

    if (st && st.active) {
        // Toggle back: restore previous dimensions
        st.active = false;
        if (btn) {
            btn.classList.remove('btn-primary');
            btn.title = 'Auto-fit terminal to panel';
        }
        const ok = await _resizePanelTo(panelId, st.prevRows, st.prevCols);
        if (!ok) {
            delete _maxFitState[panelId];
            if (btn) {
                btn.classList.remove('btn-primary');
                btn.title = 'Auto-fit terminal to panel';
            }
        }
    } else {
        // Apply max fit: calculate max rows/cols from container + current font
        const rect = vttyEl.getBoundingClientRect();
        if (rect.width < 10 || rect.height < 10) return;

        const inst = panelObj.selectedInstUrl ? state.connections.find(i => i.url === panelObj.selectedInstUrl) : null;
        const cmd = inst && inst._commands ? inst._commands.find(c => c.id === panelObj.selectedCmdId) : null;
        if (panelObj.selectedCmdId && cmd && cmd.status === 'exited') {
            return;
        }

        const fontSize = panelObj.fontSize || state.fontSize;
        const charW = fontSize * 0.6;
        const charH = fontSize * 1.2;
        const maxCols = Math.max(20, Math.min(500, Math.floor(rect.width / charW)));
        const maxRows = Math.max(5, Math.min(200, Math.floor(rect.height / charH)));

        const curRows = parseInt(document.getElementById('stResizeRows')?.value || document.getElementById('resizeRows-' + panelId)?.value) || 24;
        const curCols = parseInt(document.getElementById('stResizeCols')?.value || document.getElementById('resizeCols-' + panelId)?.value) || 80;

        _maxFitState[panelId] = { prevRows: curRows, prevCols: curCols, active: true };
        if (btn) {
            btn.classList.add('btn-primary');
            btn.title = 'Restore previous size';
        }
        const ok = await _resizePanelTo(panelId, maxRows, maxCols);
        if (!ok) {
            delete _maxFitState[panelId];
            if (btn) {
                btn.classList.remove('btn-primary');
                btn.title = 'Auto-fit terminal to panel';
            }
        }
    }
}


// ─── Max Font Toggle ───
// Per-panel state: stores the previous font size before max-font was applied,
// so toggling back restores it.
const _maxFontState = {};  // panelId → { prevFontSize, active }

/// Toggle "max font" mode: maximize the font size so the terminal rows/cols
/// still fit in the panel container at the current dimensions.  Toggle back to
/// restore the previous font size.
async function toggleMaxFont(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;

    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;
    const vttyEl = panelEl.querySelector('.vtty-container');
    if (!vttyEl) return;

    const st = _maxFontState[panelId];
    const btn = document.getElementById('stMaxFontBtn') || document.getElementById('maxFontBtn-' + panelId);

    const curRows = parseInt(document.getElementById('stResizeRows')?.value || '24') || 24;
    const curCols = parseInt(document.getElementById('stResizeCols')?.value || '80') || 80;

    if (st && st.active) {
        st.active = false;
        if (btn) {
            btn.classList.remove('btn-primary');
            btn.title = 'Maximize font to fit';
        }
        panelObj.fontSize = st.prevFontSize;
        localStorage.setItem('vrw_panel_font_' + panelId, panelObj.fontSize.toString());
        if (vttyEl) vttyEl.style.fontSize = panelObj.fontSize + 'px';
        delete _maxFontState[panelId];
    } else {
        const rect = vttyEl.getBoundingClientRect();
        if (rect.width < 10 || rect.height < 10) return;

        const maxFontW = Math.floor(rect.width / (curCols * 0.6));
        const maxFontH = Math.floor(rect.height / (curRows * 1.2));
        const maxFont = Math.max(8, Math.min(28, Math.min(maxFontW, maxFontH)));

        _maxFontState[panelId] = { prevFontSize: panelObj.fontSize, active: true };
        if (btn) {
            btn.classList.add('btn-primary');
            btn.title = 'Restore previous font size';
        }
        panelObj.fontSize = maxFont;
        localStorage.setItem('vrw_panel_font_' + panelId, panelObj.fontSize.toString());
        if (vttyEl) vttyEl.style.fontSize = panelObj.fontSize + 'px';
    }
}


// ─── Drag-and-Drop Panel Reorder ───
let _draggedPanelId = null;

function onPanelDragStart(e, panelId) {
    _draggedPanelId = panelId;
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', panelId);
    setTimeout(() => {
        const el = document.getElementById(panelId);
        if (el) el.classList.add('dragging');
    }, 0);
}

function onPanelDragOver(e) {
    e.preventDefault();
    // Sidebar command drops use effectAllowed='copy'; panel reorders use 'move'.
    // _draggedPanelId is set only for panel-to-panel drags.
    e.dataTransfer.dropEffect = _draggedPanelId ? 'move' : 'copy';
    const panel = e.target.closest('.panel');
    if (!panel || panel.id === _draggedPanelId) return;
    const rect = panel.getBoundingClientRect();
    const midX = rect.left + rect.width / 2;
    panel.classList.remove('drag-over-left', 'drag-over-right');
    if (e.clientX < midX) {
        panel.classList.add('drag-over-left');
    } else {
        panel.classList.add('drag-over-right');
    }
}

function onPanelDragLeave(e) {
    const panel = e.target.closest('.panel');
    if (panel) panel.classList.remove('drag-over-left', 'drag-over-right');
}

function onPanelDrop(e, targetPanelId) {
    e.preventDefault();
    if (e.stopPropagation) e.stopPropagation();

    // ── Command drop from sidebar (application/x-cmd data) ──
    // Dragging a command from the sidebar onto the panel area always creates
    // a NEW panel showing that command's vTTY.  This allows the same command
    // to be viewed in multiple panels simultaneously.
    // Panel reorders use _draggedPanelId; command drops use dataTransfer.
    if (!_draggedPanelId) {
        try {
            const cmdData = JSON.parse(e.dataTransfer.getData('application/x-cmd'));
            if (cmdData && cmdData.cmdId) {
                document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left', 'drag-over-right'));
                _openCommandInNewPane(cmdData.instUrl, cmdData.cmdId, cmdData.cmdName);
                return;
            }
        } catch (err) { /* ignore invalid drops */ }
        onPanelDragEnd(e);
        return;
    }

    // ── Panel reorder drop ──
    if (_draggedPanelId === targetPanelId) {
        onPanelDragEnd(e);
        return;
    }
    const container = document.getElementById('view-vtty');
    const draggedEl = document.getElementById(_draggedPanelId);
    const targetEl = document.getElementById(targetPanelId);
    if (!draggedEl || !targetEl || !container) {
        onPanelDragEnd(e);
        return;
    }
    // Determine insert position
    const rect = targetEl.getBoundingClientRect();
    const midX = rect.left + rect.width / 2;
    if (e.clientX < midX) {
        container.insertBefore(draggedEl, targetEl);
    } else {
        container.insertBefore(draggedEl, targetEl.nextSibling);
    }
    // Also remove the resize handle and re-add it after the panel
    const handle = draggedEl.nextElementSibling;
    if (handle && handle.classList.contains('panel-resize-handle')) {
        container.removeChild(handle);
        const nextEl = draggedEl.nextElementSibling;
        container.insertBefore(handle, nextEl);
    }
    // Update state.panels order to match DOM
    const panelEls = container.querySelectorAll('.panel');
    const newOrder = [];
    panelEls.forEach(el => {
        const p = state.panels.find(pp => pp.id === el.id);
        if (p) newOrder.push(p);
    });
    state.panels = newOrder;
    localStorage.setItem('vrw_panel_order', JSON.stringify(newOrder.map(p => p.id)));
    onPanelDragEnd(e);
}

// ─── Drop on empty panel area (no existing panels) ───
function onPanelAreaDragOver(e) {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'copy';
}

function onPanelAreaDrop(e) {
    e.preventDefault();
    try {
        const cmdData = JSON.parse(e.dataTransfer.getData('application/x-cmd'));
        if (cmdData && cmdData.cmdId) {
            _openCommandInNewPane(cmdData.instUrl, cmdData.cmdId, cmdData.cmdName);
        }
    } catch (err) { /* not a command drop */ }
}

function onPanelDragEnd(e) {
    _draggedPanelId = null;
    document.querySelectorAll('.panel').forEach(p => {
        p.classList.remove('dragging', 'drag-over-left', 'drag-over-right');
    });
}


    // Expose all public functions to global scope
    /// Close a panel entirely — always removes it.
function closePanelContent(panelId) {
    removePanel(panelId);
}

    window.addPanelDirect = addPanelDirect;
    window.addPanel = addPanel;
    window.closePanelModal = closePanelModal;
    window.confirmAddPanel = confirmAddPanel;
    window.removePanel = removePanel;
    window.closePanelContent = closePanelContent;
    window.toggleMinimizePanel = toggleMinimizePanel;
    window.splitPanel = splitPanel;
    window.unsplitPanel = unsplitPanel;
    window.renderPanels = renderPanels;
    window.focusPanel = focusPanel;
    window.updateSharedToolbar = updateSharedToolbar;
    window.sendKeysToPanel = sendKeysToPanel;
    window.showSpecialKeysHelp = showSpecialKeysHelp;
    window.closeSpecialKeysModal = function() {
        releaseCurrentFocusTrap();
        const modal = document.getElementById('specialKeysModal');
        if (modal) modal.remove();
    };
    window.togglePanelLayout = togglePanelLayout;
    window.toggleLayoutPresetMenu = toggleLayoutPresetMenu;
    window.applyLayoutPreset = applyLayoutPreset;
    window.copyTerminalSelection = copyTerminalSelection;
    window.exportTerminal = exportTerminal;
    window.screenshotPanel = screenshotPanel;
    window.closeContextMenu = closeContextMenu;
    window.showCmdContextMenu = showCmdContextMenu;
    window.showPanelContextMenu = showPanelContextMenu;
    window.startRenamePanel = startRenamePanel;
    window.finishRenamePanel = finishRenamePanel;
    window.copyCommandUrl = copyCommandUrl;
    window.togglePauseCmd = togglePauseCmd;
    window.autoFitActiveTerminal = autoFitActiveTerminal;
    window.toggleMaxFit = toggleMaxFit;
    window.toggleMaxFont = toggleMaxFont;
    window.onPanelDragStart = onPanelDragStart;
    window.onPanelDragOver = onPanelDragOver;
    window.onPanelDragLeave = onPanelDragLeave;
    window.onPanelDrop = onPanelDrop;
    window.onPanelDragEnd = onPanelDragEnd;
    window.onPanelAreaDragOver = onPanelAreaDragOver;
    window.onPanelAreaDrop = onPanelAreaDrop;
    window._renderVttyContainer = _renderVttyContainer;
    window._getServerLabel = _getServerLabel;
    window._getServerColor = _getServerColor;
    window._getServerTextColor = _getServerTextColor;
    window._getPanelCmdLabel = _getPanelCmdLabel;
    window._updateSplitHeaders = _updateSplitHeaders;
    window._renderSplitContainer = _renderSplitContainer;
    window._updateSplitPanelHeader = _updateSplitPanelHeader;
    window._renderMinimizedPanels = _renderMinimizedPanels;
    window._applyPanelLayoutClass = _applyPanelLayoutClass;
    window._updatePanelMultiUI = _updatePanelMultiUI;
})();

