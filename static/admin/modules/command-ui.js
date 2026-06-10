// ─── Command UI: panel header info, bottom bar label, panel resolution, terminal auto-fit ───
(function() {
    'use strict';

// Update the panel header with the selected command's full name and args.

function updatePanelCommandInfo() {
    if (!state.selectedInstUrl || !state.selectedCmdId) return;
    // Find the command data from the loaded instance commands
    let cmd = null;
    for (const inst of state.connections) {
        if (inst.url === state.selectedInstUrl && inst._commands) {
            cmd = inst._commands.find(c => c.id === state.selectedCmdId);
            break;
        }
    }
    const panel = getSelectedPanel();
    if (!panel) return;
    const nameEl = panel.querySelector('.cmd-fullname');
    const argsEl = panel.querySelector('.cmd-args');
    if (nameEl && cmd) {
        const panelObj = state.panels.find(p => p.id === panel.id);
        const fullName = cmd.name || cmd.id;
        // Append server label (name or host:port) after the command name
        const inst = panelObj && panelObj.selectedInstUrl ? state.connections.find(i => i.url === panelObj.selectedInstUrl) : null;
        const serverLabel = inst ? inst.label || inst.url.replace(/^https?:\/\//, '') : '';
        const titleWithServer = serverLabel ? fullName + ' - ' + serverLabel : fullName;
        // Show custom title if set, otherwise command name + server
        const displayTitle = (panelObj && panelObj.customTitle) ? panelObj.customTitle : titleWithServer;
        nameEl.textContent = displayTitle;
        nameEl.title = fullName + (serverLabel ? ' (' + serverLabel + ')' : '') + (panelObj && panelObj.customTitle ? ' (title: ' + panelObj.customTitle + ')' : '');
        if (argsEl) {
            const argsStr = (cmd.args || []).join(' ');
            argsEl.textContent = argsStr;
            argsEl.title = argsStr || '';
        }
        // Update bottom bar command label
        updateBottomBarLabel(cmd);

        // Update per-panel pause button
        const pauseBtn = panel.querySelector(`[id^="pauseRunBtn-"]`);
        if (pauseBtn) {
            const isAlive = cmd.alive !== false;
            const isFrozen = cmd.frozen === true;
            if (isAlive) {
                pauseBtn.style.display = '';
                pauseBtn.textContent = isFrozen ? '\u25B6 Run' : '\u23F8 Pause';
                pauseBtn.className = 'btn btn-xs' + (isFrozen ? ' btn-primary' : '');
            } else {
                pauseBtn.style.display = 'none';
            }
        }

        // Show/hide restart button next to command name
        const restartBtn = panel.querySelector(`[id^="restartBtn-"]`);
        if (restartBtn) {
            restartBtn.style.display = '';
        }

        // Update resource badge in panel header
        const resourceBadgeEl = panel.querySelector(`[id^="resourceBadge-"]`);
        if (resourceBadgeEl) {
            const res = state._resourceCache[cmd.id];
            if (state.showResources && res && (res.cpu_percent != null || res.memory_mb != null)) {
                resourceBadgeEl.style.display = '';
                resourceBadgeEl.textContent = (res.cpu_percent != null ? 'CPU ' + res.cpu_percent.toFixed(1) + '%' : '') +
                    (res.cpu_percent != null && res.memory_mb != null ? ' | ' : '') +
                    (res.memory_mb != null ? res.memory_mb.toFixed(1) + 'MB' : '');
            } else {
                resourceBadgeEl.textContent = '';
                if (!state.showResources) resourceBadgeEl.style.display = 'none';
            }
        }

        // Update exited banner on VTTY container
        const exitedBanner = panel.querySelector(`[id^="exitedBanner-"]`);
        if (exitedBanner) {
            const isAlive = cmd.alive !== false;
            const isFrozen = cmd.frozen === true;
            if (!isAlive && !isFrozen) {
                const exitCode = cmd.exit_code != null ? cmd.exit_code : '?';
                const exitClass = cmd.exit_code === 0 ? 'success' : 'failure';
                exitedBanner.innerHTML = `<span class="exited-banner-icon">&#9632;</span> Command exited <span class="exit-badge ${exitClass}">exit ${exitCode}</span>`;
                exitedBanner.style.display = 'flex';
            } else {
                exitedBanner.style.display = 'none';
            }
        }
        // Update shared toolbar
        updateSharedToolbar();
    } else if (nameEl) {
        // No command selected — show custom title if set
        const panelObj = state.panels.find(p => p.id === panel.id);
        nameEl.textContent = (panelObj && panelObj.customTitle) ? panelObj.customTitle : '';
        if (argsEl) argsEl.textContent = '';
        updateBottomBarLabel(null);
        // Hide pause button
        const pauseBtn = panel.querySelector(`[id^="pauseRunBtn-"]`);
        if (pauseBtn) pauseBtn.style.display = 'none';
        // Hide restart button
        const restartBtn = panel.querySelector(`[id^="restartBtn-"]`);
        if (restartBtn) restartBtn.style.display = 'none';
        // Hide exited banner
        const exitedBanner = panel.querySelector(`[id^="exitedBanner-"]`);
        if (exitedBanner) exitedBanner.style.display = 'none';
    }
}

// ─── Bottom bar: command label ───
function updateBottomBarLabel(cmd) {
    const el = document.getElementById('cmdLabel');
    if (!el) return;
    if (!cmd) {
        el.innerHTML = '';
        return;
    }
    const fullName = cmd.name || cmd.id;
    const argsStr = (cmd.args || []).join(' ');
    const pid = cmd.pid || '';
    const runtime = cmd.runtime_secs != null ? formatRuntime(cmd.runtime_secs) : '';
    let html = `<span class="cmd-label-name">${escHtml(fullName)}</span>`;
    if (argsStr) {
        html += `<span class="cmd-label-sep">|</span><span class="cmd-label-args">${escHtml(argsStr)}</span>`;
    }
    if (pid) {
        html += `<span class="cmd-label-sep">|</span><span class="cmd-label-pid">${escHtml(pid)}</span>`;
    }
    if (runtime) {
        html += `<span class="cmd-label-sep">|</span><span class="cmd-label-runtime">${escHtml(runtime)}</span>`;
    }
    el.innerHTML = html;
    el.title = argsStr ? `${fullName} ${argsStr} (${pid})${runtime ? ' [' + runtime + ']' : ''}` : `${fullName} (${pid})${runtime ? ' [' + runtime + ']' : ''}`;
}

// ─── Spawn: auto-fit terminal size ───
function autofitTerminalSize() {
    // Calculate optimal terminal size from the current panel container
    const panel = getSelectedPanel();
    if (!panel) {
        document.getElementById('autofitHint').textContent = 'No panel visible to measure';
        return;
    }
    const vttyEl = panel.querySelector('.vtty-container');
    if (!vttyEl) {
        document.getElementById('autofitHint').textContent = 'No terminal container found';
        return;
    }
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
    // Prefer the focused panel
    if (state._focusedPanelId) {
        panelObj = state.panels.find(p => p.id === state._focusedPanelId);
    }
    if (!panelObj && state.selectedInstUrl) {
        panelObj = state.panels.find(p => p.selectedInstUrl === state.selectedInstUrl) || null;
    }
    if (!panelObj) {
        panelObj = state.panels[0];
    }
    // Sync global state from the focused panel's per-panel selection
    state.selectedInstUrl = panelObj.selectedInstUrl;
    state.selectedCmdId = panelObj.selectedCmdId;
    return document.getElementById(panelObj.id);
}

/// Return the focused panel's ID (or first panel's ID if none focused).
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
})();
