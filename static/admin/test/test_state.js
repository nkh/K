/// test/test_state.js — Tests for state module
require('./setup');

console.log('\n=== state.js Tests ===\n');

// ── VRW namespace exists ──
console.log('VRW namespace');
assert(typeof VRW !== 'undefined', 'VRW namespace exists');
assert(typeof VRW.state !== 'undefined', 'VRW.state exists');

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

console.log('\n[state.js] Tests complete');
