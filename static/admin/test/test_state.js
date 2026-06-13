/// test/test_state.js — Tests for state module
require('./setup');

console.log('\n=== state.js Tests ===\n');

resetTestState();

// ── VRW namespace exists ──
console.log('VRW namespace');
assert(typeof VRW !== 'undefined', 'VRW namespace exists');
assert(typeof VRW.state !== 'undefined', 'VRW.state exists');
assertEq(VRW.state, state, 'VRW.state is the same object as global state');

// ── state object structure ──
console.log('state structure');
assert(Array.isArray(state.panels), 'state.panels is array');
assert(Array.isArray(state.connections), 'state.connections is array');
assert(typeof state.fontSize === 'number', 'state.fontSize is number');
assert(typeof state.currentView === 'string', 'state.currentView is string');
assert(typeof state.updateMode === 'string', 'state.updateMode is string');
assert(state.updateMode === 'push' || state.updateMode === 'poll', 'updateMode is push or poll');
assert(typeof state.panelLayout === 'string', 'state.panelLayout is string');
assert(typeof state.selectedCmdId === 'object' || typeof state.selectedCmdId === 'string' || state.selectedCmdId === null, 'selectedCmdId type');

// ── state defaults ──
console.log('state defaults');
assertEq(state.currentView, 'vtty', 'default view is vtty');
assertEq(state.panelLayout, 'row', 'default layout is row');
assert(state.fontSize >= 8 && state.fontSize <= 28, 'fontSize in valid range');

// ── state is mutable ──
console.log('state mutability');
const origLen = state.panels.length;
state.panels.push({ id: 'test-panel' });
assertEq(state.panels.length, origLen + 1, 'panels array is mutable');
state.panels.pop();
assertEq(state.panels.length, origLen, 'panels array restored');

// ── VRW module-level variables ──
console.log('VRW module-level variables');
assert(typeof VRW._showingWelcome !== 'undefined', '_showingWelcome exists');
assert(typeof VRW._lastCommandState !== 'undefined', '_lastCommandState exists');
assert(typeof VRW._navCommands !== 'undefined', '_navCommands exists');
assert(typeof VRW._sidebarSort !== 'undefined', '_sidebarSort exists');
assert(typeof VRW._searchFrozenPanelIds !== 'undefined', '_searchFrozenPanelIds exists');
assert(typeof VRW._searchFrozenCmdIds !== 'undefined', '_searchFrozenCmdIds exists');
assert(typeof VRW._lastRenderedPanelCount !== 'undefined', '_lastRenderedPanelCount exists');
assert(typeof VRW._lastRenderedPanelIds !== 'undefined', '_lastRenderedPanelIds exists');
assert(typeof VRW._lastSplitState !== 'undefined', '_lastSplitState exists');
assert(typeof VRW._lastShowingWelcome !== 'undefined', '_lastShowingWelcome exists');

// ── VRW module-level variable types ──
console.log('VRW module-level variable types');
assertEq(typeof VRW._lastCommandState, 'string', '_lastCommandState is string');
assert(Array.isArray(VRW._navCommands), '_navCommands is array');
assertEq(VRW._sidebarSort, 'name', '_sidebarSort defaults to name');
assert(VRW._searchFrozenPanelIds instanceof Set, '_searchFrozenPanelIds is a Set');
assert(Array.isArray(VRW._searchFrozenCmdIds), '_searchFrozenCmdIds is array');
assertEq(typeof VRW._lastRenderedPanelCount, 'number', '_lastRenderedPanelCount is number');
assertEq(typeof VRW._lastRenderedPanelIds, 'string', '_lastRenderedPanelIds is string');
assertEq(typeof VRW._lastSplitState, 'string', '_lastSplitState is string');

// ── state connection/VTty fields ──
console.log('state connection/VTty fields');
assertEq(state.selectedInstUrl, null, 'selectedInstUrl defaults to null');
assertEq(state.selectedCmdId, null, 'selectedCmdId defaults to null');
assertEq(state.bufferView, 'current', 'bufferView defaults to current');
// _pendingVttyData and _pendingVttyDirty removed in Phase 8a (per-panel)

// ── state cache/optimization fields ──
console.log('state cache fields');
assert(typeof state._lastGeneration === 'object', '_lastGeneration is object');
assertEq(state._userAtBottom, true, '_userAtBottom defaults to true');
// _userScrolling removed in Phase 8a (per-panel)
assert(typeof state._cellGrids === 'object', '_cellGrids is object');
assert(typeof state._cachedDomPre === 'object', '_cachedDomPre is object');
assert(typeof state._cachedScrollPos === 'object', '_cachedScrollPos is object');
assertEq(state._level3Enabled, true, '_level3Enabled defaults to true');

// ── state update mode fields ──
console.log('state update mode fields');
assert(state.updateMode === 'push' || state.updateMode === 'poll', 'updateMode is push or poll');
assert(typeof state.pollInterval === 'number', 'pollInterval is number');
assert(state.pollInterval >= 50, 'pollInterval >= 50ms minimum');
assert(state.pollInterval <= 5000, 'pollInterval <= 5000ms maximum');

// ── state refresh throttle fields ──
console.log('state refresh throttle fields');
assert(typeof state.refreshMs === 'number', 'refreshMs is number');
assert(state.refreshMs >= 0, 'refreshMs >= 0');
assertEq(state._refreshThrottleTimer, null, '_refreshThrottleTimer defaults to null');

// ── state server config fields ──
console.log('state server config fields');
assertEq(state.serverUpdateMode, null, 'serverUpdateMode defaults to null');
assertEq(state.serverPollMs, null, 'serverPollMs defaults to null');
assertEq(state.serverDirtyMs, null, 'serverDirtyMs defaults to null');
assertEq(state.serverScreenshotFontSize, 12, 'serverScreenshotFontSize defaults to 12');
assertEq(state.serverScreenshotFontName, 'monospace', 'serverScreenshotFontName defaults to monospace');

// ── state WebSocket quality fields ──
console.log('state WS quality fields');
assertEq(state._wsLatency, 0, '_wsLatency defaults to 0');
assertEq(state._wsPingInterval, null, '_wsPingInterval defaults to null');
assertEq(state._wsReconnectCount, 0, '_wsReconnectCount defaults to 0');
assertEq(state._wsPingSendTime, 0, '_wsPingSendTime defaults to 0');

// ── state resource/notification fields ──
console.log('state resource/notification fields');
assert(typeof state._resourceCache === 'object', '_resourceCache is object');
assertEq(state._resourceInterval, null, '_resourceInterval defaults to null');
assert(typeof state.showResources === 'boolean', 'showResources is boolean');
assert(typeof state.soundEnabled === 'boolean', 'soundEnabled is boolean');

// ── state panel management fields ──
console.log('state panel management fields');
assertEq(state.serverReachable, false, 'serverReachable defaults to false');
assertEq(state._focusedPanelId, null, '_focusedPanelId defaults to null');
assert(typeof state._mobileTabbedLayout === 'boolean', '_mobileTabbedLayout is boolean');

// ── state log WebSocket fields ──
console.log('state log WS fields');
assertEq(state.logWs, null, 'logWs defaults to null');
assertEq(state.logWsReconnectTimer, null, 'logWsReconnectTimer defaults to null');

// ── state can hold extra dynamic keys ──
console.log('state dynamic keys');
state._testDynamicKey = 'hello';
assertEq(state._testDynamicKey, 'hello', 'state supports dynamic keys');
delete state._testDynamicKey;

console.log('\n[state.js] Tests complete');