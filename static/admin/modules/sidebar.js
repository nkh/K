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
        // Flush any VTTY updates that arrived while logs were shown
        _flushPendingVttyUpdate();
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
            scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 500);
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
})();
