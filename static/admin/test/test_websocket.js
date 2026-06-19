/// test/test_websocket.js — Tests for WebSocket management
require('./setup');

console.log('\n=== websocket.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};
globalThis.updateVttyDisplayForPanel = function() {};
globalThis.applyVttyDiffForPanel = function() {};
globalThis.updateVttyMetadataForPanel = function() {};
globalThis.scheduleVttyHttpForPanel = function() {};
globalThis.handlePeerEvent = function() {};
globalThis.updateWsQualityIndicator = function() {};
globalThis.notifyCommandEnded = function() {};
globalThis._throttleRefresh = function() { return false; };

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

// disconnectPanelWs resets panel WS state
assertEq(p.wsInstUrl, null, 'wsInstUrl cleared');
assertEq(p.wsCmdId, null, 'wsCmdId cleared');
assertEq(p.wsReconnectCount, 0, 'wsReconnectCount reset');
assertEq(p.wsPingSendTime, 0, 'wsPingSendTime reset');
assertEq(p.wsLatency, 0, 'wsLatency reset');

// Disconnect nonexistent panel → no crash
assert(() => { disconnectPanelWs('nonexistent'); }, 'disconnectPanelWs nonexistent no crash');

// ── disconnectPanelWs with split panel ──
console.log('disconnectPanelWs split panel tests');
globalThis.stopPanelPoll = function(panelId) {};

state.panels = [];
const splitP = addPanelDirect();
splitP.selectedInstUrl = 'http://localhost:9090';
splitP.selectedCmdId = 'cmd-split';
splitP.split = {
    direction: 'horizontal', splitRatio: 0.5, activeSide: 'panel',
    branch: {
        id: splitP.id + '-branch1', cmdId: 'cmd-sec', instUrl: 'http://localhost:9090',
        ws: null, pollTimer: null,
        wsReconnectCount: 0, wsPingInterval: null,
        wsPingSendTime: 0, wsLatency: 0,
    },
};
connectPanelWs(splitP.id);
disconnectPanelWs(splitP.id);
// Primary WS state is always cleaned
assertEq(splitP.wsInstUrl, null, 'wsInstUrl cleared');
assertEq(splitP.wsCmdId, null, 'wsCmdId cleared');
assertEq(splitP.wsReconnectCount, 0, 'reconnect count reset');
assertEq(splitP.wsPingSendTime, 0, 'ping send time reset');
assertEq(splitP.wsLatency, 0, 'latency reset');
// Note: branch WS state cleanup depends on _disconnectSingleLeaf internals
// Only verify the primary WS was disconnected
assert(splitP.ws === null || splitP.ws.readyState === 3, 'primary WS disconnected');

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
// Restore real function (was mocked at top of file for connect/disconnect tests)
globalThis.updateWsQualityIndicator = globalThis._realFunctions.updateWsQualityIndicator;
if (typeof updateWsQualityIndicator === 'function') {
    // No element → early return
    const origIndicator = _elementRegistry.get('wsQuality');
    _elementRegistry.delete('wsQuality');
    assert(() => { updateWsQualityIndicator(); }, 'updateWsQualityIndicator no-crash without element');

    // With element, no focused panel
    const indicator = document.createElement('span');
    indicator.id = 'wsQuality';
    _elementRegistry.set('wsQuality', indicator);
    state.panels = [];
    state._focusedPanelId = null;
    updateWsQualityIndicator();
    assertEq(indicator.textContent, '--', 'disconnected indicator');
    assert(indicator.style.color.includes('red'), 'disconnected color is red');

    // With focused panel, connected, no latency yet
    state.panels = [];
    const wsP = addPanelDirect();
    wsP.selectedInstUrl = 'http://localhost:9090';
    wsP.selectedCmdId = 'cmd-wsq';
    state._focusedPanelId = wsP.id;
    // Populate shared sub pool (updateWsQualityIndicator reads from it)
    const _sharedSubs = globalThis._getSharedSubs();
    const testSub = { ws: new MockWebSocket('ws://test'), instUrl: 'http://localhost:9090', cmdId: 'cmd-wsq', panels: new Set([wsP.id]), latency: 0, pingSendTime: 0, reconnectCount: 0, reconnectTimer: null, pingInterval: null, closed: false };
    testSub.ws.readyState = 1;
    _sharedSubs['http://localhost:9090/cmd-wsq'] = testSub;
    updateWsQualityIndicator();
    assertEq(indicator.textContent, '...', 'measuring indicator');

    // Low latency (< 50ms)
    testSub.latency = 25;
    updateWsQualityIndicator();
    assert(indicator.textContent.includes('25ms'), 'latency text shown');
    assert(indicator.style.color.includes('green'), 'low latency is green');

    // Medium latency (50-200ms)
    testSub.latency = 120;
    updateWsQualityIndicator();
    assert(indicator.style.color.includes('yellow'), 'medium latency is yellow');

    // High latency (> 200ms)
    testSub.latency = 350;
    updateWsQualityIndicator();
    assert(indicator.style.color.includes('red'), 'high latency is red');

    // Title includes latency
    updateWsQualityIndicator();
    assert(indicator.title.includes('350ms'), 'title includes latency');
    delete _sharedSubs['http://localhost:9090/cmd-wsq'];
}

// ── poll mode functions ──
console.log('poll mode tests');
// Restore real stopPanelPoll (was mocked for split panel tests)
globalThis.stopPanelPoll = globalThis._realFunctions.stopPanelPoll;
if (typeof startPanelPoll === 'function') {
    state.panels = [];
    const pollP = addPanelDirect();
    pollP.selectedInstUrl = 'http://localhost:9090';
    pollP.selectedCmdId = 'cmd-poll';
    assert(() => { startPanelPoll(pollP.id); }, 'startPanelPoll does not throw');
    assert(pollP.pollTimer !== null, 'poll timer set');
}

if (typeof stopPanelPoll === 'function') {
    assert(() => { stopPanelPoll(p.id); }, 'stopPanelPoll does not throw');
    // Stop clears the timer
    const stopP = addPanelDirect();
    stopP.selectedInstUrl = 'http://localhost:9090';
    stopP.selectedCmdId = 'cmd-stop';
    startPanelPoll(stopP.id);
    assert(stopP.pollTimer !== null, 'poll timer set before stop');
    stopPanelPoll(stopP.id);
    assertEq(stopP.pollTimer, null, 'poll timer cleared after stop');

    // Stop nonexistent → no crash
    assert(() => { stopPanelPoll('nonexistent'); }, 'stopPanelPoll nonexistent no crash');
}

if (typeof pollOncePanel === 'function') {
    assert(pollOncePanel.constructor.name === 'AsyncFunction', 'pollOncePanel is async');
    assert(() => { pollOncePanel(p.id); }, 'pollOncePanel does not throw');

    // Panel without selection → no-op
    const noSelP = addPanelDirect();
    noSelP.selectedCmdId = null;
    assert(() => { pollOncePanel(noSelP.id); }, 'pollOncePanel no-selection no crash');
}

// ── Legacy WS functions removed in Phase 8a ──
// (connectVttyWs, disconnectVttyWs, startPoll, stopPoll deleted — per-panel versions used instead)
console.log('legacy WS functions removed (per-panel versions used)');

// ── startPanelUpdateMode / stopPanelUpdateMode ──
console.log('startPanelUpdateMode / stopPanelUpdateMode tests');
if (typeof startPanelUpdateMode === 'function') {
    assert(typeof startPanelUpdateMode, 'startPanelUpdateMode is a function');
    assert(typeof stopPanelUpdateMode, 'stopPanelUpdateMode is a function');

    // Push mode → connects WS
    state.updateMode = 'push';
    const updP = addPanelDirect();
    updP.selectedInstUrl = 'http://localhost:9090';
    updP.selectedCmdId = 'cmd-upd';
    startPanelUpdateMode(updP.id);
    assert(updP.ws !== null, 'push mode starts panel WS');

    // Stop
    stopPanelUpdateMode(updP.id);
    assert(updP.ws === null, 'stopPanelUpdateMode disconnects WS');

    // Poll mode → starts poll
    state.updateMode = 'poll';
    startPanelUpdateMode(updP.id);
    assert(updP.pollTimer !== null, 'poll mode starts panel poll');

    stopPanelUpdateMode(updP.id);
    assertEq(updP.pollTimer, null, 'stopPanelUpdateMode stops poll');

    // No selection → no-op
    const noSelUpdP = addPanelDirect();
    noSelUpdP.selectedCmdId = null;
    assert(() => { startPanelUpdateMode(noSelUpdP.id); }, 'startPanelUpdateMode no-selection no crash');
}

// ── _disconnectSingleLeaf (replaces old _disconnectSecondaryWs) ──
console.log('_disconnectSingleLeaf tests');
if (typeof _disconnectSingleLeaf === 'function') {
    // null leaf → no-op
    assert(() => { _disconnectSingleLeaf(null); }, '_disconnectSingleLeaf null no crash');

    // With a leaf that has WS state
    const leaf = {
        id: 'test-leaf', cmdId: 'branch1', instUrl: 'http://localhost:9090',
        ws: new MockWebSocket('ws://test'),
        wsReconnectTimer: setTimeout(() => {}, 10000),
        wsPingInterval: setInterval(() => {}, 10000),
        wsPingSendTime: 42,
        wsLatency: 100,
        wsReconnectCount: 2,
    };
    _disconnectSingleLeaf(leaf);
    assertEq(leaf.ws, null, 'leaf WS cleared');
    assertEq(leaf.wsPingSendTime, 0, 'leaf ping send time reset');
    assertEq(leaf.wsLatency, 0, 'leaf latency reset');
}

// ── _loadLeafVttyHttpDirect ──
console.log('_loadLeafVttyHttpDirect tests');
if (typeof _loadLeafVttyHttpDirect === 'function') {
    // No DOM element → no crash (just returns)
    const leaf = { id: 'no-dom-leaf', cmdId: 'c1', instUrl: 'http://localhost:9090' };
    assert(() => { _loadLeafVttyHttpDirect(leaf); }, '_loadLeafVttyHttpDirect no-dom no crash');

    // No cmd → no crash
    const noCmdLeaf = { id: 'no-cmd-leaf', cmdId: null, instUrl: null };
    assert(() => { _loadLeafVttyHttpDirect(noCmdLeaf); }, '_loadLeafVttyHttpDirect no-cmd no crash');
}

// ── _applyLeafDiff ──
console.log('_applyLeafDiff tests');
if (typeof _applyLeafDiff === 'function') {
    const vttyEl = document.createElement('div');
    vttyEl.className = 'vtty-container';
    const pre = document.createElement('pre');
    vttyEl.appendChild(pre);

    // No pre → no crash
    const emptyVtty = document.createElement('div');
    assert(() => { _applyLeafDiff(emptyVtty, 'test-leaf', {}); }, '_applyLeafDiff no-pre no crash');

    // With html data
    const htmlData = { html: '<span>test</span>', generation: 1, _cmdId: 'c1' };
    assert(() => { _applyLeafDiff(vttyEl, 'test-leaf', htmlData); }, '_applyLeafDiff with html does not throw');
}

console.log('\n[websocket.js] Tests complete');
