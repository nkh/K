// ─── Commands ───
(function() {
    'use strict';
// ── Command-name URL lookup ──
async function lookupAndSelectCommand(name) {
    try {
        const base = getBaseUrl();
        const res = await fetch(apiUrl('/api/commands/lookup/' + encodeURIComponent(name)), {
            headers: authHeaders()
        });
        const json = await res.json();
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
            <button class="btn" onclick="releaseCurrentFocusTrap();document.getElementById('cmdPicker').remove()">Cancel</button>
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

// ─── Commands ───

/// Fast initial load: fetch commands, VTTY HTML, and resources in a SINGLE
/// request from the primary instance.  This replaces the old flow of
/// loadCommands → _prefetchVttyHtml → pollResources (3+ serial round trips)
/// with just 1 round trip.
///
/// After the snapshot is processed, peer instances are fetched in parallel.
/// Subsequent refreshes use the lighter loadCommands() which only fetches
/// the commands list (no VTTY HTML, no resources).
let _snapshotLoaded = false;

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

async function loadCommands() {
    // Load commands from all instances in PARALLEL and track reachability.
    let anyReachableChanged = false;
    await Promise.all(state.connections.map(async (inst) => {
        try {
            const res = await fetch(apiUrl('/api/commands', inst), { headers: authHeadersForInstance(inst) });
            if (!res.ok) throw new Error('HTTP ' + res.status);
            const json = await res.json();
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

/// Cache the terminal display DOM for the currently selected command.
/// Called before switching to a different command.  Moves the <pre> children
/// into a detached DocumentFragment so they can be re-attached instantly on
/// switch-back, avoiding a full HTML fetch when the command hasn't changed.
function _cacheTerminalForSwitch() {
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    const cmdId = state.selectedCmdId;
    if (!pre || !cmdId) return;

    // Detach all children into a DocumentFragment (preserves DOM nodes)
    const frag = document.createDocumentFragment();
    while (pre.firstChild) {
        frag.appendChild(pre.firstChild);
    }
    state._cachedDomPre[cmdId] = frag;
    // Save scroll position for this command
    if (vttyEl) {
        state._cachedScrollPos[cmdId] = vttyEl.scrollTop;
    }
    // Keep _cellGrids and _lastGeneration — they are still valid for the cached DOM.
}

/// Restore a previously cached DOM tree into the <pre> element for instant display.
/// Called from selectCommand() when switching to a command that was viewed before.
/// The cached DOM is moved (not cloned) back into the document, and scroll position
/// is restored.  After this, loadVttyHttp() checks generation — if unchanged, the
/// cached DOM stays; if changed, the full HTML fetch replaces it.
function _restoreCachedDom(cmdId) {
    const frag = state._cachedDomPre[cmdId];
    if (!frag) return;
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;
    // Move the cached DocumentFragment into the <pre> (O(1), no parsing)
    pre.appendChild(frag);
    delete state._cachedDomPre[cmdId];
    // Restore scroll position
    const savedScroll = state._cachedScrollPos[cmdId];
    if (savedScroll !== undefined) {
        vttyEl.scrollTop = savedScroll;
        delete state._cachedScrollPos[cmdId];
    }
}

/// Lightweight DOM-only update: toggle the .selected class on sidebar items
/// without re-fetching /api/commands. Used by selectCommand() to avoid
/// a redundant HTTP roundtrip that would delay the initial VTTY load.
function updateSidebarSelection() {
    document.querySelectorAll('#commandList .cmd-item').forEach(el => {
        const matchInst = el.dataset.instUrl === state.selectedInstUrl;
        const matchCmd = el.dataset.cmdId === state.selectedCmdId;
        el.classList.toggle('selected', matchInst && matchCmd);
    });
}

/// Push current command selection to panel's history before switching.
/// Truncates forward history (like browser back/forward).
function _pushPanelHistory(panelObj) {
    if (!panelObj || !panelObj.selectedCmdId) return;
    // If we're not at the end of history, truncate forward entries
    if (panelObj.cmdHistoryIdx < panelObj.cmdHistory.length - 1) {
        panelObj.cmdHistory = panelObj.cmdHistory.slice(0, panelObj.cmdHistoryIdx + 1);
    }
    // Don't push duplicate of current
    const last = panelObj.cmdHistory[panelObj.cmdHistory.length - 1];
    if (last && last.instUrl === panelObj.selectedInstUrl && last.cmdId === panelObj.selectedCmdId) return;
    panelObj.cmdHistory.push({
        instUrl: panelObj.selectedInstUrl,
        cmdId: panelObj.selectedCmdId,
    });
    panelObj.cmdHistoryIdx = panelObj.cmdHistory.length - 1;
    // Cap history at 50 entries per panel
    if (panelObj.cmdHistory.length > 50) {
        panelObj.cmdHistory.shift();
        panelObj.cmdHistoryIdx--;
    }
}

/// Update back/forward button visibility for a panel.
function _updatePanelHistoryBtns(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    const backBtn = document.getElementById('histBack-' + panelId);
    const fwdBtn = document.getElementById('histFwd-' + panelId);
    if (backBtn) backBtn.style.display = (panelObj && panelObj.cmdHistoryIdx > 0) ? '' : 'none';
    if (fwdBtn) fwdBtn.style.display = (panelObj && panelObj.cmdHistoryIdx < panelObj.cmdHistory.length - 1) ? '' : 'none';
}

/// Navigate back in panel's command history.
function panelHistoryBack(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || panelObj.cmdHistoryIdx <= 0) return;
    panelObj.cmdHistoryIdx--;
    const entry = panelObj.cmdHistory[panelObj.cmdHistoryIdx];
    // Apply selection without pushing to history (we're navigating)
    _selectCommandForPanel(panelObj, entry.instUrl, entry.cmdId);
    _updatePanelHistoryBtns(panelId);
}

/// Navigate forward in panel's command history.
function panelHistoryForward(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || panelObj.cmdHistoryIdx >= panelObj.cmdHistory.length - 1) return;
    panelObj.cmdHistoryIdx++;
    const entry = panelObj.cmdHistory[panelObj.cmdHistoryIdx];
    _selectCommandForPanel(panelObj, entry.instUrl, entry.cmdId);
    _updatePanelHistoryBtns(panelId);
}


/// Internal: switch a panel to a command without recording history.
function _selectCommandForPanel(panelObj, instUrl, cmdId) {
    disconnectPanelWs(panelObj.id);
    panelObj.selectedInstUrl = instUrl;
    panelObj.selectedCmdId = cmdId;
    focusPanel(panelObj.id);
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    state._pendingVttyData = null;
    state._pendingVttyDirty = false;
    state.bufferView = 'current';
    _restoreCachedDom(cmdId);
    const globalBufferSel = document.getElementById('bufferSelect');
    if (globalBufferSel) globalBufferSel.value = 'current';
    updatePanelCommandInfo();
    updateTerminalDisconnectedOverlay();
    updateSidebarSelection();
    loadVttyHttpForPanel(panelObj.id, instUrl, cmdId);
    startPanelUpdateMode(panelObj.id);
}

function selectCommand(instUrl, cmdId, name) {
    // Determine which panel to apply the selection to.
    // If the user clicked in a specific panel, use that; otherwise use the focused panel.
    let panelObj = state.panels.find(p => p.id === state._focusedPanelId);
    if (!panelObj) panelObj = state.panels[0];
    if (!panelObj) return;

    // Check if panel is split and the active side is secondary
    const isSecondary = panelObj.split && panelObj.split.activeSide === 'secondary';

    // Record current command in history before switching
    _pushPanelHistory(panelObj);

    // Ensure this panel is visually focused
    focusPanel(panelObj.id);

    if (isSecondary) {
        // ── Secondary pane command selection ──
        // Disconnect existing secondary WS
        _disconnectSecondaryWs(panelObj);
        if (panelObj.split.secondaryPollTimer) {
            clearInterval(panelObj.split.secondaryPollTimer);
            panelObj.split.secondaryPollTimer = null;
        }

        // Update secondary pane selection
        panelObj.split.secondaryInstUrl = instUrl;
        panelObj.split.secondaryCmdId = cmdId;
        panelObj.split.secondaryScrollbackOffset = 0;

        // Also sync global state so bottom bar etc. work
        state.selectedInstUrl = instUrl;
        state.selectedCmdId = cmdId;

        // Clear any buffered update
        state._pendingVttyData = null;
        state._pendingVttyDirty = false;
        state.bufferView = 'current';

        // Fetch VTTY content for secondary pane
        _loadSecondaryVttyHttp(panelObj);

        // Start secondary WS/poll
        if (state.updateMode === 'push') {
            _connectSecondaryWs(panelObj);
        } else {
            panelObj.split.secondaryPollTimer = setInterval(() => {
                if (panelObj.split && panelObj.split.secondaryCmdId) {
                    _loadSecondaryVttyHttp(panelObj);
                }
            }, state.pollInterval);
        }

        // Update panel header to show secondary command info
        _updateSplitPanelHeader(panelObj);
        updateSidebarSelection();
        return;
    }

    // ── Primary pane command selection (existing behavior) ──
    // Cache the current command's terminal DOM before switching away.
    disconnectPanelWs(panelObj.id);
    _cacheTerminalForSwitch();

    // Update per-panel selection
    panelObj.selectedInstUrl = instUrl;
    panelObj.selectedCmdId = cmdId;
    // Sync global state
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    // Clear any buffered update — we fetch fresh data below
    state._pendingVttyData = null;
    state._pendingVttyDirty = false;
    // Restore cached DOM from previous visit if available (instant display).
    // Then loadVttyHttp will check generation — if unchanged, the cached
    // DOM is kept; if changed, a full HTML fetch replaces it.
    _restoreCachedDom(cmdId);
    state.bufferView = 'current';
    const globalBufferSel = document.getElementById('bufferSelect');
    if (globalBufferSel) globalBufferSel.value = 'current';
    // Reset panel-scoped buffer selects too
    state.panels.forEach(p => {
        const sel = document.getElementById('bufferSelect-' + p.id);
        if (sel) sel.value = 'current';
    });

    // Restore scrollback offset from sessionStorage for the new command
    const savedOffset = sessionStorage.getItem('vrw_scrollback_' + cmdId);
    const restoredOffset = savedOffset !== null ? parseInt(savedOffset, 10) : 0;
    state.panels.forEach(p => p.scrollbackOffset = restoredOffset);

    updatePanelCommandInfo();
    updateTerminalDisconnectedOverlay();
    updateSidebarSelection();
    // Fetch VTTY content — will skip DOM write if generation unchanged
    loadVttyHttpForPanel(panelObj.id, instUrl, cmdId);
    // Start per-panel WS for push mode (or poll)
    startPanelUpdateMode(panelObj.id);
    // Update history button visibility
    _updatePanelHistoryBtns(panelObj.id);
}

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
        // Show custom title if set, otherwise command name
        const displayTitle = (panelObj && panelObj.customTitle) ? panelObj.customTitle : fullName;
        nameEl.textContent = displayTitle;
        nameEl.title = fullName + (panelObj && panelObj.customTitle ? ' (title: ' + panelObj.customTitle + ')' : '');
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

// ─── Pause/Run Toggle ───
async function togglePauseRun() {
    if (!state.selectedCmdId) return;
    const inst = state.connections.find(i => i.url === state.selectedInstUrl);
    const cmd = inst && inst._commands ? inst._commands.find(c => c.id === state.selectedCmdId) : null;
    const isFrozen = cmd && cmd.frozen;
    const endpoint = isFrozen ? 'thaw' : 'freeze';
    try {
        await fetch(apiUrl(`/api/commands/${state.selectedCmdId}/${endpoint}`, { url: state.selectedInstUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: state.selectedInstUrl }),
            body: JSON.stringify({}),
        });
        loadCommands();
    } catch (e) { /* ignore */ }
}

async function togglePauseRunPanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj || !panelObj.selectedInstUrl || !panelObj.selectedCmdId) return;
    const inst = state.connections.find(i => i.url === panelObj.selectedInstUrl);
    if (!inst || !inst._commands) return;
    const cmdId = panelObj.selectedCmdId;
    const cmd = inst._commands.find(c => c.id === cmdId);
    const isFrozen = cmd && cmd.frozen;
    const endpoint = isFrozen ? 'thaw' : 'freeze';
    try {
        await fetch(apiUrl(`/api/commands/${cmdId}/${endpoint}`, { url: panelObj.selectedInstUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: panelObj.selectedInstUrl }),
            body: JSON.stringify({}),
        });
        loadCommands();
    } catch (e) { /* ignore */ }
}


// ─── VTTY Update Modes (Push / Poll) ───
// The web UI supports two modes for detecting VTTY buffer changes:
//
// PUSH MODE (default): The server monitors the buffer and sends lightweight
//   "vtty_dirty" signals over the WebSocket whenever the buffer changes.
//   On receiving a dirty signal, the client does a debounced HTTP fetch to
//   get the latest HTML.  This is the most efficient mode.
//
// POLL MODE: The client periodically calls GET /api/commands/:id/vtty/changed
//   to ask "has the buffer changed?".  If yes, it fetches the full HTML.
//   This mode is useful when WebSocket connections are unreliable.

/// Fetch server-side web config (update_mode, poll defaults) from /api/info.
/// Also tracks whether the server is reachable at all.
async function fetchServerConfig() {
    try {
        const res = await fetch(apiUrl('/api/info'), { headers: authHeaders() });
        const json = await res.json();
        const wasReachable = state.serverReachable;
        state.serverReachable = !!json.status;
        // When server transitions from unreachable → reachable, immediately
        // load commands so the auto-select logic in loadCommands fires.
        // Without this, the next loadCommands interval tick (up to 1s delay)
        // is the earliest the panel gets a command.
        if (!wasReachable && state.serverReachable) {
            loadCommands();
        }
        // Re-render panels if reachability changed (e.g. "not running" -> welcome)
        if (wasReachable !== state.serverReachable) {
            renderPanels();
            updateSidebarTabsVisibility();
        }
        if (json.status === 'ok' && json.data && json.data.web) {
            state.serverUpdateMode = json.data.web.update_mode;
            state.serverPollMs = json.data.web.default_poll_ms;
            state.serverDirtyMs = json.data.web.dirty_check_ms;
            // If no user preference is set, use the server default
            if (!localStorage.getItem('vrw_update_mode')) {
                state.updateMode = state.serverUpdateMode || 'push';
            }
            if (!localStorage.getItem('vrw_poll_interval')) {
                state.pollInterval = state.serverPollMs || 500;
            }
        }
        if (json.status === 'ok' && json.data && json.data.vtty) {
            state.serverScreenshotFontSize = json.data.vtty.screenshot_font_size || 12;
            state.serverScreenshotFontName = json.data.vtty.screenshot_font_name || 'monospace';
        }
    } catch (e) {
        const wasReachable = state.serverReachable;
        state.serverReachable = false;
        if (wasReachable !== state.serverReachable) {
            renderPanels();
            updateSidebarTabsVisibility();
        }
    }
}

/// Apply the current updateMode to the UI controls.
function applyUpdateModeUI() {
    document.getElementById('updateMode').value = state.updateMode;
    document.getElementById('pollInterval').value = state.pollInterval;
    document.getElementById('pollIntervalWrap').style.display = state.updateMode === 'poll' ? '' : 'none';
}

/// Switch update mode (called from the dropdown).
function switchUpdateMode(mode) {
    state.updateMode = mode;
    localStorage.setItem('vrw_update_mode', mode);
    applyUpdateModeUI();
    // Stop existing update mechanism and restart with new mode
    stopUpdateMode();
    if (state.selectedInstUrl && state.selectedCmdId) {
        startUpdateMode();
    }
}

/// Apply the poll interval from the input.
function applyPollInterval() {
    const val = parseInt(document.getElementById('pollInterval').value) || 500;
    state.pollInterval = Math.max(50, Math.min(5000, val));
    localStorage.setItem('vrw_poll_interval', state.pollInterval.toString());
    document.getElementById('pollInterval').value = state.pollInterval;
    // If currently polling, restart the timer with new interval
    if (state.updateMode === 'poll' && state._pollTimer) {
        stopPoll();
        startPoll();
    }
}
// ─── Certificates ───
async function loadCertificates() {
    for (const inst of state.connections) {
        try {
            const res = await fetch(apiUrl('/api/certificates', inst), { headers: authHeadersForInstance(inst) });
            const json = await res.json();
            inst._certs = json.status === 'ok' ? json.data : [];
        } catch (e) {
            inst._certs = [];
        }
    }

    const container = document.getElementById('certList');
    let html = '';
    for (const inst of state.connections) {
        html += `<div style="font-size:0.7rem;color:var(--text-muted);padding:0.3rem 0;margin-top:0.3rem;">${escHtml(inst.label)}</div>`;
        const certs = inst._certs || [];
        if (certs.length === 0) {
            html += '<div style="padding:0.3rem;font-size:0.8rem;color:var(--text-muted);">No certificates</div>';
        }
        for (const cert of certs) {
            html += `<div style="padding:0.3rem 0.5rem;border-bottom:1px solid var(--border);font-size:0.8rem;">
                <span class="cert-badge">${escHtml(cert.name)}</span>
                <span style="color:var(--text-muted);font-size:0.7rem;margin-left:0.5rem;font-family:var(--font-mono);">${escHtml(cert.token_preview || '')}...</span>
            </div>`;
        }
    }
    container.innerHTML = html;

    // Update spawn cert dropdown
    updateCertDropdown();
}

function updateCertDropdown() {
    const select = document.getElementById('spawnCert');
    let html = '<option value="">None</option>';
    for (const inst of state.connections) {
        for (const cert of (inst._certs || [])) {
            html += `<option value="${escHtml(cert.name)}">${escHtml(inst.label)}: ${escHtml(cert.name)}</option>`;
        }
    }
    select.innerHTML = html;
}

// Track the user's explicit spawn instance choice separately from
// state.selectedInstUrl.  Without this, updateInstanceDropdown() would
// reset the dropdown to whatever panel is focused, overwriting the user's
// choice every time the sidebar rebuilds.  Once set (either by the user
// manually changing the dropdown or by spawning a command), it persists
// for the lifetime of the session — it is never silently overridden by
// the focused panel's instance.
let _userSpawnInstUrl = null;

function updateInstanceDropdown() {
    const select = document.getElementById('spawnInstance');
    const current = select.value;
    let html = '';
    for (const inst of state.connections) {
        html += `<option value="${escHtml(inst.url)}">${escHtml(inst.label)} (${escHtml(inst.url.replace(/^https?:\/\//, ''))})</option>`;
    }
    select.innerHTML = html;

    // The spawn instance dropdown is fully decoupled from the focused panel.
    // It only changes when the user explicitly selects a different instance.
    // Priority:
    // 1. The user's explicit spawn-instance choice (set when the user
    //    manually changes the dropdown or when a command is spawned).
    // 2. The previous dropdown value, if it still exists in the list.
    // 3. Fall back to the first connection (never to the focused panel,
    //    since that would re-introduce the coupling bug).
    if (_userSpawnInstUrl && state.connections.some(i => i.url === _userSpawnInstUrl)) {
        select.value = _userSpawnInstUrl;
    } else if (current && state.connections.some(i => i.url === current)) {
        select.value = current;
        _userSpawnInstUrl = current;  // remember the restored value
    } else if (state.connections.length > 0) {
        select.value = state.connections[0].url;
        _userSpawnInstUrl = state.connections[0].url;
    }
}

// ─── Server Connection Management ───
// Connections are separate from panels. Adding a connection makes its
// commands available in the sidebar. Removing a connection removes its
// commands from the sidebar but does NOT close any panels (they keep
// their last VTTY state).
function addConnection(url, label, token) {
    // Idempotent: if connection already exists, return it unchanged.
    // This prevents accidental overwrites of user-set labels/tokens.
    const existing = state.connections.find(c => c.url === url);
    if (existing) {
        return existing;
    }
    const conn = { url, label: label || url, token: token || '', reachable: undefined, _lastError: null, _commands: null, _certs: null };
    state.connections.push(conn);
    return conn;
}

function removeConnection(url) {
    state.connections = state.connections.filter(c => c.url !== url);
    _lastCommandState = ''; // force sidebar rebuild
    loadCommands();
    updateDisconnectedUI();
}

function disconnectServer(url) {
    const inst = state.connections.find(c => c.url === url);
    if (!inst) return;
    // Check if any panels are connected to commands on this server
    const activePanels = state.panels.filter(p => p.selectedInstUrl === url && p.selectedCmdId);
    if (activePanels.length > 0) {
        if (!confirm(`Disconnect from "${inst.label}"? ${activePanels.length} panel(s) showing commands from this server will keep their last state.`)) return;
    } else {
        if (!confirm(`Disconnect from "${inst.label}"?`)) return;
    }
    // Disconnect WS and poll for panels on this server
    for (const panel of activePanels) {
        disconnectPanelWs(panel.id);
        stopPanelPoll(panel.id);
    }
    removeConnection(url);
}

// ─── Add Server Modal (sidebar only, no panel) ───
function showAddServerModal() {
    const modal = document.getElementById('addServerModal');
    modal.style.display = '';
    document.getElementById('addServerUrl').value = 'http://localhost:9090';
    document.getElementById('addServerLabel').value = '';
    document.getElementById('addServerToken').value = '';
    document.getElementById('addServerOpenPane').checked = true;
    const modalInner = modal.querySelector('.modal');
    if (modalInner) trapFocus(modalInner);
    document.getElementById('addServerUrl').focus();
}

function closeAddServerModal() {
    releaseCurrentFocusTrap();
    document.getElementById('addServerModal').style.display = 'none';
}

async function confirmAddServer() {
    const url = document.getElementById('addServerUrl').value.trim();
    if (!url) return;
    const token = document.getElementById('addServerToken').value.trim();
    let label = document.getElementById('addServerLabel').value.trim();
    if (!label) {
        try { label = new URL(url).host; } catch (e) { label = url; }
    }
    const openPane = document.getElementById('addServerOpenPane').checked;
    const isNew = !state.connections.some(c => c.url === url);
    const conn = addConnection(url, label, token);
    closeAddServerModal();
    loadCommands();
    loadCertificates();
    fetchServerTemplates();

    if (openPane) {
        // Wait for commands to load, then open a pane connected to the server's
        // main command (first spawned, i.e. spawn_order 0) or the first command.
        await loadCommands();
        const targetCmd = (conn._commands || []).find(c => c.spawn_order === 0) ||
                         (conn._commands || [])[0];
        if (targetCmd) {
            _cacheTerminalForSwitch();
            // Create a new panel and connect it to the server's main/first command
            const panelObj = addPanelDirect();
            panelObj.selectedInstUrl = url;
            panelObj.selectedCmdId = targetCmd.id;
            focusPanel(panelObj.id);
            state.selectedInstUrl = url;
            state.selectedCmdId = targetCmd.id;
            state._pendingVttyData = null;
            state._pendingVttyDirty = false;
            state.bufferView = 'current';
            _restoreCachedDom(targetCmd.id);
            updatePanelCommandInfo();
            updateTerminalDisconnectedOverlay();
            updateSidebarSelection();
            loadVttyHttpForPanel(panelObj.id, url, targetCmd.id);
            startPanelUpdateMode(panelObj.id);
        } else {
            // No commands yet — create an empty panel focused on this server
            const panelObj = addPanelDirect();
            panelObj.selectedInstUrl = url;
            focusPanel(panelObj.id);
        }
        renderPanels();
    }
}


// ─── Restart Command ───
async function restartCommand(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const inst = panelObj.selectedInstUrl ? state.connections.find(i => i.url === panelObj.selectedInstUrl) : null;
    if (!inst || !inst._commands) return;
    const cmdId = panelObj.selectedCmdId;
    if (!cmdId) return;
    await restartCommandById(panelObj.selectedInstUrl, cmdId);
}

async function restartCommandById(instUrl, cmdId) {
    // Use the atomic restart endpoint: the server spawns the new command
    // FIRST, then kills the old one.  This prevents the server from
    // shutting down when the old command was the last one running.
    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}/restart`, { url: instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: instUrl }),
            body: JSON.stringify({}),
        });
        const json = await res.json();
        if (json.status === 'ok' && json.data && json.data.id) {
            const newId = json.data.id;
            state.selectedInstUrl = instUrl;
            state.selectedCmdId = newId;
            _lastCommandState = '';
            // Reload command list so the sidebar contains the new command.
            await loadCommands();
            // Find the new command's name from the refreshed list.
            const inst = state.connections.find(i => i.url === instUrl);
            let newName = newId;
            if (inst && inst._commands) {
                const newCmd = inst._commands.find(c => c.id === newId);
                if (newCmd) newName = newCmd.name || newCmd.id;
            }
            // Stop the old WS/poll (connected to the now-dead old command)
            // and start fresh with the new command.
            selectCommand(instUrl, newId, newName);
        }
    } catch (e) { /* ignore */ }
}


// ─── Welcome Panel Spawn ───
async function spawnFromWelcome() {
    const input = document.getElementById('welcomeCmd');
    if (!input || !input.value.trim()) return;
    const cmd = input.value.trim();
    const instUrl = getBaseUrl();
    try {
        const res = await fetch(apiUrl('/api/commands', { url: instUrl }), {
            method: 'POST',
            headers: authHeaders(),
            body: JSON.stringify({ cmd }),
        });
        const json = await res.json();
        if (json.status === 'ok') {
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                _cacheTerminalForSwitch();
                state._pendingSelectId = newId;
            }
            loadCommands();
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    } catch (e) {
        alert('Spawn failed: ' + e.message);
    }
}



    window.lookupAndSelectCommand = lookupAndSelectCommand;
    window.showCommandPicker = showCommandPicker;
    window.pickCommand = pickCommand;
    window.loadSnapshot = loadSnapshot;
    window.navigateCommand = navigateCommand;
    window.navigatePrevCommand = navigatePrevCommand;
    window.navigateNextCommand = navigateNextCommand;
    window.loadCommands = loadCommands;
    window.selectCommand = selectCommand;
    window.updatePanelCommandInfo = updatePanelCommandInfo;
    window.updateBottomBarLabel = updateBottomBarLabel;
    window.autofitTerminalSize = autofitTerminalSize;
    window.getSelectedPanel = getSelectedPanel;
    window.getActivePanelId = getActivePanelId;
    window.togglePauseRun = togglePauseRun;
    window.togglePauseRunPanel = togglePauseRunPanel;
    window.fetchServerConfig = fetchServerConfig;
    window.applyUpdateModeUI = applyUpdateModeUI;
    window.switchUpdateMode = switchUpdateMode;
    window.applyPollInterval = applyPollInterval;
    window._isTerminalVisible = _isTerminalVisible;
    window._flushPendingVttyUpdate = _flushPendingVttyUpdate;
    window.startUpdateMode = startUpdateMode;
    window.startPanelUpdateMode = startPanelUpdateMode;
    window.stopUpdateMode = stopUpdateMode;
    window.stopPanelUpdateMode = stopPanelUpdateMode;
    window.loadCertificates = loadCertificates;
    window.updateCertDropdown = updateCertDropdown;
    window.updateInstanceDropdown = updateInstanceDropdown;
    window.addConnection = addConnection;
    window.removeConnection = removeConnection;
    window.disconnectServer = disconnectServer;
    window.showAddServerModal = showAddServerModal;
    window.closeAddServerModal = closeAddServerModal;
    window.confirmAddServer = confirmAddServer;
    window.restartCommand = restartCommand;
    window.restartCommandById = restartCommandById;
    window.spawnFromWelcome = spawnFromWelcome;
    window.updateSidebarSelection = updateSidebarSelection;
    window._cacheTerminalForSwitch = _cacheTerminalForSwitch;
    window._restoreCachedDom = _restoreCachedDom;
    window._pushPanelHistory = _pushPanelHistory;
    window._updatePanelHistoryBtns = _updatePanelHistoryBtns;
    window.panelHistoryBack = panelHistoryBack;
    window.panelHistoryForward = panelHistoryForward;
    window._selectCommandForPanel = _selectCommandForPanel;
})();
