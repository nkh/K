// ─── Commands Core: lookup, picker, navigation, loadCommands ───
(function() {
    'use strict';

// ── Command-name URL lookup ──
async function lookupAndSelectCommand(name) {
    try {
        const json = await api.lookupCommand(name);
        if (json.status !== 'ok') return;
        const matches = json.data;
        if (matches.length === 0) return; // no match, show admin page

        if (matches.length === 1) {
            // Single match — auto-select after loadCommands has run
            state._pendingSelectId = matches[0].id;
            loadCommands();
        } else {
            // Multiple matches — show picker overlay
            showCommandPicker(matches);
        }
    } catch (e) { /* ignore */ }
}

function showCommandPicker(matches) {
    // Remove existing picker if any
    const old = document.getElementById('cmdPicker');
    if (old) old.remove();

    let items = matches.map(m => {
        const argsStr = (m.args || []).join(' ');
        const detail = argsStr ? `${argsStr} (${m.pid})` : String(m.pid);
        const aliveBadge = m.alive
            ? '<span style="color:var(--green);font-size:0.65rem;">● running ' + formatRuntime(m.runtime_secs) + '</span>'
            : '<span style="color:var(--red);font-size:0.65rem;">● exited</span>';
        return `<div class="cmd-item" data-cmd-id="${escHtml(m.id)}" data-cmd-name="${escHtml(m.name)}" style="cursor:pointer;">
            <div class="cmd-item-row">
                <div style="flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-family:var(--font-mono);font-size:0.75rem;color:var(--text-primary);">${escHtml(m.name)}</div>
                ${aliveBadge}
                <span class="pid" style="color:var(--text-muted);font-size:0.7rem;">${escHtml(String(m.pid))}</span>
            </div>
            <div class="cmd-detail" style="font-family:var(--font-mono);font-size:0.65rem;color:var(--text-muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;padding-left:1.1rem;">${escHtml(detail)}</div>
        </div>`;
    }).join('');

    const overlay = document.createElement('div');
    overlay.id = 'cmdPicker';
    overlay.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.6);z-index:100;display:flex;align-items:center;justify-content:center;';
    overlay.onclick = (e) => { if (e.target === overlay) { releaseCurrentFocusTrap(); overlay.remove(); } };
    overlay.innerHTML = `<div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:8px;padding:1.25rem;min-width:420px;max-width:90vw;">
        <h2 style="font-size:1rem;color:var(--accent);margin-bottom:0.75rem;">Multiple commands matching "${escHtml(window.location.pathname.replace(/^\/+|\/+$/g, ''))}"</h2>
        <p style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.75rem;">Click a command to view its terminal:</p>
        <div style="max-height:50vh;overflow-y:auto;">${items}</div>
        <div style="margin-top:0.75rem;text-align:right;">
            <button class="btn" data-action="CloseCmdPicker">Cancel</button>
        </div>
    </div>`;
    document.body.appendChild(overlay);
    // Event delegation for command picker items (no inline onclick to avoid XSS)
    overlay.addEventListener('click', (e) => {
        const item = e.target.closest('.cmd-item[data-cmd-id]');
        if (item) {
            pickCommand(item.dataset.cmdId, item.dataset.cmdName);
        }
    });
    // Trap focus inside the picker and focus the first command item
    const panel = overlay.querySelector('div[style*="background:var(--bg-secondary)"]');
    if (panel) trapFocus(panel);
    const firstItem = overlay.querySelector('.cmd-item');
    if (firstItem) firstItem.focus();
}

function pickCommand(id, name) {
    releaseCurrentFocusTrap();
    const picker = document.getElementById('cmdPicker');
    if (picker) picker.remove();
    state._pendingSelectId = id;
    loadCommands();
}

// ── Command Navigation (prev/next) ──
// Navigate through the flat command list. These functions are called by
// the prev/next buttons in the topbar, useful when the sidebar is hidden.

function navigateCommand(direction) {
    if (_navCommands.length === 0) return;
    const currentIdx = _navCommands.findIndex(
        c => c.instUrl === state.selectedInstUrl && c.cmdId === state.selectedCmdId
    );
    let nextIdx;
    if (currentIdx === -1) {
        // No command selected — go to first
        nextIdx = direction > 0 ? 0 : _navCommands.length - 1;
    } else {
        nextIdx = (currentIdx + direction + _navCommands.length) % _navCommands.length;
    }
    const target = _navCommands[nextIdx];
    if (target) {
        selectCommand(target.instUrl, target.cmdId, target.name);
    }
}

function navigatePrevCommand() {
    navigateCommand(-1);
}

function navigateNextCommand() {
    navigateCommand(1);
}

async function loadCommands() {
    // Load commands from all instances in PARALLEL and track reachability.
    let anyReachableChanged = false;
    await Promise.all(state.connections.map(async (inst) => {
        try {
            const json = await api.getCommands(inst.url);
            inst._commands = json.status === 'ok' ? json.data : [];
            const wasReachable = inst.reachable;
            inst.reachable = true;
            inst._lastError = null;
            if (wasReachable !== true) anyReachableChanged = true;
        } catch (e) {
            inst._commands = inst._commands || [];
            const wasReachable = inst.reachable;
            inst.reachable = false;
            inst._lastError = 'connection lost (instance may have exited)';
            if (wasReachable !== false) anyReachableChanged = true;
        }
    }));
    if (anyReachableChanged) {
        updateDisconnectedUI();
    }

    // Check if welcome-panel state changed and re-render panels if so
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
        renderPanels();
    }

    // ── Auto-select first command when none selected ──
    // This handles two cases:
    //   1. Server started after page load: loadSnapshot already failed,
    //      _snapshotLoaded=true, so only loadCommands runs in the interval.
    //      Without auto-select, the panel stays empty ("No command selected").
    //   2. First load where loadSnapshot succeeded but selectedCmdId was
    //      somehow lost (edge case with concurrent renderPanels calls).
    if (hasAnyCommands && !state.selectedCmdId) {
        const panelObj = state.panels[0];
        if (panelObj && !panelObj.selectedCmdId) {
            // Find the first alive command (or first command if none alive)
            let targetInst = null, targetCmd = null;
            for (const inst of state.connections) {
                if (!inst._commands || inst._commands.length === 0) continue;
                const alive = inst._commands.find(c => c.alive);
                if (alive) { targetInst = inst; targetCmd = alive; break; }
                if (!targetCmd) { targetInst = inst; targetCmd = inst._commands[0]; }
            }
            if (targetInst && targetCmd) {
                panelObj.selectedInstUrl = targetInst.url;
                panelObj.selectedCmdId = targetCmd.id;
                state.selectedInstUrl = targetInst.url;
                state.selectedCmdId = targetCmd.id;
                state.bufferView = 'current';
                // Load VTTY content and start updates
                loadVttyHttpForPanel(panelObj.id, targetInst.url, targetCmd.id);
                startPanelUpdateMode(panelObj.id);
                updatePanelCommandInfo();
                updateTerminalDisconnectedOverlay();
                updateSidebarSelection();
            }
        }
    }

    // Build sidebar (reuses extracted _buildSidebar for consistency)
    _buildSidebar();
}

    window.lookupAndSelectCommand = lookupAndSelectCommand;
    window.showCommandPicker = showCommandPicker;
    window.pickCommand = pickCommand;
    window.closeCmdPicker = function() {
        releaseCurrentFocusTrap();
        const picker = document.getElementById('cmdPicker');
        if (picker) picker.remove();
    };
    window.navigateCommand = navigateCommand;
    window.navigatePrevCommand = navigatePrevCommand;
    window.navigateNextCommand = navigateNextCommand;
    window.loadCommands = loadCommands;

// ─── Command UI: panel header info, bottom bar label, panel resolution, terminal auto-fit ───
function updatePanelCommandInfo() {
    for (const panelObj of state.panels) {
        if (!panelObj.selectedInstUrl || !panelObj.selectedCmdId) continue;
        const panelEl = document.getElementById(panelObj.id);
        if (!panelEl) continue;
        let cmd = null;
        const inst = state.connections.find(i => i.url === panelObj.selectedInstUrl);
        if (inst && inst._commands) cmd = inst._commands.find(c => c.id === panelObj.selectedCmdId);
        const nameEl = panelEl.querySelector(':scope > .panel-header .cmd-fullname');
        const argsEl = panelEl.querySelector(':scope > .panel-header .cmd-args');
        const serverBadge = panelEl.querySelector(':scope > .panel-header .panel-server-badge');
        if (serverBadge) {
            const sLabel = _getServerLabel(inst, panelObj.selectedInstUrl);
            serverBadge.textContent = sLabel;
            serverBadge.classList.toggle('hidden', !sLabel);
        }
        if (nameEl && cmd) {
            const fullName = cmd.name || cmd.id;
            const displayTitle = panelObj.customTitle || fullName;
            nameEl.textContent = displayTitle;
            nameEl.title = fullName + (sLabel ? ' (' + sLabel + ')' : '') + (panelObj.customTitle ? ' (title: ' + panelObj.customTitle + ')' : '');
            if (argsEl) { const argsStr = (cmd.args || []).join(' '); argsEl.textContent = argsStr; argsEl.title = argsStr || ''; }
            const pauseBtn = panelEl.querySelector(`[id^="pauseRunBtn-"]`);
            if (pauseBtn) {
                const isAlive = cmd.alive !== false;
                const isFrozen = cmd.frozen === true;
                if (isAlive) { pauseBtn.classList.remove('hidden'); pauseBtn.textContent = isFrozen ? '\u25B6 Run' : '\u23F8 Pause'; pauseBtn.className = 'btn btn-xs' + (isFrozen ? ' btn-primary' : ''); }
                else pauseBtn.classList.add('hidden');
            }
            const restartBtn = panelEl.querySelector(`[id^="restartBtn-"]`);
            if (restartBtn) restartBtn.classList.remove('hidden');
            const resourceBadgeEl = panelEl.querySelector(`[id^="resourceBadge-"]`);
            if (resourceBadgeEl) {
                const res = state._resourceCache[cmd.id];
                if (state.showResources && res && (res.cpu_percent != null || res.memory_mb != null)) {
                    resourceBadgeEl.classList.remove('hidden');
                    resourceBadgeEl.textContent = (res.cpu_percent != null ? 'CPU ' + res.cpu_percent.toFixed(1) + '%' : '') +
                        (res.cpu_percent != null && res.memory_mb != null ? ' | ' : '') +
                        (res.memory_mb != null ? res.memory_mb.toFixed(1) + 'MB' : '');
                } else { resourceBadgeEl.textContent = ''; if (!state.showResources) resourceBadgeEl.classList.add('hidden'); }
            }
            const exitedBanner = panelEl.querySelector(`[id^="exitedBanner-"]`);
            if (exitedBanner) {
                const isAlive = cmd.alive !== false;
                const isFrozen = cmd.frozen === true;
                if (!isAlive && !isFrozen) {
                    const exitCode = cmd.exit_code != null ? cmd.exit_code : '?';
                    const exitClass = cmd.exit_code === 0 ? 'success' : 'failure';
                    exitedBanner.innerHTML = `<span class="exited-banner-icon">&#9632;</span> Command exited <span class="exit-badge ${exitClass}">exit ${exitCode}</span>`;
                    exitedBanner.classList.remove('hidden');
                } else exitedBanner.classList.add('hidden');
            }
        } else if (nameEl) {
            nameEl.textContent = panelObj.customTitle || '';
            if (argsEl) argsEl.textContent = '';
            const pauseBtn = panelEl.querySelector(`[id^="pauseRunBtn-"]`); if (pauseBtn) pauseBtn.classList.add('hidden');
            const restartBtn = panelEl.querySelector(`[id^="restartBtn-"]`); if (restartBtn) restartBtn.classList.add('hidden');
            const exitedBanner = panelEl.querySelector(`[id^="exitedBanner-"]`); if (exitedBanner) exitedBanner.classList.add('hidden');
        }
    }
    const focusedPanelObj = state.panels.find(p => p.id === state._focusedPanelId);
    if (focusedPanelObj && focusedPanelObj.selectedCmdId) {
        const inst = state.connections.find(i => i.url === focusedPanelObj.selectedInstUrl);
        const cmd = inst && inst._commands ? inst._commands.find(c => c.id === focusedPanelObj.selectedCmdId) : null;
        updateBottomBarLabel(cmd);
    } else updateBottomBarLabel(null);
    updateSharedToolbar();
}

function updateBottomBarLabel(cmd) {
    const el = document.getElementById('cmdLabel');
    if (!el) return;
    if (!cmd) { el.innerHTML = ''; return; }
    const fullName = cmd.name || cmd.id;
    const argsStr = (cmd.args || []).join(' ');
    const pid = cmd.pid || '';
    const runtime = cmd.runtime_secs != null ? formatRuntime(cmd.runtime_secs) : '';
    let html = `<span class="cmd-label-name">${escHtml(fullName)}</span>`;
    if (argsStr) html += `<span class="cmd-label-sep">|</span><span class="cmd-label-args">${escHtml(argsStr)}</span>`;
    if (pid) html += `<span class="cmd-label-sep">|</span><span class="cmd-label-pid">${escHtml(pid)}</span>`;
    if (runtime) html += `<span class="cmd-label-sep">|</span><span class="cmd-label-runtime">${escHtml(runtime)}</span>`;
    el.innerHTML = html;
    el.title = argsStr ? `${fullName} ${argsStr} (${pid})${runtime ? ' [' + runtime + ']' : ''}` : `${fullName} (${pid})${runtime ? ' [' + runtime + ']' : ''}`;
}

function autofitTerminalSize() {
    const panel = getSelectedPanel();
    if (!panel) { document.getElementById('autofitHint').textContent = 'No panel visible to measure'; return; }
    const vttyEl = panel.querySelector('.vtty-container');
    if (!vttyEl) { document.getElementById('autofitHint').textContent = 'No terminal container found'; return; }
    const rect = vttyEl.getBoundingClientRect();
    const charW = state.fontSize * 0.6;
    const charH = state.fontSize * 1.2;
    const cols = Math.max(20, Math.min(500, Math.floor(rect.width / charW)));
    const rows = Math.max(5, Math.min(200, Math.floor(rect.height / charH)));
    document.getElementById('spawnRows').value = rows;
    document.getElementById('spawnCols').value = cols;
    document.getElementById('autofitHint').textContent = `Panel is ${Math.floor(rect.width)}x${Math.floor(rect.height)}px → ${rows} rows × ${cols} cols`;
}

function getSelectedPanel() {
    if (state.panels.length === 0) return null;
    let panelObj;
    if (state._focusedPanelId) panelObj = state.panels.find(p => p.id === state._focusedPanelId);
    if (!panelObj && state.selectedInstUrl) panelObj = state.panels.find(p => p.selectedInstUrl === state.selectedInstUrl) || null;
    if (!panelObj) panelObj = state.panels[0];
    state.selectedInstUrl = panelObj.selectedInstUrl;
    state.selectedCmdId = panelObj.selectedCmdId;
    return document.getElementById(panelObj.id);
}

function getActivePanelId() {
    if (state._focusedPanelId) return state._focusedPanelId;
    if (state.panels.length > 0) return state.panels[0].id;
    return null;
}

    window.updatePanelCommandInfo = updatePanelCommandInfo;
    window.updateBottomBarLabel = updateBottomBarLabel;
    window.autofitTerminalSize = autofitTerminalSize;
    window.getSelectedPanel = getSelectedPanel;
    window.getActivePanelId = getActivePanelId;

// ─── Snapshot (Initial Load) ───
let _snapshotLoaded = false;
Object.defineProperty(window, '_snapshotLoaded', {
    get() { return _snapshotLoaded; },
    set(v) { _snapshotLoaded = v; },
    configurable: true,
});

async function loadSnapshot() {
    if (_snapshotLoaded) { loadCommands(); return; }
    _snapshotLoaded = true;
    const primaryInst = state.connections[0];
    if (!primaryInst) { loadCommands(); return; }
    try {
        const json = await api.getSnapshot(primaryInst.url);
        if (json.status !== 'ok' || !json.data) throw new Error('bad snapshot');
        const { commands, vtty, resources } = json.data;
        primaryInst._commands = commands || [];
        primaryInst.reachable = true;
        primaryInst._lastError = null;
        if (resources) {
            for (const [cmdId, resData] of Object.entries(resources)) state._resourceCache[cmdId] = resData;
        }
        const peerPromises = state.connections.slice(1).map(async (inst) => {
            try {
                const j = await api.getCommands(inst.url);
                inst._commands = j.status === 'ok' ? j.data : [];
                inst.reachable = true; inst._lastError = null;
            } catch (e) { inst._commands = inst._commands || []; inst.reachable = false; inst._lastError = 'connection lost'; }
        });
        const peersDone = Promise.all(peerPromises).then(() => { updateDisconnectedUI(); });
        const hasAnyCommands = commands && commands.length > 0;
        const firstCmd = hasAnyCommands ? (commands.find(c => c.alive) || commands[0]) : null;
        const shouldShowWelcome = (!hasAnyCommands && !state.selectedCmdId && !state.serverReachable);
        if (shouldShowWelcome !== _showingWelcome) { _showingWelcome = shouldShowWelcome; renderPanels(); }
        if (vtty && vtty.html !== undefined && firstCmd) {
            state.selectedInstUrl = primaryInst.url;
            state.selectedCmdId = firstCmd.id;
            state.bufferView = 'current';
            const panelObj = state.panels.find(p => p.id === (state._focusedPanelId || state.panels[0].id));
            if (panelObj) { panelObj.selectedInstUrl = primaryInst.url; panelObj.selectedCmdId = firstCmd.id; }
            getSelectedPanel();
            if (vtty.generation !== undefined) state._lastGeneration[firstCmd.id] = vtty.generation;
            const panelEl = document.getElementById(panelObj ? panelObj.id : (state._focusedPanelId || (state.panels[0] || {}).id));
            if (panelEl) {
                const vttyEl = panelEl.querySelector('.vtty-container');
                const pre = vttyEl ? vttyEl.querySelector('pre') : null;
                if (pre) {
                    pre.innerHTML = vtty.html;
                    if (state._level3Enabled && vtty.dimensions) buildCellGrid(firstCmd.id, pre, vtty.dimensions.rows, vtty.dimensions.cols);
                    updateVttyMetadataFromHttp(vtty, panelEl, panelObj, 0);
                }
            }
            updatePanelCommandInfo();
            updateTerminalDisconnectedOverlay();
            startPanelUpdateMode(state._focusedPanelId);
        } else { _showingWelcome = shouldShowWelcome; updateDisconnectedUI(); }
        await peersDone;
        _buildSidebar();
    } catch (e) {
        primaryInst._commands = primaryInst._commands || [];
        primaryInst.reachable = false;
        primaryInst._lastError = 'connection lost';
        updateDisconnectedUI();
        loadCommands();
    }
}
    window.loadSnapshot = loadSnapshot;
})();
