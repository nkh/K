// ─── Spawn & Command Management ───
(function() {
    'use strict';
// ─── Spawn Command Tab Completion ───
// Stores the last fetched completions for cycling with Tab.
let _spawnCompletions = [];
let _spawnCompletionIdx = -1;
let _spawnCompletionBase = '';

/// Tab completion for the spawn command input field.
/// On first Tab: fetches executables from server PATH matching the current prefix.
/// On subsequent Tabs: cycles through the matches.
/// On Escape or input change: resets the completion state.
async function spawnCmdTabComplete(event) {
    const input = document.getElementById('spawnCmd');
    const val = input.value;
    const caretPos = input.selectionStart;

    // Extract the word being completed — from the last space before caret to caret
    const beforeCaret = val.substring(0, caretPos);
    const lastSpace = beforeCaret.lastIndexOf(' ');
    const currentWord = lastSpace >= 0 ? beforeCaret.substring(lastSpace + 1) : beforeCaret;

    // Determine the prefix to match: extract basename from a path like /usr/bin/ht
    const slashIdx = currentWord.lastIndexOf('/');
    const prefix = slashIdx >= 0 ? currentWord.substring(slashIdx + 1) : currentWord;
    const dirPart = slashIdx >= 0 ? currentWord.substring(0, slashIdx + 1) : '';

    // Reset if the input changed since last completion
    if (currentWord !== _spawnCompletionBase) {
        _spawnCompletions = [];
        _spawnCompletionIdx = -1;
        _spawnCompletionBase = currentWord;
    }

    // If we have cached completions, cycle through them
    if (_spawnCompletions.length > 0) {
        _spawnCompletionIdx = (_spawnCompletionIdx + 1) % _spawnCompletions.length;
        const match = _spawnCompletions[_spawnCompletionIdx];
        const replacement = dirPart + match;
        input.value = val.substring(0, beforeCaret.length - currentWord.length) + replacement + val.substring(caretPos);
        input.setSelectionRange(
            beforeCaret.length - currentWord.length + replacement.length,
            beforeCaret.length - currentWord.length + replacement.length
        );
        return;
    }

    // Fetch completions from server
    if (!prefix) return;

    try {
        const instSelect = document.getElementById('spawnInstance');
        const instUrl = instSelect ? instSelect.value : '';
        const json = await api.getCompletions(instUrl, prefix);
        if (json.status !== 'ok' || !Array.isArray(json.data)) return;

        _spawnCompletions = json.data;
        _spawnCompletionIdx = -1;
        _spawnCompletionBase = currentWord;

        if (_spawnCompletions.length === 0) return;

        // Apply first completion
        _spawnCompletionIdx = 0;
        const match = _spawnCompletions[0];
        const replacement = dirPart + match;
        input.value = val.substring(0, beforeCaret.length - currentWord.length) + replacement + val.substring(caretPos);
        input.setSelectionRange(
            beforeCaret.length - currentWord.length + replacement.length,
            beforeCaret.length - currentWord.length + replacement.length
        );
    } catch (e) {
        // Silently ignore — tab completion is best-effort
    }
}

/// Reset tab completion state when spawn command input changes.
function _resetSpawnCompletion() {
    _spawnCompletions = [];
    _spawnCompletionIdx = -1;
    _spawnCompletionBase = '';
}

// ─── Spawn form command history ───
const SPAWN_HISTORY_KEY = 'vrw_spawn_history';
const SPAWN_HISTORY_MAX = 20;

function _loadSpawnHistory() {
    try { return JSON.parse(localStorage.getItem(SPAWN_HISTORY_KEY)) || []; } catch { return []; }
}

function _saveSpawnHistory(history) {
    localStorage.setItem(SPAWN_HISTORY_KEY, JSON.stringify(history.slice(0, SPAWN_HISTORY_MAX)));
}

function _addSpawnHistoryEntry(cmd, args, dir, envText) {
    let history = _loadSpawnHistory();
    // Remove duplicate (same cmd+args+dir)
    history = history.filter(h => !(h.cmd === cmd && h.args === args && h.dir === dir));
    history.unshift({ cmd, args, dir, env: envText, ts: Date.now() });
    _saveSpawnHistory(history);
}

function _renderSpawnHistoryDropdown(inputEl) {
    // Remove existing dropdown
    _removeSpawnHistoryDropdown();
    const history = _loadSpawnHistory();
    if (history.length === 0) return;

    const dd = document.createElement('div');
    dd.id = 'spawnHistoryDropdown';
    dd.className = 'spawn-history-dropdown';

    history.forEach((entry, idx) => {
        const item = document.createElement('div');
        item.className = 'spawn-history-item';
        item.setAttribute('data-idx', idx);
        const displayArgs = entry.args ? ' ' + entry.args : '';
        const displayDir = entry.dir ? ' \n  dir: ' + entry.dir : '';
        item.innerHTML = '<span class="spawn-history-cmd">' + escHtml(entry.cmd) + escHtml(displayArgs) + '</span>';
        item.title = entry.cmd + (entry.args ? ' ' + entry.args : '') + (entry.dir ? '\nDir: ' + entry.dir : '');
        item.addEventListener('mousedown', (e) => {
            e.preventDefault();
            e.stopPropagation();
            _applySpawnHistoryEntry(entry);
            _removeSpawnHistoryDropdown();
        });
        dd.appendChild(item);
    });

    // Clear history button
    const clearBtn = document.createElement('div');
    clearBtn.className = 'spawn-history-clear';
    clearBtn.textContent = 'Clear History';
    clearBtn.addEventListener('mousedown', (e) => {
        e.preventDefault();
        e.stopPropagation();
        localStorage.removeItem(SPAWN_HISTORY_KEY);
        _removeSpawnHistoryDropdown();
    });
    dd.appendChild(clearBtn);

    // Position below the input
    const rect = inputEl.getBoundingClientRect();
    dd.style.top = (rect.bottom + 2) + 'px';
    dd.style.left = rect.left + 'px';
    dd.style.minWidth = rect.width + 'px';

    document.body.appendChild(dd);
}

function _removeSpawnHistoryDropdown() {
    const dd = document.getElementById('spawnHistoryDropdown');
    if (dd) dd.remove();
}

function _applySpawnHistoryEntry(entry) {
    document.getElementById('spawnCmd').value = entry.cmd || '';
    // spawnArgs field no longer exists; all args are in the single field
    document.getElementById('spawnDir').value = entry.dir || '';
    document.getElementById('spawnEnv').value = entry.env || '';
}

function _onSpawnCmdFocus() {
    const inputEl = document.getElementById('spawnCmd');
    if (inputEl && !inputEl.value.trim()) {
        _renderSpawnHistoryDropdown(inputEl);
    }
}

function _onSpawnCmdKeydownForHistory(e) {
    const dd = document.getElementById('spawnHistoryDropdown');
    if (!dd) return;
    const items = dd.querySelectorAll('.spawn-history-item');
    if (items.length === 0) return;

    const current = dd.querySelector('.spawn-history-item.selected');
    let idx = current ? parseInt(current.getAttribute('data-idx')) : -1;

    if (e.key === 'ArrowDown') {
        e.preventDefault();
        if (current) current.classList.remove('selected');
        idx = (idx + 1) % items.length;
        items[idx].classList.add('selected');
        items[idx].scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (current) current.classList.remove('selected');
        idx = idx <= 0 ? items.length - 1 : idx - 1;
        items[idx].classList.add('selected');
        items[idx].scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'Enter' && current) {
        e.preventDefault();
        const entryIdx = parseInt(current.getAttribute('data-idx'));
        const history = _loadSpawnHistory();
        if (history[entryIdx]) {
            _applySpawnHistoryEntry(history[entryIdx]);
            _removeSpawnHistoryDropdown();
        }
    } else if (e.key === 'Escape') {
        _removeSpawnHistoryDropdown();
    }
}

async function spawnCommand() {
    const fullCmd = document.getElementById('spawnCmd').value.trim();
    if (!fullCmd) return;
    const spaceIdx = fullCmd.indexOf(' ');
    const cmd = spaceIdx === -1 ? fullCmd : fullCmd.substring(0, spaceIdx);
    const argsStr = spaceIdx === -1 ? '' : fullCmd.substring(spaceIdx + 1).trim();
    // Parse arguments with support for quoted strings (double and single quotes)
    const args = parseSpawnArgs(argsStr);
    const cert = document.getElementById('spawnCert').value || null;
    const instSelect = document.getElementById('spawnInstance');
    const instUrl = instSelect.value;
    // Remember the user's chosen instance so updateInstanceDropdown won't
    // overwrite it during the subsequent loadCommands() rebuild.
    window._userSpawnInstUrl = instUrl;

    // Terminal size from spawn form (optional, use server defaults if empty)
    const body = { cmd, args, certificate: cert };
    const rows = parseInt(document.getElementById('spawnRows').value);
    const cols = parseInt(document.getElementById('spawnCols').value);
    if (rows > 0) body.rows = rows;
    if (cols > 0) body.cols = cols;

    // Working directory (optional)
    const dir = document.getElementById('spawnDir').value.trim();
    if (dir) body.dir = dir;

    // Retain on exit (optional)
    if (document.getElementById('spawnRetainOnExit').checked) {
        body.retain_on_exit = true;
    }

    // Per-command environment variables (optional)
    const envVars = parseSpawnEnvVars(document.getElementById('spawnEnv').value);
    if (Object.keys(envVars).length > 0) {
        body.env = envVars;
    }

    // Whether to open the spawned command in a new panel
    const openInPanel = document.getElementById('spawnOpenPanel').checked;

    try {
        const json = await api.spawnCommand(instUrl, body);
        if (json.status === 'ok') {
            // Save to spawn history before clearing form
            _addSpawnHistoryEntry(fullCmd, '', dir || '', document.getElementById('spawnEnv').value);
            document.getElementById('spawnCmd').value = '';
            document.getElementById('spawnEnv').value = '';
            document.getElementById('spawnDir').value = '';
            document.getElementById('spawnRows').value = '';
            document.getElementById('spawnCols').value = '';
            document.getElementById('spawnRetainOnExit').checked = false;
            // Auto-select the newly spawned command so its terminal output appears
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                if (openInPanel) {
                    // Create a new panel for this command instead of taking over the
                    // focused panel.  This decouples the spawn target from the current
                    // view, so spawning never disturbs the user's focused workspace.
                    const newPanel = addPanelDirect();
                    focusPanel(newPanel.id);
                    _cacheTerminalForSwitch();
                    state._pendingSelectId = newId;
                } else {
                    // Traditional behavior: take over the focused panel.
                    const focusedId = state._focusedPanelId || getActivePanelId();
                    if (focusedId) disconnectPanelWs(focusedId);
                    _cacheTerminalForSwitch();
                    state._pendingSelectId = newId;
                }
            }
            loadCommands();
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    } catch (e) {
        alert('Spawn failed: ' + e.message);
    }
}

/// Toggle keep/unkeep on a command via the API.
/// When kept, the terminal rendering is retained after the command exits.
async function toggleKeepCmd(instUrl, cmdId) {
    // Determine current state from the sidebar data
    const inst = state.connections.find(i => i.url === instUrl);
    const cmd = inst && inst._commands ? inst._commands.find(c => c.id === cmdId) : null;
    const isKept = cmd && cmd.exit && cmd.exit.retain_on_exit === true;
    const endpoint = isKept ? 'unkeep' : 'keep';
    try {
        if (isKept) {
            await api.unkeep(instUrl, cmdId);
        } else {
            await api.keep(instUrl, cmdId);
        }
        loadCommands();
    } catch (e) { /* ignore */ }
}

async function killCommand(instUrl, cmdId) {
    try {
        await api.kill(instUrl, cmdId);
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
        const json = await api.purge(instUrl, cmdId);
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

async function purgeKeptCommand(instUrl, cmdId, cmdName) {
    // Same as purgeCommand but skips the "are you sure" dialog for kept commands
    try {
        const json = await api.purge(instUrl, cmdId);
        if (json.status === 'ok') {
            if (state.selectedInstUrl === instUrl && state.selectedCmdId === cmdId) {
                state.selectedInstUrl = null;
                state.selectedCmdId = null;
            }
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
        }
    } catch (e) { /* ignore */ }
}

async function killAllCommands() {
    const filter = (document.getElementById('cmdFilter') || {}).value || '';
    const filterLower = filter.toLowerCase();
    let count = 0;
    // Count matching commands to give a useful confirmation message
    for (const inst of state.connections) {
        if (!inst.reachable) continue;
        for (const cmd of (inst._commands || [])) {
            if (!cmd.alive) continue;
            if (filterLower) {
                const cmdName = cmd.name || cmd.id;
                if (!cmdName.toLowerCase().includes(filterLower) &&
                    !(cmd.args || []).join(' ').toLowerCase().includes(filterLower) &&
                    !String(cmd.pid).includes(filterLower)) continue;
            }
            count++;
        }
    }
    if (count === 0) {
        if (filterLower) alert('No running commands match the filter.');
        else alert('No running commands to kill.');
        return;
    }
    const scopeMsg = filterLower
        ? `Kill ${count} matching command(s)? (filter: "${filter}")`
        : `Kill all ${count} running command(s) on all servers?`;
    if (!confirm(scopeMsg)) return;

    if (!filterLower) {
        // No filter — use the server-side kill-all endpoint per server (atomic)
        const promises = [];
        for (const inst of state.connections) {
            if (!inst.reachable) continue;
            promises.push(
                api.killAll(inst.url).catch(() => {})
            );
        }
        await Promise.all(promises);
    } else {
        // Filter active — must kill individually per command
        const promises = [];
        for (const inst of state.connections) {
            if (!inst.reachable) continue;
            for (const cmd of (inst._commands || [])) {
                if (!cmd.alive) continue;
                if (filterLower) {
                    const cmdName = cmd.name || cmd.id;
                    if (!cmdName.toLowerCase().includes(filterLower) &&
                        !(cmd.args || []).join(' ').toLowerCase().includes(filterLower) &&
                        !String(cmd.pid).includes(filterLower)) continue;
                }
                promises.push(
                    api.kill(inst.url, cmd.id).catch(() => {})
                );
            }
        }
        await Promise.all(promises);
    }
    // Re-fetch from server to get accurate state (some kills may have failed)
    await loadCommands();

    // Clear commands for servers that are unreachable (they can't have been killed
    // by the API, so their stale command list would remain otherwise).
    for (const inst of state.connections) {
        if (inst.reachable === false) {
            inst._commands = [];
        }
    }
    // Clear panel selectedCmdId for panels pointing to servers with no commands
    for (const panel of state.panels) {
        if (panel.selectedInstUrl) {
            const inst = state.connections.find(i => i.url === panel.selectedInstUrl);
            if (inst && (!inst._commands || inst._commands.length === 0)) {
                panel.selectedCmdId = null;
                panel.selectedInstUrl = null;
            }
        }
    }
    _buildSidebar();
    updateSharedToolbar();
}

async function freezeAllCommands() {
    // Toggle: if any alive command is not frozen → freeze all; otherwise thaw all
    const cmds = [];
    for (const inst of state.connections) {
        if (!inst.reachable) continue;
        for (const cmd of (inst._commands || [])) {
            if (cmd.alive === false) continue;
            cmds.push({ inst, cmd });
        }
    }
    if (cmds.length === 0) return;
    const anyRunning = cmds.some(c => c.cmd.frozen !== true);
    const endpoint = anyRunning ? 'freeze' : 'thaw';
    const doFreeze = endpoint === 'freeze';
    const promises = cmds.map(({ inst, cmd }) =>
        (doFreeze ? api.freeze(inst.url, cmd.id) : api.thaw(inst.url, cmd.id)).catch(() => {})
    );
    await Promise.all(promises);
    await loadCommands();
}

async function resizeTerminalPanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    // Use the per-panel selected command
    const cmdId = panelObj.selectedCmdId;
    if (!cmdId) return;
    // Try shared toolbar inputs first, fall back to per-panel inputs
    const rows = parseInt(document.getElementById('stResizeRows')?.value || document.getElementById('resizeRows-' + panelId)?.value) || 24;
    const cols = parseInt(document.getElementById('stResizeCols')?.value || document.getElementById('resizeCols-' + panelId)?.value) || 80;
    try {
        await api.resize(panelObj.selectedInstUrl, cmdId, { rows, cols });
        const ri = document.getElementById('resizeRows-' + panelId);
        const ci = document.getElementById('resizeCols-' + panelId);
        if (ri) ri.value = rows;
        if (ci) ci.value = cols;
    } catch (e) { /* ignore */ }
}

function switchBufferPanel(panelId, view) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    // Update the shared toolbar select element
    const sel = document.getElementById('stBufferSelect') || document.getElementById('bufferSelect-' + panelId);
    if (sel) sel.value = view;
    // If this is the currently selected panel, apply the buffer switch
    if (panelObj.selectedInstUrl === state.selectedInstUrl && state.selectedCmdId) {
        state.bufferView = view;
        state.panels.forEach(p => p.scrollbackOffset = 0);
        sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);
        if (view === 'current') {
            startPanelUpdateMode(panelId);
        } else {
            stopPanelUpdateMode(panelId);
            loadVttyHttpForPanel(panelId, panelObj.selectedInstUrl, state.selectedCmdId);
        }
    }
}



    window.spawnCmdTabComplete = spawnCmdTabComplete;
    window._resetSpawnCompletion = _resetSpawnCompletion;
    window._removeSpawnHistoryDropdown = _removeSpawnHistoryDropdown;
    window._onSpawnCmdFocus = _onSpawnCmdFocus;
    window._onSpawnCmdKeydownForHistory = _onSpawnCmdKeydownForHistory;
    window.spawnCommand = spawnCommand;
    window.toggleKeepCmd = toggleKeepCmd;
    window.killCommand = killCommand;
    window.purgeCommand = purgeCommand;
    window.purgeKeptCommand = purgeKeptCommand;
    window.killAllCommands = killAllCommands;
    window.freezeAllCommands = freezeAllCommands;
    window.resizeTerminalPanel = resizeTerminalPanel;
    window.switchBufferPanel = switchBufferPanel;
})();
