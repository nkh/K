// ─── Sidebar ───
(function() {
    'use strict';

function _lsGet(key, fallback) {
    try { const v = localStorage.getItem(key); return v ? JSON.parse(v) : fallback; } catch { return fallback; }
}
function _lsSet(key, val) {
    try { localStorage.setItem(key, JSON.stringify(val)); } catch {}
}

function _shortLabel(inst) {
    const label = inst._serverName || inst.label || inst.url;
    // For localhost:port, show just the port
    try {
        const u = new URL(inst.url);
        if (u.hostname === 'localhost' || u.hostname === '127.0.0.1') return u.port || label;
    } catch {}
    return label;
}

function toggleSidebar() {
    const sidebar = document.getElementById('sidebar');
    sidebar.classList.toggle('collapsed');
    if (sidebar.classList.contains('collapsed')) sidebar.style.width = '';
}

function toggleResources() {
    state.showResources = !state.showResources;
    localStorage.setItem('vrw_show_resources', state.showResources.toString());
    document.querySelectorAll('.resource-badge, .instance-url').forEach(el => el.classList.toggle('hidden', !state.showResources));
    const stBadge = document.getElementById('stResourceBadge');
    if (stBadge && !state.showResources) stBadge.classList.add('hidden');
    const stUrl = document.getElementById('stInstanceUrl');
    if (stUrl) stUrl.classList.toggle('hidden', !state.showResources);
    if (state.showResources) updateSharedToolbar();
}

function toggleBottombar() {
    const bar = document.getElementById('bottomBar');
    const btn = document.getElementById('statusBtn');
    bar.classList.toggle('hidden');
    const isHidden = bar.classList.contains('hidden');
    if (btn) { btn.style.background = isHidden ? '' : 'var(--accent)'; btn.style.color = isHidden ? '' : 'var(--bg-primary)'; }
    localStorage.setItem('vrw_bottombar_hidden', isHidden ? 'true' : 'false');
}

function initBottombar() {
    const shouldHide = localStorage.getItem('vrw_bottombar_hidden') !== 'false';
    const bar = document.getElementById('bottomBar');
    const btn = document.getElementById('statusBtn');
    if (shouldHide) { bar.classList.add('hidden'); }
    else { bar.classList.remove('hidden'); if (btn) { btn.style.background = 'var(--accent)'; btn.style.color = 'var(--bg-primary)'; } }
}

function _setViewBtnStyle(btn, active) {
    if (!btn) return;
    btn.style.background = active ? 'var(--accent)' : '';
    btn.style.color = active ? 'var(--bg-primary)' : '';
}

function toggleLogsView() {
    const btn = document.getElementById('logsBtn');
    const vtty = document.getElementById('view-vtty');
    const log = document.getElementById('view-log');
    if (state.currentView === 'log') {
        state.currentView = 'vtty';
        vtty.classList.remove('hidden'); log.classList.add('hidden');
        _setViewBtnStyle(btn, false); disconnectLogWs();
    } else {
        state.currentView = 'log';
        vtty.classList.add('hidden'); log.classList.remove('hidden');
        _setViewBtnStyle(btn, true); loadDocs();
        if (!document.getElementById('logSearch').value) connectLogWs();
    }
}

function switchSidebarTab(tab, el) {
    document.querySelectorAll('.sidebar-tab').forEach(t => t.classList.remove('active'));
    el.classList.add('active');
    ['servers', 'spawn', 'templates', 'certs', 'groups'].forEach(t =>
        document.getElementById('tab-' + t).classList.toggle('hidden', t !== tab));
    if (tab === 'templates') renderTemplates();
    if (tab === 'groups') renderGroups();
}

function updateSidebarTabsVisibility() {
    const spawnTab = document.querySelector('.sidebar-tab:nth-child(2)');
    const spawnContent = document.getElementById('tab-spawn');
    const anyReachable = state.connections.some(i => i.reachable === true);
    const show = anyReachable;
    if (spawnTab) spawnTab.classList.toggle('hidden', !show);
    if (!show && spawnContent) spawnContent.classList.add('hidden');
    if (show && spawnContent && spawnTab && spawnTab.classList.contains('active')) spawnContent.classList.remove('hidden');
    if (!show && spawnTab && spawnTab.classList.contains('active')) {
        const cmdsTab = document.querySelector('.sidebar-tab:first-child');
        if (cmdsTab) switchSidebarTab('commands', cmdsTab);
    }
}

function updateCmdToolbarVisibility() {
    const killAllBtn = document.getElementById('killAllBtn');
    if (!killAllBtn) return;
    const show = state.connections.some(i => i.reachable === true) && state.connections.some(i => i._commands && i._commands.length > 0);
    killAllBtn.classList.toggle('hidden', !show);
    const freezeAllBtn = document.getElementById('freezeAllBtn');
    if (freezeAllBtn) freezeAllBtn.classList.toggle('hidden', !show);
}

// Click-delay mechanism to distinguish single-click from double-click on cmd-items.
// When a cmd-item is clicked, we delay the SelectCommand action by 250ms.
// If a dblclick arrives within that window, we cancel the pending click and handle dblclick instead.
let _cmdClickTimer = null;
let _cmdClickPending = null;

function _cancelPendingCmdClick() {
    if (_cmdClickTimer) { clearTimeout(_cmdClickTimer); _cmdClickTimer = null; }
    _cmdClickPending = null;
}

function _handleCmdItemClick(e) {
    // Skip clicks on buttons (kill, freeze, pin, keep, grab-handle) — they have their own handlers
    if (e.target.closest('button')) return;
    const item = e.target.closest('.cmd-item[data-cmd-id]');
    if (!item) return;
    // If ctrl/meta is held, always open new pane immediately (no delay needed)
    if (e.ctrlKey || e.metaKey) {
        _cancelPendingCmdClick();
        _openCommandInNewPane(item.dataset.instUrl, item.dataset.cmdId, item.dataset.cmdName);
        e.stopPropagation();
        e.preventDefault();
        return;
    }
    // Delay single-click to wait for possible double-click
    _cancelPendingCmdClick();
    const instUrl = item.dataset.instUrl;
    const cmdId = item.dataset.cmdId;
    const cmdName = item.dataset.cmdName;
    _cmdClickPending = { instUrl, cmdId, cmdName };
    _cmdClickTimer = setTimeout(() => {
        if (_cmdClickPending) {
            selectCommand(_cmdClickPending.instUrl, _cmdClickPending.cmdId, _cmdClickPending.cmdName);
            _cmdClickPending = null;
        }
        _cmdClickTimer = null;
    }, 250);
    e.stopPropagation();
    e.preventDefault();
}

function _handleCmdItemDblClick(e) {
    if (e.target.closest('button')) return;
    const item = e.target.closest('.cmd-item[data-cmd-id]');
    if (!item) return;
    e.stopPropagation();
    _cancelPendingCmdClick();
    if (e.ctrlKey || e.metaKey) {
        _openCommandInNewPane(item.dataset.instUrl, item.dataset.cmdId, item.dataset.cmdName);
    } else {
        selectCommand(item.dataset.instUrl, item.dataset.cmdId, item.dataset.cmdName);
    }
}

function _buildSidebar() {
    const filter = (document.getElementById('cmdFilter') || {}).value || '';
    const filterLower = filter.toLowerCase();
    if (!state._sidebarSort) state._sidebarSort = 'name';

    if (state._pendingSelectId) {
        const pendingId = state._pendingSelectId;
        state._pendingSelectId = null;
        for (const inst of state.connections) {
            const cmd = (Array.isArray(inst._commands) ? inst._commands : []).find(c => c.id === pendingId);
            if (cmd) { selectCommand(inst.url, cmd.id, cmd.name || cmd.id); return; }
        }
    }

    const container = document.getElementById('commandList');
    let html = '';

    const originUrl = window.location.origin;

    // Server tabs — always shown (All + one per server)
    html += '<div class="sidebar-sort-bar">';
    html += `<span class="sidebar-sort-item${state._sidebarSort === 'name' ? ' active' : ''}" data-action="SortSidebarBy" data-value="name">All<span class="server-tab-spawn-btn" data-action="SwitchSidebarTab" data-tab="spawn" title="Spawn command">+</span></span>`;
    for (const inst of state.connections) {
        const rCls = inst.reachable === true ? 'reachable' : inst.reachable === false ? 'unreachable' : 'unknown';
        const isActive = state._sidebarSort === inst.url;
        // Determine freeze/thaw state for this server
        const instCmds = inst._commands || [];
        const aliveCmds = instCmds.filter(c => c.alive !== false);
        const allFrozen = aliveCmds.length > 0 && aliveCmds.every(c => c.frozen);
        const anyFrozen = aliveCmds.some(c => c.frozen);
        const freezeIcon = allFrozen ? '&#9654;' : '&#8545;';
        const freezeActive = allFrozen ? ' active' : '';
        html += `<span class="sidebar-sort-item${isActive ? ' active' : ''}" data-action="SortSidebarBy" data-value="${escHtml(inst.url)}"><span class="server-reach-dot ${rCls}" style="margin-right:0.15rem;"></span>${escHtml(_shortLabel(inst))}<span class="server-tab-spawn-btn" data-action="SpawnOnServer" data-inst-url="${escHtml(inst.url)}" title="Spawn on this server">+</span><button class="server-tab-freeze-btn${freezeActive}" data-action="FreezeThawServer" data-inst-url="${escHtml(inst.url)}" title="${allFrozen ? 'Thaw all' : 'Freeze all'}">${freezeIcon}</button><button class="server-tab-btn" data-action="DisconnectServer" data-inst-url="${escHtml(inst.url)}" title="Disconnect">&#x2715;</button></span>`;
    }
    html += '</div>';

    let allCmds = [];
    for (const inst of state.connections) {
        for (const cmd of (Array.isArray(inst._commands) ? inst._commands : [])) {
            const cmdName = cmd.name || cmd.id;
            if (filterLower && !cmdName.toLowerCase().includes(filterLower) &&
                !(cmd.args || []).join(' ').toLowerCase().includes(filterLower) &&
                !String(cmd.pid).includes(filterLower)) continue;
            allCmds.push({ inst, cmd, cmdName });
        }
    }

    // Nav list for prev/next
    const activePanel = state._focusedPanelId ? state.panels.find(p => p.id === state._focusedPanelId) : null;
    const navInstUrl = activePanel && activePanel.selectedInstUrl ? activePanel.selectedInstUrl : null;
    const navCmds = (navInstUrl ? allCmds.filter(c => c.inst.url === navInstUrl) : allCmds);
    navCmds.sort((a, b) => (a.cmd.spawn_order ?? 0) - (b.cmd.spawn_order ?? 0));
    state._navCommands = navCmds.map(({ inst, cmd, cmdName }) => ({ instUrl: inst.url, cmdId: cmd.id, name: cmdName }));

    if (state._sidebarSort === 'name') {
        allCmds.sort((a, b) => a.cmdName.localeCompare(b.cmdName));
        html += renderCmdList(allCmds, state.connections.length > 1);
    } else {
        const grouped = state._sidebarSort;
        for (const inst of state.connections) {
            if (inst.url !== grouped) continue;
            const instCmds = allCmds.filter(c => c.inst.url === inst.url);
            if (inst._lastError) { html += `<div style="padding:0.5rem;color:var(--red);font-size:var(--ui-fs);">${escHtml(inst.label)}: ${escHtml(inst._lastError)}</div>`; continue; }
            if (instCmds.length === 0) { html += '<div style="padding:0.3rem 0.4rem;color:var(--text-muted);font-size:var(--ui-fs);">No commands</div>'; continue; }
            const orderedCmds = getOrderedCmds(inst.url, instCmds);
            orderedCmds.sort((a, b) => a.cmdName.localeCompare(b.cmdName));
            html += renderCmdList(orderedCmds);
        }
    }

    function renderCmdList(cmds, showServerBadge) {
        let out = '';
        for (const { inst, cmd, cmdName } of cmds) {
            const cert = cmd.certificate || '';
            const certBadge = cert ? `<span class="cert-badge" title="Bound to: ${escHtml(cert)}">${escHtml(cert)}</span>` : '';
            const selected = (state.selectedInstUrl === inst.url && state.selectedCmdId === cmd.id) ? ' selected' : '';
            const isAlive = cmd.alive !== false;
            const isFrozen = cmd.frozen === true;
            const runtimeStr = (isAlive || isFrozen) && cmd.runtime_secs > 0 ? formatRuntime(cmd.runtime_secs) : '';
            const exitBadge = cmd.exit_code != null ? `<span class="exit-badge ${cmd.exit_code === 0 ? 'success' : 'failure'}">exit ${cmd.exit_code}</span>` : '';
            const res = state._resourceCache[cmd.id];
            const pinnedNames = getPinnedNames();
            const isPinned = pinnedNames.includes(cmdName);
            const retainOnExit = cmd.exit && cmd.exit.retain_on_exit === true;
            const keepBtnHtml = isAlive
                ? `<button class="keep-btn${retainOnExit ? ' active' : ''}" data-action="ToggleKeepCmd" data-inst-url="${escHtml(inst.url)}" data-cmd-id="${escHtml(cmd.id)}" title="${retainOnExit ? 'Unkeep (terminal will be removed on exit)' : 'Keep (retain terminal after exit)'}">${retainOnExit ? '&#9733;' : '&#9734;'}</button>`
                : (retainOnExit ? '<span class="keep-badge" title="Terminal kept after exit">&#9733;</span>' : '');
            const freezeBtnHtml = isAlive
                ? `<button class="cmd-freeze-btn${isFrozen ? ' active' : ''}" data-action="TogglePauseRunByIdx" data-inst-url="${escHtml(inst.url)}" data-cmd-id="${escHtml(cmd.id)}" title="${isFrozen ? 'Thaw' : 'Freeze'}">${isFrozen ? '&#9654;' : '&#8545;'}</button>`
                : '';
            const dp = [];
            if (runtimeStr) dp.push(escHtml(runtimeStr));
            if (isFrozen) dp.push('PAUSED');
            if (res && res.cpu_percent != null) dp.push(res.cpu_percent.toFixed(1) + '%');
            if (res && res.memory_mb != null) { const mb = res.memory_mb; dp.push(mb >= 1024 ? (mb / 1024).toFixed(1) + 'G' : mb.toFixed(1) + 'M'); }
            const unreachableTitle = inst.reachable === false ? ' [disconnected]' : '';
            const serverBadge = showServerBadge
                ? `<span class="resource-badge" style="font-size:0.55rem;opacity:0.7;" title="${escHtml(inst.url)}">${escHtml(inst.label)}</span>`
                : '';
            const reachCls = inst.reachable === true ? 'reachable' : inst.reachable === false ? 'unreachable' : 'unknown';
            const dimStyle = inst.reachable === false ? 'opacity:0.4;' : ((isAlive || isFrozen) ? '' : 'opacity:0.6;');
            out += `<div class="cmd-item${selected}${isFrozen ? ' frozen' : ''}${!isAlive && !isFrozen ? ' exited' : ''}${inst.reachable === false ? ' unreachable' : ''}" data-inst-url="${escHtml(inst.url)}" data-cmd-id="${escHtml(cmd.id)}" data-cmd-name="${escHtml(cmdName)}" data-cmd-alive="${isAlive}" data-cmd-frozen="${isFrozen}" data-cmd-retained="${retainOnExit}" tabindex="0" role="button" aria-label="Command ${escHtml(cmdName)}" draggable="true" ondragstart="onCmdDragStart(event,this.dataset.instUrl,this.dataset.cmdId,this.dataset.cmdName)" oncontextmenu="showCmdContextMenu(event,this.dataset.instUrl,this.dataset.cmdId,this.dataset.cmdName,this.dataset.cmdAlive==='true',this.dataset.cmdRetained==='true')" title="${escHtml(inst.label)} / ${escHtml(cmdName)}${unreachableTitle}" style="${dimStyle}"><div class="cmd-item-row"><button class="cmd-kill-btn" data-inst-url="${escHtml(inst.url)}" data-cmd-id="${escHtml(cmd.id)}" data-cmd-retained="${retainOnExit}" data-cmd-alive="${isAlive}">&#x2715;</button><span class="server-reach-dot ${reachCls}" style="flex-shrink:0;"></span>${keepBtnHtml}<button class="pin-btn${isPinned ? ' active' : ''}" data-action="TogglePinCmd" data-cmd-name="${escHtml(cmdName)}" title="${isPinned ? 'Unpin' : 'Pin'}">${isPinned ? '◉' : '◎'}</button><span class="cmd-grab-handle" onmousedown="_cmdReorderMouseDown(event,'${escHtml(inst.url)}','${escHtml(cmd.id)}','${escHtml(cmdName)}')" title="Drag to reorder / drop on pane to open">&#x2807;</span><span class="name">${escHtml(cmdName)}</span><span class="cmd-detail-inline">${dp.map(p => escHtml(p)).join(' · ')}</span>${serverBadge}${certBadge}${exitBadge}${freezeBtnHtml}</div>${dp.length > 0 ? `<div class="cmd-detail-row">${dp.join(' · ')}</div>` : ''}</div>`;
        }
        return out;
    }

    rearrangePinnedCommands(container);
    container.innerHTML = html || '<div style="padding:1rem;color:var(--text-muted);text-align:center;">No running commands</div>';
    // Use click/dblclick handlers with delay to distinguish single from double click.
    // The cmd-items no longer have data-action="SelectCommand" — clicks are handled here.
    container.onclick = _handleCmdItemClick;
    container.ondblclick = _handleCmdItemDblClick;
    updateInstanceDropdown();
    updateCmdToolbarVisibility();

    if (!state.selectedCmdId) {
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

// ─── Command Pinning ───
function getPinnedNames() { return _lsGet('vrw_pinned_cmds', []); }
function setPinnedNames(names) { _lsSet('vrw_pinned_cmds', names); }

function togglePinCmd(cmdName) {
    const pinned = getPinnedNames();
    const idx = pinned.indexOf(cmdName);
    if (idx >= 0) pinned.splice(idx, 1); else pinned.push(cmdName);
    setPinnedNames(pinned);
    loadCommands();
}

function rearrangePinnedCommands(container) {
    setTimeout(() => {
        if (!container) return;
        const items = container.querySelectorAll('.cmd-item[data-cmd-name]');
        const pinned = getPinnedNames();
        const [pinnedItems, unpinnedItems] = [[], []];
        items.forEach(item => (pinned.includes(item.dataset.cmdName) ? pinnedItems : unpinnedItems).push(item));
        if (pinnedItems.length > 0 && unpinnedItems.length > 0) {
            const header = document.createElement('div');
            header.className = 'pinned-section-header';
            header.textContent = '◉ Pinned';
            const parent = items[0] && items[0].parentNode;
            if (parent) {
                const first = parent.firstChild;
                items.forEach(item => item.remove());
                if (first) {
                    parent.insertBefore(header, first);
                    pinnedItems.forEach(item => parent.insertBefore(item, first));
                }
                unpinnedItems.forEach(item => parent.appendChild(item));
            }
        }
        container.querySelectorAll('.pin-btn').forEach(btn => {
            const item = btn.closest('.cmd-item');
            const isPinned = item && pinned.includes(item.dataset.cmdName);
            btn.classList.toggle('active', !!isPinned);
            btn.textContent = isPinned ? '◉' : '◎';
            btn.title = isPinned ? 'Unpin' : 'Pin';
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
            document.getElementById('sidebarContent').insertBefore(banner, document.getElementById('sidebarContent').firstChild);
        }
        banner.innerHTML = '<span class="disconnected-icon">&#9888;</span> Server disconnected: ' + escHtml(unreachable.map(i => i.label).join(', ')) + ' &mdash; output may be stale';
    } else if (banner) {
        banner.remove();
    }
}

function updateTerminalDisconnectedOverlay() {
    for (const panelObj of state.panels) {
        const panelEl = document.getElementById(panelObj.id);
        if (!panelEl) continue;
        // Remove any stale overlay
        const old = panelEl.querySelector('.disconnected-overlay');
        if (old) old.remove();
        // Update the reach dot in the panel header
        const dot = panelEl.querySelector('.panel-reach-dot');
        const inst = panelObj.selectedInstUrl ? state.connections.find(i => i.url === panelObj.selectedInstUrl) : null;
        if (dot && inst) {
            const rCls = inst.reachable === true ? 'reachable' : inst.reachable === false ? 'unreachable' : 'unknown';
            dot.className = 'panel-reach-dot ' + rCls;
            dot.title = inst.reachable === true ? 'Server connected' : inst.reachable === false ? 'Server unreachable' : 'Checking server...';
        }
    }
}

// ─── Documentation ───
function showDocs() {
    const btn = document.getElementById('docsBtn');
    const vtty = document.getElementById('view-vtty');
    const log = document.getElementById('view-log');
    const docs = document.getElementById('view-docs');
    if (state.currentView === 'docs') {
        state.currentView = 'vtty';
        vtty.classList.remove('hidden'); docs.classList.add('hidden');
        _setViewBtnStyle(btn, false);
    } else {
        if (state.currentView === 'log') { disconnectLogWs(); if (log) log.classList.add('hidden'); }
        state.currentView = 'docs';
        vtty.classList.add('hidden'); docs.classList.remove('hidden');
        _setViewBtnStyle(btn, true); loadDocs();
    }
}

async function loadDocs() {
    const container = document.getElementById('view-docs');
    container.innerHTML = '<div style="padding:2rem;text-align:center;color:var(--text-muted);">Loading documentation...</div>';
    try { container.innerHTML = renderMarkdown(await api.getDocs()); return; } catch (e) { /* fall through */ }
    container.innerHTML = renderEmbeddedDocs();
}

function renderMarkdown(md) {
    let html = md
        .replace(/^### (.+)$/gm, '<h3>$1</h3>').replace(/^## (.+)$/gm, '<h2>$1</h2>').replace(/^# (.+)$/gm, '<h1>$1</h1>')
        .replace(/```(\w*)\n([\s\S]*?)```/g, '<pre><code>$2</code></pre>').replace(/`([^`]+)`/g, '<code>$1</code>')
        .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
        .replace(/\[(.+?)\]\((.+?)\)/g, '<a href="$2" target="_blank" style="color:var(--accent);">$1</a>')
        .replace(/^\- (.+)$/gm, '<li>$1</li>').replace(/^(\d+)\. (.+)$/gm, '<li>$2</li>')
        .replace(/\n\n/g, '</p><p>').replace(/\n/g, '<br>');
    return '<p>' + html + '</p>';
}

function renderEmbeddedDocs() {
    return `<h1>vrw Administration</h1>
<h2>Overview</h2><p>vrw is a virtual terminal runner with a web control plane. It manages terminal applications, exposing their output through a web interface and REST API. This admin panel provides real-time monitoring and control of all running commands.</p>
<h2>Getting Started</h2><p>The admin panel connects to one or more vrw instances. Each instance manages its own set of terminal commands. Use the <strong>+ Panel</strong> button in the top bar to add connections to additional vrw instances.</p>
<h3>Connecting to an Instance</h3><p>By default, the admin panel connects to the vrw instance serving it. To add more instances:</p>
<ol><li>Click <strong>+ Panel</strong> in the top bar</li><li>Enter the instance URL (e.g., <code>http://localhost:9090</code>)</li><li>Optionally set a label and auth token</li><li>Click <strong>Add Panel</strong></li></ol>
<p>You can also use URL arguments: <code>?instance=http://host:8080&label=Prod&instance=http://host:9090&label=Dev</code></p>
<h2>URL Arguments for Multi-Instance</h2><p>The admin page accepts query parameters to pre-configure multi-panel views:</p>
<table><tr><th>Parameter</th><th>Description</th><th>Example</th></tr>
<tr><td><code>instance</code></td><td>vrw instance URL (repeatable)</td><td><code>?instance=http://host:8080</code></td></tr>
<tr><td><code>label</code></td><td>Panel label (matches instance order)</td><td><code>&label=Production</code></td></tr>
<tr><td><code>token</code></td><td>Auth token for instance (matches order)</td><td><code>&token=abc123</code></td></tr></table>
<p><strong>Full example:</strong> <code>/admin?instance=http://prod:8080&label=Production&instance=http://dev:9090&label=Development</code></p>
<h2>Managing Commands</h2>
<h3>Viewing Terminal Output</h3><p>Click on a command in the sidebar to view its real-time ANSI-rendered terminal output. The terminal emulator supports:</p>
<ul><li>Full ANSI color rendering (16, 256, and 24-bit truecolor)</li><li>Cursor position indicator (blue highlight)</li><li>Text attributes: bold, italic, underline, strikethrough</li><li>Scrollback buffer navigation via scrollbar</li></ul>
<h3>Spawning Commands</h3><p>Switch to the <strong>Spawn</strong> tab in the sidebar to create new commands. Specify the command path, optional arguments, an optional certificate for access control, and the target vrw instance.</p>
<h3>Sending Keystrokes</h3><p>Use the key input field in the panel header to send keystrokes to the selected command. Press <strong>Enter</strong> or click <strong>Send</strong> to transmit. Supports special keys using angle bracket notation:</p>
<ul><li><code>&lt;Enter&gt;</code>, <code>&lt;Esc&gt;</code>, <code>&lt;Tab&gt;</code>, <code>&lt;Backspace&gt;</code></li><li><code>&lt;Up&gt;</code>, <code>&lt;Down&gt;</code>, <code>&lt;Left&gt;</code>, <code>&lt;Right&gt;</code></li><li><code>&lt;C-c&gt;</code> (Ctrl+C), <code>&lt;C-d&gt;</code> (Ctrl+D)</li><li><code>&lt;F1&gt;</code> through <code>&lt;F12&gt;</code></li></ul>
<h3>Resizing the Terminal</h3><p>Use the <strong>R</strong> (rows) and <strong>C</strong> (columns) inputs in the top bar to resize the virtual terminal. Click <strong>Resize</strong> to apply. Valid ranges: rows 1-200, columns 1-500.</p>
<h3>Killing Commands</h3><p>Click the <strong>&#x2715;</strong> button next to a command in the sidebar to send SIGINT (Ctrl+C) to the process.</p>
<h2>Certificates</h2><p>The <strong>Certs</strong> tab shows all certificates configured in the connected instances' certificate pools. Certificates provide per-command access control — only clients presenting a certificate's derived token can interact with commands bound to that certificate.</p>
<h2>Log Viewer</h2><p>The <strong>Logs</strong> tab provides access to the vrw command log. Use the search bar to filter log entries by content.</p>
<h2>Font Size</h2><p>Use the <strong>A-</strong> and <strong>A+</strong> buttons in the top bar to adjust the terminal font size (8px-28px). Your preference is saved in localStorage.</p>
<h2>VTTY Update Modes</h2>
<h3>Push Mode (default)</h3><p>The server monitors each command's VTTY buffer at a configurable interval (default 200ms). When changes are detected, the server sends a lightweight <code>vtty_dirty</code> signal over the existing WebSocket connection containing only the command ID. The web UI then fetches the full HTML via <code>GET /api/commands/:id/vtty/html</code> at its own pace (debounced at 50ms).</p>
<h3>Poll Mode</h3><p>The web client periodically calls <code>GET /api/commands/:id/vtty/changed</code> to ask "has the buffer changed since the last check?". If changed, the client fetches the full HTML. The poll interval is configurable (50ms–5000ms, default 500ms).</p>
<h3>Server Configuration</h3><pre><code>web:
  update_mode: push       # "push" (default) or "poll"
  dirty_check_ms: 200     # server dirty-check interval (push mode)
  default_poll_ms: 500    # suggested client poll interval (poll mode)
</code></pre>
<h2>API Reference</h2>
<table><tr><th>Method</th><th>Endpoint</th><th>Description</th></tr>
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
<tr><td>POST</td><td><code>/api/shutdown</code></td><td>Graceful shutdown</td></tr></table>
<h2>Keyboard Shortcuts</h2><table><tr><th>Shortcut</th><th>Action</th></tr><tr><td><code>Enter</code> in key input</td><td>Send keystrokes</td></tr></table>`;
}

// ─── Workspace Environments ──
let _serverEnvironments = [];

async function fetchEnvironments() {
    try {
        const json = await api.getEnvironments();
        if (json.status === 'ok' && Array.isArray(json.data)) _serverEnvironments = json.data;
    } catch (e) { /* optional */ }
}

async function activateEnvironment(name) {
    const allEnvs = [..._serverEnvironments, ...JSON.parse(localStorage.getItem('vrw_environments') || '[]')];
    const env = allEnvs.find(e => e.name === name);
    if (!env) return;
    for (const id of state.panels.map(p => p.id)) { disconnectPanelWs(id); stopPanelPoll(id); }
    state.panels = []; state._focusedPanelId = null;
    if (env.layout === 'vertical') state.panelLayout = 'column';
    else if (env.layout === 'horizontal') state.panelLayout = 'row';
    localStorage.setItem('vrw_panel_layout', state.panelLayout);
    const defaultServer = env.default_server || getBaseUrl();
    const defaultToken = env.default_token || '';
    for (const pd of (env.panels || [])) addConnection(pd.server || defaultServer, pd.server_label || '', pd.token || defaultToken);
    for (let i = 0; i < (env.panels || []).length; i++) {
        const pd = env.panels[i];
        const panel = addPanelDirect();
        if (!panel) continue;
        panel.selectedInstUrl = pd.server || defaultServer;
        if (i === 0) focusPanel(panel.id);
        if (pd.commands && pd.commands.length > 0) {
            const cmdDef = pd.commands[0];
            try {
                const body = { cmd: cmdDef.cmd };
                if (cmdDef.args) body.args = cmdDef.args.split(' ');
                if (cmdDef.workdir) body.dir = cmdDef.workdir;
                if (cmdDef.certificate) body.certificate = cmdDef.certificate;
                if (cmdDef.rows) body.rows = cmdDef.rows;
                if (cmdDef.cols) body.cols = cmdDef.cols;
                if (cmdDef.retain_on_exit) body.retain_on_exit = true;
                const json = await api.activateEnvironment(pd.server || defaultServer, body);
                if (json.status === 'ok' && json.data && json.data.id) panel.selectedCmdId = json.data.id;
            } catch (e) { console.error('[vrw] Failed to spawn command for panel:', e); }
        }
    }
    renderPanels(); loadCommands(); loadCertificates();
    const serversTab = document.querySelector('.sidebar-tab:first-child');
    if (serversTab) switchSidebarTab('servers', serversTab);
}

// ─── Command Groups ───
function getCmdGroups() { return _lsGet('vrw_cmd_groups', {}); }
function saveCmdGroups(g) { _lsSet('vrw_cmd_groups', g); }
function getGroupCollapsedState() { return _lsGet('vrw_group_collapsed', {}); }
function saveGroupCollapsedState(s) { _lsSet('vrw_group_collapsed', s); }

function createCmdGroup() {
    const input = document.getElementById('newGroupName');
    if (!input) return;
    const name = input.value.trim();
    if (!name) return;
    const groups = getCmdGroups();
    if (groups[name]) { input.value = ''; renderGroups(); return; }
    groups[name] = [];
    saveCmdGroups(groups); input.value = ''; renderGroups();
}

function deleteCmdGroup(groupName) {
    const groups = getCmdGroups(); delete groups[groupName]; saveCmdGroups(groups); renderGroups();
}

function renameCmdGroup(oldName) {
    const newName = prompt('Rename group "' + oldName + '" to:');
    if (!newName || !(newName = newName.trim()) || newName === oldName) return;
    const groups = getCmdGroups();
    if (groups[newName]) { alert('A group named "' + newName + '" already exists.'); return; }
    groups[newName] = groups[oldName] || []; delete groups[oldName]; saveCmdGroups(groups);
    const collapsed = getGroupCollapsedState();
    if (collapsed[oldName] !== undefined) { collapsed[newName] = collapsed[oldName]; delete collapsed[oldName]; saveGroupCollapsedState(collapsed); }
    renderGroups();
}

function toggleCmdInGroup(groupName, cmdName) {
    const groups = getCmdGroups();
    if (!groups[groupName]) groups[groupName] = [];
    const idx = groups[groupName].indexOf(cmdName);
    if (idx >= 0) groups[groupName].splice(idx, 1); else groups[groupName].push(cmdName);
    saveCmdGroups(groups);
    if (document.getElementById('tab-groups') && !document.getElementById('tab-groups').classList.contains('hidden')) renderGroups();
}

function toggleGroupCollapse(groupName) {
    const collapsed = getGroupCollapsedState();
    collapsed[groupName] = !collapsed[groupName];
    saveGroupCollapsedState(collapsed); renderGroups();
}

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
    const cmdMap = {};
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) { const cmdName = cmd.name || cmd.id; if (!cmdMap[cmdName]) cmdMap[cmdName] = { inst, cmd, cmdName }; }
    }
    let html = '';
    for (const gName of groupNames) {
        const isCollapsed = collapsed[gName] === true;
        const cmdNames = groups[gName] || [];
        html += `<div class="group-section"><div class="group-header" data-action="ToggleGroupCollapse" data-name="${escHtml(gName)}">` +
            `<span class="group-caret">${isCollapsed ? '&#x25B6;' : '&#x25BC;'}</span>` +
            `<span class="group-name">${escHtml(gName)}</span><span class="group-count">${cmdNames.length}</span>` +
            `<span class="group-actions"><button class="btn btn-xs" data-action="RenameCmdGroup" data-name="${escHtml(gName)}" title="Rename group">&#9998;</button>` +
            `<button class="btn btn-xs btn-danger" data-action="DeleteCmdGroup" data-name="${escHtml(gName)}" title="Delete group">&#x2715;</button></span></div>`;
        if (!isCollapsed) {
            if (cmdNames.length === 0) {
                html += '<div style="padding:0.3rem 0.5rem 0.3rem 1.5rem;color:var(--text-muted);font-size:0.65rem;font-style:italic;">Empty — right-click a command to add it here</div>';
            } else {
                for (const cmdName of cmdNames) {
                    const entry = cmdMap[cmdName];
                    if (entry) {
                        const isAlive = entry.cmd.alive !== false;
                        const dot = isAlive ? (entry.cmd.frozen ? 'frozen' : 'running') : 'exited';
                        const selected = (state.selectedInstUrl === entry.inst.url && state.selectedCmdId === entry.cmd.id) ? ' group-cmd-selected' : '';
                        const unreachableStyle = entry.inst.reachable === false ? ' style="opacity:0.4;"' : '';
                        html += `<div class="group-cmd-item${selected}" data-inst-url="${escHtml(entry.inst.url)}" data-cmd-id="${escHtml(entry.cmd.id)}" data-cmd-name="${escHtml(cmdName)}"${unreachableStyle} data-action="SelectCommand" title="${escHtml(entry.inst.label)} / ${escHtml(cmdName)}"><span class="status-dot status-${dot}"></span><span class="group-cmd-name">${escHtml(cmdName)}</span><button class="btn btn-xs" data-action="ToggleCmdInGroup" data-name="${escHtml(gName)}" data-cmd-name="${escHtml(cmdName)}" title="Remove from group" style="margin-left:auto;padding:0 0.2rem;font-size:0.55rem;">&#x2715;</button></div>`;
                    } else {
                        html += `<div class="group-cmd-item" style="opacity:0.4;cursor:default;"><span class="group-cmd-name" style="text-decoration:line-through;">${escHtml(cmdName)}</span><span style="font-size:0.55rem;color:var(--text-muted);margin-left:auto;">(not running)</span><button class="btn btn-xs" data-action="ToggleCmdInGroup" data-name="${escHtml(gName)}" data-cmd-name="${escHtml(cmdName)}" title="Remove from group" style="margin-left:auto;padding:0 0.2rem;font-size:0.55rem;">&#x2715;</button></div>`;
                    }
                }
            }
        }
        html += '</div>';
    }
    container.innerHTML = html;
}

// ─── Workspaces ───
function getWorkspaces() { return _lsGet('vrw_workspaces', {}); }
function saveWorkspaces(ws) { _lsSet('vrw_workspaces', ws); }

function renderWorkspaceList() {
    const container = document.getElementById('workspaceList');
    if (!container) return;
    const workspaces = getWorkspaces();
    const names = Object.keys(workspaces);
    if (names.length === 0) { container.innerHTML = '<div style="padding:0.3rem 0.5rem;color:var(--text-muted);font-size:0.65rem;">No saved workspaces</div>'; return; }
    let html = '';
    for (const name of names) {
        const pc = (workspaces[name].panels || []).length;
        html += `<div style="display:flex;align-items:center;gap:0.3rem;"><button class="ws-load-btn" data-action="LoadWorkspace" data-name="${escHtml(name)}" style="flex:1;text-align:left;"><span style="color:var(--accent);">&#x1F4C2;</span> ${escHtml(name)} <span style="color:var(--text-muted);font-size:0.55rem;">(${pc} panels)</span></button><button class="btn btn-xs" data-action="DeleteWorkspace" data-name="${escHtml(name)}" title="Delete" style="font-size:0.55rem;">&#x2715;</button></div>`;
    }
    container.innerHTML = html;
}

function loadWorkspace(name) {
    const ws = getWorkspaces()[name];
    if (!ws) return;
    if (ws.layout) { state.panelLayout = ws.layout; localStorage.setItem('vrw_panel_layout', ws.layout); }
    for (const p of state.panels) {
        if (p.ws) { try { p.ws.close(); } catch (e) {} p.ws = null; }
        if (p.pollTimer) { clearInterval(p.pollTimer); p.pollTimer = null; }
    }
    state.panels = [];
    const panelConfigs = ws.panels || [];
    if (panelConfigs.length === 0) { addPanelDirect(); }
    else { for (const cfg of panelConfigs) { const panel = addPanelDirect(); panel.fontSize = cfg.fontSize || state.fontSize; panel.theme = cfg.theme || ''; panel.customTitle = cfg.customTitle || ''; panel.selectedInstUrl = cfg.instUrl || null; panel.selectedCmdId = cfg.cmdId || null; } }
    renderPanels();
    if (state.panels.length > 0) focusPanel(state.panels[0].id);
    if (panelConfigs.length > 0) loadCommands();
    const menu = document.getElementById('workspaceMenu');
    if (menu) menu.classList.add('hidden');
}

function deleteWorkspace(name) {
    const workspaces = getWorkspaces(); delete workspaces[name]; saveWorkspaces(workspaces); renderWorkspaceList();
}

document.addEventListener('click', (e) => {
    const dropdown = document.getElementById('workspaceDropdown');
    const menu = document.getElementById('workspaceMenu');
    if (dropdown && menu && !menu.classList.contains('hidden') && !dropdown.contains(e.target)) menu.classList.add('hidden');
});

    Object.assign(window, {
        initBottombar, toggleSidebar, toggleBottombar, toggleLogsView, toggleResources,
        switchSidebarTab, updateSidebarTabsVisibility, updateCmdToolbarVisibility,
        updateDisconnectedUI, updateSidebarBanner, updateTerminalDisconnectedOverlay,
        _buildSidebar, getPinnedNames, togglePinCmd, rearrangePinnedCommands,
        _sortSidebarBy(sortKey) { state._sidebarSort = sortKey; if (sortKey !== 'name') window._userSpawnInstUrl = sortKey; loadCommands(); },
        togglePauseRunPanelByIdx(instUrl, cmdId) {
            _doFreezeThaw(instUrl, cmdId).then(() => loadCommands()).catch(() => {});
        },
        _freezeThawServer(instUrl) {
            const inst = state.connections.find(i => i.url === instUrl);
            if (!inst || !inst._commands) return;
            const aliveCmds = inst._commands.filter(c => c.alive !== false);
            const allFrozen = aliveCmds.length > 0 && aliveCmds.every(c => c.frozen);
            const promises = aliveCmds.map(c => _doFreezeThaw(instUrl, c.id));
            Promise.all(promises).then(() => loadCommands()).catch(() => loadCommands());
        },
        _spawnOnServer(instUrl) {
            window._userSpawnInstUrl = instUrl;
            const spawnTab = document.querySelector('.sidebar-tab[data-tab="spawn"]');
            if (spawnTab) switchSidebarTab('spawn', spawnTab);
            updateInstanceDropdown();
        },
        showDocs, fetchEnvironments, activateEnvironment, getCmdGroups, createCmdGroup,
        deleteCmdGroup, renameCmdGroup, toggleCmdInGroup, toggleGroupCollapse, renderGroups,
        loadWorkspace, deleteWorkspace, saveWorkspaces, getWorkspaces,
        _toggleCmdInGroupAndRender(groupName, cmdName) { toggleCmdInGroup(groupName, cmdName); renderGroups(); },
        closeWorkspaceManage() { releaseCurrentFocusTrap(); const o = document.getElementById('workspaceManageOverlay'); if (o) o.remove(); },
    });
})();