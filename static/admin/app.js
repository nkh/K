// ─── State ───
const state = {
    panels: [],
    // Store instUrl and cmdId separately to avoid ':' conflicts in URLs.
    selectedInstUrl: null,
    selectedCmdId: null,
    authToken: localStorage.getItem('vrunner_auth_token') || '',
    refreshInterval: null,
    fontSize: parseInt(localStorage.getItem('vrunner_font_size') || '10'),
    instanceUrls: [],
    currentView: 'vtty',
    // WebSocket for real-time VTTY streaming
    vttyWs: null,
    vttyWsUrl: null,
    vttyWsCmdId: null,
    // Buffer view: 'current', 'main', 'alt'
    bufferView: 'current',
    // Debounce timer for throttled HTTP VTTY fetches.
    _vttyHttpTimer: null,
    // VTTY update mode: 'push' (server sends dirty signals via WS)
    // or 'poll' (client polls /api/commands/:id/vtty/changed)
    updateMode: localStorage.getItem('vrunner_update_mode') || 'push',
    pollInterval: parseInt(localStorage.getItem('vrunner_poll_interval') || '500'),
    _pollTimer: null,
    // Server-configured defaults (fetched from /api/info)
    serverUpdateMode: null,
    serverPollMs: null,
    serverDirtyMs: null,
};

// ─── Initialization ───
(function init() {
    document.getElementById('authToken').value = state.authToken;
    applyFontSize();
    initBottombar();

    // Parse URL arguments for multi-instance
    const params = new URLSearchParams(window.location.search);
    const instances = params.getAll('instance');
    if (instances.length > 0) {
        // First instance is the primary (current origin), rest are additional panels
        state.instanceUrls = instances.map((u, i) => ({
            url: u,
            label: params.getAll('label')[i] || `Instance ${i + 1}`,
            token: params.getAll('token')[i] || '',
        }));
    } else {
        // Default: current origin
        state.instanceUrls = [{
            url: window.location.origin,
            label: 'Local',
            token: '',
        }];
    }

    // Create initial panels
    state.instanceUrls.forEach(inst => addPanelDirect(inst.url, inst.label, inst.token));

    // Start refresh
    startRefresh();
    loadCertificates();
    // Fetch server config and apply update mode defaults
    fetchServerConfig();
    applyUpdateModeUI();

    // Auto-collapse sidebar on small screens
    if (window.innerWidth <= 768) {
        document.getElementById('sidebar').classList.add('collapsed');
    }

    // Auto-fit terminal on window resize (debounced)
    let _resizeTimer = null;
    window.addEventListener('resize', () => {
        if (_resizeTimer) clearTimeout(_resizeTimer);
        _resizeTimer = setTimeout(() => {
            // Auto-collapse/expand sidebar on resize
            const sidebar = document.getElementById('sidebar');
            if (window.innerWidth <= 768) {
                sidebar.classList.add('collapsed');
            }
            // Auto-fit terminal to panel size
            autoFitActiveTerminal();
        }, 300);
    });

    // ── Command-name URL routing ──
    // If the path is /command-name (e.g. /htop, /btop), auto-select
    // that command.  Supports basename matching so /usr/bin/htop works too.
    // If multiple commands share the same name, show a picker.
    const pathname = window.location.pathname.replace(/^\/+|\/+$/g, '');
    if (pathname && pathname !== 'admin' && !pathname.startsWith('api/')) {
        lookupAndSelectCommand(pathname);
    }
})();

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

    function formatRuntime(secs) {
        if (secs < 60) return Math.floor(secs) + 's';
        if (secs < 3600) return Math.floor(secs / 60) + 'm ' + Math.floor(secs % 60) + 's';
        const h = Math.floor(secs / 3600);
        const m = Math.floor((secs % 3600) / 60);
        return h + 'h ' + m + 'm';
    }

    let items = matches.map(m => {
        const argsStr = (m.args || []).join(' ');
        const detail = argsStr ? `${argsStr} (pid ${m.pid})` : `pid ${m.pid}`;
        const aliveBadge = m.alive
            ? '<span style="color:var(--green);font-size:0.65rem;">● running ' + formatRuntime(m.runtime_secs) + '</span>'
            : '<span style="color:var(--red);font-size:0.65rem;">● exited</span>';
        return `<div class="cmd-item" onclick="pickCommand('${m.id}','${escHtml(m.name)}')" style="cursor:pointer;">
            <div class="cmd-item-row">
                <div style="flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-family:var(--font-mono);font-size:0.75rem;color:var(--text-primary);">${escHtml(m.name)}</div>
                ${aliveBadge}
                <span class="pid" style="color:var(--text-muted);font-size:0.7rem;">pid ${m.pid}</span>
            </div>
            <div class="cmd-detail" style="font-family:var(--font-mono);font-size:0.65rem;color:var(--text-muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;padding-left:1.1rem;">${escHtml(detail)}</div>
        </div>`;
    }).join('');

    const overlay = document.createElement('div');
    overlay.id = 'cmdPicker';
    overlay.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.6);z-index:100;display:flex;align-items:center;justify-content:center;';
    overlay.onclick = (e) => { if (e.target === overlay) overlay.remove(); };
    overlay.innerHTML = `<div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:8px;padding:1.25rem;min-width:420px;max-width:90vw;">
        <h2 style="font-size:1rem;color:var(--accent);margin-bottom:0.75rem;">Multiple commands matching "${escHtml(window.location.pathname.replace(/^\/+|\/+$/g, ''))}"</h2>
        <p style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.75rem;">Click a command to view its terminal:</p>
        <div style="max-height:50vh;overflow-y:auto;">${items}</div>
        <div style="margin-top:0.75rem;text-align:right;">
            <button onclick="document.getElementById('cmdPicker').remove()" style="font-size:0.8rem;">Cancel</button>
        </div>
    </div>`;
    document.body.appendChild(overlay);
}

function pickCommand(id, name) {
    const picker = document.getElementById('cmdPicker');
    if (picker) picker.remove();
    state._pendingSelectId = id;
    loadCommands();
}

function getBaseUrl() {
    return state.instanceUrls.length > 0 ? state.instanceUrls[0].url : window.location.origin;
}

function authHeaders(token) {
    const t = token || state.authToken;
    const headers = { 'Content-Type': 'application/json' };
    if (t) headers['Authorization'] = 'Bearer ' + t;
    return headers;
}

function authHeadersForInstance(inst) {
    return authHeaders(inst.token || state.authToken);
}

function apiUrl(path, inst) {
    const base = inst ? inst.url : getBaseUrl();
    return base + path;
}

// ─── Token Management ───
function saveToken() {
    state.authToken = document.getElementById('authToken').value.trim();
    if (state.authToken) {
        localStorage.setItem('vrunner_auth_token', state.authToken);
    } else {
        localStorage.removeItem('vrunner_auth_token');
    }
}

// ─── Font Size ───
function changeFontSize(delta) {
    state.fontSize = Math.max(8, Math.min(28, state.fontSize + delta));
    applyFontSize();
}

function applyFontSize() {
    document.documentElement.style.setProperty('--font-size', state.fontSize + 'px');
    document.getElementById('fontSizeLabel').textContent = state.fontSize + 'px';
    localStorage.setItem('vrunner_font_size', state.fontSize.toString());
}

// ─── Sidebar ───
function toggleSidebar() {
    document.getElementById('sidebar').classList.toggle('collapsed');
}

// ─── Bottom bar toggle ───
function toggleBottombar() {
    const bar = document.getElementById('bottomBar');
    const toggle = document.getElementById('statusToggle');
    bar.classList.toggle('hidden');
    const isHidden = bar.classList.contains('hidden');
    toggle.style.display = isHidden ? '' : 'none';
    localStorage.setItem('vrunner_bottombar_hidden', isHidden ? 'true' : 'false');
}

function initBottombar() {
    const shouldHide = localStorage.getItem('vrunner_bottombar_hidden') === 'true';
    const bar = document.getElementById('bottomBar');
    const toggle = document.getElementById('statusToggle');
    if (shouldHide) {
        bar.classList.add('hidden');
        toggle.style.display = '';
    } else {
        bar.classList.remove('hidden');
        toggle.style.display = 'none';
    }
}

function switchSidebarTab(tab, el) {
    document.querySelectorAll('.sidebar-tab').forEach(t => t.classList.remove('active'));
    el.classList.add('active');
    document.getElementById('tab-commands').style.display = tab === 'commands' ? '' : 'none';
    document.getElementById('tab-spawn').style.display = tab === 'spawn' ? '' : 'none';
    document.getElementById('tab-certs').style.display = tab === 'certs' ? '' : 'none';
}

// ─── View Tabs ───
function switchViewTab(view, el) {
    document.querySelectorAll('.view-tab').forEach(t => t.classList.remove('active'));
    el.classList.add('active');
    state.currentView = view;
    document.getElementById('view-vtty').style.display = view === 'vtty' ? 'flex' : 'none';
    document.getElementById('view-log').style.display = view === 'log' ? 'flex' : 'none';
    document.getElementById('view-docs').style.display = view === 'docs' ? 'block' : 'none';
    if (view === 'log') loadLog();
}

// ─── Commands ───
async function loadCommands() {
    // Load commands from all instances
    for (const inst of state.instanceUrls) {
        try {
            const res = await fetch(apiUrl('/api/commands', inst), { headers: authHeadersForInstance(inst) });
            const json = await res.json();
            inst._commands = json.status === 'ok' ? json.data : [];
            inst._lastError = null;
        } catch (e) {
            inst._commands = [];
            inst._lastError = 'connection lost (instance may have exited)';
        }
    }

    // Render command list (merge all instances)
    const container = document.getElementById('commandList');
    let html = '';
    const filter = (document.getElementById('cmdFilter') || {}).value || '';
    const filterLower = filter.toLowerCase();

    function formatRuntime(secs) {
        if (!secs || secs < 0) return '';
        if (secs < 60) return Math.floor(secs) + 's';
        if (secs < 3600) return Math.floor(secs / 60) + 'm ' + Math.floor(secs % 60) + 's';
        const h = Math.floor(secs / 3600);
        const m = Math.floor((secs % 3600) / 60);
        return h + 'h ' + m + 'm';
    }

    for (const inst of state.instanceUrls) {
        if (inst._lastError) {
            html += `<div style="padding:0.5rem;color:var(--red);font-size:0.75rem;">${escHtml(inst.label)}: ${escHtml(inst._lastError)}</div>`;
        } else if (inst._commands.length === 0) {
            html += `<div style="padding:0.5rem;color:var(--text-muted);font-size:0.75rem;">${escHtml(inst.label)}: No commands</div>`;
        }

        for (const cmd of (inst._commands || [])) {
            const cmdName = cmd.name || cmd.id;
            // Apply filter
            if (filterLower && !cmdName.toLowerCase().includes(filterLower) &&
                !(cmd.args || []).join(' ').toLowerCase().includes(filterLower) &&
                !String(cmd.pid).includes(filterLower)) continue;
            const cert = cmd.certificate || '';
            const certBadge = cert
                ? `<span class="cert-badge" title="Bound to: ${escHtml(cert)}">${escHtml(cert)}</span>`
                : `<span class="cert-badge empty">--</span>`;
            const key = inst.url + ':' + cmd.id;
            const selected = (state.selectedInstUrl === inst.url && state.selectedCmdId === cmd.id) ? ' selected' : '';
            const argsStr = (cmd.args || []).join(' ');
            const detail = argsStr ? `${argsStr}  (pid ${cmd.pid})` : `pid ${cmd.pid}`;
            const isAlive = cmd.alive !== false;
            const statusDot = isAlive
                ? '<div class="status-dot status-running"></div>'
                : '<div class="status-dot status-exited"></div>';
            const runtimeStr = isAlive && cmd.runtime_secs > 0
                ? `<span style="color:var(--text-muted);font-size:0.6rem;flex-shrink:0;">${formatRuntime(cmd.runtime_secs)}</span>`
                : '';
            html += `
                <div class="cmd-item${selected}" onclick="selectCommand('${inst.url}','${cmd.id}','${escHtml(cmdName)}')" oncontextmenu="event.preventDefault();showCmdContextMenu(event,'${inst.url}','${cmd.id}','${escHtml(cmdName)}',${isAlive})" title="${escHtml(inst.label)} / ${escHtml(cmdName)} ${escHtml(argsStr)}" style="${isAlive ? '' : 'opacity:0.6;'}">
                    <div class="cmd-item-row">
                        ${statusDot}
                        <span class="name">${escHtml(cmdName)}</span>
                        ${runtimeStr}
                        ${certBadge}
                        <span class="pid">${cmd.pid}</span>
                        <button class="small danger" onclick="event.stopPropagation();killCommand('${inst.url}','${cmd.id}')" title="Kill">&#x2715;</button>
                    </div>
                    <div class="cmd-detail">${escHtml(detail)}</div>
                </div>`;
        }
    }

    container.innerHTML = html || '<div style="padding:1rem;color:var(--text-muted);text-align:center;">No running commands</div>';

    // Update spawn instance dropdown
    updateInstanceDropdown();

    // Auto-select a pending command (from URL routing /command-name)
    if (state._pendingSelectId) {
        const pendingId = state._pendingSelectId;
        state._pendingSelectId = null;
        for (const inst of state.instanceUrls) {
            if (inst._commands && inst._commands.find(c => c.id === pendingId)) {
                const cmd = inst._commands.find(c => c.id === pendingId);
                selectCommand(inst.url, cmd.id, cmd.name || cmd.id);
                return;
            }
        }
    }

    // Auto-select the first command if none is selected yet
    if (!state.selectedCmdId) {
        for (const inst of state.instanceUrls) {
            if (inst._commands && inst._commands.length > 0) {
                const cmd = inst._commands[0];
                selectCommand(inst.url, cmd.id, cmd.name || cmd.id);
                return; // selectCommand triggers loadCommands, so stop here
            }
        }
    }

    // If a command is selected, refresh its VTTY
    if (state.selectedInstUrl && state.selectedCmdId) {
        updatePanelCommandInfo();
        // Only poll via HTTP if using poll mode or viewing a non-current buffer
        if (state.updateMode === 'poll' || state.bufferView !== 'current') {
            scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 500);
        }
    }
}

function selectCommand(instUrl, cmdId, name) {
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    state.bufferView = 'current';
    document.getElementById('bufferSelect').value = 'current';
    // Reset scrollback offset when switching commands
    state.panels.forEach(p => p.scrollbackOffset = 0);
    updatePanelCommandInfo();
    loadCommands(); // Re-render to update selection
    // Immediately load VTTY content via HTTP
    loadVttyHttp(instUrl, cmdId);
    // Start the active update mode (push or poll)
    startUpdateMode();
}

// Update the panel header with the selected command's full name and args.
function updatePanelCommandInfo() {
    if (!state.selectedInstUrl || !state.selectedCmdId) return;
    // Find the command data from the loaded instance commands
    let cmd = null;
    for (const inst of state.instanceUrls) {
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
        const fullName = cmd.name || cmd.id;
        nameEl.textContent = fullName;
        nameEl.title = fullName;
        if (argsEl) {
            const argsStr = (cmd.args || []).join(' ');
            argsEl.textContent = argsStr;
            argsEl.title = argsStr || '';
        }
        // Update bottom bar command label
        updateBottomBarLabel(cmd);
    } else if (nameEl) {
        nameEl.textContent = '';
        if (argsEl) argsEl.textContent = '';
        updateBottomBarLabel(null);
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
    let html = `<span class="cmd-label-name">${escHtml(fullName)}</span>`;
    if (argsStr) {
        html += `<span class="cmd-label-sep">|</span><span class="cmd-label-args">${escHtml(argsStr)}</span>`;
    }
    if (pid) {
        html += `<span class="cmd-label-sep">|</span><span class="cmd-label-pid">pid ${pid}</span>`;
    }
    el.innerHTML = html;
    el.title = argsStr ? `${fullName} ${argsStr} (pid ${pid})` : `${fullName} (pid ${pid})`;
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
    // Find the panel STATE for the selected command's instance, then return
    // the actual DOM element (not the plain JS object).  Previously this
    // returned the object from state.panels, causing .querySelector() to
    // throw TypeError since plain objects lack that method — silently
    // preventing ALL VTTY content from ever being displayed.
    if (state.panels.length === 0) return null;
    let panelObj;
    if (!state.selectedInstUrl) {
        panelObj = state.panels[0];
    } else {
        panelObj = state.panels.find(p => p.instUrl === state.selectedInstUrl) || state.panels[0];
    }
    return document.getElementById(panelObj.id);
}

// ─── Pause/Run Toggle ───
async function togglePauseRun() {
    if (!state.selectedCmdId) return;
    const cmdId = state.selectedCmdId;
    const instUrl = state.selectedInstUrl;

    const btn = document.getElementById('pauseRunBtn');
    const isFrozen = btn.dataset.frozen === 'true';

    const endpoint = isFrozen ? 'thaw' : 'freeze';
    try {
        await fetch(apiUrl(`/api/commands/${cmdId}/${endpoint}`, { url: instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: instUrl }),
            body: JSON.stringify({}),
        });
        btn.dataset.frozen = isFrozen ? 'false' : 'true';
        btn.textContent = isFrozen ? '\u23F8 Pause' : '\u25B6 Run';
        btn.className = 'small ' + (isFrozen ? '' : 'primary');
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
async function fetchServerConfig() {
    try {
        const res = await fetch(apiUrl('/api/info'), { headers: authHeaders() });
        const json = await res.json();
        if (json.status === 'ok' && json.data && json.data.web) {
            state.serverUpdateMode = json.data.web.update_mode;
            state.serverPollMs = json.data.web.default_poll_ms;
            state.serverDirtyMs = json.data.web.dirty_check_ms;
            // If no user preference is set, use the server default
            if (!localStorage.getItem('vrunner_update_mode')) {
                state.updateMode = state.serverUpdateMode || 'push';
            }
            if (!localStorage.getItem('vrunner_poll_interval')) {
                state.pollInterval = state.serverPollMs || 500;
            }
        }
    } catch (e) { /* ignore — use client defaults */ }
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
    localStorage.setItem('vrunner_update_mode', mode);
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
    localStorage.setItem('vrunner_poll_interval', state.pollInterval.toString());
    document.getElementById('pollInterval').value = state.pollInterval;
    // If currently polling, restart the timer with new interval
    if (state.updateMode === 'poll' && state._pollTimer) {
        stopPoll();
        startPoll();
    }
}

/// Start the active update mode (push or poll).
function startUpdateMode() {
    stopUpdateMode();
    if (state.bufferView !== 'current') return;
    if (state.updateMode === 'push') {
        connectVttyWs(state.selectedInstUrl, state.selectedCmdId);
    } else {
        startPoll();
    }
}

/// Stop the active update mode.
function stopUpdateMode() {
    disconnectVttyWs();
    stopPoll();
}

// ─── Push Mode: WebSocket ───
function connectVttyWs(instUrl, cmdId) {
    // Close existing connection if any
    disconnectVttyWs();

    const wsUrl = instUrl.replace(/^http/, 'ws');
    const token = state.authToken || (state.instanceUrls.find(i => i.url === instUrl) || {}).token || '';
    const sep = token ? '?' : '';
    const url = `${wsUrl}/api/commands/${cmdId}/ws${sep}${token ? 'token=' + encodeURIComponent(token) : ''}`;

    try {
        const ws = new WebSocket(url);
        state.vttyWs = ws;
        state.vttyWsUrl = instUrl;
        state.vttyWsCmdId = cmdId;

        ws.onopen = () => {
            document.getElementById('connStatus').textContent = 'WS Connected';
        };

        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                if (msg.type === 'vtty_full' && msg.data) {
                    // Initial full snapshot — apply it directly
                    if (state.bufferView === 'current') {
                        updateVttyDisplay(msg.data);
                    }
                    const badge = document.getElementById('altScreenBadge');
                    if (badge) badge.style.display = msg.data.alternate_screen ? '' : 'none';
                } else if (msg.type === 'vtty_dirty' && msg.data) {
                    // Buffer has changed — schedule a debounced HTTP fetch.
                    // The server doesn't send any cell data, just a notification.
                    if (state.bufferView === 'current') {
                        scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 50);
                    }
                } else if (msg.type === 'command_ended') {
                    document.getElementById('connStatus').textContent = 'Command ended';
                    disconnectVttyWs();
                    // Browser notification on command exit
                    notifyCommandEnded(state.vttyWsCmdId);
                } else if (msg.type === 'connected') {
                    // Server confirms connection. A vtty_full follows immediately.
                }
            } catch (e) {
                console.error('WS message parse error:', e);
            }
        };

        ws.onclose = () => {
            if (state.vttyWs === ws) {
                state.vttyWs = null;
                document.getElementById('connStatus').textContent = 'WS Disconnected';
                // When WebSocket disconnects, schedule an HTTP fetch to keep display alive
                if (state.selectedInstUrl && state.selectedCmdId) {
                    scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 0);
                }
                // Auto-reconnect after 2 seconds if the command is still selected and alive
                if (state.selectedInstUrl && state.selectedCmdId && !state._wsReconnectTimer) {
                    state._wsReconnectTimer = setTimeout(() => {
                        state._wsReconnectTimer = null;
                        if (state.selectedInstUrl && state.selectedCmdId && state.updateMode === 'push') {
                            connectVttyWs(state.selectedInstUrl, state.selectedCmdId);
                        }
                    }, 2000);
                }
            }
        };

        ws.onerror = (err) => {
            console.error('WebSocket error:', err);
            document.getElementById('connStatus').textContent = 'WS Error';
        };
    } catch (e) {
        console.error('WebSocket connect failed:', e);
    }
}

function disconnectVttyWs() {
    if (state._wsReconnectTimer) {
        clearTimeout(state._wsReconnectTimer);
        state._wsReconnectTimer = null;
    }
    if (state.vttyWs) {
        state.vttyWs.onclose = null; // prevent re-entry
        state.vttyWs.close();
        state.vttyWs = null;
        state.vttyWsUrl = null;
        state.vttyWsCmdId = null;
    }
}

// ─── Poll Mode ───
function startPoll() {
    stopPoll();
    if (!state.selectedInstUrl || !state.selectedCmdId) return;
    state._pollTimer = setInterval(() => pollOnce(), state.pollInterval);
    // Also poll immediately
    pollOnce();
}

function stopPoll() {
    if (state._pollTimer) {
        clearInterval(state._pollTimer);
        state._pollTimer = null;
    }
}

async function pollOnce() {
    if (!state.selectedInstUrl || !state.selectedCmdId) return;
    const cmdId = state.selectedCmdId;
    const instUrl = state.selectedInstUrl;
    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}/vtty/changed`, { url: instUrl }), { headers: authHeadersForInstance({ url: instUrl }) });
        const json = await res.json();
        if (json.status === 'ok' && json.data && json.data.changed) {
            loadVttyHttp(instUrl, cmdId);
        }
    } catch (e) {
        // Silently ignore — next poll will retry
    }
}

function updateVttyDisplay(data) {
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    if (data.html !== undefined && data.html !== null) {
        pre.innerHTML = data.html;
    }

    // Cursor position
    const cursor = data.cursor || {};
    const dims = data.dimensions || {};
    document.getElementById('cursorPos').textContent = `Cursor: ${cursor.row + 1},${cursor.col + 1}`;
    document.getElementById('termDims').textContent = `${dims.rows}x${dims.cols}`;
    document.getElementById('resizeRows').value = dims.rows || 24;
    document.getElementById('resizeCols').value = dims.cols || 80;

    // Show cursor indicator (hide when in scrollback)
    const panelObj = state.panels.find(p => p.id === panel.id);
    const inScrollback = panelObj && panelObj.scrollbackOffset > 0;
    const cursorEl = vttyEl ? vttyEl.querySelector('.cursor-indicator') : null;
    if (cursorEl && cursor.row !== undefined && !inScrollback) {
        const charW = state.fontSize * 0.6;
        const charH = state.fontSize * 1.2;
        cursorEl.style.top = (cursor.row * charH) + 'px';
        cursorEl.style.left = (cursor.col * charW) + 'px';
        cursorEl.style.width = charW + 'px';
        cursorEl.style.height = charH + 'px';
        cursorEl.style.display = '';
    } else if (cursorEl) {
        cursorEl.style.display = 'none';
    }

    // Track mouse state from the server response
    if (panelObj) {
        panelObj.mouseTracking = !!data.mouse_tracking;
        panelObj.mouseSgr = !!data.mouse_sgr;
    }

    state._termRows = dims.rows;
    state._termCols = dims.cols;
}

// ─── Debounced VTTY HTTP Fetch ───
// Prevents request flooding when multiple code paths (dirty signals, onclose,
// periodic refresh, sendKeys) all want to refresh the VTTY display.
// Only the last call within the debounce window actually fires.
function scheduleVttyHttp(instUrl, cmdId, delayMs) {
    if (state._vttyHttpTimer) clearTimeout(state._vttyHttpTimer);
    state._vttyHttpTimer = setTimeout(() => {
        state._vttyHttpTimer = null;
        loadVttyHttp(instUrl, cmdId);
    }, delayMs);
}

async function loadVttyHttp(instUrl, cmdId) {
    const panel = getSelectedPanel();
    if (!panel) return;

    // Get panel state for scrollback offset
    const panelObj = state.panels.find(p => p.id === panel.id);
    const sbOffset = panelObj ? panelObj.scrollbackOffset : 0;

    // If viewing a specific buffer, use the buffer endpoint
    let endpoint;
    if (state.bufferView !== 'current') {
        const screenParam = `?screen=${state.bufferView}`;
        endpoint = `/api/commands/${cmdId}/vtty/buffer${screenParam}`;
    } else if (sbOffset > 0) {
        endpoint = `/api/commands/${cmdId}/vtty/html?scrollback_offset=${sbOffset}`;
    } else {
        endpoint = `/api/commands/${cmdId}/vtty/html`;
    }

    try {
        const res = await fetch(apiUrl(endpoint, { url: instUrl }), { headers: authHeadersForInstance({ url: instUrl }) });
        if (!res.ok) {
            console.warn('VTTY HTTP fetch failed:', res.status, res.statusText);
            return;
        }
        const json = await res.json();
        if (json.status === 'ok' && json.data) {
            const vttyEl = panel.querySelector('.vtty-container');
            const pre = vttyEl ? vttyEl.querySelector('pre') : null;
            if (pre && json.data.html !== undefined) {
                pre.innerHTML = json.data.html;
            }

            const cursor = json.data.cursor || {};
            const dims = json.data.dimensions || {};
            document.getElementById('cursorPos').textContent = `Cursor: ${(cursor.row + 1) || '-'},${(cursor.col + 1) || '-'}`;
            document.getElementById('termDims').textContent = `${dims.rows || '-'}x${dims.cols || '-'}`;

            // Update alt screen badge
            const badge = document.getElementById('altScreenBadge');
            if (badge) badge.style.display = json.data.alternate_screen ? '' : 'none';

            // Update mouse state
            if (panelObj) {
                panelObj.mouseTracking = !!json.data.mouse_tracking;
                panelObj.mouseSgr = !!json.data.mouse_sgr;
            }

            // Hide cursor when in scrollback view
            const cursorEl = vttyEl ? vttyEl.querySelector('.cursor-indicator') : null;
            if (cursorEl) {
                if (sbOffset > 0) {
                    cursorEl.style.display = 'none';
                } else {
                    cursorEl.style.display = '';
                }
            }

            // Show/hide scrollback indicator in bottom bar
            const sbIndicator = document.getElementById('scrollbackIndicator');
            if (sbIndicator) {
                sbIndicator.style.display = sbOffset > 0 ? '' : 'none';
            }
        }
    } catch (e) {
        console.error('Failed to load VTTY:', e);
    }
}

function switchBuffer(view) {
    state.bufferView = view;
    if (!state.selectedCmdId) return;

    // Reset scrollback when switching buffer views
    state.panels.forEach(p => p.scrollbackOffset = 0);

    if (view === 'current') {
        // Re-enable the active update mode for live updates
        startUpdateMode();
    } else {
        // Disconnect WS / stop poll — we're viewing a static snapshot
        stopUpdateMode();
        loadVttyHttp(state.selectedInstUrl, state.selectedCmdId);
    }
}

async function spawnCommand() {
    const cmd = document.getElementById('spawnCmd').value.trim();
    if (!cmd) return;
    const argsStr = document.getElementById('spawnArgs').value.trim();
    const args = argsStr ? argsStr.split(/\s+/) : [];
    const cert = document.getElementById('spawnCert').value || null;
    const instSelect = document.getElementById('spawnInstance');
    const instUrl = instSelect.value;

    // Terminal size from spawn form (optional, use server defaults if empty)
    const body = { cmd, args, certificate: cert };
    const rows = parseInt(document.getElementById('spawnRows').value);
    const cols = parseInt(document.getElementById('spawnCols').value);
    if (rows > 0) body.rows = rows;
    if (cols > 0) body.cols = cols;

    try {
        const res = await fetch(apiUrl('/api/commands', { url: instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: instUrl }),
            body: JSON.stringify(body),
        });
        const json = await res.json();
        if (json.status === 'ok') {
            document.getElementById('spawnCmd').value = '';
            document.getElementById('spawnArgs').value = '';
            document.getElementById('spawnRows').value = '';
            document.getElementById('spawnCols').value = '';
            // Auto-select the newly spawned command so its terminal output appears
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                state.selectedCmdId = newId;
            }
            loadCommands();
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    } catch (e) {
        alert('Spawn failed: ' + e.message);
    }
}

async function killCommand(instUrl, cmdId) {
    try {
        await fetch(apiUrl(`/api/commands/${cmdId}/kill`, { url: instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: instUrl }),
            body: JSON.stringify({}),
        });
        if (state.selectedInstUrl === instUrl && state.selectedCmdId === cmdId) {
            state.selectedInstUrl = null;
            state.selectedCmdId = null;
        }
        loadCommands();
    } catch (e) { /* ignore */ }
}

async function purgeCommand(instUrl, cmdId, cmdName) {
    if (!confirm(`Purge "${cmdName || cmdId}"?\nThis permanently discards the VTTY buffer and all associated state.`)) return;
    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}`, { url: instUrl }), {
            method: 'DELETE',
            headers: authHeadersForInstance({ url: instUrl }),
        });
        const json = await res.json();
        if (json.status === 'ok') {
            if (state.selectedInstUrl === instUrl && state.selectedCmdId === cmdId) {
                state.selectedInstUrl = null;
                state.selectedCmdId = null;
            }
            // Clear the VTTY display
            const panel = getSelectedPanel();
            if (panel) {
                const pre = panel.querySelector('.vtty-container pre');
                if (pre) pre.innerHTML = '';
                const nameEl = panel.querySelector('.cmd-fullname');
                if (nameEl) nameEl.textContent = '';
                const argsEl = panel.querySelector('.cmd-args');
                if (argsEl) argsEl.textContent = '';
            }
            loadCommands();
        } else {
            alert('Purge failed: ' + (json.error || 'Unknown error'));
        }
    } catch (e) {
        alert('Purge failed: ' + e.message);
    }
}

async function killAllCommands() {
    if (!confirm('Kill all running commands?')) return;
    const promises = [];
    for (const inst of state.instanceUrls) {
        for (const cmd of (inst._commands || [])) {
            if (cmd.alive) {
                promises.push(
                    fetch(apiUrl(`/api/commands/${cmd.id}/kill`, { url: inst.url }), {
                        method: 'POST',
                        headers: authHeadersForInstance({ url: inst.url }),
                        body: JSON.stringify({}),
                    }).catch(() => {})
                );
            }
        }
    }
    await Promise.all(promises);
    state.selectedInstUrl = null;
    state.selectedCmdId = null;
    loadCommands();
}

async function sendKeys() {
    // Delegate to the per-panel sendKeysToPanel using the selected panel
    const panel = getSelectedPanel();
    if (!panel) return;
    await sendKeysToPanel(panel.id);
}

async function resizeTerminal() {
    if (!state.selectedCmdId) return;
    const cmdId = state.selectedCmdId;
    const instUrl = state.selectedInstUrl;
    const rows = parseInt(document.getElementById('resizeRows').value) || 24;
    const cols = parseInt(document.getElementById('resizeCols').value) || 80;

    try {
        await fetch(apiUrl(`/api/commands/${cmdId}/resize`, { url: instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: instUrl }),
            body: JSON.stringify({ rows, cols }),
        });
    } catch (e) { /* ignore */ }
}

// ─── Certificates ───
async function loadCertificates() {
    for (const inst of state.instanceUrls) {
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
    for (const inst of state.instanceUrls) {
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
    for (const inst of state.instanceUrls) {
        for (const cert of (inst._certs || [])) {
            html += `<option value="${escHtml(cert.name)}">${escHtml(inst.label)}: ${escHtml(cert.name)}</option>`;
        }
    }
    select.innerHTML = html;
}

function updateInstanceDropdown() {
    const select = document.getElementById('spawnInstance');
    const current = select.value;
    let html = '';
    for (const inst of state.instanceUrls) {
        html += `<option value="${escHtml(inst.url)}">${escHtml(inst.label)} (${escHtml(inst.url.replace(/^https?:\/\//, ''))})</option>`;
    }
    select.innerHTML = html;
    if (current) select.value = current;
}

// ─── Panels (Multi-view) ───
function addPanelDirect(instUrl, label, token) {
    const id = 'panel-' + Date.now() + '-' + Math.random().toString(36).slice(2, 6);
    const panel = { id, instUrl, label, token, scrollbackOffset: 0, mouseTracking: false, mouseSgr: false, focused: false };
    state.panels.push(panel);
    renderPanels();
    return panel;
}

function addPanel() {
    document.getElementById('panelModal').style.display = '';
    document.getElementById('panelUrl').value = 'http://localhost:9090';
    document.getElementById('panelLabel').value = '';
    document.getElementById('panelToken').value = '';
    document.getElementById('panelUrl').focus();
}

function closePanelModal() {
    document.getElementById('panelModal').style.display = 'none';
}

function confirmAddPanel() {
    const url = document.getElementById('panelUrl').value.trim();
    const label = document.getElementById('panelLabel').value.trim() || new URL(url).host;
    const token = document.getElementById('panelToken').value.trim();

    if (!url) return;

    // Check if we already have this instance
    if (state.instanceUrls.some(i => i.url === url)) {
        alert('This instance is already connected.');
        closePanelModal();
        return;
    }

    const inst = { url, label, token };
    state.instanceUrls.push(inst);
    addPanelDirect(url, label, token);
    closePanelModal();
    loadCommands();
    loadCertificates();
}

function removePanel(id) {
    state.panels = state.panels.filter(p => p.id !== id);
    // Remove from instanceUrls if no more panels for it
    const remainingUrls = new Set(state.panels.map(p => p.instUrl));
    state.instanceUrls = state.instanceUrls.filter(i => remainingUrls.has(i.url));
    renderPanels();
}

function renderPanels() {
    const container = document.getElementById('view-vtty');
    let html = '';
    for (const panel of state.panels) {
        const inst = state.instanceUrls.find(i => i.url === panel.instUrl);
        const resizeHandle = state.panels.length > 1 ? `<div class="panel-resize-handle" data-panel="${panel.id}"></div>` : '';
        html += `
            <div class="panel" id="${panel.id}" style="flex: 1 1 0;">
                <div class="panel-header">
                    <div class="cmd-info" id="cmdInfo-${panel.id}">
                        <span class="cmd-fullname" id="cmdName-${panel.id}"></span>
                        <span class="cmd-args" id="cmdArgs-${panel.id}"></span>
                    </div>
                    <span class="instance-url">${escHtml(panel.instUrl.replace(/^https?:\/\//, ''))}</span>
                    <div class="input-row" style="flex:1;min-width:120px;">
                        <input type="text" id="keyInput-${panel.id}" placeholder="Send keys... (e.g. q, <Enter>, <C-c>)" style="font-size:0.7rem;" onkeydown="if(event.key==='Enter'){event.preventDefault();sendKeysToPanel('${panel.id}')}">
                        <button class="small" onclick="sendKeysToPanel('${panel.id}')">Send</button>
                    </div>
                    ${state.panels.length > 1 ? `<button class="small danger" onclick="removePanel('${panel.id}')" title="Remove panel">&#x2715;</button>` : ''}
                    <button class="small" onclick="exportTerminal('${panel.id}')" title="Export terminal as text">&#x2913;</button>
                </div>
                <div class="vtty-container" id="vtty-${panel.id}">
                    <div class="search-bar" id="searchBar-${panel.id}">
                        <input type="text" id="searchInput-${panel.id}" placeholder="Search terminal..." oninput="vttySearch('${panel.id}')">
                        <span class="search-count" id="searchCount-${panel.id}"></span>
                        <button onclick="vttySearchNext('${panel.id}')" title="Next match">&#x25BC;</button>
                        <button onclick="vttySearchPrev('${panel.id}')" title="Previous match">&#x25B2;</button>
                        <button onclick="vttySearchClose('${panel.id}')" title="Close search">&#x2715;</button>
                    </div>
                    <pre style="color:#484f58;">No command selected — spawn or select a command to view its output</pre>
                    <div class="cursor-indicator" style="display:none;"></div>
                    <button class="scroll-bottom-btn" id="scrollBtn-${panel.id}" onclick="scrollTerminalBottom('${panel.id}')" title="Scroll to bottom">&#x25BC;</button>
                </div>
            </div>
            ${resizeHandle}`;
    }
    container.innerHTML = html;
    // Attach scroll listeners for scroll-to-bottom button visibility
    document.querySelectorAll('.vtty-container').forEach(vtty => {
        vtty.addEventListener('scroll', () => {
            const panelEl = vtty.closest('.panel');
            if (!panelEl) return;
            const btn = panelEl.querySelector('.scroll-bottom-btn');
            if (!btn) return;
            const isNearBottom = vtty.scrollHeight - vtty.scrollTop - vtty.clientHeight < 50;
            btn.classList.toggle('visible', !isNearBottom);
        });
    });
}

async function sendKeysToPanel(panelId) {
    const panel = state.panels.find(p => p.id === panelId);
    if (!panel) return;
    const input = document.getElementById('keyInput-' + panelId);
    if (!input || !input.value || !state.selectedCmdId) return;

    const keysValue = input.value;
    const cmdId = state.selectedCmdId;
    const instUrl = panel.instUrl;

    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}/keys`, { url: instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: instUrl, token: panel.token }),
            body: JSON.stringify({ keys: keysValue }),
        });
        let json;
        try {
            json = await res.json();
        } catch (parseErr) {
            console.error('send_keys: non-JSON response', res.status, res.statusText);
            input.value = '';
            loadVttyHttp(instUrl, cmdId);
            return;
        }
        if (json.status === 'ok') {
            input.value = '';
            loadVttyHttp(instUrl, cmdId);
        } else {
            console.error('send_keys server error:', res.status, json.error);
            input.value = '';
        }
    } catch (e) {
        console.error('send_keys network error:', e);
    }
}

// ─── Logs ───
async function loadLog() {
    try {
        const search = document.getElementById('logSearch').value;
        const params = new URLSearchParams();
        if (search) params.set('search', search);
        params.set('limit', '500');

        const res = await fetch(apiUrl('/api/log?' + params.toString()), { headers: authHeaders() });
        const json = await res.json();

        if (json.status === 'ok' && json.data) {
            const container = document.getElementById('logContent');
            const lines = json.data.lines || [];
            document.getElementById('logCount').textContent = `${json.data.filtered_lines}/${json.data.total_lines} lines`;

            if (lines.length === 0) {
                container.innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;">No log entries found.' + (json.data.message ? ' ' + json.data.message : '') + '</div>';
                return;
            }

            container.innerHTML = lines.map(line => {
                const parsed = parseLogLine(line);
                if (search && line.toLowerCase().includes(search.toLowerCase())) {
                    return `<div class="log-line highlight">${formatLogLine(parsed, line)}</div>`;
                }
                return `<div class="log-line">${formatLogLine(parsed, line)}</div>`;
            }).join('');

            // Auto-scroll to bottom
            container.scrollTop = container.scrollHeight;
        }
    } catch (e) {
        document.getElementById('logContent').innerHTML = `<div style="padding:1rem;color:var(--red);">Failed to load log: ${escHtml(e.message)}</div>`;
    }
}

function parseLogLine(line) {
    // Try to parse [timestamp] command: details
    const match = line.match(/^\[([^\]]+)\]\s+(\w+):\s+(.*)$/);
    if (match) {
        return { timestamp: match[1], command: match[2], details: match[3], raw: line };
    }
    return { timestamp: '', command: '', details: line, raw: line };
}

function formatLogLine(parsed, raw) {
    if (parsed.timestamp) {
        return `<span class="timestamp">[${escHtml(parsed.timestamp)}]</span> <span class="cmd-type">${escHtml(parsed.command)}</span> <span class="details">${escHtml(parsed.details)}</span>`;
    }
    return escHtml(raw);
}

function searchLogs() { loadLog(); }
function clearLogSearch() {
    document.getElementById('logSearch').value = '';
    loadLog();
}

// ─── Documentation ───
function showDocs() {
    switchViewTab('docs', document.getElementById('docsTab'));
    loadDocs();
}

async function loadDocs() {
    const container = document.getElementById('view-docs');
    container.innerHTML = '<div style="padding:2rem;text-align:center;color:var(--text-muted);">Loading documentation...</div>';

    // Try fetching docs from the server, fall back to embedded docs
    try {
        const res = await fetch('/admin/docs.md', { headers: authHeaders() });
        if (res.ok) {
            const text = await res.text();
            container.innerHTML = renderMarkdown(text);
            return;
        }
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
<h1>vrunner Administration</h1>

<h2>Overview</h2>
<p>vrunner is a virtual terminal runner with a web control plane. It manages terminal applications, exposing their output through a web interface and REST API. This admin panel provides real-time monitoring and control of all running commands.</p>

<h2>Getting Started</h2>
<p>The admin panel connects to one or more vrunner instances. Each instance manages its own set of terminal commands. Use the <strong>+ Panel</strong> button in the top bar to add connections to additional vrunner instances.</p>

<h3>Connecting to an Instance</h3>
<p>By default, the admin panel connects to the vrunner instance serving it. To add more instances:</p>
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
    <tr><td><code>instance</code></td><td>vrunner instance URL (repeatable)</td><td><code>?instance=http://host:8080</code></td></tr>
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
<p>Switch to the <strong>Spawn</strong> tab in the sidebar to create new commands. Specify the command path, optional arguments, an optional certificate for access control, and the target vrunner instance.</p>

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
<p>The <strong>Logs</strong> tab provides access to the vrunner command log. Use the search bar to filter log entries by content. Each entry shows a timestamp, the command type (spawn, kill, send_keys, etc.), and relevant details.</p>

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
<p>The server-side update settings can be configured in the vrunner config file under the <code>web</code> section:</p>
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

// ─── Utilities ───
function escHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

// ─── Refresh Loop ───
function startRefresh() {
    loadCommands();
    if (state.refreshInterval) clearInterval(state.refreshInterval);
    state.refreshInterval = setInterval(() => {
        loadCommands();
        checkForExitedCommands();
    }, 1000);
}

// ─── Keyboard handling ───
document.addEventListener('keydown', (e) => {
    // Direct terminal keyboard input: when a panel is focused,
    // capture keystrokes and send them to the PTY directly.
    if (state.currentView === 'vtty') {
        const panel = getSelectedPanel();
        if (panel) {
            const panelObj = state.panels.find(p => p.id === panel.id);
            if (panelObj && panelObj.focused && state.selectedCmdId) {
                // Skip if user is in a search input
                const searchBar = document.getElementById('searchBar-' + panel.id);
                if (searchBar && searchBar.classList.contains('visible') &&
                    document.activeElement && document.activeElement.id === 'searchInput-' + panel.id) {
                    // Let search input handle the key
                } else if (e.key === 'Escape') {
                    vttySearchClose(panel.id);
                    closeContextMenu();
                    closeShortcuts();
                    return;
                } else if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
                    e.preventDefault();
                    const sb = document.getElementById('searchBar-' + panel.id);
                    if (sb) {
                        sb.classList.add('visible');
                        const si = document.getElementById('searchInput-' + panel.id);
                        if (si) { si.focus(); si.select(); }
                    }
                    return;
                } else {
                    e.preventDefault();
                    sendDirectKey(e, panelObj);
                    return;
                }
            }
        }
    }

    // Focus key input when not in an input field and a command is selected
    if (state.currentView === 'vtty' && state.selectedCmdId &&
        !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        const panel = getSelectedPanel();
        if (panel) {
            const input = document.getElementById('keyInput-' + panel.id);
            if (input) input.focus();
        }
    }
    // Ctrl+F — open terminal search bar
    if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        const vttyContainer = e.target.closest && e.target.closest('.vtty-container');
        if (vttyContainer || state.currentView === 'vtty') {
            e.preventDefault();
            const panel = getSelectedPanel();
            if (panel) {
                const searchBar = document.getElementById('searchBar-' + panel.id);
                if (searchBar) {
                    searchBar.classList.add('visible');
                    const searchInput = document.getElementById('searchInput-' + panel.id);
                    if (searchInput) { searchInput.focus(); searchInput.select(); }
                }
            }
        }
    }
    // Escape — close terminal search bar
    if (e.key === 'Escape') {
        const panel = getSelectedPanel();
        if (panel) {
            vttySearchClose(panel.id);
        }
        closeContextMenu();
        closeShortcuts();
    }
    // ? — show keyboard shortcuts
    if (e.key === '?' && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        showShortcuts();
    }
});

// ─── Direct key sending (when terminal is focused) ───
// Encodes a KeyboardEvent into escape sequences and sends to the PTY.
async function sendDirectKey(e, panelObj) {
    if (!state.selectedCmdId || !panelObj.instUrl) return;

    // Map common special keys to escape sequences
    const keyMap = {
        'Enter': '\r',
        'Backspace': '\x7f',
        'Tab': '\t',
        'Escape': '\x1b',
        'Home': '\x1b[H',
        'End': '\x1b[F',
        'Delete': '\x1b[3~',
        'ArrowUp': '\x1b[A',
        'ArrowDown': '\x1b[B',
        'ArrowRight': '\x1b[C',
        'ArrowLeft': '\x1b[D',
        'PageUp': '\x1b[5~',
        'PageDown': '\x1b[6~',
        'Insert': '\x1b[2~',
        'F1': '\x1bOP',
        'F2': '\x1bOQ',
        'F3': '\x1bOR',
        'F4': '\x1bOS',
        'F5': '\x1b[15~',
        'F6': '\x1b[17~',
        'F7': '\x1b[18~',
        'F8': '\x1b[19~',
        'F9': '\x1b[20~',
        'F10': '\x1b[21~',
        'F11': '\x1b[23~',
        'F12': '\x1b[24~',
    };

    let seq = '';
    if (e.ctrlKey && !e.altKey && !e.metaKey) {
        // Ctrl+letter
        if (e.key.length === 1 && e.key >= 'a' && e.key <= 'z') {
            seq = String.fromCharCode(e.key.charCodeAt(0) - 96);
        } else if (e.key === '[') seq = '\x1b'; // Ctrl+[ = ESC
        else if (e.key === '\\') seq = '\x1c';
        else if (e.key === ']') seq = '\x1d';
        else if (e.key === '^') seq = '\x1e';
        else if (e.key === '_') seq = '\x1f';
    } else if (e.altKey && !e.ctrlKey && !e.metaKey) {
        // Alt+letter = ESC + letter
        if (e.key.length === 1) seq = '\x1b' + e.key;
    } else if (keyMap[e.key]) {
        seq = keyMap[e.key];
    } else if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
        // Regular printable character
        seq = e.key;
    }

    if (!seq) return;

    try {
        const res = await fetch(apiUrl(`/api/commands/${state.selectedCmdId}/keys`, { url: panelObj.instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: panelObj.instUrl }),
            body: JSON.stringify({ keys: seq }),
        });
        const json = await res.json();
        if (json.status === 'ok') {
            // Trigger a refresh
            scheduleVttyHttp(panelObj.instUrl, state.selectedCmdId, 50);
        }
    } catch (err) {
        console.error('Direct key send error:', err);
    }
}

// ─── Click-to-focus terminal ───
// Clicking on the VTTY container focuses the terminal for direct keyboard input.
// A second click on an already-focused terminal blurs it.
document.addEventListener('click', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (vttyContainer && state.currentView === 'vtty') {
        const panelEl = vttyContainer.closest('.panel');
        if (panelEl) {
            const panelObj = state.panels.find(p => p.id === panelEl.id);
            if (panelObj) {
                // Check if click is on a button inside the vtty container (search bar, scroll btn)
                if (e.target.closest('button') || e.target.closest('input')) return;

                if (panelObj.focused) {
                    // Already focused — blur
                    panelObj.focused = false;
                    vttyContainer.style.outline = '';
                } else {
                    // Focus this panel's terminal
                    state.panels.forEach(p => p.focused = false);
                    document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
                    panelObj.focused = true;
                    vttyContainer.style.outline = '2px solid var(--accent)';
                    vttyContainer.setAttribute('tabindex', '0');
                    vttyContainer.focus();
                }
            }
        }
    } else if (!vttyContainer) {
        // Click outside any terminal — blur all
        state.panels.forEach(p => p.focused = false);
        document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
    }
});

// ─── Mouse wheel scrollback on terminal ───
document.addEventListener('wheel', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') return;

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    e.preventDefault();

    // If the child has mouse tracking enabled, forward wheel events to the PTY
    if (panelObj.mouseTracking) {
        const wheelEvent = e.deltaY < 0 ? 'wheel_up' : 'wheel_down';
        sendMouseEvent(panelObj, wheelEvent, 0, e);
        return;
    }

    // Otherwise, use wheel for scrollback navigation
    if (e.deltaY > 0) {
        // Wheel down: decrease scrollback offset (move toward live view)
        panelObj.scrollbackOffset = Math.max(0, panelObj.scrollbackOffset - 3);
    } else {
        // Wheel up: increase scrollback offset (move into history)
        panelObj.scrollbackOffset += 3;
    }

    // Fetch updated HTML with new scrollback offset
    loadVttyHttp(panelObj.instUrl, state.selectedCmdId);

    // Update scroll-to-bottom button visibility and scrollback indicator
    const btn = panelEl.querySelector('.scroll-bottom-btn');
    if (btn) btn.classList.toggle('visible', panelObj.scrollbackOffset > 0);
    const sbIndicator = document.getElementById('scrollbackIndicator');
    if (sbIndicator) sbIndicator.style.display = panelObj.scrollbackOffset > 0 ? '' : 'none';
}, { passive: false });

// ─── Mouse event forwarding to PTY ───
// Forwards mousedown, mouseup, mousemove events to the PTY when the child
// has enabled mouse tracking mode. Events are sent as escape sequences via
// POST /api/commands/:id/mouse.

let _mouseDownButton = null; // Track which button is pressed

document.addEventListener('mousedown', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') {
        _mouseDownButton = null;
        return;
    }

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    // Skip if clicking on buttons/inputs inside vtty container
    if (e.target.closest('button') || e.target.closest('input')) return;

    // If mouse tracking is enabled, forward the event to PTY
    if (panelObj.mouseTracking) {
        e.preventDefault();
        _mouseDownButton = e.button; // 0=left, 1=middle, 2=right
        sendMouseEvent(panelObj, 'down', e.button, e);
    }
});

document.addEventListener('mouseup', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') {
        _mouseDownButton = null;
        return;
    }

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    if (panelObj.mouseTracking && _mouseDownButton !== null) {
        e.preventDefault();
        sendMouseEvent(panelObj, 'up', _mouseDownButton, e);
        _mouseDownButton = null;
    }
});

document.addEventListener('mousemove', (e) => {
    if (_mouseDownButton === null) return; // Only track during drag

    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') return;

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId || !panelObj.mouseTracking) return;

    // Throttle mouse move events to avoid flooding
    if (!panelObj._lastMoveTime || Date.now() - panelObj._lastMoveTime > 16) {
        panelObj._lastMoveTime = Date.now();
        sendMouseEvent(panelObj, 'move', _mouseDownButton, e);
    }
});

// Send a mouse event to the PTY via the API
async function sendMouseEvent(panelObj, eventType, button, e) {
    if (!state.selectedCmdId || !panelObj.instUrl) return;

    // Calculate terminal cell coordinates from pixel position
    const vttyEl = document.getElementById(panelObj.id)?.querySelector('.vtty-container');
    if (!vttyEl) return;

    const rect = vttyEl.getBoundingClientRect();
    const charW = state.fontSize * 0.6;
    const charH = state.fontSize * 1.2;

    const x = Math.max(1, Math.floor((e.clientX - rect.left) / charW) + 1);
    const y = Math.max(1, Math.floor((e.clientY - rect.top) / charH) + 1);

    try {
        await fetch(apiUrl(`/api/commands/${state.selectedCmdId}/mouse`, { url: panelObj.instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: panelObj.instUrl }),
            body: JSON.stringify({
                event: eventType,
                button: button,
                x: x,
                y: y,
            }),
        });
        // Refresh display after mouse events (the child may have reacted)
        scheduleVttyHttp(panelObj.instUrl, state.selectedCmdId, 30);
    } catch (err) {
        // Silently ignore — mouse events are best-effort
    }
}

// ─── Terminal Search ───
const vttySearchState = { matchIndex: 0, matches: [], panelId: null };

function vttySearch(panelId) {
    const input = document.getElementById('searchInput-' + panelId);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (!input || !countEl) return;
    const query = input.value;
    vttySearchState.panelId = panelId;
    vttySearchState.matchIndex = 0;

    // Get the text content of the terminal
    const panel = document.getElementById(panelId);
    const pre = panel ? panel.querySelector('pre') : null;
    if (!pre) { countEl.textContent = '0/0'; return; }

    // Remove previous highlights
    vttyRemoveHighlights(pre);

    if (!query) {
        vttySearchState.matches = [];
        countEl.textContent = '';
        return;
    }

    // Find all text nodes and mark matches
    const text = pre.textContent || '';
    const lowerText = text.toLowerCase();
    const lowerQuery = query.toLowerCase();
    vttySearchState.matches = [];

    let pos = 0;
    while ((pos = lowerText.indexOf(lowerQuery, pos)) !== -1) {
        vttySearchState.matches.push(pos);
        pos += lowerQuery.length;
    }

    if (vttySearchState.matches.length > 0) {
        vttyApplyHighlights(pre, text, query);
        vttyScrollToMatch(panelId, 0);
        countEl.textContent = '1/' + vttySearchState.matches.length;
    } else {
        countEl.textContent = '0/0';
    }
}

function vttyApplyHighlights(pre, text, query) {
    // Walk through text and highlight matches
    const lowerText = text.toLowerCase();
    const lowerQuery = query.toLowerCase();
    const fragment = document.createDocumentFragment();
    let lastIdx = 0;
    let matchIdx = 0;
    let pos = 0;

    // We need to rebuild using the pre's innerHTML which has spans for ANSI
    // Instead, work at the text level using a tree walker on text nodes
    const walker = document.createTreeWalker(pre, NodeFilter.SHOW_TEXT, null);
    const textNodes = [];
    while (walker.nextNode()) textNodes.push(walker.currentNode);

    if (textNodes.length === 0) return;

    // Simple approach: highlight by rebuilding innerHTML with mark spans
    // Get the full innerHTML and do string replacement on text portions
    let html = pre.innerHTML;
    const escaped = escHtml(query);
    // Use a regex that matches the query text (case insensitive) but only
    // within text content, not inside HTML tags
    const regex = new RegExp('(?![^<]*>)(' + escaped.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + ')', 'gi');
    const results = [];
    let match;
    while ((match = regex.exec(html)) !== null) {
        results.push(match.index);
    }

    // Apply highlights in reverse order to preserve indices
    for (let i = results.length - 1; i >= 0; i--) {
        const idx = results[i];
        const originalLen = query.length;
        // Find the end of the match in the HTML
        const endIdx = html.indexOf(match[1], idx) + match[1].length;
        if (endIdx <= idx) continue;
        const cls = i === 0 ? 'vtty-search-highlight current' : 'vtty-search-highlight';
        html = html.substring(0, idx) + '<mark class="' + cls + '" data-match-idx="' + i + '">' + html.substring(idx, endIdx) + '</mark>' + html.substring(endIdx);
    }

    pre.innerHTML = html;
}

function vttyRemoveHighlights(pre) {
    const marks = pre.querySelectorAll('mark.vtty-search-highlight');
    marks.forEach(mark => {
        const parent = mark.parentNode;
        parent.replaceChild(document.createTextNode(mark.textContent), mark);
        parent.normalize();
    });
}

function vttyScrollToMatch(panelId, idx) {
    const panel = document.getElementById(panelId);
    if (!panel) return;
    const mark = panel.querySelector('mark.vtty-search-highlight.current');
    if (mark) mark.classList.remove('current');
    const marks = panel.querySelectorAll('mark.vtty-search-highlight');
    if (marks[idx]) {
        marks[idx].classList.add('current');
        marks[idx].scrollIntoView({ block: 'center', behavior: 'smooth' });
    }
}

function vttySearchNext(panelId) {
    if (vttySearchState.matches.length === 0) return;
    vttySearchState.matchIndex = (vttySearchState.matchIndex + 1) % vttySearchState.matches.length;
    vttyScrollToMatch(panelId, vttySearchState.matchIndex);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = (vttySearchState.matchIndex + 1) + '/' + vttySearchState.matches.length;
}

function vttySearchPrev(panelId) {
    if (vttySearchState.matches.length === 0) return;
    vttySearchState.matchIndex = (vttySearchState.matchIndex - 1 + vttySearchState.matches.length) % vttySearchState.matches.length;
    vttyScrollToMatch(panelId, vttySearchState.matchIndex);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = (vttySearchState.matchIndex + 1) + '/' + vttySearchState.matches.length;
}

function vttySearchClose(panelId) {
    const searchBar = document.getElementById('searchBar-' + panelId);
    if (searchBar) searchBar.classList.remove('visible');
    const panel = document.getElementById(panelId);
    const pre = panel ? panel.querySelector('pre') : null;
    if (pre) vttyRemoveHighlights(pre);
    vttySearchState.matches = [];
    vttySearchState.matchIndex = 0;
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = '';
}

// ─── Scroll to Bottom ───
function scrollTerminalBottom(panelId) {
    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;
    const vtty = panelEl.querySelector('.vtty-container');
    if (vtty) {
        vtty.scrollTop = vtty.scrollHeight;
    }
    // Reset scrollback offset and re-fetch
    const panelObj = state.panels.find(p => p.id === panelId);
    if (panelObj && panelObj.scrollbackOffset > 0) {
        panelObj.scrollbackOffset = 0;
        const sbIndicator = document.getElementById('scrollbackIndicator');
        if (sbIndicator) sbIndicator.style.display = 'none';
        if (state.selectedCmdId && panelObj.instUrl) {
            loadVttyHttp(panelObj.instUrl, state.selectedCmdId);
        }
    }
}

// ─── Browser Notification on Command Exit ───
const _notifiedExits = new Set();

function notifyCommandEnded(cmdId) {
    if (!cmdId || _notifiedExits.has(cmdId)) return;
    _notifiedExits.add(cmdId);

    // Find command name
    let cmdName = cmdId;
    for (const inst of state.instanceUrls) {
        if (inst._commands) {
            const cmd = inst._commands.find(c => c.id === cmdId);
            if (cmd) { cmdName = cmd.name || cmdId; break; }
        }
    }

    if ('Notification' in window) {
        if (Notification.permission === 'granted') {
            new Notification('vrunner: Command exited', { body: cmdName, icon: '/favicon.ico' });
        } else if (Notification.permission !== 'denied') {
            Notification.requestPermission().then(perm => {
                if (perm === 'granted') {
                    new Notification('vrunner: Command exited', { body: cmdName, icon: '/favicon.ico' });
                }
            });
        }
    }
}

// Also detect command exits via polling — notify when a previously-alive command exits
function checkForExitedCommands() {
    for (const inst of state.instanceUrls) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false && !_notifiedExits.has(cmd.id)) {
                notifyCommandEnded(cmd.id);
            }
        }
    }
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
    for (const inst of state.instanceUrls) {
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

// ─── Right-click Context Menu ───
function closeContextMenu() {
    const el = document.getElementById('ctxMenu');
    if (el) el.remove();
}

function showCmdContextMenu(e, instUrl, cmdId, cmdName, isAlive) {
    closeContextMenu();
    const menu = document.createElement('div');
    menu.id = 'ctxMenu';
    menu.className = 'ctx-menu';
    menu.style.left = e.clientX + 'px';
    menu.style.top = e.clientY + 'px';

    // Ensure menu stays within viewport
    document.body.appendChild(menu);
    const rect = menu.getBoundingClientRect();
    if (rect.right > window.innerWidth) menu.style.left = (window.innerWidth - rect.width - 4) + 'px';
    if (rect.bottom > window.innerHeight) menu.style.top = (window.innerHeight - rect.height - 4) + 'px';

    const baseUrl = instUrl.replace(/^https?:\/\//, '');
    let items = `<div class="ctx-menu-item" onclick="selectCommand('${instUrl}','${cmdId}','${escHtml(cmdName)}');closeContextMenu();">View Terminal</div>`;
    items += `<div class="ctx-menu-item" onclick="copyCommandUrl('${instUrl}','${cmdId}','${escHtml(cmdName)}');closeContextMenu();">Copy URL</div>`;
    if (isAlive) {
        items += `<div class="ctx-menu-sep"></div>`;
        items += `<div class="ctx-menu-item" onclick="togglePauseCmd('${instUrl}','${cmdId}');closeContextMenu();">Pause/Resume</div>`;
        items += `<div class="ctx-menu-item danger" onclick="killCommand('${instUrl}','${cmdId}');closeContextMenu();">Kill</div>`;
    } else {
        items += `<div class="ctx-menu-sep"></div>`;
        items += `<div class="ctx-menu-item danger" onclick="purgeCommand('${instUrl}','${cmdId}','${escHtml(cmdName)}');closeContextMenu();">Purge</div>`;
    }
    menu.innerHTML = items;

    // Close on click outside
    setTimeout(() => {
        document.addEventListener('click', closeContextMenu, { once: true });
    }, 0);
}

function copyCommandUrl(instUrl, cmdId, cmdName) {
    const base = cmdName.replace(/.*\//, ''); // basename
    const url = instUrl.replace(/^http/, 'http') + '/' + encodeURIComponent(base);
    navigator.clipboard.writeText(url).catch(() => {});
}

function togglePauseCmd(instUrl, cmdId) {
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    togglePauseRun();
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
        fetch(apiUrl(`/api/commands/${state.selectedCmdId}/resize`, { url: state.selectedInstUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: state.selectedInstUrl }),
            body: JSON.stringify({ rows, cols }),
        }).catch(() => {});
    }
}

// ─── Keyboard Shortcuts Help ───
function showShortcuts() {
    closeShortcuts();
    const overlay = document.createElement('div');
    overlay.className = 'shortcuts-overlay';
    overlay.id = 'shortcutsOverlay';
    overlay.onclick = (e) => { if (e.target === overlay) closeShortcuts(); };
    overlay.innerHTML = `<div class="shortcuts-panel">
        <h2>Keyboard Shortcuts</h2>
        <table>
            <tr><td>?</td><td>Show this help</td></tr>
            <tr><td>Ctrl+F</td><td>Search in terminal</td></tr>
            <tr><td>Escape</td><td>Close search / menu</td></tr>
            <tr><td>Any key</td><td>Focus key input (when not in a field)</td></tr>
            <tr><td>Enter</td><td>Send keystrokes to terminal</td></tr>
        </table>
        <p style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.5rem;">Click on the terminal to focus the key input field.</p>
        <div style="text-align:right;margin-top:0.75rem;">
            <button onclick="closeShortcuts()" style="font-size:0.8rem;">Close</button>
        </div>
    </div>`;
    document.body.appendChild(overlay);
}

function closeShortcuts() {
    const el = document.getElementById('shortcutsOverlay');
    if (el) el.remove();
}
