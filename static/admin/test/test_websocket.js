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
    direction: 'horizontal', splitRatio: 0.5, activeSide: 'primary',
    secondaryCmdId: 'cmd-sec', secondaryInstUrl: 'http://localhost:9090',
    secondaryWs: null, secondaryPollTimer: null,
    secondaryWsReconnectCount: 0, secondaryWsPingInterval: null,
    secondaryWsPingSendTime: 0, secondaryWsLatency: 0,
};
connectPanelWs(splitP.id);
disconnectPanelWs(splitP.id);
// Primary WS state is always cleaned
assertEq(splitP.wsInstUrl, null, 'wsInstUrl cleared');
assertEq(splitP.wsCmdId, null, 'wsCmdId cleared');
assertEq(splitP.wsReconnectCount, 0, 'reconnect count reset');
assertEq(splitP.wsPingSendTime, 0, 'ping send time reset');
assertEq(splitP.wsLatency, 0, 'latency reset');
// Note: secondary WS state cleanup depends on _disconnectSecondaryWs internals
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
    state._focusedPanelId = wsP.id;
    wsP.ws = new MockWebSocket('ws://test');
    wsP.ws.readyState = 1;
    wsP.wsLatency = 0;
    updateWsQualityIndicator();
    assertEq(indicator.textContent, '...', 'measuring indicator');

    // Low latency (< 50ms)
    wsP.wsLatency = 25;
    updateWsQualityIndicator();
    assert(indicator.textContent.includes('25ms'), 'latency text shown');
    assert(indicator.style.color.includes('green'), 'low latency is green');

    // Medium latency (50-200ms)
    wsP.wsLatency = 120;
    updateWsQualityIndicator();
    assert(indicator.style.color.includes('yellow'), 'medium latency is yellow');

    // High latency (> 200ms)
    wsP.wsLatency = 350;
    updateWsQualityIndicator();
    assert(indicator.style.color.includes('red'), 'high latency is red');

    // Title includes reconnect count
    wsP.wsReconnectCount = 3;
    updateWsQualityIndicator();
    assert(indicator.title.includes('3'), 'title includes reconnect count');
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

// ── _disconnectSecondaryWs ──
console.log('_disconnectSecondaryWs tests');
if (typeof _disconnectSecondaryWs === 'function') {
    // No split → no-op
    const noSplitP = addPanelDirect();
    assert(() => { _disconnectSecondaryWs(noSplitP); }, '_disconnectSecondaryWs no-split no crash');

    // With split
    const splitP2 = addPanelDirect();
    splitP2.split = {
        direction: 'horizontal', splitRatio: 0.5, activeSide: 'primary',
        secondaryCmdId: 's1', secondaryInstUrl: 'http://localhost:9090',
        secondaryWs: new MockWebSocket('ws://test'),
        secondaryWsReconnectTimer: setTimeout(() => {}, 10000),
        secondaryWsPingInterval: setInterval(() => {}, 10000),
        secondaryWsPingSendTime: 42,
        secondaryWsLatency: 100,
        secondaryWsReconnectCount: 2,
        secondaryWsCmdId: 's1', secondaryWsInstUrl: 'http://localhost:9090',
    };
    _disconnectSecondaryWs(splitP2);
    assertEq(splitP2.split.secondaryWs, null, 'secondary WS cleared');
    assertEq(splitP2.split.secondaryWsCmdId, null, 'secondary wsCmdId cleared');
    assertEq(splitP2.split.secondaryWsInstUrl, null, 'secondary wsInstUrl cleared');
    assertEq(splitP2.split.secondaryWsPingSendTime, 0, 'secondary ping send time reset');
    assertEq(splitP2.split.secondaryWsLatency, 0, 'secondary latency reset');
    assertEq(splitP2.split.secondaryWsReconnectCount, 0, 'secondary reconnect count reset');
}

// ── scheduleSecondaryVttyHttp (now in vtty.js) ──
console.log('scheduleSecondaryVttyHttp tests');
if (typeof scheduleSecondaryVttyHttp === 'function') {
    const schedP = addPanelDirect();
    schedP.split = { secondaryCmdId: 's1', secondaryInstUrl: 'http://localhost:9090' };
    assert(() => { scheduleSecondaryVttyHttp(schedP, 50); }, 'scheduleSecondaryVttyHttp does not throw');

    const noSplitSchedP = addPanelDirect();
    assert(() => { scheduleSecondaryVttyHttp(noSplitSchedP, 50); }, 'scheduleSecondaryVttyHttp no-split no crash');

    const noCmdSplitP = addPanelDirect();
    noCmdSplitP.split = { secondaryCmdId: null, secondaryInstUrl: null };
    assert(() => { scheduleSecondaryVttyHttp(noCmdSplitP, 50); }, 'scheduleSecondaryVttyHttp no-cmd no crash');
}

// ── updateSecondaryVttyDisplay (now in vtty.js) ──
console.log('updateSecondaryVttyDisplay tests');
if (typeof updateSecondaryVttyDisplay === 'function') {
    const dispP = addPanelDirect();
    dispP.split = { secondaryCmdId: 's1', secondaryInstUrl: 'http://localhost:9090' };
    const dispVtty = document.createElement('div');
    const dispPre = document.createElement('pre');
    dispVtty.appendChild(dispPre);
    assert(() => { updateSecondaryVttyDisplay(dispP, dispVtty, { html: '<span>hi</span>', generation: 1 }); }, 'updateSecondaryVttyDisplay does not throw');
}

// ── applySecondaryVttyDiff (now in vtty.js) ──
console.log('applySecondaryVttyDiff tests');
if (typeof applySecondaryVttyDiff === 'function') {
    const diffP = addPanelDirect();
    diffP.split = { secondaryCmdId: 's1' };
    const diffVtty = document.createElement('div');
    diffVtty.className = 'vtty-container';
    const diffPre = document.createElement('pre');
    diffVtty.appendChild(diffPre);

    const noCmdDiffP = addPanelDirect();
    noCmdDiffP.split = { secondaryCmdId: null };
    assert(() => { applySecondaryVttyDiff(noCmdDiffP, diffVtty, {}); }, 'applySecondaryVttyDiff no-cmd no crash');

    const htmlData = { html: '<span>test</span>', generation: 1 };
    assert(() => { applySecondaryVttyDiff(diffP, diffVtty, htmlData); }, 'applySecondaryVttyDiff with html does not throw');
}

console.log('\n[websocket.js] Tests complete');
