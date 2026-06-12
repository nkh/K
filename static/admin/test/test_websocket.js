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
globalThis._disconnectSecondaryWs = function(panelObj) {
    // Track calls
    disconnectPanelWs._secondaryDisconnected = true;
};
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
assert(disconnectPanelWs._secondaryDisconnected === true, 'secondary WS disconnected for split panel');
disconnectPanelWs._secondaryDisconnected = false;

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
    // No element → early return
    const origIndicator = _elementRegistry.get('wsQuality');
    _elementRegistry.delete('wsQuality');
    assert(() => { updateWsQualityIndicator(); }, 'updateWsQualityIndicator no-crash without element');

    // With element, no focused panel
    const indicator = document.createElement('span');
    indicator.id = 'wsQuality';
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

// ── Legacy WS functions ──
console.log('legacy WS functions');
if (typeof connectVttyWs === 'function') {
    assert(() => { connectVttyWs(); }, 'connectVttyWs does not throw');
}
if (typeof disconnectVttyWs === 'function') {
    disconnectVttyWs();
    assertEq(state._wsReconnectCount, 0, 'disconnectVttyWs resets reconnect count');
    assertEq(state._wsPingSendTime, 0, 'disconnectVttyWs resets ping send time');
    assertEq(state._wsLatency, 0, 'disconnectVttyWs resets latency');
}
if (typeof startPoll === 'function') {
    assert(() => { startPoll(); }, 'startPoll does not throw');
}
if (typeof stopPoll === 'function') {
    assert(() => { stopPoll(); }, 'stopPoll does not throw');
}

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

// ── scheduleSecondaryVttyHttp ──
console.log('scheduleSecondaryVttyHttp tests');
if (typeof scheduleSecondaryVttyHttp === 'function') {
    const schedP = addPanelDirect();
    schedP.split = { secondaryCmdId: 's1', secondaryInstUrl: 'http://localhost:9090' };
    assert(() => { scheduleSecondaryVttyHttp(schedP, 50); }, 'scheduleSecondaryVttyHttp does not throw');

    // No split → no-op
    const noSplitSchedP = addPanelDirect();
    assert(() => { scheduleSecondaryVttyHttp(noSplitSchedP, 50); }, 'scheduleSecondaryVttyHttp no-split no crash');

    // No cmd → no-op
    const noCmdSplitP = addPanelDirect();
    noCmdSplitP.split = { secondaryCmdId: null, secondaryInstUrl: null };
    assert(() => { scheduleSecondaryVttyHttp(noCmdSplitP, 50); }, 'scheduleSecondaryVttyHttp no-cmd no crash');
}

// ── _loadSecondaryVttyHttp ──
console.log('_loadSecondaryVttyHttp tests');
if (typeof _loadSecondaryVttyHttp === 'function') {
    assert(_loadSecondaryVttyHttp.constructor.name === 'AsyncFunction', '_loadSecondaryVttyHttp is async');

    // No split → no-op
    const noSplitLdP = addPanelDirect();
    assert(() => { _loadSecondaryVttyHttp(noSplitLdP); }, '_loadSecondaryVttyHttp no-split no crash');

    // No vtty element → no-op
    const noVttyP = addPanelDirect();
    noVttyP.split = { secondaryCmdId: 's1', secondaryInstUrl: 'http://localhost:9090' };
    assert(() => { _loadSecondaryVttyHttp(noVttyP); }, '_loadSecondaryVttyHttp no-vtty no crash');
}

// ── _updateSecondaryVttyMetadata ──
console.log('_updateSecondaryVttyMetadata tests');
if (typeof _updateSecondaryVttyMetadata === 'function') {
    const metaP = addPanelDirect();
    metaP.split = { secondaryCmdId: 's1', secondaryScrollbackOffset: 0, secondaryMouseTracking: false, secondaryMouseSgr: false };
    metaP.fontSize = 10;

    const vttyEl = document.createElement('div');
    vttyEl.className = 'vtty-container';
    const cursorEl = document.createElement('div');
    cursorEl.className = 'cursor-indicator';
    cursorEl.style.display = 'none';
    const pre = document.createElement('pre');
    vttyEl.appendChild(cursorEl);
    vttyEl.appendChild(pre);

    // With cursor data
    _updateSecondaryVttyMetadata(metaP, vttyEl, {
        cursor: { row: 5, col: 10, cursor_visible: true },
        dimensions: { rows: 24, cols: 80 },
        mouse_tracking: true,
        mouse_sgr: true,
    });
    assertEq(cursorEl.style.display, '', 'cursor shown when visible');
    assert(cursorEl.style.top.includes('px'), 'cursor top set');
    assert(cursorEl.style.left.includes('px'), 'cursor left set');
    assertEq(metaP.split.secondaryMouseTracking, true, 'mouse tracking updated');
    assertEq(metaP.split.secondaryMouseSgr, true, 'mouse sgr updated');
    assertEq(pre._vttyRows, 24, 'vttyRows stored');
    assertEq(pre._vttyCols, 80, 'vttyCols stored');

    // Cursor hidden
    _updateSecondaryVttyMetadata(metaP, vttyEl, { cursor_visible: false });
    assertEq(cursorEl.style.display, 'none', 'cursor hidden when cursor_visible false');

    // In scrollback → cursor hidden
    metaP.split.secondaryScrollbackOffset = 10;
    _updateSecondaryVttyMetadata(metaP, vttyEl, { cursor: { row: 1, col: 1, cursor_visible: true } });
    assertEq(cursorEl.style.display, 'none', 'cursor hidden in scrollback');
    metaP.split.secondaryScrollbackOffset = 0;
}

// ── _applySecondaryVttyDiff ──
console.log('_applySecondaryVttyDiff tests');
if (typeof _applySecondaryVttyDiff === 'function') {
    const diffP = addPanelDirect();
    diffP.split = { secondaryCmdId: 's1' };
    const diffVtty = document.createElement('div');
    diffVtty.className = 'vtty-container';
    const diffPre = document.createElement('pre');
    diffVtty.appendChild(diffPre);

    // No cmdId → no-op
    const noCmdDiffP = addPanelDirect();
    noCmdDiffP.split = { secondaryCmdId: null };
    assert(() => { _applySecondaryVttyDiff(noCmdDiffP, diffVtty, {}); }, '_applySecondaryVttyDiff no-cmd no crash');

    // With full HTML fallback
    const htmlData = { html: '<span>test</span>', generation: 1 };
    assert(() => { _applySecondaryVttyDiff(diffP, diffVtty, htmlData); }, '_applySecondaryVttyDiff with html does not throw');
}

console.log('\n[websocket.js] Tests complete');process.exit(0);
