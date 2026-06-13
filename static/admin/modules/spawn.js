// ─── Spawn & Command Management ───
(function() {
    'use strict';

// ─── Spawn Command Tab Completion ───
let _spawnCompletions = [];
let _spawnCompletionIdx = -1;
let _spawnCompletionBase = '';

function _applyCompletion(input, val, beforeCaret, currentWord, dirPart, caretPos, match) {
    const replacement = dirPart + match;
    input.value = val.substring(0, beforeCaret.length - currentWord.length) + replacement + val.substring(caretPos);
    input.setSelectionRange(
        beforeCaret.length - currentWord.length + replacement.length,
        beforeCaret.length - currentWord.length + replacement.length
    );
}

async function spawnCmdTabComplete(event) {
    const input = document.getElementById('spawnCmd');
    const val = input.value;
    const caretPos = input.selectionStart;

    const beforeCaret = val.substring(0, caretPos);
    const lastSpace = beforeCaret.lastIndexOf(' ');
    const currentWord = lastSpace >= 0 ? beforeCaret.substring(lastSpace + 1) : beforeCaret;

    const slashIdx = currentWord.lastIndexOf('/');
    const prefix = slashIdx >= 0 ? currentWord.substring(slashIdx + 1) : currentWord;
    const dirPart = slashIdx >= 0 ? currentWord.substring(0, slashIdx + 1) : '';

    if (currentWord !== _spawnCompletionBase) {
        _spawnCompletions = [];
        _spawnCompletionIdx = -1;
        _spawnCompletionBase = currentWord;
    }

    if (_spawnCompletions.length > 0) {
        _spawnCompletionIdx = (_spawnCompletionIdx + 1) % _spawnCompletions.length;
        _applyCompletion(input, val, beforeCaret, currentWord, dirPart, caretPos, _spawnCompletions[_spawnCompletionIdx]);
        return;
    }

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

        _spawnCompletionIdx = 0;
        _applyCompletion(input, val, beforeCaret, currentWord, dirPart, caretPos, _spawnCompletions[0]);
    } catch (e) {}
}

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
    history = history.filter(h => !(h.cmd === cmd && h.args === args && h.dir === dir));
    history.unshift({ cmd, args, dir, env: envText, ts: Date.now() });
    _saveSpawnHistory(history);
}

function _renderSpawnHistoryDropdown(inputEl) {
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

    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        if (current) current.classList.remove('selected');
        const dir = e.key === 'ArrowDown' ? 1 : -1;
        idx = (idx + dir + items.length) % items.length;
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

function _clearCmdSelection() {
    state.selectedInstUrl = null;
    state.selectedCmdId = null;
}

function _clearPanelVtty(panel) {
    if (!panel) return;
    const pre = panel.querySelector('.vtty-container pre');
    if (pre) pre.innerHTML = '';
    const nameEl = panel.querySelector('.cmd-fullname');
    if (nameEl) nameEl.textContent = '';
    const argsEl = panel.querySelector('.cmd-args');
    if (argsEl) argsEl.textContent = '';
}

async function _purgeCmd(instUrl, cmdId, skipConfirm, cmdName) {
    if (!skipConfirm && !confirm(`Purge "${cmdName || cmdId}"?\nThis permanently discards the VTTY buffer and all associated state.`)) return;
    try {
        const json = await api.purge(instUrl, cmdId);
        if (json.status === 'ok') {
            if (state.selectedInstUrl === instUrl && state.selectedCmdId === cmdId) _clearCmdSelection();
            _clearPanelVtty(getSelectedPanel());
            loadCommands();
        } else if (!skipConfirm) {
            alert('Purge failed: ' + (json.error || 'Unknown error'));
        }
    } catch (e) {
        if (!skipConfirm) alert('Purge failed: ' + e.message);
    }
}

async function purgeCommand(instUrl, cmdId, cmdName) {
    return _purgeCmd(instUrl, cmdId, false, cmdName);
}

async function purgeKeptCommand(instUrl, cmdId, cmdName) {
    return _purgeCmd(instUrl, cmdId, true, cmdName);
}

async function spawnCommand() {
    const fullCmd = document.getElementById('spawnCmd').value.trim();
    if (!fullCmd) return;
    const spaceIdx = fullCmd.indexOf(' ');
    const cmd = spaceIdx === -1 ? fullCmd : fullCmd.substring(0, spaceIdx);
    const argsStr = spaceIdx === -1 ? '' : fullCmd.substring(spaceIdx + 1).trim();
    const args = parseSpawnArgs(argsStr);
    const cert = document.getElementById('spawnCert').value || null;
    const instSelect = document.getElementById('spawnInstance');
    const instUrl = instSelect.value;
    window._userSpawnInstUrl = instUrl;

    const body = { cmd, args, certificate: cert };
    const rows = parseInt(document.getElementById('spawnRows').value);
    const cols = parseInt(document.getElementById('spawnCols').value);
    if (rows > 0) body.rows = rows;
    if (cols > 0) body.cols = cols;

    const dir = document.getElementById('spawnDir').value.trim();
    if (dir) body.dir = dir;

    if (document.getElementById('spawnRetainOnExit').checked) body.retain_on_exit = true;

    const envVars = parseSpawnEnvVars(document.getElementById('spawnEnv').value);
    if (Object.keys(envVars).length > 0) body.env = envVars;

    const openInPanel = document.getElementById('spawnOpenPanel').checked;

    try {
        const json = await api.spawnCommand(instUrl, body);
        if (json.status === 'ok') {
            _addSpawnHistoryEntry(fullCmd, '', dir || '', document.getElementById('spawnEnv').value);
            ['spawnCmd', 'spawnEnv', 'spawnDir', 'spawnRows', 'spawnCols'].forEach(id => {
                document.getElementById(id).value = '';
            });
            document.getElementById('spawnRetainOnExit').checked = false;

            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                if (openInPanel) {
                    const newPanel = addPanelDirect();
                    focusPanel(newPanel.id);
                    _cacheTerminalForSwitch();
                    state._pendingSelectId = newId;
                } else {
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

async function toggleKeepCmd(instUrl, cmdId) {
    const inst = state.connections.find(i => i.url === instUrl);
    const cmd = inst && inst._commands ? inst._commands.find(c => c.id === cmdId) : null;
    const isKept = cmd && cmd.exit && cmd.exit.retain_on_exit === true;
    try {
        await (isKept ? api.unkeep(instUrl, cmdId) : api.keep(instUrl, cmdId));
        loadCommands();
    } catch (e) {}
}

async function killCommand(instUrl, cmdId) {
    try {
        await api.kill(instUrl, cmdId);
        if (state.selectedInstUrl === instUrl && state.selectedCmdId === cmdId) _clearCmdSelection();
        loadCommands();
    } catch (e) {}
}

function _matchesCmdFilter(cmd, filterLower) {
    const cmdName = cmd.name || cmd.id;
    return cmdName.toLowerCase().includes(filterLower) ||
        (cmd.args || []).join(' ').toLowerCase().includes(filterLower) ||
        String(cmd.pid).includes(filterLower);
}

async function killAllCommands() {
    const filter = (document.getElementById('cmdFilter') || {}).value || '';
    const filterLower = filter.toLowerCase();
    let count = 0;

    for (const inst of state.connections) {
        if (!inst.reachable) continue;
        for (const cmd of (inst._commands || [])) {
            if (!cmd.alive || (filterLower && !_matchesCmdFilter(cmd, filterLower))) continue;
            count++;
        }
    }
    if (count === 0) {
        alert(filterLower ? 'No running commands match the filter.' : 'No running commands to kill.');
        return;
    }
    const scopeMsg = filterLower
        ? `Kill ${count} matching command(s)? (filter: "${filter}")`
        : `Kill all ${count} running command(s) on all servers?`;
    if (!confirm(scopeMsg)) return;

    if (!filterLower) {
        await Promise.all(state.connections.filter(i => i.reachable).map(i => api.killAll(i.url).catch(() => {})));
    } else {
        const promises = [];
        for (const inst of state.connections) {
            if (!inst.reachable) continue;
            for (const cmd of (inst._commands || [])) {
                if (!cmd.alive || !_matchesCmdFilter(cmd, filterLower)) continue;
                promises.push(api.kill(inst.url, cmd.id).catch(() => {}));
            }
        }
        await Promise.all(promises);
    }
    await loadCommands();

    for (const inst of state.connections) {
        if (inst.reachable === false) inst._commands = [];
    }
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
    const cmds = [];
    for (const inst of state.connections) {
        if (!inst.reachable) continue;
        for (const cmd of (inst._commands || [])) {
            if (cmd.alive === false) continue;
            cmds.push({ inst, cmd });
        }
    }
    if (cmds.length === 0) return;
    const doFreeze = cmds.some(c => c.cmd.frozen !== true);
    await Promise.all(cmds.map(({ inst, cmd }) =>
        (doFreeze ? api.freeze(inst.url, cmd.id) : api.thaw(inst.url, cmd.id)).catch(() => {})));
    await loadCommands();
}

async function resizeTerminalPanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const cmdId = panelObj.selectedCmdId;
    if (!cmdId) return;
    const rows = parseInt(document.getElementById('stResizeRows')?.value || document.getElementById('resizeRows-' + panelId)?.value) || 24;
    const cols = parseInt(document.getElementById('stResizeCols')?.value || document.getElementById('resizeCols-' + panelId)?.value) || 80;
    try {
        await api.resize(panelObj.selectedInstUrl, cmdId, { rows, cols });
        const ri = document.getElementById('resizeRows-' + panelId);
        const ci = document.getElementById('resizeCols-' + panelId);
        if (ri) ri.value = rows;
        if (ci) ci.value = cols;
    } catch (e) {}
}

function switchBufferPanel(panelId, view) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const sel = document.getElementById('stBufferSelect') || document.getElementById('bufferSelect-' + panelId);
    if (sel) sel.value = view;
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

Object.assign(window, {
    spawnCmdTabComplete, _resetSpawnCompletion, _removeSpawnHistoryDropdown,
    _onSpawnCmdFocus, _onSpawnCmdKeydownForHistory, spawnCommand,
    toggleKeepCmd, killCommand, purgeCommand, purgeKeptCommand,
    killAllCommands, freezeAllCommands, resizeTerminalPanel, switchBufferPanel,
});
})();
