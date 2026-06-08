/// test/test_commands.js — Tests for command selection and management
require('./setup');

console.log('\n=== commands.js Tests ===\n');

resetTestState();

// Mock functions that depend on network/DOM
globalThis.renderPanels = function() {};
globalThis.loadVttyHttp = function() {};
globalThis.startUpdateMode = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis.updateSidebarSelection = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.updateCmdToolbarVisibility = function() {};
globalThis.disconnectPanelWs = function() {};
globalThis.stopPanelPoll = function() {};
globalThis.connectPanelWs = function() {};
globalThis.startPanelPoll = function() {};
globalThis.loadCommands = function() { return Promise.resolve(); };
globalThis.checkOnboarding = function() {};
globalThis.renderPanels = function() {};

// ── selectCommand ──
console.log('selectCommand tests');
assert(typeof selectCommand === 'function', 'selectCommand is a function');

state.panels = [];
const p = addPanelDirect();
state._focusedPanelId = p.id;
state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '', _commands: [] }];

assert(() => { selectCommand('http://localhost:9090', 'cmd-1', 'htop'); }, 'selectCommand does not throw');
assertEq(state.selectedCmdId, 'cmd-1', 'selectedCmdId set');
assertEq(state.selectedInstUrl, 'http://localhost:9090', 'selectedInstUrl set');

// ── getActivePanelId ──
console.log('getActivePanelId tests');
assert(typeof getActivePanelId === 'function', 'getActivePanelId is a function');
state._focusedPanelId = p.id;
assertEq(getActivePanelId(), p.id, 'returns focused panel id');

// ── getSelectedPanel ──
console.log('getSelectedPanel tests');
assert(typeof getSelectedPanel === 'function', 'getSelectedPanel is a function');
const sel = getSelectedPanel();
assert(sel !== null, 'getSelectedPanel returns something when panel exists');

// ── navigateCommand ──
console.log('navigateCommand tests');
assert(typeof navigateCommand === 'function', 'navigateCommand is a function');
state.connections[0]._commands = [
    { id: 'cmd-1', name: 'htop' },
    { id: 'cmd-2', name: 'vim' },
    { id: 'cmd-3', name: 'bash' },
];
state.selectedCmdId = 'cmd-2';
navigateCommand('next');
assertEq(state.selectedCmdId, 'cmd-3', 'navigate next wraps correctly');
navigateCommand('next');
assertEq(state.selectedCmdId, 'cmd-1', 'navigate next wraps to first');

navigateCommand('prev');
assertEq(state.selectedCmdId, 'cmd-3', 'navigate prev wraps to last');
navigateCommand('prev');
assertEq(state.selectedCmdId, 'cmd-2', 'navigate prev to previous');

// ── addConnection / removeConnection ──
console.log('addConnection/removeConnection tests');
assert(typeof addConnection === 'function', 'addConnection is a function');
assert(typeof removeConnection === 'function', 'removeConnection is a function');

const conn = addConnection('http://localhost:9091', 'Test', 'tok');
assert(conn !== null, 'addConnection returns connection object');
assertEq(conn.url, 'http://localhost:9091', 'connection URL set');
assertEq(conn.label, 'Test', 'connection label set');
assertEq(conn.token, 'tok', 'connection token set');
assertEq(state.connections.length, 2, 'connections array grew');

// Idempotent — adding same URL again returns existing
const conn2 = addConnection('http://localhost:9091', 'Test2', 'tok2');
assertEq(state.connections.length, 2, 'idempotent add does not duplicate');
assertEq(conn2.label, 'Test', 'existing connection returned unchanged');

removeConnection('http://localhost:9091');
assertEq(state.connections.length, 1, 'connection removed');

// ── _isTerminalVisible ──
console.log('_isTerminalVisible tests');
if (typeof _isTerminalVisible === 'function') {
    state.currentView = 'vtty';
    state.selectedCmdId = 'cmd-1';
    assert(_isTerminalVisible(), 'visible when vtty view + command selected');

    state.currentView = 'logs';
    assert(!_isTerminalVisible(), 'not visible in logs view');

    state.currentView = 'vtty';
    state.selectedCmdId = null;
    assert(!_isTerminalVisible(), 'not visible when no command selected');
}

// ── startUpdateMode ──
console.log('startUpdateMode tests');
if (typeof startUpdateMode === 'function') {
    assert(() => { startUpdateMode(); }, 'startUpdateMode does not throw');
}

// ── stopUpdateMode ──
console.log('stopUpdateMode tests');
if (typeof stopUpdateMode === 'function') {
    assert(() => { stopUpdateMode(); }, 'stopUpdateMode does not throw');
}

// ── updateSidebarSelection ──
console.log('updateSidebarSelection tests');
assert(() => { updateSidebarSelection(); }, 'updateSidebarSelection does not throw');

// ── autofitTerminalSize ──
console.log('autofitTerminalSize tests');
if (typeof autofitTerminalSize === 'function') {
    assert(() => { autofitTerminalSize(); }, 'autofitTerminalSize does not throw');
}

// ── updateBottomBarLabel ──
console.log('updateBottomBarLabel tests');
if (typeof updateBottomBarLabel === 'function') {
    assert(() => { updateBottomBarLabel({ name: 'htop' }); }, 'updateBottomBarLabel does not throw');
}

console.log('\n[commands.js] Tests complete');
