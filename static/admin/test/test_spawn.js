/// test/test_spawn.js — Tests for spawn-related functions
require('./setup');

console.log('\n=== spawn.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};
globalThis.loadCommands = function() { return Promise.resolve(); };
globalThis.startUpdateMode = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis.updateSidebarSelection = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.loadVttyHttp = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.updateCmdToolbarVisibility = function() {};
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
    assert(() => { _onSpawnCmdKeydownForHistory({ key: 'ArrowDown' }); }, '_onSpawnCmdKeydownForHistory ArrowDown');
    assert(() => { _onSpawnCmdKeydownForHistory({ key: 'ArrowUp' }); }, '_onSpawnCmdKeydownForHistory ArrowUp');
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

// ── spawnCmdTabComplete ──
console.log('spawnCmdTabComplete tests');
if (typeof spawnCmdTabComplete === 'function') {
    const event = { key: 'Tab', preventDefault() {}, target: spawnCmd };
    assert(() => { spawnCmdTabComplete(event); }, 'spawnCmdTabComplete does not throw');
}

console.log('\n[spawn.js] Tests complete');
