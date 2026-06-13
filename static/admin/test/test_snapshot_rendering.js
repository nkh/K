/// test/test_snapshot_rendering.js — Snapshot selection, rendering, edge cases
require('./setup');

console.log('\n=== snapshot.js Tests — Rendering & Selection ===\n');

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

const origLoadCommands = globalThis.loadCommands;
const origFetch = globalThis.fetch;

// Helper: create a panel with vtty-container > pre DOM structure
function setupPanelWithDom() {
    const panel = addPanelDirect();
    state._focusedPanelId = panel.id;
    const panelEl = document.createElement('div');
    panelEl.id = panel.id;
    _elementRegistry.set(panel.id, panelEl);
    const vttyCont = document.createElement('div');
    vttyCont.className = 'vtty-container';
    panelEl.appendChild(vttyCont);
    const pre = document.createElement('pre');
    vttyCont.appendChild(pre);
    return { panel, panelEl, pre };
}

// Helper: setup standard mock environment
function setupSnapshotEnv() {
    resetTestState();
    reloadSnapshotModule();
    globalThis.loadCommands = function() {};
    globalThis.updateDisconnectedUI = function() {};
    globalThis.renderPanels = function() {};
    globalThis.startPanelUpdateMode = function() {};
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
        _commands: null,
        _lastError: null,
    }];
}

(async function runTests() {

// ── Snapshot: idempotent guard (second call skips snapshot) ──
console.log('snapshot idempotent guard');
setupSnapshotEnv();
const { panel: guardPanel, pre: guardPre } = setupPanelWithDom();

state.connections[0]._commands = [];

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
setupSnapshotEnv();
setupPanelWithDom();

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

// ── Snapshot: alive command preferred over dead ──
console.log('snapshot alive preferred over dead');
setupSnapshotEnv();
setupPanelWithDom();

globalThis.fetch = async function(url, opts) {
    return {
        ok: true, status: 200, statusText: 'OK',
        json: async () => ({
            status: 'ok',
            data: {
                commands: [
                    { id: 'dead-first', name: 'dead', alive: false },
                    { id: 'alive-second', name: 'alive', alive: true },
                ],
                vtty: { html: '<span>output</span>', generation: 7, dimensions: { rows: 24, cols: 80 } },
            },
        }),
        text: async () => '', clone() { return this; },
    };
};

await loadSnapshot();
assertEq(state.selectedCmdId, 'alive-second', 'alive command selected over dead ones');
assertEq(state._lastGeneration['alive-second'], 7, 'generation stored for alive command');

// ── Snapshot: VTTY HTML written to panel DOM ──
console.log('snapshot VTTY HTML rendering');
setupSnapshotEnv();
const { pre: domPre } = setupPanelWithDom();

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

// ── Snapshot: VTTY HTML with special characters ──
console.log('snapshot VTTY HTML special chars');
setupSnapshotEnv();
const { pre: specPre } = setupPanelWithDom();

const specialHtml = '<span class="fg-red">&lt;tag&gt; &amp; "quoted" \'</span>';

globalThis.fetch = async function(url, opts) {
    return {
        ok: true, status: 200, statusText: 'OK',
        json: async () => ({
            status: 'ok',
            data: {
                commands: [{ id: 'spec-cmd', name: 'spectest', alive: true }],
                vtty: { html: specialHtml, generation: 10, dimensions: { rows: 1, cols: 40 } },
            },
        }),
        text: async () => '', clone() { return this; },
    };
};

await loadSnapshot();
assertEq(specPre.innerHTML, specialHtml, 'special HTML chars preserved correctly');

// ── Snapshot: no VTTY data still stores commands ──
console.log('snapshot no VTTY still stores commands');
setupSnapshotEnv();

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
assertEq(state.selectedCmdId, null, 'no command selected when VTTY is empty');

// ── Snapshot: multiple connections — uses primary ──
console.log('snapshot multiple connections uses primary');
setupSnapshotEnv();
setupPanelWithDom();

state.connections.push({
    url: 'http://localhost:9091',
    label: 'Secondary',
    token: '',
    reachable: undefined,
    _commands: null,
    _lastError: null,
});

let primaryFetched = false;
globalThis.fetch = async function(url, opts) {
    if (url.includes('localhost:9090')) primaryFetched = true;
    return {
        ok: true, status: 200, statusText: 'OK',
        json: async () => ({
            status: 'ok',
            data: {
                commands: [{ id: 'multi-cmd', name: 'multi', alive: true }],
                vtty: { html: 'multi-out', generation: 3 },
            },
        }),
        text: async () => '', clone() { return this; },
    };
};

await loadSnapshot();
assert(primaryFetched, 'snapshot fetched from primary connection URL');

// Restore originals
globalThis.fetch = origFetch;
globalThis.loadCommands = origLoadCommands;

console.log('\n[snapshot-rendering.js] ' + _testPassed + ' passed so far');

})().catch(e => {
    console.error('Fatal test error:', e.message);
    process.exit(2);
});
