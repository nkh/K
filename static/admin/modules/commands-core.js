(function() {
    'use strict';

    async function lookupAndSelectCommand(name) {
        try {
            const json = await api.lookupCommand(name);
            if (json.status !== 'ok' || !json.data.length) return;
            if (json.data.length === 1) {
                state._pendingSelectId = json.data[0].id;
                loadCommands();
            } else showCommandPicker(json.data);
        } catch (e) { /* ignore */ }
    }

    function showCommandPicker(matches) {
        const old = document.getElementById('cmdPicker');
        if (old) old.remove();
        const pathName = escHtml(window.location.pathname.replace(/^\/+|\/+$/g, ''));
        const items = matches.map(m => {
            const argsStr = (m.args || []).join(' ');
            const detail = argsStr ? `${argsStr} (${m.pid})` : String(m.pid);
            const badge = m.alive
                ? `<span style="color:var(--green);font-size:.65rem">● running ${formatRuntime(m.runtime_secs)}</span>`
                : '<span style="color:var(--red);font-size:.65rem">● exited</span>';
            return `<div class="cmd-item" data-cmd-id="${escHtml(m.id)}" data-cmd-name="${escHtml(m.name)}" style="cursor:pointer">
                <div class="cmd-item-row">
                    <div style="flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-family:var(--font-mono);font-size:.75rem;color:var(--text-primary)">${escHtml(m.name)}</div>
                    ${badge}
                    <span class="pid" style="color:var(--text-muted);font-size:.7rem">${escHtml(String(m.pid))}</span>
                </div>
                <div class="cmd-detail" style="font-family:var(--font-mono);font-size:.65rem;color:var(--text-muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;padding-left:1.1rem">${escHtml(detail)}</div>
            </div>`;
        }).join('');
        const overlay = document.createElement('div');
        overlay.id = 'cmdPicker';
        overlay.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,.6);z-index:100;display:flex;align-items:center;justify-content:center';
        overlay.onclick = e => { if (e.target === overlay) { releaseCurrentFocusTrap(); overlay.remove(); } };
        overlay.innerHTML = `<div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:8px;padding:1.25rem;min-width:420px;max-width:90vw">
            <h2 style="font-size:1rem;color:var(--accent);margin-bottom:.75rem">Multiple commands matching "${pathName}"</h2>
            <p style="font-size:.75rem;color:var(--text-secondary);margin-bottom:.75rem">Click a command to view its terminal:</p>
            <div style="max-height:50vh;overflow-y:auto">${items}</div>
            <div style="margin-top:.75rem;text-align:right"><button class="btn" data-action="CloseCmdPicker">Cancel</button></div>
        </div>`;
        document.body.appendChild(overlay);
        overlay.addEventListener('click', e => {
            const item = e.target.closest('.cmd-item[data-cmd-id]');
            if (item) pickCommand(item.dataset.cmdId, item.dataset.cmdName);
        });
        const panel = overlay.querySelector('div[style*="background:var(--bg-secondary)"]');
        if (panel) trapFocus(panel);
        const first = overlay.querySelector('.cmd-item');
        if (first) first.focus();
    }

    function pickCommand(id) {
        releaseCurrentFocusTrap();
        const picker = document.getElementById('cmdPicker');
        if (picker) picker.remove();
        state._pendingSelectId = id;
        loadCommands();
    }

    function navigateCommand(dir) {
        if (!state._navCommands.length) return;
        const idx = state._navCommands.findIndex(c => c.instUrl === state.selectedInstUrl && c.cmdId === state.selectedCmdId);
        const n = (idx + dir + state._navCommands.length) % state._navCommands.length;
        const t = state._navCommands[idx === -1 ? (dir > 0 ? 0 : state._navCommands.length - 1) : n];
        if (t) selectCommand(t.instUrl, t.cmdId, t.name);
    }

    function _debouncedBuildSidebar() {
        if (state._sidebarBuildTimer) return;
        state._sidebarBuildTimer = setTimeout(() => { state._sidebarBuildTimer = null; _buildSidebar(); }, 150);
    }

    async function loadCommands() {
        let changed = false;
        await Promise.all(state.connections.map(async inst => {
            try {
                const json = await api.getCommands(inst.url);
                inst._commands = (json.status === 'ok' && Array.isArray(json.data)) ? json.data : [];
                const was = inst.reachable;
                inst.reachable = true; inst._lastError = null;
                if (was !== true) changed = true;
            } catch (e) {
                inst._commands = inst._commands || [];
                const was = inst.reachable;
                inst.reachable = false; inst._lastError = 'connection lost (instance may have exited)';
                if (was !== false) changed = true;
            }
        }));
        if (changed) updateDisconnectedUI();

        // Process URL-based command opening (state._urlCmdSpecs set from ?cmd=&server= params)
        if (state._urlCmdSpecs && state._urlCmdSpecs.length > 0) {
            const specs = state._urlCmdSpecs;
            state._urlCmdSpecs = null; // consume once
            let opened = 0;
            for (const spec of specs) {
                if (!spec.cmd) continue;
                const inst = state.connections.find(c => c.url === spec.server);
                if (!inst || !inst._commands) continue;
                const cmd = inst._commands.find(c => (c.name || c.id) === spec.cmd);
                if (!cmd) continue;
                if (opened === 0) {
                    const panel = state.panels[0];
                    if (panel) {
                        focusPanel(panel.id);
                        _selectCommandForPanel(panel, spec.server, cmd.id);
                    }
                } else {
                    const p = addPanelDirect();
                    _selectCommandForPanel(p, spec.server, cmd.id);
                }
                opened++;
            }
            if (opened > 0) renderPanels();
        }

        const hasAny = state.connections.some(i => i._commands && i._commands.length > 0);
        const showWelcome = !hasAny && !state.selectedCmdId && !state.serverReachable;
        if (showWelcome !== state._showingWelcome) { state._showingWelcome = showWelcome; renderPanels(); }

        if (hasAny && !state.selectedCmdId) {
            const p = state.panels[0];
            if (p && !p.selectedCmdId) {
                let tInst = null, tCmd = null;
                for (const inst of state.connections) {
                    if (!inst._commands || !inst._commands.length) continue;
                    const alive = inst._commands.find(c => c.alive);
                    if (alive) { tInst = inst; tCmd = alive; break; }
                    if (!tCmd) { tInst = inst; tCmd = inst._commands[0]; }
                }
                if (tInst && tCmd) {
                    p.selectedInstUrl = tInst.url; p.selectedCmdId = tCmd.id;
                    state.selectedInstUrl = tInst.url; state.selectedCmdId = tCmd.id;
                    state.bufferView = 'current';
                    loadVttyHttpForPanel(p.id, tInst.url, tCmd.id);
                    startPanelUpdateMode(p.id);
                    updatePanelCommandInfo();
                    updateTerminalDisconnectedOverlay();
                    updateSidebarSelection();
                }
            }
        }
        _debouncedBuildSidebar();
    }

    function updatePanelCommandInfo() {
        for (const p of state.panels) {
            if (!p.selectedInstUrl || !p.selectedCmdId) continue;
            const el = document.getElementById(p.id);
            if (!el) continue;
            const inst = state.connections.find(i => i.url === p.selectedInstUrl);
            const cmd = inst && inst._commands ? inst._commands.find(c => c.id === p.selectedCmdId) : null;
            const nameEl = el.querySelector(':scope > .panel-header .cmd-fullname');
            const argsEl = el.querySelector(':scope > .panel-header .cmd-args');
            const metaEl = el.querySelector(':scope > .panel-header .panel-header-meta');
            const freezeBtn = el.querySelector(':scope > .panel-header .panel-freeze-btn');
            const reachDot = el.querySelector(':scope > .panel-header .panel-reach-dot');
            if (reachDot && inst) {
                const rCls = inst.reachable === true ? 'reachable' : inst.reachable === false ? 'unreachable' : 'unknown';
                reachDot.className = 'panel-reach-dot ' + rCls;
                reachDot.title = inst.reachable === true ? 'Server connected' : inst.reachable === false ? 'Server unreachable' : 'Checking server...';
            }
            if (metaEl && cmd) {
                const sLabel = _getServerLabel(inst, p.selectedInstUrl);
                const pid = cmd.pid || '';
                metaEl.textContent = (sLabel || '') + (pid ? ' - ' + pid : '');
                metaEl.title = p.selectedInstUrl || '';
            }
            if (nameEl && cmd) {
                const fullName = cmd.name || cmd.id;
                nameEl.textContent = p.customTitle || fullName;
                nameEl.title = fullName + (p.customTitle ? ' (title: ' + p.customTitle + ')' : '');
                if (argsEl) { const a = (cmd.args || []).join(' '); argsEl.textContent = a ? ' ' + a : ''; argsEl.title = a || ''; }
                const restartBtn = el.querySelector('[id^="restartBtn-"]');
                if (restartBtn) restartBtn.classList.remove('hidden');
                if (freezeBtn && cmd.alive !== false) {
                    freezeBtn.classList.remove('hidden');
                    freezeBtn.textContent = cmd.frozen ? '\u25B6' : '\u2161';
                    freezeBtn.title = cmd.frozen ? 'Thaw command' : 'Freeze command';
                    freezeBtn.classList.toggle('active', cmd.frozen);
                } else if (freezeBtn) { freezeBtn.classList.add('hidden'); }
                const resEl = el.querySelector('[id^="resourceBadge-"]');
                if (resEl) {
                    const res = state._resourceCache[cmd.id];
                    if (state.showResources && res && (res.cpu_percent != null || res.memory_mb != null)) {
                        resEl.classList.remove('hidden');
                        resEl.textContent = (res.cpu_percent != null ? 'CPU ' + res.cpu_percent.toFixed(1) + '%' : '') +
                            (res.cpu_percent != null && res.memory_mb != null ? ' | ' : '') +
                            (res.memory_mb != null ? res.memory_mb.toFixed(1) + 'MB' : '');
                    } else { resEl.textContent = ''; if (!state.showResources) resEl.classList.add('hidden'); }
                }
                const exitBanner = el.querySelector(':scope > .panel-header .panel-exit-banner');
                if (exitBanner) {
                    if (cmd.alive === false && cmd.frozen !== true) {
                        const ec = cmd.exit_code != null ? cmd.exit_code : '?';
                        exitBanner.innerHTML = `&#9632; exited <span class="exit-badge ${cmd.exit_code === 0 ? 'success' : 'failure'}">${ec}</span>`;
                        exitBanner.classList.remove('hidden');
                    } else exitBanner.classList.add('hidden');
                }
            } else if (nameEl) {
                nameEl.textContent = p.customTitle || '';
                if (argsEl) argsEl.textContent = '';
                const rb = el.querySelector('[id^="restartBtn-"]'); if (rb) rb.classList.add('hidden');
                const eb = el.querySelector(':scope > .panel-header .panel-exit-banner'); if (eb) eb.classList.add('hidden');
            }
        }
        const fp = state.panels.find(p => p.id === state._focusedPanelId);
        if (fp && fp.selectedCmdId) updateBottomBarLabel(_findCmd(fp.selectedInstUrl, fp.selectedCmdId));
        else updateBottomBarLabel(null);
        updateSharedToolbar();
    }

    function updateBottomBarLabel(cmd) {
        const el = document.getElementById('cmdLabel');
        if (!el) return;
        if (!cmd) { el.innerHTML = ''; return; }
        const fullName = cmd.name || cmd.id, args = (cmd.args || []).join(' '), pid = cmd.pid || '';
        const runtime = cmd.runtime_secs != null ? formatRuntime(cmd.runtime_secs) : '';
        let html = `<span class="cmd-label-name">${escHtml(fullName)}</span>`;
        if (args) html += `<span class="cmd-label-sep">|</span><span class="cmd-label-args">${escHtml(args)}</span>`;
        if (pid) html += `<span class="cmd-label-sep">|</span><span class="cmd-label-pid">${escHtml(pid)}</span>`;
        if (runtime) html += `<span class="cmd-label-sep">|</span><span class="cmd-label-runtime">${escHtml(runtime)}</span>`;
        el.innerHTML = html;
        el.title = args ? `${fullName} ${args} (${pid})${runtime ? ' [' + runtime + ']' : ''}` : `${fullName} (${pid})${runtime ? ' [' + runtime + ']' : ''}`;
    }

    function autofitTerminalSize() {
        const panel = getSelectedPanel();
        const hint = document.getElementById('autofitHint');
        if (!panel) { hint.textContent = 'No panel visible to measure'; return; }
        const vttyEl = panel.querySelector('.vtty-container');
        if (!vttyEl) { hint.textContent = 'No terminal container found'; return; }
        const { width, height } = vttyEl.getBoundingClientRect();
        const cols = Math.max(20, Math.min(500, Math.floor(width / (state.fontSize * 0.6))));
        const rows = Math.max(5, Math.min(200, Math.floor(height / (state.fontSize * 1.2))));
        document.getElementById('spawnRows').value = rows;
        document.getElementById('spawnCols').value = cols;
        hint.textContent = `Panel is ${Math.floor(width)}x${Math.floor(height)}px → ${rows} rows × ${cols} cols`;
    }

    function getSelectedPanel() {
        if (!state.panels.length) return null;
        let p;
        if (state._focusedPanelId) p = state.panels.find(x => x.id === state._focusedPanelId);
        if (!p && state.selectedInstUrl) p = state.panels.find(x => x.selectedInstUrl === state.selectedInstUrl);
        if (!p) p = state.panels[0];
        state.selectedInstUrl = p.selectedInstUrl;
        state.selectedCmdId = p.selectedCmdId;
        return document.getElementById(p.id);
    }

    function getActivePanelId() {
        return state._focusedPanelId || (state.panels.length > 0 ? state.panels[0].id : null);
    }

    let _snapshotLoaded = false;
    Object.defineProperty(window, '_snapshotLoaded', { get() { return _snapshotLoaded; }, set(v) { _snapshotLoaded = v; }, configurable: true });

    async function loadSnapshot() {
        if (_snapshotLoaded) { loadCommands(); return; }
        _snapshotLoaded = true;
        const primary = state.connections[0];
        if (!primary) { loadCommands(); return; }
        try {
            const json = await api.getSnapshot(primary.url);
            if (json.status !== 'ok' || !json.data) throw new Error('bad snapshot');
            const { commands, vtty, resources } = json.data;
            primary._commands = commands || [];
            primary.reachable = true; primary._lastError = null;
            if (resources) for (const [k, v] of Object.entries(resources)) state._resourceCache[k] = v;
            const peersDone = Promise.all(state.connections.slice(1).map(async inst => {
                try {
                    const j = await api.getCommands(inst.url);
                    inst._commands = j.status === 'ok' ? j.data : [];
                    inst.reachable = true; inst._lastError = null;
                } catch (e) { inst._commands = inst._commands || []; inst.reachable = false; inst._lastError = 'connection lost'; }
            })).then(() => updateDisconnectedUI());
            const hasAny = commands && commands.length > 0;
            const firstCmd = hasAny ? (commands.find(c => c.alive) || commands[0]) : null;
            const showWelcome = !hasAny && !state.selectedCmdId && !state.serverReachable;
            if (showWelcome !== state._showingWelcome) { state._showingWelcome = showWelcome; renderPanels(); }
            if (vtty && vtty.html !== undefined && firstCmd) {
                state.selectedInstUrl = primary.url;
                state.selectedCmdId = firstCmd.id;
                state.bufferView = 'current';
                const panelObj = state.panels.find(p => p.id === (state._focusedPanelId || state.panels[0].id));
                if (panelObj) { panelObj.selectedInstUrl = primary.url; panelObj.selectedCmdId = firstCmd.id; }
                getSelectedPanel();
                if (vtty.generation !== undefined) state._lastGeneration[(panelObj ? panelObj.id : 'panel-0') + '/' + firstCmd.id] = vtty.generation;
                const panelEl = document.getElementById(panelObj ? panelObj.id : (state._focusedPanelId || (state.panels[0] || {}).id));
                if (panelEl) {
                    const vttyEl = panelEl.querySelector('.vtty-container');
                    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
                    if (pre) {
                        pre.innerHTML = vtty.html;
                        if (state._level3Enabled && vtty.dimensions) buildCellGrid((panelObj ? panelObj.id : 'panel-0') + '/' + firstCmd.id, pre, vtty.dimensions.rows, vtty.dimensions.cols);
                        updateVttyMetadataFromHttp(vtty, panelEl, panelObj, 0);
                    }
                }
                updatePanelCommandInfo();
                updateTerminalDisconnectedOverlay();
                startPanelUpdateMode(state._focusedPanelId);
            } else { state._showingWelcome = showWelcome; updateDisconnectedUI(); }
            await peersDone;
            _debouncedBuildSidebar();
        } catch (e) {
            primary._commands = primary._commands || [];
            primary.reachable = false; primary._lastError = 'connection lost';
            updateDisconnectedUI();
            loadCommands();
        }
    }

    Object.assign(window, {
        lookupAndSelectCommand, showCommandPicker, pickCommand,
        navigateCommand, navigatePrevCommand: () => navigateCommand(-1), navigateNextCommand: () => navigateCommand(1),
        loadCommands, updatePanelCommandInfo, updateBottomBarLabel,
        autofitTerminalSize, getSelectedPanel, getActivePanelId, loadSnapshot,
        closeCmdPicker() { releaseCurrentFocusTrap(); const p = document.getElementById('cmdPicker'); if (p) p.remove(); },
    });
})();