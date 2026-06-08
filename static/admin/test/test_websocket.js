/// test/test_websocket.js — Tests for WebSocket management
require('./setup');

console.log('\n=== websocket.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};
globalThis.updateVttyDisplayForPanel = function() {};
globalThis.applyVttyDiffForPanel = function() {};
globalThis.updateVttyMetadataForPanel = function() {};
globalThis.scheduleVttyHttpForPanel = function() {};

// ── connectPanelWs ──
console.log('connectPanelWs tests');
assert(typeof connectPanelWs === 'function', 'connectPanelWs is a function');
state.panels = [];
state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
const p = addPanelDirect();
p.selectedInstUrl = 'http://localhost:9090';
p.selectedCmdId = 'cmd-test-ws';
state._focusedPanelId = p.id;

assert(() => { connectPanelWs(p.id); }, 'connectPanelWs does not throw');
// WebSocket should be created
assert(p.ws !== null, 'WebSocket created for panel');

// ── disconnectPanelWs ──
console.log('disconnectPanelWs tests');
assert(typeof disconnectPanelWs === 'function', 'disconnectPanelWs is a function');
assert(() => { disconnectPanelWs(p.id); }, 'disconnectPanelWs does not throw');
assert(p.ws === null || p.ws.readyState === 3, 'WebSocket disconnected');

// ── disconnectAllPanelWs ──
console.log('disconnectAllPanelWs tests');
if (typeof disconnectAllPanelWs === 'function') {
    state.panels = [];
    const p1 = addPanelDirect();
    const p2 = addPanelDirect();
    p1.selectedInstUrl = 'http://localhost:9090';
    p1.selectedCmdId = 'cmd-1';
    p2.selectedInstUrl = 'http://localhost:9090';
    p2.selectedCmdId = 'cmd-2';
    connectPanelWs(p1.id);
    connectPanelWs(p2.id);
    disconnectAllPanelWs();
    assert(p1.ws === null || p1.ws.readyState === 3, 'p1 ws disconnected');
    assert(p2.ws === null || p2.ws.readyState === 3, 'p2 ws disconnected');
}

// ── updateWsQualityIndicator ──
console.log('updateWsQualityIndicator tests');
if (typeof updateWsQualityIndicator === 'function') {
    const indicator = document.createElement('span');
    indicator.id = 'wsQuality';
    assert(() => { updateWsQualityIndicator(); }, 'updateWsQualityIndicator does not throw');
}

// ── poll mode functions ──
console.log('poll mode tests');
if (typeof startPanelPoll === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-poll';
    assert(() => { startPanelPoll(p.id); }, 'startPanelPoll does not throw');
}

if (typeof stopPanelPoll === 'function') {
    assert(() => { stopPanelPoll(p.id); }, 'stopPanelPoll does not throw');
}

if (typeof pollOncePanel === 'function') {
    assert(() => { pollOncePanel(p.id); }, 'pollOncePanel does not throw');
}

// ── Legacy WS functions ──
console.log('legacy WS functions');
if (typeof connectVttyWs === 'function') {
    assert(() => { connectVttyWs(); }, 'connectVttyWs does not throw');
}
if (typeof disconnectVttyWs === 'function') {
    assert(() => { disconnectVttyWs(); }, 'disconnectVttyWs does not throw');
}
if (typeof startPoll === 'function') {
    assert(() => { startPoll(); }, 'startPoll does not throw');
}
if (typeof stopPoll === 'function') {
    assert(() => { stopPoll(); }, 'stopPoll does not throw');
}

console.log('\n[websocket.js] Tests complete');
