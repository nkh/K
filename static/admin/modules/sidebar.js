// ─── Sidebar ───
(function() {
    'use strict';
// ─── Sidebar ───
function toggleSidebar() {
    const sidebar = document.getElementById('sidebar');
    sidebar.classList.toggle('collapsed');
    // Clear inline width set by the drag handle so the CSS class takes effect.
    // Without this, an inline style.width from dragging overrides .collapsed { width: 0 }.
    if (sidebar.classList.contains('collapsed')) {
        sidebar.style.width = '';
    }
}

// ─── Resource toggle ───
function toggleResources() {
    state.showResources = !state.showResources;
    localStorage.setItem('vrw_show_resources', state.showResources.toString());
    document.querySelectorAll('.resource-badge, .instance-url').forEach(el => {
        el.classList.toggle('hidden', !state.showResources);
    });
    // Also toggle shared toolbar elements
    const stBadge = document.getElementById('stResourceBadge');
    if (stBadge && !state.showResources) stBadge.classList.add('hidden');
    const stUrl = document.getElementById('stInstanceUrl');
    if (stUrl) stUrl.classList.toggle('hidden', !state.showResources);
    // If toggling on, refresh the badge
    if (state.showResources) updateSharedToolbar();
}

// ─── Bottom bar toggle ───
function toggleBottombar() {
    const bar = document.getElementById('bottomBar');
    const btn = document.getElementById('statusBtn');
    bar.classList.toggle('hidden');
    const isHidden = bar.classList.contains('hidden');
    if (btn) {
        btn.style.background = isHidden ? '' : 'var(--accent)';
        btn.style.color = isHidden ? '' : '#fff';
    }
    localStorage.setItem('vrw_bottombar_hidden', isHidden ? 'true' : 'false');
}

function initBottombar() {
    const shouldHide = localStorage.getItem('vrw_bottombar_hidden') !== 'false'; // hidden by default
    const bar = document.getElementById('bottomBar');
    const btn = document.getElementById('statusBtn');
    if (shouldHide) {
        bar.classList.add('hidden');
    } else {
        bar.classList.remove('hidden');
        if (btn) { btn.style.background = 'var(--accent)'; btn.style.color = '#fff'; }
    }
}

// ─── Logs view toggle ───
function toggleLogsView() {
    const btn = document.getElementById('logsBtn');
    const vtty = document.getElementById('view-vtty');
    const log = document.getElementById('view-log');
    const prevView = state.currentView;
    if (state.currentView === 'log') {
        // Switch back to terminal
        state.currentView = 'vtty';
        vtty.classList.remove('hidden');
        log.classList.add('hidden');
        if (btn) { btn.style.background = ''; btn.style.color = ''; }
        disconnectLogWs();
    } else {
        // Switch to logs
        state.currentView = 'log';
        vtty.classList.add('hidden');
        log.classList.remove('hidden');
        if (btn) { btn.style.background = 'var(--accent)'; btn.style.color = '#fff'; }
        loadLog();
        if (!document.getElementById('logSearch').value) {
            connectLogWs();
        }
    }
}

function switchSidebarTab(tab, el) {
    document.querySelectorAll('.sidebar-tab').forEach(t => t.classList.remove('active'));
    el.classList.add('active');
    document.getElementById('tab-servers').classList.toggle('hidden', tab !== 'servers');
    document.getElementById('tab-spawn').classList.toggle('hidden', tab !== 'spawn');
    document.getElementById('tab-templates').classList.toggle('hidden', tab !== 'templates');
    document.getElementById('tab-certs').classList.toggle('hidden', tab !== 'certs');
    document.getElementById('tab-groups').classList.toggle('hidden', tab !== 'groups');
    if (tab === 'templates') renderTemplates();
    if (tab === 'groups') renderGroups();
}

// Update sidebar tab visibility based on server reachability.
// When no vrw instance is reachable, hide the Spawn tab.
function updateSidebarTabsVisibility() {
    const spawnTab = document.querySelector('.sidebar-tab:nth-child(2)');
    const spawnContent = document.getElementById('tab-spawn');
    const anyReachable = state.connections.some(i => i.reachable === true);
    if (anyReachable) {
        if (spawnTab) spawnTab.classList.remove('hidden');
        // Only show spawn content if the spawn tab is currently active;
        // otherwise let switchSidebarTab() manage content visibility.
        if (spawnContent && spawnTab && spawnTab.classList.contains('active')) {
            spawnContent.classList.remove('hidden');
        }
    } else {
        if (spawnTab) spawnTab.classList.add('hidden');
        if (spawnContent) spawnContent.classList.add('hidden');
        // If spawn tab was active, switch to commands
        const activeTab = document.querySelector('.sidebar-tab.active');
        if (activeTab && activeTab === spawnTab) {
            const cmdsTab = document.querySelector('.sidebar-tab:first-child');
            if (cmdsTab) switchSidebarTab('commands', cmdsTab);
        }
    }
}

/// Show/hide the command toolbar (filter + kill all) based on whether
/// there is a reachable server with commands.  Hidden when no server is
/// reachable or when there are zero commands across all instances.
function updateCmdToolbarVisibility() {
    const killAllBtn = document.getElementById('killAllBtn');
    if (!killAllBtn) return;
    const anyReachable = state.connections.some(i => i.reachable === true);
    const anyCommands = state.connections.some(
        i => i._commands && i._commands.length > 0
    );
    killAllBtn.classList.toggle('hidden', !(anyReachable && anyCommands));
    const freezeAllBtn = document.getElementById('freezeAllBtn');
    if (freezeAllBtn) freezeAllBtn.classList.toggle('hidden', !(anyReachable && anyCommands));
}

/// Extract sidebar-building logic into a reusable function so both
/// loadSnapshot() and loadCommands() can use it.
function _buildSidebar() {
    const filter = (document.getElementById('cmdFilter') || {}).value || '';
    const filterLower = filter.toLowerCase();

    // Initialize sidebar sort to 'name' (All) if not yet set
    if (!_sidebarSort) {
        _sidebarSort = 'name';
    }

    // Handle pending command selection (e.g. from workspace restore)
    if (state._pendingSelectId) {
        const pendingId = state._pendingSelectId;
        state._pendingSelectId = null;
        for (const inst of state.connections) {
            if (inst._commands && inst._commands.find(c => c.id === pendingId)) {
                const cmd = inst._commands.find(c => c.id === pendingId);
                selectCommand(inst.url, cmd.id, cmd.name || cmd.id);
                return;
            }
        }
    }

    const container = document.getElementById('commandList');
    let html = '';

    // Server connections bar: show reach indicator + label + close button for
    // each non-origin server.  Always visible when multiple connections exist.
    const originUrl = window.location.origin;
    const nonOriginConns = state.connections.filter(c => c.url !== originUrl);
    if (nonOriginConns.length > 0) {
        html += '<div class="server-connections-bar">';
        for (const inst of nonOriginConns) {
            const reachClass = inst.reachable === true ? 'reachable' : inst.reachable === false ? 'unreachable' : 'unknown';
            html += `<div class="server-conn-item" title="${escHtml(inst.url)} — ${inst.reachable === true ? 'connected' : inst.reachable === false ? 'unreachable' : 'checking...'}">`;
            html += `<span class="server-reach-dot ${reachClass}"></span>`;
            html += `<span class="server-conn-label">${escHtml(inst.label)}</span>`;
            html += `<button class="server-conn-close-btn" data-action="DisconnectServer" data-inst-url="${escHtml(inst.url)}" title="Disconnect from ${escHtml(inst.label)}">&#x2715;</button>`;
            html += '</div>';
        }
        html += '</div>';
    }

    if (state.connections.length > 1) {
        html += '<div class="sidebar-sort-bar">';
        html += `<span class="sidebar-sort-item${_sidebarSort === 'name' ? ' active' : ''}" data-action="SortSidebarBy" data-value="name">All</span>`;
        for (const inst of state.connections) {
            const active = _sidebarSort === inst.url ? ' active' : '';
            html += `<span class="sidebar-sort-item${active}" data-action="SortSidebarBy" data-value="${escHtml(inst.url)}">${escHtml(inst.label)}</span>`;
        }
        html += '</div>';
    }

    let allCmds = [];
    for (const inst of state.connections) {
        for (const cmd of (inst._commands || [])) {
            const cmdName = cmd.name || cmd.id;
            if (filterLower && !cmdName.toLowerCase().includes(filterLower) &&
                !(cmd.args || []).join(' ').toLowerCase().includes(filterLower) &&
                !String(cmd.pid).includes(filterLower)) continue;
            allCmds.push({ inst, cmd, cmdName });
        }
    }

    // Build the navigation list for prev/next: commands from the active
    // panel's server only, sorted by spawn_order (chronological).
    // Falls back to all commands in spawn order if no panel has a server.
    const activePanelId = getActivePanelId();
    const activePanel = activePanelId ? state.panels.find(p => p.id === activePanelId) : null;
    const navInstUrl = activePanel && activePanel.selectedInstUrl ? activePanel.selectedInstUrl : null;
    const navCmds = navInstUrl
        ? allCmds.filter(c => c.inst.url === navInstUrl)
        : allCmds;
    navCmds.sort((a, b) => (a.cmd.spawn_order ?? 0) - (b.cmd.spawn_order ?? 0));
    _navCommands = navCmds.map(({ inst, cmd, cmdName }) => ({
        instUrl: inst.url,
        cmdId: cmd.id,
        name: cmdName,
    }));

    if (_sidebarSort === 'name') {
        // Sidebar "All" view: alphabetical by name
        // When multiple servers exist, show a server badge to distinguish
        // same-named commands on different servers.
        const multiServer = state.connections.length > 1;
        allCmds.sort((a, b) => a.cmdName.localeCompare(b.cmdName));
        html += renderCmdList(allCmds, multiServer);
    } else {
        const targetUrl = _sidebarSort;
        const grouped = targetUrl === 'all' ? null : targetUrl;
        for (const inst of state.connections) {
            if (grouped && inst.url !== grouped) continue;
            const instCmds = allCmds.filter(c => c.inst.url === inst.url);
            if (instCmds.length === 0 && grouped) continue;
            if (inst._lastError && (!grouped || inst.url === grouped)) {
                html += `<div style="padding:0.5rem;color:var(--red);font-size:0.7rem;">${escHtml(inst.label)}: ${escHtml(inst._lastError)}</div>`;
                continue;
            }
            if (state.connections.length > 1) {
                html += `<div class="pinned-section-header">${escHtml(inst.label)}<button class="server-close-btn" data-action="DisconnectServer" data-inst-url="${escHtml(inst.url)}" title="Disconnect this server">&#x2715;</button></div>`;
            }
            if (instCmds.length === 0) {
                html += `<div style="padding:0.3rem 0.4rem;color:var(--text-muted);font-size:0.7rem;">No commands</div>`;
                continue;
            }
            // Apply custom reorder if set for this instance
            const orderedCmds = getOrderedCmds(inst.url, instCmds);
            orderedCmds.sort((a, b) => a.cmdName.localeCompare(b.cmdName));
            html += renderCmdList(orderedCmds);
        }
    }

    function renderCmdList(cmds, showServerBadge) {
        let out = '';
        for (const { inst, cmd, cmdName } of cmds) {
            const cert = cmd.certificate || '';
            const certBadge = cert
                ? `<span class="cert-badge" title="Bound to: ${escHtml(cert)}">${escHtml(cert)}</span>`
                : '';
            const selected = (state.selectedInstUrl === inst.url && state.selectedCmdId === cmd.id) ? ' selected' : '';
            const isAlive = cmd.alive !== false;
            const isFrozen = cmd.frozen === true;
            const runtimeStr = (isAlive || isFrozen) && cmd.runtime_secs > 0
                ? formatRuntime(cmd.runtime_secs)
                : '';
            const frozenBadge = isFrozen ? 'PAUSED ' : '';
            const exitBadge = (cmd.exit_code != null)
                ? `<span class="exit-badge ${cmd.exit_code === 0 ? 'success' : 'failure'}">exit ${cmd.exit_code}</span>`
                : '';
            const res = state._resourceCache[cmd.id];
            const resourceStr = (res && (res.cpu_percent != null || res.memory_mb != null))
                ? `${res.cpu_percent != null ? res.cpu_percent.toFixed(1) + '%' : ''}${res.cpu_percent != null && res.memory_mb != null ? ' ' : ''}${res.memory_mb != null ? res.memory_mb.toFixed(1) + 'MB' : ''}`
                : '';
            const pinnedNames = getPinnedNames();
            const isPinned = pinnedNames.includes(cmdName);
            const frozenClass = isFrozen ? ' frozen' : '';
            const exitedClass = (!isAlive && !isFrozen) ? ' exited' : '';
            const instUnreachable = inst.reachable === false;
            const dimStyle = instUnreachable ? 'opacity:0.4;' : ((isAlive || isFrozen) ? '' : 'opacity:0.6;');
            const killDisabled = ''; // Kill buttons always active — let the server-side API return an error if the kill can't succeed
            const retainOnExit = cmd.exit && cmd.exit.retain_on_exit === true;
            const keepTitle = retainOnExit ? 'Unkeep (terminal will be removed on exit)' : 'Keep (retain terminal after exit)';
            const keepBtnHtml = isAlive
                ? `<button class="keep-btn${retainOnExit ? ' active' : ''}" data-action="ToggleKeepCmd" data-inst-url="${escHtml(inst.url)}" data-cmd-id="${escHtml(cmd.id)}" title="${keepTitle}">${retainOnExit ? '&#9733;' : '&#9734;'}</button>`
                : (retainOnExit
                    ? `<span class="keep-badge" title="Terminal kept after exit">&#9733;</span>`
                    : '');
            // Freeze/thaw button for alive commands
            const freezeBtnHtml = isAlive
                ? `<button class="cmd-freeze-btn${isFrozen ? ' active' : ''}" data-action="TogglePauseRunByIdx" data-inst-url="${escHtml(inst.url)}" data-cmd-id="${escHtml(cmd.id)}" title="${isFrozen ? 'Thaw' : 'Freeze'}">${isFrozen ? '&#9654;' : '&#9208;'}</button>`
                : '';
            // Build detail parts as separate spans for the detail row
            // Compact: runtime · cpu% · memM · pid  (numeric only, no labels)
            const detailParts = [];
            if (runtimeStr) detailParts.push(escHtml(runtimeStr));
            if (frozenBadge) detailParts.push(escHtml(frozenBadge.trim()));
            if (res && res.cpu_percent != null) detailParts.push(res.cpu_percent.toFixed(1) + '%');
            if (res && res.memory_mb != null) {
                const mb = res.memory_mb;
                detailParts.push(mb >= 1024 ? (mb / 1024).toFixed(1) + 'G' : mb.toFixed(1) + 'M');
            }
            if (cmd.pid) detailParts.push(escHtml(String(cmd.pid)));
            // NOTE: cmd.pid available here because renderCmdList receives {cmd,...} objects
            const unreachableTitle = instUnreachable ? ` [disconnected]` : '';
            // In "All" view with multiple servers, show a server badge to
            // distinguish same-named commands on different servers.
            const serverBadge = showServerBadge
                ? `<span class="resource-badge" style="font-size:0.55rem;opacity:0.7;" title="${escHtml(inst.url)}">${escHtml(inst.label)}</span>`
                : '';
            out += `
                <div class="cmd-item${selected}${frozenClass}${exitedClass}${instUnreachable ? ' unreachable' : ''}" data-action="SelectCommand" data-inst-url="${escHtml(inst.url)}" data-cmd-id="${escHtml(cmd.id)}" data-cmd-name="${escHtml(cmdName)}" data-cmd-alive="${isAlive}" data-cmd-frozen="${isFrozen}" data-cmd-retained="${retainOnExit}" tabindex="0" role="button" aria-label="Command ${escHtml(cmdName)}" draggable="true" ondragstart="onCmdDragStart(event,this.dataset.instUrl,this.dataset.cmdId,this.dataset.cmdName)" oncontextmenu="showCmdContextMenu(event,this.dataset.instUrl,this.dataset.cmdId,this.dataset.cmdName,this.dataset.cmdAlive==='true',this.dataset.cmdRetained==='true')" title="${escHtml(inst.label)} / ${escHtml(cmdName)}${unreachableTitle}" style="${dimStyle}">
                    <div class="cmd-item-row">
                        <button class="btn btn-xs btn-danger cmd-kill-btn" data-inst-url="${escHtml(inst.url)}" data-cmd-id="${escHtml(cmd.id)}" data-cmd-retained="${retainOnExit}" data-cmd-alive="${isAlive}"${killDisabled}>&#x2715;</button>
                        ${keepBtnHtml}
                        <button class="pin-btn${isPinned ? ' active' : ''}" data-action="TogglePinCmd" data-cmd-name="${escHtml(cmdName)}" title="${isPinned ? 'Unpin' : 'Pin'}">${isPinned ? '◉' : '◎'}</button>
                        <span class="cmd-grab-handle" onmousedown="_cmdReorderMouseDown(event,'${escHtml(inst.url)}','${escHtml(cmd.id)}','${escHtml(cmdName)}')" title="Drag to reorder / drop on pane to open">&#x2807;</span>
                        <span class="name">${escHtml(cmdName)}</span>
                        <span class="cmd-detail-inline">${detailParts.map(p => escHtml(p)).join(' · ')}</span>
                        ${serverBadge}
                        ${certBadge}
                        ${exitBadge}
                        ${freezeBtnHtml}
                    </div>
                    ${detailParts.length > 0 ? `<div class="cmd-detail-row">${detailParts.join(' · ')}</div>` : ''}
                </div>`;
        }
        return out;
    }

    rearrangePinnedCommands(container);
    container.innerHTML = html || '<div style="padding:1rem;color:var(--text-muted);text-align:center;">No running commands</div>';
    updateInstanceDropdown();
    updateCmdToolbarVisibility();
    // initPanelDropTargets() intentionally NOT called — panel inline handlers
    // (onPanelDragOver/onPanelDrop) already handle command drag-and-drop from sidebar.

    if (state._pendingSelectId) {
        const pendingId = state._pendingSelectId;
        state._pendingSelectId = null;
        for (const inst of state.connections) {
            if (inst._commands && inst._commands.find(c => c.id === pendingId)) {
                const cmd = inst._commands.find(c => c.id === pendingId);
                selectCommand(inst.url, cmd.id, cmd.name || cmd.id);
                return;
            }
        }
    }

    if (!state.selectedCmdId) {
        // Auto-select only from the focused panel's server (if set).
        // This prevents assigning a command from server X to a panel
        // that was just created for server Y (which has no commands yet).
        // IMPORTANT: We do NOT fallback to commands from other servers.
        // If the focused panel's server has no commands, the panel stays empty.
        const focusedPanel = state.panels.find(p => p.id === state._focusedPanelId);
        const targetInstUrl = (focusedPanel && focusedPanel.selectedInstUrl) || state.selectedInstUrl;
        if (targetInstUrl) {
            const inst = state.connections.find(i => i.url === targetInstUrl);
            if (inst && inst._commands && inst._commands.length > 0) {
                const cmd = inst._commands[0];
                selectCommand(inst.url, cmd.id, cmd.name || cmd.id);
                return;
            }
        }
    }

    if (state.selectedInstUrl && state.selectedCmdId) {
        updatePanelCommandInfo();
        if (state.updateMode === 'poll' || state.bufferView !== 'current') {
            scheduleVttyHttpForPanel(state._focusedPanelId, state.selectedInstUrl, state.selectedCmdId, 500);
        }
    }
}


// ─── Command Pinning / Favorites ───
function getPinnedNames() {
    try {
        return JSON.parse(localStorage.getItem('vrw_pinned_cmds') || '[]');
    } catch { return []; }
}

function setPinnedNames(names) {
    localStorage.setItem('vrw_pinned_cmds', JSON.stringify(names));
}

function togglePinCmd(cmdName) {
    const pinned = getPinnedNames();
    const idx = pinned.indexOf(cmdName);
    if (idx >= 0) {
        pinned.splice(idx, 1);
    } else {
        pinned.push(cmdName);
    }
    setPinnedNames(pinned);
    loadCommands();
}

function rearrangePinnedCommands(container) {
    // This is called before innerHTML is set, so we work with the container
    // after it's rendered. The actual DOM rearrangement happens after container.innerHTML is set.
    // We use a MutationObserver-like approach: after innerHTML, rearrange.
    setTimeout(() => {
        if (!container) return;
        const items = container.querySelectorAll('.cmd-item[data-cmd-name]');
        const pinned = getPinnedNames();
        const pinnedItems = [];
        const unpinnedItems = [];
        items.forEach(item => {
            const name = item.dataset.cmdName;
            if (pinned.includes(name)) {
                pinnedItems.push(item);
            } else {
                unpinnedItems.push(item);
            }
        });
        if (pinnedItems.length > 0 && unpinnedItems.length > 0) {
            // Create pinned section header
            const header = document.createElement('div');
            header.className = 'pinned-section-header';
            header.textContent = '◉ Pinned';
            // Insert pinned items first
            const parent = items[0] && items[0].parentNode;
            if (parent) {
                const first = parent.firstChild;
                // Remove all items, then re-add in pinned-first order
                items.forEach(item => item.remove());
                if (first) {
                    parent.insertBefore(header, first);
                    pinnedItems.forEach(item => parent.insertBefore(item, first));
                }
                unpinnedItems.forEach(item => parent.appendChild(item));
            }
        }
        // Update pin button icons
        container.querySelectorAll('.pin-btn').forEach(btn => {
            const item = btn.closest('.cmd-item');
            if (item && pinned.includes(item.dataset.cmdName)) {
                btn.classList.add('active');
                btn.textContent = '◉';
                btn.title = 'Unpin';
            } else {
                btn.classList.remove('active');
                btn.textContent = '◎';
                btn.title = 'Pin';
            }
        });
    }, 0);
}



// ─── Disconnected UI ───
function updateDisconnectedUI() {
    updateSidebarBanner();
    updateSidebarTabsVisibility();
    updateTerminalDisconnectedOverlay();
    updateCmdToolbarVisibility();
}

function updateSidebarBanner() {
    let banner = document.getElementById('disconnectedBanner');
    const unreachable = state.connections.filter(i => i.reachable === false);
    if (unreachable.length > 0) {
        if (!banner) {
            banner = document.createElement('div');
            banner.id = 'disconnectedBanner';
            banner.className = 'disconnected-banner';
            const content = document.getElementById('sidebarContent');
            content.insertBefore(banner, content.firstChild);
        }
        const labels = unreachable.map(i => i.label).join(', ');
        banner.innerHTML = '<span class="disconnected-icon">&#9888;</span> Server disconnected: ' +
            escHtml(labels) + ' &mdash; output may be stale';
    } else {
        if (banner) banner.remove();
    }
}

function updateTerminalDisconnectedOverlay() {
    for (const panelObj of state.panels) {
        const panelEl = document.getElementById(panelObj.id);
        if (!panelEl) continue;
        let overlay = panelEl.querySelector('.disconnected-overlay');
        const inst = panelObj.selectedInstUrl ? state.connections.find(i => i.url === panelObj.selectedInstUrl) : null;
        if (inst && inst.reachable === false) {
            if (!overlay) {
                overlay = document.createElement('div');
                overlay.className = 'disconnected-overlay';
                overlay.innerHTML = '<span>&#9888; Server unreachable &mdash; output is stale</span>';
                const vttyEl = panelEl.querySelector('.vtty-container');
                if (vttyEl) vttyEl.appendChild(overlay);
            }
        } else {
            if (overlay) overlay.remove();
        }
    }
}

    window.initBottombar = initBottombar;
    window.toggleSidebar = toggleSidebar;
    window.toggleBottombar = toggleBottombar;
    window.toggleLogsView = toggleLogsView;
    window.toggleResources = toggleResources;
    window.switchSidebarTab = switchSidebarTab;
    window.updateSidebarTabsVisibility = updateSidebarTabsVisibility;
    window.updateCmdToolbarVisibility = updateCmdToolbarVisibility;
    window.updateDisconnectedUI = updateDisconnectedUI;
    window.updateSidebarBanner = updateSidebarBanner;
    window.updateTerminalDisconnectedOverlay = updateTerminalDisconnectedOverlay;
    window._buildSidebar = _buildSidebar;
    window.getPinnedNames = getPinnedNames;
    window.setPinnedNames = setPinnedNames;
    window.togglePinCmd = togglePinCmd;
    window.rearrangePinnedCommands = rearrangePinnedCommands;
    window._sortSidebarBy = function(sortKey) {
        _sidebarSort = sortKey;
        if (sortKey !== 'name') window._userSpawnInstUrl = sortKey;
        loadCommands();
    };
    window.togglePauseRunPanelByIdx = function(instUrl, cmdId) {
        // Like togglePauseRunPanel but without touching focus/selection state
        const inst = state.connections.find(i => i.url === instUrl);
        if (!inst || !inst._commands) return;
        const cmd = inst._commands.find(c => c.id === cmdId);
        const isFrozen = cmd && cmd.frozen;
        (isFrozen ? api.thaw(instUrl, cmdId) : api.freeze(instUrl, cmdId))
            .then(() => loadCommands()).catch(() => {});
    };
// ─── Documentation ───
function showDocs() {
    const btn = document.getElementById('docsBtn');
    const vtty = document.getElementById('view-vtty');
    const log = document.getElementById('view-log');
    const docs = document.getElementById('view-docs');
    if (state.currentView === 'docs') {
        // Switch back to terminal
        state.currentView = 'vtty';
        vtty.classList.remove('hidden');
        docs.classList.add('hidden');
        if (btn) { btn.style.background = ''; btn.style.color = ''; }
    } else {
        // Disconnect log WS if active
        if (state.currentView === 'log') {
            disconnectLogWs();
            if (log) log.classList.add('hidden');
        }
        state.currentView = 'docs';
        vtty.classList.add('hidden');
        docs.classList.remove('hidden');
        if (btn) { btn.style.background = 'var(--accent)'; btn.style.color = '#fff'; }
        loadDocs();
    }
}

async function loadDocs() {
    const container = document.getElementById('view-docs');
    container.innerHTML = '<div style="padding:2rem;text-align:center;color:var(--text-muted);">Loading documentation...</div>';

    // Try fetching docs from the server, fall back to embedded docs
    try {
        const text = await api.getDocs();
        container.innerHTML = renderMarkdown(text);
        return;
    } catch (e) { /* fall through */ }

    // Embedded documentation
    container.innerHTML = renderEmbeddedDocs();
}

function renderMarkdown(md) {
    // Simple markdown to HTML (no external lib)
    let html = md
        .replace(/^### (.+)$/gm, '<h3>$1</h3>')
        .replace(/^## (.+)$/gm, '<h2>$1</h2>')
        .replace(/^# (.+)$/gm, '<h1>$1</h1>')
        .replace(/```(\w*)\n([\s\S]*?)```/g, '<pre><code>$2</code></pre>')
        .replace(/`([^`]+)`/g, '<code>$1</code>')
        .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
        .replace(/\[(.+?)\]\((.+?)\)/g, '<a href="$2" target="_blank" style="color:var(--accent);">$1</a>')
        .replace(/^\- (.+)$/gm, '<li>$1</li>')
        .replace(/^(\d+)\. (.+)$/gm, '<li>$2</li>')
        .replace(/\n\n/g, '</p><p>')
        .replace(/\n/g, '<br>');
    return '<p>' + html + '</p>';
}

function renderEmbeddedDocs() {
    return `
<h1>vrw Administration</h1>

<h2>Overview</h2>
<p>vrw is a virtual terminal runner with a web control plane. It manages terminal applications, exposing their output through a web interface and REST API. This admin panel provides real-time monitoring and control of all running commands.</p>

<h2>Getting Started</h2>
<p>The admin panel connects to one or more vrw instances. Each instance manages its own set of terminal commands. Use the <strong>+ Panel</strong> button in the top bar to add connections to additional vrw instances.</p>

<h3>Connecting to an Instance</h3>
<p>By default, the admin panel connects to the vrw instance serving it. To add more instances:</p>
<ol>
    <li>Click <strong>+ Panel</strong> in the top bar</li>
    <li>Enter the instance URL (e.g., <code>http://localhost:9090</code>)</li>
    <li>Optionally set a label and auth token</li>
    <li>Click <strong>Add Panel</strong></li>
</ol>
<p>You can also use URL arguments: <code>?instance=http://host:8080&label=Prod&instance=http://host:9090&label=Dev</code></p>

<h2>URL Arguments for Multi-Instance</h2>
<p>The admin page accepts query parameters to pre-configure multi-panel views:</p>
<table>
    <tr><th>Parameter</th><th>Description</th><th>Example</th></tr>
    <tr><td><code>instance</code></td><td>vrw instance URL (repeatable)</td><td><code>?instance=http://host:8080</code></td></tr>
    <tr><td><code>label</code></td><td>Panel label (matches instance order)</td><td><code>&label=Production</code></td></tr>
    <tr><td><code>token</code></td><td>Auth token for instance (matches order)</td><td><code>&token=abc123</code></td></tr>
</table>
<p><strong>Full example:</strong> <code>/admin?instance=http://prod:8080&label=Production&instance=http://dev:9090&label=Development</code></p>

<h2>Managing Commands</h2>

<h3>Viewing Terminal Output</h3>
<p>Click on a command in the sidebar to view its real-time ANSI-rendered terminal output. The terminal emulator supports:</p>
<ul>
    <li>Full ANSI color rendering (16, 256, and 24-bit truecolor)</li>
    <li>Cursor position indicator (blue highlight)</li>
    <li>Text attributes: bold, italic, underline, strikethrough</li>
    <li>Scrollback buffer navigation via scrollbar</li>
</ul>

<h3>Spawning Commands</h3>
<p>Switch to the <strong>Spawn</strong> tab in the sidebar to create new commands. Specify the command path, optional arguments, an optional certificate for access control, and the target vrw instance.</p>

<h3>Sending Keystrokes</h3>
<p>Use the key input field in the panel header to send keystrokes to the selected command. Press <strong>Enter</strong> or click <strong>Send</strong> to transmit. Supports special keys using angle bracket notation:</p>
<ul>
    <li><code>&lt;Enter&gt;</code>, <code>&lt;Esc&gt;</code>, <code>&lt;Tab&gt;</code>, <code>&lt;Backspace&gt;</code></li>
    <li><code>&lt;Up&gt;</code>, <code>&lt;Down&gt;</code>, <code>&lt;Left&gt;</code>, <code>&lt;Right&gt;</code></li>
    <li><code>&lt;C-c&gt;</code> (Ctrl+C), <code>&lt;C-d&gt;</code> (Ctrl+D)</li>
    <li><code>&lt;F1&gt;</code> through <code>&lt;F12&gt;</code></li>
</ul>

<h3>Resizing the Terminal</h3>
<p>Use the <strong>R</strong> (rows) and <strong>C</strong> (columns) inputs in the top bar to resize the virtual terminal. Click <strong>Resize</strong> to apply. Valid ranges: rows 1-200, columns 1-500.</p>

<h3>Killing Commands</h3>
<p>Click the <strong>&#x2715;</strong> button next to a command in the sidebar to send SIGINT (Ctrl+C) to the process.</p>

<h2>Certificates</h2>
<p>The <strong>Certs</strong> tab in the sidebar shows all certificates configured in the connected instances' certificate pools. Certificates provide per-command access control — only clients presenting a certificate's derived token can interact with commands bound to that certificate.</p>
<p>When spawning a command, you can select a certificate to bind it. The certificate badge next to each command in the sidebar shows its binding status.</p>

<h2>Log Viewer</h2>
<p>The <strong>Logs</strong> tab provides access to the vrw command log. Use the search bar to filter log entries by content. Each entry shows a timestamp, the command type (spawn, kill, send_keys, etc.), and relevant details.</p>

<h2>Font Size</h2>
<p>Use the <strong>A-</strong> and <strong>A+</strong> buttons in the top bar to adjust the terminal font size (8px-28px). Your preference is saved in localStorage.</p>

<h2>VTTY Update Modes</h2>
<p>The web UI supports two modes for detecting when a terminal buffer has changed. You can switch between them using the <strong>Update</strong> dropdown in the bottom status bar. Your choice is saved in localStorage and will be restored on the next visit.</p>

<h3>Push Mode (default)</h3>
<p>In push mode, the server monitors each command's VTTY buffer at a configurable interval (default 200ms). When changes are detected, the server sends a lightweight <code>vtty_dirty</code> signal over the existing WebSocket connection. The signal contains only the command ID — no cell data, no HTML. The web UI then fetches the full HTML via <code>GET /api/commands/:id/vtty/html</code> at its own pace (debounced at 50ms). This is the most efficient mode because no polling is required; the server only sends when something actually changed.</p>
<p>Push mode is the default and is recommended for most use cases. It provides the lowest latency and lowest bandwidth overhead.</p>

<h3>Poll Mode</h3>
<p>In poll mode, the web client periodically calls <code>GET /api/commands/:id/vtty/changed</code> to ask "has the buffer changed since the last check?". The response is a simple <code>{ "changed": true/false }</code> with no HTML or diff data. If changed, the client then fetches the full HTML via the standard endpoint. The poll interval is configurable via the input next to the mode dropdown (50ms–5000ms, default 500ms).</p>
<p>Poll mode is useful when WebSocket connections are unreliable — for example, when a reverse proxy buffers WebSocket frames, when network conditions cause frequent WS reconnections, or when the client wants full control over refresh timing. The bandwidth overhead is slightly higher than push mode because the changed-check runs continuously even when nothing is changing.</p>

<h3>Server Configuration</h3>
<p>The server-side update settings can be configured in the vrw config file under the <code>web</code> section:</p>
<pre><code>web:
  update_mode: push       # "push" (default) or "poll"
  dirty_check_ms: 200     # server dirty-check interval (push mode)
  default_poll_ms: 500    # suggested client poll interval (poll mode)
</code></pre>
<p>The <code>dirty_check_ms</code> controls how often the server compares the VTTY buffer against the last-known snapshot in push mode. Lower values provide faster updates but increase CPU usage slightly. The <code>default_poll_ms</code> is the suggested interval that the web UI will use when in poll mode, but the user can override it via the UI controls at any time.</p>

<h2>API Reference</h2>
<table>
    <tr><th>Method</th><th>Endpoint</th><th>Description</th></tr>
    <tr><td>GET</td><td><code>/api/commands</code></td><td>List all running commands</td></tr>
    <tr><td>POST</td><td><code>/api/commands</code></td><td>Spawn a new command</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/vtty</code></td><td>Get VTTY output as ANSI text</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/vtty/html</code></td><td>Get VTTY as rendered HTML + cursor</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/vtty/changed</code></td><td>Check if VTTY buffer changed (poll mode)</td></tr>
    <tr><td>POST</td><td><code>/api/commands/:id/keys</code></td><td>Send keystrokes to a command</td></tr>
    <tr><td>POST</td><td><code>/api/commands/:id/kill</code></td><td>Kill a running command</td></tr>
    <tr><td>POST</td><td><code>/api/commands/:id/resize</code></td><td>Resize virtual terminal</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/handles</code></td><td>List handles for a command</td></tr>
    <tr><td>GET</td><td><code>/api/certificates</code></td><td>List certificate pool</td></tr>
    <tr><td>GET</td><td><code>/api/info</code></td><td>Instance info and stats</td></tr>
    <tr><td>GET</td><td><code>/api/log</code></td><td>Command log with search</td></tr>
    <tr><td>POST</td><td><code>/api/shutdown</code></td><td>Graceful shutdown</td></tr>
</table>

<h2>Keyboard Shortcuts</h2>
<table>
    <tr><th>Shortcut</th><th>Action</th></tr>
    <tr><td><code>Enter</code> in key input</td><td>Send keystrokes</td></tr>
</table>
`;
}


// ─── Workspace Environments ──
// Environments are named sets of [panels, servers, commands] defined in
// the server config file ([[environments]]).  They allow the user to
// switch between predefined workspaces with a single click.
//
// On the CLI, environments can be specified in separate config files or
// inline in the main config.  The server exposes them via /api/environments.
// Environments with auto_start=true are pre-spawned when the server loads.

// Server-side environments fetched from /api/environments.
let _serverEnvironments = [];

/// Fetch workspace environments from the server.
async function fetchEnvironments() {
    try {
        const json = await api.getEnvironments();
        if (json.status === 'ok' && Array.isArray(json.data)) {
            _serverEnvironments = json.data;
        }
    } catch (e) {
        // Not critical — environments are optional
    }
}

/// Activate a workspace environment: create panels, connect servers, and spawn commands.
async function activateEnvironment(name) {
    const allEnvs = [..._serverEnvironments, ...JSON.parse(localStorage.getItem('vrw_environments') || '[]')];
    const env = allEnvs.find(e => e.name === name);
    if (!env) {
        console.error('[vrw] Environment not found:', name);
        return;
    }

    // Remove all existing panels
    const existingIds = state.panels.map(p => p.id);
    for (const id of existingIds) {
        disconnectPanelWs(id);
        stopPanelPoll(id);
    }
    state.panels = [];
    state._focusedPanelId = null;

    // Set layout direction
    if (env.layout === 'vertical') {
        state.panelLayout = 'column';
    } else if (env.layout === 'horizontal') {
        state.panelLayout = 'row';
    }
    localStorage.setItem('vrw_panel_layout', state.panelLayout);

    const defaultServer = env.default_server || getBaseUrl();
    const defaultToken = env.default_token || '';

    // Register all servers from the environment
    for (const panelDef of (env.panels || [])) {
        const serverUrl = panelDef.server || defaultServer;
        const serverToken = panelDef.token || defaultToken;
        const serverLabel = panelDef.server_label || '';
        addConnection(serverUrl, serverLabel, serverToken);
    }

    // Create panels and spawn commands
    for (let i = 0; i < (env.panels || []).length; i++) {
        const panelDef = env.panels[i];
        const panel = addPanelDirect();
        if (!panel) continue;

        const serverUrl = panelDef.server || defaultServer;
        panel.selectedInstUrl = serverUrl;

        // Focus the first panel
        if (i === 0) focusPanel(panel.id);

        // Spawn the first command in this panel (others can be spawned later)
        if (panelDef.commands && panelDef.commands.length > 0) {
            const cmdDef = panelDef.commands[0];
            try {
                const body = { cmd: cmdDef.cmd };
                if (cmdDef.args) body.args = cmdDef.args.split(' ');
                if (cmdDef.workdir) body.dir = cmdDef.workdir;
                if (cmdDef.certificate) body.certificate = cmdDef.certificate;
                if (cmdDef.rows) body.rows = cmdDef.rows;
                if (cmdDef.cols) body.cols = cmdDef.cols;
                if (cmdDef.retain_on_exit) body.retain_on_exit = true;

                const json = await api.activateEnvironment(serverUrl, body);
                if (json.status === 'ok' && json.data && json.data.id) {
                    panel.selectedCmdId = json.data.id;
                }
            } catch (e) {
                console.error('[vrw] Failed to spawn command for panel:', e);
            }
        }
    }

    // Re-render panels
    renderPanels();

    // Reload commands list to show spawned commands in sidebar
    loadCommands();
    loadCertificates();

    // Switch to Servers tab to show the results
    const serversTab = document.querySelector('.sidebar-tab:first-child');
    if (serversTab) switchSidebarTab('servers', serversTab);

    console.log('[vrw] Environment activated:', name, '—', (env.panels || []).length, 'panels');
}

// ─── Command Groups ───
// Groups are stored in localStorage as vrw_cmd_groups = { "group-name": ["cmdName1", ...], ... }

/// Load command groups from localStorage.
function getCmdGroups() {
    try {
        const raw = localStorage.getItem('vrw_cmd_groups');
        if (!raw) return {};
        return JSON.parse(raw);
    } catch (e) {
        return {};
    }
}

/// Save command groups to localStorage.
function saveCmdGroups(groups) {
    try {
        localStorage.setItem('vrw_cmd_groups', JSON.stringify(groups));
    } catch (e) { /* quota exceeded */ }
}

/// Load collapsed state for group sections.
function getGroupCollapsedState() {
    try {
        const raw = localStorage.getItem('vrw_group_collapsed');
        if (!raw) return {};
        return JSON.parse(raw);
    } catch (e) {
        return {};
    }
}

function saveGroupCollapsedState(state) {
    try {
        localStorage.setItem('vrw_group_collapsed', JSON.stringify(state));
    } catch (e) { /* ignore */ }
}

/// Create a new command group.
function createCmdGroup() {
    const input = document.getElementById('newGroupName');
    if (!input) return;
    const name = input.value.trim();
    if (!name) return;
    const groups = getCmdGroups();
    if (groups[name]) {
        // Group already exists
        input.value = '';
        renderGroups();
        return;
    }
    groups[name] = [];
    saveCmdGroups(groups);
    input.value = '';
    renderGroups();
}

/// Delete a command group.
function deleteCmdGroup(groupName) {
    const groups = getCmdGroups();
    delete groups[groupName];
    saveCmdGroups(groups);
    renderGroups();
}

/// Rename a command group.
function renameCmdGroup(oldName) {
    const newName = prompt('Rename group "' + oldName + '" to:');
    if (!newName || !newName.trim()) return;
    const trimmed = newName.trim();
    if (trimmed === oldName) return;
    const groups = getCmdGroups();
    if (groups[trimmed]) {
        alert('A group named "' + trimmed + '" already exists.');
        return;
    }
    groups[trimmed] = groups[oldName] || [];
    delete groups[oldName];
    saveCmdGroups(groups);
    // Also update collapsed state
    const collapsed = getGroupCollapsedState();
    if (collapsed[oldName] !== undefined) {
        collapsed[trimmed] = collapsed[oldName];
        delete collapsed[oldName];
        saveGroupCollapsedState(collapsed);
    }
    renderGroups();
}

/// Toggle a command name in a group (add if absent, remove if present).
function toggleCmdInGroup(groupName, cmdName) {
    const groups = getCmdGroups();
    if (!groups[groupName]) groups[groupName] = [];
    const idx = groups[groupName].indexOf(cmdName);
    if (idx >= 0) {
        groups[groupName].splice(idx, 1);
    } else {
        groups[groupName].push(cmdName);
    }
    saveCmdGroups(groups);
    // Re-render groups if the tab is visible
    if (document.getElementById('tab-groups') && !document.getElementById('tab-groups').classList.contains('hidden')) {
        renderGroups();
    }
}

/// Toggle collapsed state of a group section.
function toggleGroupCollapse(groupName) {
    const collapsed = getGroupCollapsedState();
    collapsed[groupName] = !collapsed[groupName];
    saveGroupCollapsedState(collapsed);
    renderGroups();
}

/// Render the groups tab content.
function renderGroups() {
    const container = document.getElementById('groupList');
    if (!container) return;
    const groups = getCmdGroups();
    const groupNames = Object.keys(groups);
    const collapsed = getGroupCollapsedState();

    if (groupNames.length === 0) {
        container.innerHTML = '<div style="padding:0.5rem;color:var(--text-muted);font-size:0.7rem;text-align:center;">No groups created yet. Right-click a command in the Servers tab to add it to a group.</div>';
        return;
    }

    // Build a lookup of all available commands: cmdName → { inst, cmd }
    const cmdMap = {};
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            const cmdName = cmd.name || cmd.id;
            if (!cmdMap[cmdName]) {
                cmdMap[cmdName] = { inst, cmd, cmdName };
            }
        }
    }

    let html = '';
    for (const gName of groupNames) {
        const isCollapsed = collapsed[gName] === true;
        const cmdNames = groups[gName] || [];
        html += '<div class="group-section">';
        html += '<div class="group-header" data-action="ToggleGroupCollapse" data-name="' + escHtml(gName) + '">';
        html += '<span class="group-caret">' + (isCollapsed ? '&#x25B6;' : '&#x25BC;') + '</span>';
        html += '<span class="group-name">' + escHtml(gName) + '</span>';
        html += '<span class="group-count">' + cmdNames.length + '</span>';
        html += '<span class="group-actions">';
        html += '<button class="btn btn-xs" data-action="RenameCmdGroup" data-name="' + escHtml(gName) + '" title="Rename group">&#9998;</button>';
        html += '<button class="btn btn-xs btn-danger" data-action="DeleteCmdGroup" data-name="' + escHtml(gName) + '" title="Delete group">&#x2715;</button>';
        html += '</span>';
        html += '</div>';
        if (!isCollapsed) {
            if (cmdNames.length === 0) {
                html += '<div style="padding:0.3rem 0.5rem 0.3rem 1.5rem;color:var(--text-muted);font-size:0.65rem;font-style:italic;">Empty — right-click a command to add it here</div>';
            } else {
                for (const cmdName of cmdNames) {
                    const entry = cmdMap[cmdName];
                    if (entry) {
                        const isAlive = entry.cmd.alive !== false;
                        const isFrozen = entry.cmd.frozen === true;
                        const isUnreachable = entry.inst.reachable === false;
                        const statusDot = isAlive ? '<span class="status-dot status-running"></span>' :
                            (isFrozen ? '<span class="status-dot status-frozen"></span>' : '<span class="status-dot status-exited"></span>');
                        const selected = (state.selectedInstUrl === entry.inst.url && state.selectedCmdId === entry.cmd.id) ? ' group-cmd-selected' : '';
                        html += '<div class="group-cmd-item' + selected + '"' +
                            ' data-inst-url="' + escHtml(entry.inst.url) + '"' +
                            ' data-cmd-id="' + escHtml(entry.cmd.id) + '"' +
                            ' data-cmd-name="' + escHtml(cmdName) + '"' +
                            (isUnreachable ? ' style="opacity:0.4;"' : '') +
                            ' data-action="SelectCommand"' +
                            ' title="' + escHtml(entry.inst.label) + ' / ' + escHtml(cmdName) + '">' +
                            statusDot +
                            '<span class="group-cmd-name">' + escHtml(cmdName) + '</span>' +
                            '<button class="btn btn-xs" data-action="ToggleCmdInGroup" data-name="' + escHtml(gName) + '" data-cmd-name="' + escHtml(cmdName) + '" title="Remove from group" style="margin-left:auto;padding:0 0.2rem;font-size:0.55rem;">&#x2715;</button>' +
                            '</div>';
                    } else {
                        html += '<div class="group-cmd-item" style="opacity:0.4;cursor:default;">' +
                            '<span class="group-cmd-name" style="text-decoration:line-through;">' + escHtml(cmdName) + '</span>' +
                            '<span style="font-size:0.55rem;color:var(--text-muted);margin-left:auto;">(not running)</span>' +
                            '<button class="btn btn-xs" data-action="ToggleCmdInGroup" data-name="' + escHtml(gName) + '" data-cmd-name="' + escHtml(cmdName) + '" title="Remove from group" style="margin-left:auto;padding:0 0.2rem;font-size:0.55rem;">&#x2715;</button>' +
                            '</div>';
                    }
                }
            }
        }
        html += '</div>';
    }
    container.innerHTML = html;
}

// ─── Workspaces ───
// Workspaces save/restore the current panel configuration.
// Stored in localStorage as vrw_workspaces = { "name": { panels: [...], layout: "row", ... }, ... }

/// Load workspaces from localStorage.
function getWorkspaces() {
    try {
        const raw = localStorage.getItem('vrw_workspaces');
        if (!raw) return {};
        return JSON.parse(raw);
    } catch (e) {
        return {};
    }
}

/// Save workspaces to localStorage.
function saveWorkspaces(workspaces) {
    try {
        localStorage.setItem('vrw_workspaces', JSON.stringify(workspaces));
    } catch (e) { /* quota exceeded */ }
}

/// Render the workspace list inside the dropdown.
function renderWorkspaceList() {
    const container = document.getElementById('workspaceList');
    if (!container) return;
    const workspaces = getWorkspaces();
    const names = Object.keys(workspaces);

    if (names.length === 0) {
        container.innerHTML = '<div style="padding:0.3rem 0.5rem;color:var(--text-muted);font-size:0.65rem;">No saved workspaces</div>';
        return;
    }

    let html = '';
    for (const name of names) {
        const panelCount = (workspaces[name].panels || []).length;
        html += '<div style="display:flex;align-items:center;gap:0.3rem;">';
        html += '<button class="ws-load-btn" data-action="LoadWorkspace" data-name="' + escHtml(name) + '" style="flex:1;text-align:left;">' +
            '<span style="color:var(--accent);">&#x1F4C2;</span> ' + escHtml(name) +
            ' <span style="color:var(--text-muted);font-size:0.55rem;">(' + panelCount + ' panels)</span></button>';
        html += '<button class="btn btn-xs" data-action="DeleteWorkspace" data-name="' + escHtml(name) + '" title="Delete" style="font-size:0.55rem;">&#x2715;</button>';
        html += '</div>';
    }
    container.innerHTML = html;
}

/// Get the command name for a panel from the current connections.
function _getPanelCmdName(panel) {
    if (!panel.selectedInstUrl || !panel.selectedCmdId) return null;
    const inst = state.connections.find(i => i.url === panel.selectedInstUrl);
    if (!inst || !inst._commands) return null;
    const cmd = inst._commands.find(c => c.id === panel.selectedCmdId);
    return cmd ? (cmd.name || cmd.id) : null;
}

/// Load a workspace and restore panel configuration.
function loadWorkspace(name) {
    const workspaces = getWorkspaces();
    const ws = workspaces[name];
    if (!ws) return;

    // Apply layout
    if (ws.layout) {
        state.panelLayout = ws.layout;
        localStorage.setItem('vrw_panel_layout', ws.layout);
    }

    // Clear existing panels (keep connections)
    // Disconnect WS for all existing panels first
    for (const p of state.panels) {
        if (p.ws) {
            try { p.ws.close(); } catch (e) { /* ignore */ }
            p.ws = null;
        }
        if (p.pollTimer) {
            clearInterval(p.pollTimer);
            p.pollTimer = null;
        }
    }
    state.panels = [];

    // Create panels from saved config
    const panelConfigs = ws.panels || [];
    if (panelConfigs.length === 0) {
        addPanelDirect();
    } else {
        for (const cfg of panelConfigs) {
            const panel = addPanelDirect();
            panel.fontSize = cfg.fontSize || state.fontSize;
            panel.theme = cfg.theme || '';
            panel.customTitle = cfg.customTitle || '';
            panel.selectedInstUrl = cfg.instUrl || null;
            panel.selectedCmdId = cfg.cmdId || null;
        }
    }

    // Force panel re-render
    renderPanels();

    // Focus the first panel
    if (state.panels.length > 0) {
        focusPanel(state.panels[0].id);
    }

    // Trigger command loading to populate sidebar and auto-select
    if (panelConfigs.length > 0) {
        loadCommands();
    }

    // Close the workspace menu
    const menu = document.getElementById('workspaceMenu');
    if (menu) menu.classList.add('hidden');
}

/// Delete a workspace.
function deleteWorkspace(name) {
    const workspaces = getWorkspaces();
    delete workspaces[name];
    saveWorkspaces(workspaces);
    renderWorkspaceList();
}

// Close workspace menu when clicking outside
document.addEventListener('click', (e) => {
    const dropdown = document.getElementById('workspaceDropdown');
    const menu = document.getElementById('workspaceMenu');
    if (dropdown && menu && !menu.classList.contains('hidden') && !dropdown.contains(e.target)) {
        menu.classList.add('hidden');
    }
});

    // Expose to global scope
    // Docs
    window.showDocs = showDocs;
    // Environments
    window.fetchEnvironments = fetchEnvironments;
    window.activateEnvironment = activateEnvironment;
    // Groups
    window.getCmdGroups = getCmdGroups;
    window.createCmdGroup = createCmdGroup;
    window.deleteCmdGroup = deleteCmdGroup;
    window.renameCmdGroup = renameCmdGroup;
    window.toggleCmdInGroup = toggleCmdInGroup;
    window.toggleGroupCollapse = toggleGroupCollapse;
    window.renderGroups = renderGroups;
    // Workspaces
    window.loadWorkspace = loadWorkspace;
    window.deleteWorkspace = deleteWorkspace;
    window._toggleCmdInGroupAndRender = function(groupName, cmdName) {
        toggleCmdInGroup(groupName, cmdName);
        renderGroups();
    };
    window.closeWorkspaceManage = function() {
        releaseCurrentFocusTrap();
        const overlay = document.getElementById('workspaceManageOverlay');
        if (overlay) overlay.remove();
    };
})();
