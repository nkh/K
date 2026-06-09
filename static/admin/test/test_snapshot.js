/// test/test_snapshot.js — Tests for snapshot module (initial load)
require('./setup');

console.log('\n=== snapshot.js Tests ===\n');

// ── Load snapshot.js module (not included in setup.js moduleOrder) ──
const fs = require('fs');
const path = require('path');

function reloadSnapshotModule() {
    const code = fs.readFileSync(path.join(__dirname, '..', 'modules', 'snapshot.js'), 'utf8');
    try {
        (0, eval)(code);
    } catch (e) {
        console.error('ERROR loading snapshot.js:', e.message);
    }
}
reloadSnapshotModule();

// ── loadSnapshot function exists ──
console.log('loadSnapshot function');
assert(typeof loadSnapshot === 'function', 'loadSnapshot is a function');
assert(loadSnapshot.constructor.name === 'AsyncFunction', 'loadSnapshot is async');

// ── Run async tests ──
const origLoadCommands = globalThis.loadCommands;
const origFetch = globalThis.fetch;

(async function runTests() {

// ── Snapshot guard: no connections → falls through to loadCommands ──
console.log('snapshot guard: no connections');
resetTestState();
reloadSnapshotModule();

let loadCommandsCalled = false;
globalThis.loadCommands = function() { loadCommandsCalled = true; };
globalThis.updateDisconnectedUI = function() {};
globalThis.renderPanels = function() {};

// No connections set up — loadSnapshot should call loadCommands
state.connections = [];
await loadSnapshot();
assert(loadCommandsCalled, 'loadSnapshot calls loadCommands when no connections');

// ── Snapshot: HTTP error handling ──
console.log('snapshot HTTP error handling');
resetTestState();
reloadSnapshotModule();
loadCommandsCalled = false;
let disconnectUICalled = false;
globalThis.loadCommands = function() { loadCommandsCalled = true; };
globalThis.updateDisconnectedUI = function() { disconnectUICalled = true; };
globalThis.renderPanels = function() {};
globalThis.startUpdateMode = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};

// Set up a primary connection
state.connections = [{
    url: 'http://localhost:9090',
    label: 'Local',
    token: '',
    reachable: undefined,
    _commands: null,
    _lastError: null,
}];

// Mock fetch to return an error
globalThis.fetch = async function(url, opts) {
    return { ok: false, status: 500, statusText: 'Internal Server Error', json: async () => ({ status: 'error' }), text: async () => '', clone() { return this; } };
};

await loadSnapshot();
assert(loadCommandsCalled, 'loadSnapshot falls back to loadCommands on HTTP error');
assert(disconnectUICalled, 'updateDisconnectedUI called on error');
assertEq(state.connections[0].reachable, false, 'primary marked unreachable on error');
assert(state.connections[0]._lastError !== null, '_lastError set on error');

// ── Snapshot: successful load with commands and VTTY ──
console.log('snapshot successful load');
resetTestState();
reloadSnapshotModule();
loadCommandsCalled = false;
disconnectUICalled = false;
let sidebarBuilt = false;
let updateModeStarted = false;
let panelInfoUpdated = false;

globalThis.loadCommands = function() { loadCommandsCalled = true; };
globalThis.updateDisconnectedUI = function() { disconnectUICalled = true; };
globalThis.renderPanels = function() {};
globalThis.startUpdateMode = function() { updateModeStarted = true; };
globalThis.updatePanelCommandInfo = function() { panelInfoUpdated = true; };
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis._buildSidebar = function() { sidebarBuilt = true; };
globalThis.buildCellGrid = function() {};
globalThis.updateVttyMetadataFromHttp = function() {};

// Create panel first
const panel = addPanelDirect();
state._focusedPanelId = panel.id;

state.connections = [{
    url: 'http://localhost:9090',
    label: 'Local',
    token: '',
    reachable: undefined,
    _commands: null,
    _lastError: null,
}];

// Create a mock DOM element for the panel
const panelEl = document.createElement('div');
panelEl.id = panel.id;
_elementRegistry.set(panel.id, panelEl);
const vttyContainer = document.createElement('div');
vttyContainer.className = 'vtty-container';
panelEl.appendChild(vttyContainer);
const pre = document.createElement('pre');
vttyContainer.appendChild(pre);

// Mock fetch to return valid snapshot data
const snapshotCommands = [
    { id: 'cmd-1', name: 'myapp', alive: true, pid: 123 },
    { id: 'cmd-2', name: 'other', alive: false, pid: 124 },
];
const snapshotResources = {
    'cmd-1': { cpu: 0.5, memory_mb: 128 },
};
const snapshotVtty = {
    html: '<span class="a">Hello</span>\n<span class="a">World</span>',
    generation: 42,
    dimensions: { rows: 24, cols: 80 },
};

globalThis.fetch = async function(url, opts) {
    if (url.includes('/api/snapshot')) {
        return {
            ok: true, status: 200, statusText: 'OK',
            json: async () => ({
                status: 'ok',
                data: { commands: snapshotCommands, vtty: snapshotVtty, resources: snapshotResources },
            }),
            text: async () => '', clone() { return this; },
        };
    }
    return {
        ok: true, status: 200, statusText: 'OK',
        json: async () => ({ status: 'ok', data: [] }),
        text: async () => '', clone() { return this; },
    };
};

await loadSnapshot();

// Verify commands stored on primary instance
assert(state.connections[0]._commands !== null, 'commands stored on primary instance');
assertEq(state.connections[0]._commands.length, 2, '2 commands loaded');
assertEq(state.connections[0]._commands[0].id, 'cmd-1', 'first command ID correct');
assertEq(state.connections[0].reachable, true, 'primary marked reachable');
assertEq(state.connections[0]._lastError, null, 'no error on success');

// Verify selection: first alive command should be selected
assertEq(state.selectedInstUrl, 'http://localhost:9090', 'selectedInstUrl set to primary');
assertEq(state.selectedCmdId, 'cmd-1', 'selectedCmdId set to first alive command');

// Verify resources cached
assert(state._resourceCache['cmd-1'] !== undefined, 'resources cached for cmd-1');
assertEq(state._resourceCache['cmd-1'].cpu, 0.5, 'CPU resource cached correctly');
assertEq(state._resourceCache['cmd-1'].memory_mb, 128, 'memory resource cached correctly');

// Verify generation stored
assert(state._lastGeneration['cmd-1'] !== undefined, 'generation stored for cmd-1');
assertEq(state._lastGeneration['cmd-1'], 42, 'generation value correct');

// Verify VTTY state cleared for fresh start
assertEq(state._pendingVttyData, null, 'pending VTTY data cleared');
assertEq(state._pendingVttyDirty, false, 'pending VTTY dirty cleared');
assertEq(state.bufferView, 'current', 'bufferView reset to current');

// Verify side effects called
assert(sidebarBuilt, '_buildSidebar called after snapshot');
assert(updateModeStarted, 'startUpdateMode called');
assert(panelInfoUpdated, 'updatePanelCommandInfo called');
assert(disconnectUICalled, 'updateDisconnectedUI called');

// ── Snapshot: welcome state when no commands ──
console.log('snapshot welcome state');
resetTestState();
reloadSnapshotModule();
let welcomeShown = false;
globalThis.loadCommands = function() {};
globalThis.updateDisconnectedUI = function() {};
globalThis.renderPanels = function() { welcomeShown = true; };
globalThis.startUpdateMode = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis._buildSidebar = function() {};

state.connections = [{
    url: 'http://localhost:9090',
    label: 'Local',
    token: '',
    reachable: undefined,
    _commands: null,
    _lastError: null,
}];
state.serverReachable = false;
state.selectedCmdId = null;

globalThis.fetch = async function(url, opts) {
    return {
        ok: true, status: 200, statusText: 'OK',
        json: async () => ({
            status: 'ok',
            data: { commands: [], vtty: {} },
        }),
        text: async () => '', clone() { return this; },
    };
};

await loadSnapshot();
// After resetTestState, _showingWelcome is already true, so no transition needed.
// renderPanels is not called when welcome state doesn't change.
assertEq(welcomeShown, false, 'renderPanels not called when welcome already showing');
// But _showingWelcome should still be true (correct state maintained)
assertEq(_showingWelcome, true, '_showingWelcome remains true with no commands');

// Now test the transition: set _showingWelcome to false first, then load empty snapshot
resetTestState();
reloadSnapshotModule();
let welcomeTriggered = false;
globalThis.loadCommands = function() {};
globalThis.updateDisconnectedUI = function() {};
globalThis.renderPanels = function() { welcomeTriggered = true; };
globalThis.startUpdateMode = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis._buildSidebar = function() {};

state.connections = [{
    url: 'http://localhost:9090',
    label: 'Local',
    token: '',
    reachable: undefined,
    _commands: null,
    _lastError: null,
}];
state.serverReachable = false;
state.selectedCmdId = null;
// Force _showingWelcome to false to simulate transition
_showingWelcome = false;
_lastShowingWelcome = false;
if (typeof VRW !== 'undefined') {
    VRW._showingWelcome = false;
    VRW._lastShowingWelcome = false;
}

await loadSnapshot();
assert(welcomeTriggered, 'renderPanels called when transitioning to welcome');
assertEq(_showingWelcome, true, '_showingWelcome set to true after empty snapshot');

// ── Snapshot: bad response format ──
console.log('snapshot bad response format');
resetTestState();
reloadSnapshotModule();
loadCommandsCalled = false;
globalThis.loadCommands = function() { loadCommandsCalled = true; };
globalThis.updateDisconnectedUI = function() {};
globalThis.renderPanels = function() {};

state.connections = [{
    url: 'http://localhost:9090',
    label: 'Local',
    token: '',
    reachable: undefined,
    _commands: null,
    _lastError: null,
}];

globalThis.fetch = async function(url, opts) {
    return {
        ok: true, status: 200, statusText: 'OK',
        json: async () => ({ status: 'error', data: null }),
        text: async () => '', clone() { return this; },
    };
};

await loadSnapshot();
assert(loadCommandsCalled, 'loadSnapshot falls back to loadCommands on bad response');
assertEq(state.connections[0].reachable, false, 'primary marked unreachable on bad response');

// ── Snapshot: network failure ──
console.log('snapshot network failure');
resetTestState();
reloadSnapshotModule();
loadCommandsCalled = false;
globalThis.loadCommands = function() { loadCommandsCalled = true; };
globalThis.updateDisconnectedUI = function() {};
globalThis.renderPanels = function() {};

state.connections = [{
    url: 'http://localhost:9090',
    label: 'Local',
    token: '',
    reachable: undefined,
    _commands: null,
    _lastError: null,
}];

globalThis.fetch = async function(url, opts) {
    throw new Error('NetworkError: Failed to fetch');
};

await loadSnapshot();
assert(loadCommandsCalled, 'loadSnapshot falls back to loadCommands on network error');
assertEq(state.connections[0].reachable, false, 'primary marked unreachable on network error');
assertEq(state.connections[0]._lastError, 'connection lost', '_lastError set to connection lost');

// ── Snapshot: idempotent guard (second call skips snapshot) ──
console.log('snapshot idempotent guard');
resetTestState();
reloadSnapshotModule();
globalThis.loadCommands = function() {};
globalThis.updateDisconnectedUI = function() {};
globalThis.renderPanels = function() {};
globalThis.startUpdateMode = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis._buildSidebar = function() {};
globalThis.buildCellGrid = function() {};
globalThis.updateVttyMetadataFromHttp = function() {};

state.connections = [{
    url: 'http://localhost:9090',
    label: 'Local',
    token: '',
    reachable: undefined,
    _commands: [],
    _lastError: null,
}];

const guardPanel = addPanelDirect();
state._focusedPanelId = guardPanel.id;
const guardPanelEl = document.createElement('div');
guardPanelEl.id = guardPanel.id;
_elementRegistry.set(guardPanel.id, guardPanelEl);
const guardVtty = document.createElement('div');
guardVtty.className = 'vtty-container';
guardPanelEl.appendChild(guardVtty);
const guardPre = document.createElement('pre');
guardVtty.appendChild(guardPre);

let loadCommandsFromGuard = false;
globalThis.fetch = async function(url, opts) {
    return {
        ok: true, status: 200, statusText: 'OK',
        json: async () => ({
            status: 'ok',
            data: {
                commands: [{ id: 'cmd-1', name: 'test', alive: true }],
                vtty: { html: 'test', generation: 1 },
            },
        }),
        text: async () => '', clone() { return this; },
    };
};

// First load
await loadSnapshot();
assert(state.connections[0]._commands !== null, 'first load processes snapshot');

// Mock for second call — loadCommands should be called by guard
globalThis.loadCommands = function() { loadCommandsFromGuard = true; };
// Second load should hit the guard and call loadCommands instead
await loadSnapshot();
assert(loadCommandsFromGuard, 'second loadSnapshot call skips to loadCommands (guard)');

// ── Snapshot: prefers alive command for initial selection ──
console.log('snapshot alive command selection');
resetTestState();
reloadSnapshotModule();
globalThis.renderPanels = function() {};
globalThis.startUpdateMode = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis._buildSidebar = function() {};
globalThis.buildCellGrid = function() {};
globalThis.updateVttyMetadataFromHttp = function() {};

const selPanel = addPanelDirect();
state._focusedPanelId = selPanel.id;

state.connections = [{
    url: 'http://localhost:9090',
    label: 'Local',
    token: '',
    reachable: undefined,
    _commands: null,
    _lastError: null,
}];

const selPanelEl = document.createElement('div');
selPanelEl.id = selPanel.id;
_elementRegistry.set(selPanel.id, selPanelEl);
const selVtty = document.createElement('div');
selVtty.className = 'vtty-container';
selPanelEl.appendChild(selVtty);
const selPre = document.createElement('pre');
selVtty.appendChild(selPre);

globalThis.fetch = async function(url, opts) {
    return {
        ok: true, status: 200, statusText: 'OK',
        json: async () => ({
            status: 'ok',
            data: {
                commands: [
                    { id: 'dead-1', name: 'dead-first', alive: false },
                    { id: 'dead-2', name: 'dead-second', alive: false },
                ],
                vtty: { html: '<span>output</span>', generation: 5, dimensions: { rows: 24, cols: 80 } },
            },
        }),
        text: async () => '', clone() { return this; },
    };
};

await loadSnapshot();
assertEq(state.selectedCmdId, 'dead-1', 'first command selected when all are dead');
assertEq(state._lastGeneration['dead-1'], 5, 'generation stored for dead-first');

// ── Snapshot: VTTY HTML written to panel DOM ──
console.log('snapshot VTTY HTML rendering');
resetTestState();
reloadSnapshotModule();
globalThis.renderPanels = function() {};
globalThis.startUpdateMode = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis._buildSidebar = function() {};
globalThis.buildCellGrid = function() {};
globalThis.updateVttyMetadataFromHttp = function() {};

const domPanel = addPanelDirect();
state._focusedPanelId = domPanel.id;

state.connections = [{
    url: 'http://localhost:9090',
    label: 'Local',
    token: '',
    reachable: undefined,
    _commands: null,
    _lastError: null,
}];

const domPanelEl = document.createElement('div');
domPanelEl.id = domPanel.id;
_elementRegistry.set(domPanel.id, domPanelEl);
const domVttyCont = document.createElement('div');
domVttyCont.className = 'vtty-container';
domPanelEl.appendChild(domVttyCont);
const domPre = document.createElement('pre');
domVttyCont.appendChild(domPre);

const testHtml = '<span style="color:red">Hello World</span>\n<span>Line 2</span>';

globalThis.fetch = async function(url, opts) {
    return {
        ok: true, status: 200, statusText: 'OK',
        json: async () => ({
            status: 'ok',
            data: {
                commands: [{ id: 'dom-cmd', name: 'domtest', alive: true }],
                vtty: { html: testHtml, generation: 99, dimensions: { rows: 2, cols: 40 } },
            },
        }),
        text: async () => '', clone() { return this; },
    };
};

await loadSnapshot();
assertEq(domPre.innerHTML, testHtml, 'VTTY HTML written to panel <pre> element');

// ── Snapshot: no VTTY data still stores commands ──
console.log('snapshot no VTTY still stores commands');
resetTestState();
reloadSnapshotModule();
loadCommandsCalled = false;
globalThis.loadCommands = function() {};
globalThis.updateDisconnectedUI = function() {};
globalThis.renderPanels = function() {};
globalThis.startUpdateMode = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis._buildSidebar = function() {};

state.connections = [{
    url: 'http://localhost:9090',
    label: 'Local',
    token: '',
    reachable: undefined,
    _commands: null,
    _lastError: null,
}];

globalThis.fetch = async function(url, opts) {
    return {
        ok: true, status: 200, statusText: 'OK',
        json: async () => ({
            status: 'ok',
            data: {
                commands: [{ id: 'no-vtty-cmd', name: 'novtty', alive: true }],
                vtty: {},  // no html, no generation
            },
        }),
        text: async () => '', clone() { return this; },
    };
};

await loadSnapshot();
assert(state.connections[0]._commands !== null, 'commands stored even without VTTY');
assertEq(state.connections[0]._commands.length, 1, '1 command loaded');
assertEq(state.connections[0].reachable, true, 'primary marked reachable even without VTTY');
// Without VTTY html, selection should NOT happen
assertEq(state.selectedCmdId, null, 'no command selected when VTTY is empty');

// Restore originals
globalThis.fetch = origFetch;
globalThis.loadCommands = origLoadCommands;

console.log('\n[snapshot.js] ' + _testPassed + ' passed so far');

})().catch(e => {
    console.error('Fatal test error:', e.message);
    process.exit(2);
});
