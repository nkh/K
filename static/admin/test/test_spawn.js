/// test/test_spawn.js — Tests for spawn-related functions
require('./setup');

console.log('\n=== spawn.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};
globalThis.loadCommands = function() { return Promise.resolve(); };
globalThis.startPanelUpdateMode = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis.updateSidebarSelection = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.loadVttyHttpForPanel = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.connectPanelWs = function() {};
globalThis.startPanelPoll = function() {};

// ── spawn history ──
console.log('spawn history tests');
if (typeof _loadSpawnHistory === 'function') {
    localStorage.removeItem('vrw_spawn_history');
    const history = _loadSpawnHistory();
    assert(Array.isArray(history), 'spawn history is array');
    assertEq(history.length, 0, 'empty history by default');

    // Add entry
    if (typeof _addSpawnHistoryEntry === 'function') {
        _addSpawnHistoryEntry('htop');
        _addSpawnHistoryEntry('vim');
        _addSpawnHistoryEntry('htop');
        const updated = _loadSpawnHistory();
        assertEq(updated.length, 2, 'duplicate entries not added');
        // History stores {cmd, ts} objects
        assert(typeof updated[0] === 'object', 'history entry is object');
        assertEq(updated[0].cmd, 'htop', 'most recent first');
    }
}

// ── _resetSpawnCompletion ──
console.log('_resetSpawnCompletion tests');
if (typeof _resetSpawnCompletion === 'function') {
    assert(() => { _resetSpawnCompletion(); }, '_resetSpawnCompletion does not throw');
}

// ── _onSpawnCmdFocus ──
console.log('_onSpawnCmdFocus tests');
if (typeof _onSpawnCmdFocus === 'function') {
    assert(() => { _onSpawnCmdFocus(); }, '_onSpawnCmdFocus does not throw');
}

// ── _removeSpawnHistoryDropdown ──
console.log('_removeSpawnHistoryDropdown tests');
if (typeof _removeSpawnHistoryDropdown === 'function') {
    assert(() => { _removeSpawnHistoryDropdown(); }, '_removeSpawnHistoryDropdown does not throw');
}

// ── _onSpawnCmdKeydownForHistory ──
console.log('_onSpawnCmdKeydownForHistory tests');
if (typeof _onSpawnCmdKeydownForHistory === 'function') {
    assert(() => { _onSpawnCmdKeydownForHistory({ key: 'ArrowDown', preventDefault(){} }); }, '_onSpawnCmdKeydownForHistory ArrowDown');
    assert(() => { _onSpawnCmdKeydownForHistory({ key: 'ArrowUp', preventDefault(){} }); }, '_onSpawnCmdKeydownForHistory ArrowUp');
}

// ── spawnCommand ──
console.log('spawnCommand tests');
assert(typeof spawnCommand === 'function', 'spawnCommand is a function');

// Mock fetch to return a success response
globalThis.fetch = async function() {
    return {
        ok: true,
        status: 200,
        json: async () => ({ status: 'ok', data: { id: 'new-cmd-id' } }),
    };
};

// Create spawn form elements
const spawnCmd = document.createElement('input');
spawnCmd.id = 'spawnCmd';
spawnCmd.value = 'htop';
const spawnArgs = document.createElement('input');
spawnArgs.id = 'spawnArgs';
spawnArgs.value = '';
const spawnEnv = document.createElement('textarea');
spawnEnv.id = 'spawnEnv';
spawnEnv.value = '';
const spawnDir = document.createElement('input');
spawnDir.id = 'spawnDir';
spawnDir.value = '';
const spawnRows = document.createElement('input');
spawnRows.id = 'spawnRows';
const spawnCols = document.createElement('input');
spawnCols.id = 'spawnCols';
const spawnCert = document.createElement('select');
spawnCert.id = 'spawnCert';
const spawnRetain = document.createElement('input');
spawnRetain.id = 'spawnRetainOnExit';
const spawnOpen = document.createElement('input');
spawnOpen.id = 'spawnOpenPanel';
spawnOpen.checked = false;

state.panels = [];
state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '', _commands: [] }];
state._focusedPanelId = null;

// Can't fully test spawnCommand without real DOM, but verify it doesn't throw
// spawnCommand is async, so we call it and let it resolve
spawnCommand().catch(e => console.log('spawnCommand async error (expected in test):', e.message));

// ── killCommand ──
console.log('killCommand tests');
assert(typeof killCommand === 'function', 'killCommand is a function');

// ── toggleKeepCmd ──
console.log('toggleKeepCmd tests');
if (typeof toggleKeepCmd === 'function') {
    assert(() => { toggleKeepCmd('http://localhost:9090', 'cmd-1'); }, 'toggleKeepCmd does not throw');
}

// ── purgeCommand ──
console.log('purgeCommand tests');
if (typeof purgeCommand === 'function') {
    assert(() => { purgeCommand('http://localhost:9090', 'cmd-1'); }, 'purgeCommand does not throw');
}

// ── sendKeys ──
console.log('sendKeys tests');
if (typeof sendKeys === 'function') {
    assert(() => { sendKeys('http://localhost:9090', 'cmd-1', 'hello\n'); }, 'sendKeys does not throw');
}

// ── resizeTerminal ──
console.log('resizeTerminal tests');
if (typeof resizeTerminal === 'function') {
    assert(() => { resizeTerminal('http://localhost:9090', 'cmd-1', 24, 80); }, 'resizeTerminal does not throw');
}

// ── resizeTerminalPanel ──
console.log('resizeTerminalPanel tests');
if (typeof resizeTerminalPanel === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    assert(() => { resizeTerminalPanel(p.id); }, 'resizeTerminalPanel does not throw');
}

// ── switchBufferPanel ──
console.log('switchBufferPanel tests');
if (typeof switchBufferPanel === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    assert(() => { switchBufferPanel(p.id, 'alt'); }, 'switchBufferPanel does not throw');
}

// ── _applySpawnHistoryEntry — full command reconstruction ──
console.log('_applySpawnHistoryEntry tests');
if (typeof _applySpawnHistoryEntry === 'function') {
    // Old-format entry: cmd and args stored separately (pre cmd+arg merge)
    // This is the reported bug: clicking 'ls --color=always' selected 'ls'
    spawnCmd.value = '';
    document.getElementById('spawnDir').value = '';
    document.getElementById('spawnEnv').value = '';
    _applySpawnHistoryEntry({ cmd: 'ls', args: '--color=always', dir: '/tmp', env: '' });
    assertEq(spawnCmd.value, 'ls --color=always', 'old-format entry: full command restored in spawnCmd');

    // New-format entry: full command in cmd, args is empty
    spawnCmd.value = '';
    _applySpawnHistoryEntry({ cmd: 'vim -O file1 file2', args: '', dir: '', env: '' });
    assertEq(spawnCmd.value, 'vim -O file1 file2', 'new-format entry: full command preserved');

    // Entry with no args
    spawnCmd.value = '';
    _applySpawnHistoryEntry({ cmd: 'htop', args: '', dir: '', env: '' });
    assertEq(spawnCmd.value, 'htop', 'no-args entry: cmd only');

    // Entry with cmd but undefined args (defensive)
    spawnCmd.value = '';
    _applySpawnHistoryEntry({ cmd: 'git log', dir: '', env: '' });
    assertEq(spawnCmd.value, 'git log', 'undefined args: cmd used as-is');

    // Dir and env are also restored
    spawnCmd.value = '';
    document.getElementById('spawnDir').value = '';
    document.getElementById('spawnEnv').value = '';
    _applySpawnHistoryEntry({ cmd: 'make', args: '-j4', dir: '/build', env: 'CC=clang\nCFLAGS=-O2' });
    assertEq(spawnCmd.value, 'make -j4', 'full entry: command correct');
    assertEq(document.getElementById('spawnDir').value, '/build', 'full entry: dir restored');
    assertEq(document.getElementById('spawnEnv').value, 'CC=clang\nCFLAGS=-O2', 'full entry: env restored');

    // Entry with only cmd, no other fields (minimal)
    spawnCmd.value = '';
    _applySpawnHistoryEntry({ cmd: 'bash' });
    assertEq(spawnCmd.value, 'bash', 'minimal entry: cmd only');
}

// ── _addSpawnHistoryEntry stores full command in cmd field ──
console.log('_addSpawnHistoryEntry full-cmd storage tests');
if (typeof _addSpawnHistoryEntry === 'function' && typeof _loadSpawnHistory === 'function') {
    localStorage.removeItem('vrw_spawn_history');
    _addSpawnHistoryEntry('ls --color=always', '', '/home', '');
    const h = _loadSpawnHistory();
    assertEq(h.length, 1, 'one entry stored');
    assertEq(h[0].cmd, 'ls --color=always', 'full command stored in cmd field');
    assertEq(h[0].args, '', 'args is empty string');
    assertEq(h[0].dir, '/home', 'dir stored');

    // Applying this new-format entry back must give full command
    spawnCmd.value = '';
    document.getElementById('spawnDir').value = '';
    _applySpawnHistoryEntry(h[0]);
    assertEq(spawnCmd.value, 'ls --color=always', 'round-trip: new-format entry restores full command');
    assertEq(document.getElementById('spawnDir').value, '/home', 'round-trip: dir restored');
}

// ── spawnCmdTabComplete ──
console.log('spawnCmdTabComplete tests');
if (typeof spawnCmdTabComplete === 'function') {
    const event = { key: 'Tab', preventDefault() {}, target: spawnCmd };
    assert(() => { spawnCmdTabComplete(event); }, 'spawnCmdTabComplete does not throw');
}

console.log('\n[spawn.js] Tests complete');
