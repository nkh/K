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
})();
