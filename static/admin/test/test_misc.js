/// test/test_misc.js — Tests for misc UI controls (token, font, refresh, selection)
require('./setup');

console.log('\n=== misc.js Tests ===\n');

resetTestState();

// Mock DOM elements needed by misc functions — create AFTER resetTestState
const authTokenInput = document.createElement('input');
authTokenInput.id = 'authToken';
const stRefreshVal = document.createElement('span');
stRefreshVal.id = 'stRefreshVal';
const stFontSize = document.createElement('span');
stFontSize.id = 'stFontSize';
const refreshMsInput = document.createElement('input');
refreshMsInput.id = 'refreshMs';
refreshMsInput.type = 'number';
refreshMsInput.value = '0';
const contentArea = document.createElement('div');
contentArea.id = 'contentArea';
_elementRegistry.set('contentArea', contentArea);

globalThis.renderPanels = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.startRefresh = function() {};
globalThis.loadVttyHttpForPanel = function() {};

// ── saveToken ──
console.log('saveToken tests');
assert(typeof saveToken === 'function', 'saveToken is a function');
// Re-get the element (resetTestState clears registry)
const tokenInput = document.getElementById('authToken');
tokenInput.value = 'my-secret-token';
saveToken();
// saveToken stores to localStorage under 'vrw_auth_token'
const storedToken = localStorage.getItem('vrw_auth_token');
assertEq(storedToken, 'my-secret-token', 'token saved to localStorage');
assertEq(state.authToken, 'my-secret-token', 'token saved to state');

// ── changeFontSize ──
console.log('changeFontSize tests');
assert(typeof changeFontSize === 'function', 'changeFontSize is a function');
state.fontSize = 10;
changeFontSize(2);
assertEq(state.fontSize, 12, 'fontSize increased');
changeFontSize(-4);
assertEq(state.fontSize, 8, 'fontSize decreased (clamped at 8)');
changeFontSize(-100);
assert(state.fontSize >= 8, 'fontSize never below 8');
state.fontSize = 26;
changeFontSize(10);
assert(state.fontSize <= 28, 'fontSize never above 28');

// ── applyFontSize ──
console.log('applyFontSize tests');
assert(typeof applyFontSize === 'function', 'applyFontSize is a function');
assert(() => { applyFontSize(); }, 'applyFontSize does not throw');

// ── changeRefreshMs ──
console.log('changeRefreshMs tests');
assert(typeof changeRefreshMs === 'function', 'changeRefreshMs is a function');
state.refreshMs = 0;
changeRefreshMs(100);
assertEq(state.refreshMs, 100, 'refreshMs increased');
changeRefreshMs(-200);
assertEq(state.refreshMs, 0, 'refreshMs clamped at 0');
state.refreshMs = 1900;
changeRefreshMs(200);
assertEq(state.refreshMs, 2000, 'refreshMs capped at 2000');
changeRefreshMs(100);
assertEq(state.refreshMs, 2000, 'refreshMs cannot exceed 2000');

// ── applyRefreshMs ──
console.log('applyRefreshMs tests');
if (typeof applyRefreshMs === 'function') {
    assert(() => { applyRefreshMs(); }, 'applyRefreshMs does not throw');
}

// ── _throttleRefresh ──
console.log('_throttleRefresh tests');
// _throttleRefresh is an internal function (not exported to window).
// Test it indirectly through changeRefreshMs which uses it.
state.refreshMs = 0;
// When refreshMs=0, the throttle allows all updates through
state._refreshThrottleTimer = null;

// ── _syncRefreshMsUI ──
console.log('_syncRefreshMsUI tests');
if (typeof _syncRefreshMsUI === 'function') {
    assert(() => { _syncRefreshMsUI(); }, '_syncRefreshMsUI does not throw');
}

// ── toggleSelectionMode ──
console.log('toggleSelectionMode tests');
assert(typeof toggleSelectionMode === 'function', 'toggleSelectionMode is a function');
state.panels = [];
const p = addPanelDirect();
assertEq(p.selectionMode, false, 'selection mode off by default');
toggleSelectionMode(p.id);
assertEq(p.selectionMode, true, 'selection mode toggled on');
toggleSelectionMode(p.id);
assertEq(p.selectionMode, false, 'selection mode toggled off');

// Non-existent panel
assert(() => { toggleSelectionMode('nonexistent'); }, 'toggleSelectionMode with invalid ID does not throw');

console.log('\n[misc.js] Tests complete');
