// ─── Command UI: panel header info, bottom bar label, panel resolution, terminal auto-fit ───
(function() {
    'use strict';

// Update the panel header with the selected command's full name and args.

function updatePanelCommandInfo() {
    // Update ALL panels that have a selected command — not just the focused one.
    // This ensures headers stay accurate when commands exit or servers die.
    for (const panelObj of state.panels) {
        if (!panelObj.selectedInstUrl || !panelObj.selectedCmdId) continue;
        const panelEl = document.getElementById(panelObj.id);
        if (!panelEl) continue;

        let cmd = null;
        const inst = state.connections.find(i => i.url === panelObj.selectedInstUrl);
        if (inst && inst._commands) {
            cmd = inst._commands.find(c => c.id === panelObj.selectedCmdId);
        }

        const nameEl = panelEl.querySelector(':scope > .panel-header .cmd-fullname');
        const argsEl = panelEl.querySelector(':scope > .panel-header .cmd-args');
        const serverBadge = panelEl.querySelector(':scope > .panel-header .panel-server-badge');

        // Update server badge
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
            if (argsEl) {
                const argsStr = (cmd.args || []).join(' ');
                argsEl.textContent = argsStr;
                argsEl.title = argsStr || '';
            }

            // Update per-panel pause button
            const pauseBtn = panelEl.querySelector(`[id^="pauseRunBtn-"]`);
            if (pauseBtn) {
                const isAlive = cmd.alive !== false;
                const isFrozen = cmd.frozen === true;
                if (isAlive) {
                    pauseBtn.classList.remove('hidden');
                    pauseBtn.textContent = isFrozen ? '\u25B6 Run' : '\u23F8 Pause';
                    pauseBtn.className = 'btn btn-xs' + (isFrozen ? ' btn-primary' : '');
                } else {
                    pauseBtn.classList.add('hidden');
                }
            }

            // Show/hide restart button
            const restartBtn = panelEl.querySelector(`[id^="restartBtn-"]`);
            if (restartBtn) {
                restartBtn.classList.remove('hidden');
            }

            // Update resource badge
            const resourceBadgeEl = panelEl.querySelector(`[id^="resourceBadge-"]`);
            if (resourceBadgeEl) {
                const res = state._resourceCache[cmd.id];
                if (state.showResources && res && (res.cpu_percent != null || res.memory_mb != null)) {
                    resourceBadgeEl.classList.remove('hidden');
                    resourceBadgeEl.textContent = (res.cpu_percent != null ? 'CPU ' + res.cpu_percent.toFixed(1) + '%' : '') +
                        (res.cpu_percent != null && res.memory_mb != null ? ' | ' : '') +
                        (res.memory_mb != null ? res.memory_mb.toFixed(1) + 'MB' : '');
                } else {
                    resourceBadgeEl.textContent = '';
                    if (!state.showResources) resourceBadgeEl.classList.add('hidden');
                }
            }

            // Update exited banner
            const exitedBanner = panelEl.querySelector(`[id^="exitedBanner-"]`);
            if (exitedBanner) {
                const isAlive = cmd.alive !== false;
                const isFrozen = cmd.frozen === true;
                if (!isAlive && !isFrozen) {
                    const exitCode = cmd.exit_code != null ? cmd.exit_code : '?';
                    const exitClass = cmd.exit_code === 0 ? 'success' : 'failure';
                    exitedBanner.innerHTML = `<span class="exited-banner-icon">&#9632;</span> Command exited <span class="exit-badge ${exitClass}">exit ${exitCode}</span>`;
                    exitedBanner.classList.remove('hidden');
                } else {
                    exitedBanner.classList.add('hidden');
                }
            }
        } else if (nameEl) {
            // No command found (killed, gone, etc.) — clear header
            nameEl.textContent = panelObj.customTitle || '';
            if (argsEl) argsEl.textContent = '';
            const pauseBtn = panelEl.querySelector(`[id^="pauseRunBtn-"]`);
            if (pauseBtn) pauseBtn.classList.add('hidden');
            const restartBtn = panelEl.querySelector(`[id^="restartBtn-"]`);
            if (restartBtn) restartBtn.classList.add('hidden');
            const exitedBanner = panelEl.querySelector(`[id^="exitedBanner-"]`);
            if (exitedBanner) exitedBanner.classList.add('hidden');
        }
    }

    // Also update the bottom bar for the focused panel
    const focusedPanelObj = state.panels.find(p => p.id === state._focusedPanelId);
    if (focusedPanelObj && focusedPanelObj.selectedCmdId) {
        const inst = state.connections.find(i => i.url === focusedPanelObj.selectedInstUrl);
        const cmd = inst && inst._commands ? inst._commands.find(c => c.id === focusedPanelObj.selectedCmdId) : null;
        updateBottomBarLabel(cmd);
    } else {
        updateBottomBarLabel(null);
    }

    // Update shared toolbar
    updateSharedToolbar();
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
