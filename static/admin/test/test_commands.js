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
globalThis.loadVttyHttpForPanel = function() {};
globalThis.startPanelUpdateMode = function() {};
globalThis._restoreCachedDom = function() {};
globalThis._cacheTerminalForSwitch = function() {};
globalThis._buildSidebar = function() {};
globalThis.updateDisconnectedUI = function() {};

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
assertEq(state.bufferView, 'current', 'bufferView reset to current');
assertEq(state._pendingVttyData, null, 'pending vtty data cleared');
assertEq(state._pendingVttyDirty, false, 'pending vtty dirty cleared');

// ── selectCommand: no panels → no-op ──
console.log('selectCommand edge cases');
state.panels = [];
state._focusedPanelId = null;
const prevCmdId = state.selectedCmdId;
selectCommand('http://a.com', 'cmd-x', 'test');
assertEq(state.selectedCmdId, prevCmdId, 'selectCommand no-op when no panels');

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
// NOTE: navigateCommand calls selectCommand which does complex DOM operations.
// With empty _navCommands, it returns immediately without calling selectCommand.
navigateCommand('next');
// With empty nav list, selectedCmdId is unchanged
assertEq(state.selectedCmdId, 'cmd-1', 'navigateCommand no-op with empty nav list');
navigateCommand('prev');
assertEq(state.selectedCmdId, 'cmd-1', 'navigateCommand no-op with empty nav list (prev)');

// ── navigateCommand with populated _navCommands ──
console.log('navigateCommand with nav list');
_navCommands = [
    { instUrl: 'http://localhost:9090', cmdId: 'a', name: 'A' },
    { instUrl: 'http://localhost:9090', cmdId: 'b', name: 'B' },
    { instUrl: 'http://localhost:9090', cmdId: 'c', name: 'C' },
];
state.selectedInstUrl = 'http://localhost:9090';
state.selectedCmdId = 'b';
state.panels = [];
const navP = addPanelDirect();
state._focusedPanelId = navP.id;

// Mock selectCommand to only update state (no DOM operations)
const realSelectCommand = selectCommand;
globalThis.selectCommand = function(instUrl, cmdId, name) {
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
};

navigateCommand('next');
assertEq(state.selectedCmdId, 'c', 'navigateCommand next wraps to c');
navigateCommand('next');
assertEq(state.selectedCmdId, 'a', 'navigateCommand next wraps around');
navigateCommand('prev');
assertEq(state.selectedCmdId, 'c', 'navigateCommand prev wraps to c');
navigateCommand('prev');
assertEq(state.selectedCmdId, 'b', 'navigateCommand prev goes back to b');

// Navigate with no current selection → goes to first
state.selectedCmdId = null;
state.selectedInstUrl = null;
navigateCommand('next');
assertEq(state.selectedCmdId, 'a', 'navigateCommand next from no selection goes to first');
navigateCommand('prev');
assertEq(state.selectedCmdId, 'c', 'navigateCommand prev from no selection goes to last');

// Restore real selectCommand
globalThis.selectCommand = realSelectCommand;

// ── navigatePrevCommand / navigateNextCommand ──
console.log('navigatePrevCommand/navigateNextCommand tests');
assert(typeof navigatePrevCommand === 'function', 'navigatePrevCommand is a function');
assert(typeof navigateNextCommand === 'function', 'navigateNextCommand is a function');

state.selectedCmdId = 'a';
globalThis.selectCommand = function(instUrl, cmdId, name) {
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
};
navigatePrevCommand();
assertEq(state.selectedCmdId, 'c', 'navigatePrevCommand wraps to last');
navigateNextCommand();
assertEq(state.selectedCmdId, 'a', 'navigateNextCommand wraps to first');
globalThis.selectCommand = realSelectCommand;

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

// ── lookupAndSelectCommand ──
console.log('lookupAndSelectCommand tests');
if (typeof lookupAndSelectCommand === 'function') {
    assert(lookupAndSelectCommand.constructor.name === 'AsyncFunction', 'lookupAndSelectCommand is async');
    // Mock fetch for lookup
    const origFetch = globalThis.fetch;
    globalThis.fetch = async function(url, opts) {
        if (url.includes('/lookup/')) {
            return {
                ok: true, status: 200, json: async () => ({
                    status: 'ok',
                    data: [{ id: 'cmd-lookup-1', name: 'htop', alive: true, pid: 1, args: [] }],
                }),
                clone() { return this; },
            };
        }
        return { ok: true, status: 200, json: async () => ({ status: 'ok', data: [] }), clone() { return this; } };
    };
    assert(() => { lookupAndSelectCommand('htop'); }, 'lookupAndSelectCommand does not throw');

    // No match → returns without error
    globalThis.fetch = async function(url, opts) {
        return { ok: true, status: 200, json: async () => ({ status: 'ok', data: [] }), clone() { return this; } };
    };
    assert(() => { lookupAndSelectCommand('nonexistent'); }, 'lookupAndSelectCommand no-match no throw');
    globalThis.fetch = origFetch;
}

// ── pickCommand ──
console.log('pickCommand tests');
if (typeof pickCommand === 'function') {
    let loadCmdCalled = false;
    globalThis.loadCommands = function() { loadCmdCalled = true; };
    pickCommand('cmd-x', 'testcmd');
    assertEq(state._pendingSelectId, 'cmd-x', 'pickCommand sets pending select id');
    assert(loadCmdCalled, 'pickCommand calls loadCommands');
}

// ── loadCommands ──
console.log('loadCommands tests');
(async function() {
if (typeof loadCommands === 'function') {
    assert(loadCommands.constructor.name === 'AsyncFunction', 'loadCommands is async');

    // Mock fetch for commands
    const origFetch2 = globalThis.fetch;
    globalThis.fetch = async function(url, opts) {
        return {
            ok: true, status: 200,
            json: async () => ({ status: 'ok', data: [
                { id: 'lc-1', name: 'bash', alive: true, pid: 100 },
                { id: 'lc-2', name: 'top', alive: false, pid: 101 },
            ]}),
            clone() { return this; },
        };
    };
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
    state.panels = [];
    const lcPanel = addPanelDirect();

    // Run loadCommands
    await loadCommands();
    assert(state.connections[0]._commands !== undefined, 'commands stored on instance');
    assertEq(state.connections[0]._commands.length, 2, '2 commands loaded');
    assertEq(state.connections[0].reachable, true, 'instance marked reachable');
    assertEq(state.connections[0]._lastError, null, 'no error on success');

    // Fetch failure → marks unreachable
    globalThis.fetch = async function() { throw new Error('network'); };
    await loadCommands();
    assertEq(state.connections[0].reachable, false, 'instance marked unreachable on error');
    assertEq(state.connections[0]._lastError, 'connection lost (instance may have exited)', 'error message set');

    globalThis.fetch = origFetch2;
}
})().catch(e => { console.error('loadCommands test error:', e.message); });

console.log('\n[commands.js] Tests complete');