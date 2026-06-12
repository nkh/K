/// test/test_commands-core.js — Tests for commands-core module
require('./setup');

console.log('\n=== commands-core.js Tests ===\n');

resetTestState();

// ── lookupAndSelectCommand ──
console.log('lookupAndSelectCommand tests');
assert(typeof lookupAndSelectCommand === 'function', 'lookupAndSelectCommand is a function');
assert(() => { lookupAndSelectCommand('htop'); }, 'lookupAndSelectCommand does not throw on no-match');

// ── showCommandPicker ──
console.log('showCommandPicker tests');
assert(typeof showCommandPicker === 'function', 'showCommandPicker is a function');

// Test: showCommandPicker removes existing picker
const oldPicker = document.createElement('div');
oldPicker.id = 'cmdPicker';
document.body.appendChild(oldPicker);
assert(() => { showCommandPicker([]); }, 'showCommandPicker with empty matches does not throw');
assertEq(document.getElementById('cmdPicker') === oldPicker, false, 'old picker was removed');

// Test: showCommandPicker with matches creates overlay
const matches = [
    { id: 'cmd-1', name: 'htop', args: [], pid: 1234, alive: true, runtime_secs: 300 },
    { id: 'cmd-2', name: 'vim', args: ['file.txt'], pid: 5678, alive: false, runtime_secs: 0 },
];
showCommandPicker(matches);
const picker = document.getElementById('cmdPicker');
assert(picker !== null, 'cmdPicker element created');
assert(picker.style.cssText.includes('position:fixed'), 'picker is a fixed overlay');
assert(picker.innerHTML.includes('Multiple commands matching'), 'picker shows header text');
assert(picker.innerHTML.includes('cmd-1'), 'picker contains first match id');
assert(picker.innerHTML.includes('htop'), 'picker contains first match name');
assert(picker.innerHTML.includes('5678'), 'picker contains second match pid');
assert(picker.innerHTML.includes('running'), 'alive badge shown');
assert(picker.innerHTML.includes('exited'), 'exited badge shown');
// Clean up
picker.remove();

// Test: showCommandPicker with escaped HTML in names
const xssMatches = [
    { id: 'cmd-x', name: '<script>alert(1)</script>', args: [], pid: 999, alive: true, runtime_secs: 0 },
];
showCommandPicker(xssMatches);
const xssPicker = document.getElementById('cmdPicker');
assert(!xssPicker.innerHTML.includes('<script>'), 'XSS in name is escaped');
xssPicker.remove();

// Test: showCommandPicker with args in detail
const argsMatches = [
    { id: 'cmd-a', name: 'python', args: ['-u', 'app.py'], pid: 100, alive: true, runtime_secs: 0 },
];
showCommandPicker(argsMatches);
const argsPicker = document.getElementById('cmdPicker');
assert(argsPicker.innerHTML.includes('-u app.py'), 'args shown in detail');
assert(argsPicker.innerHTML.includes('100'), 'pid shown');
argsPicker.remove();

// ── pickCommand ──
console.log('pickCommand tests');
assert(typeof pickCommand === 'function', 'pickCommand is a function');
assert(() => { pickCommand('cmd-1', 'htop'); }, 'pickCommand does not throw');

// Test: pickCommand removes existing picker and sets pending select
const pickerBefore = document.createElement('div');
pickerBefore.id = 'cmdPicker';
document.body.appendChild(pickerBefore);
state._pendingSelectId = null;
pickCommand('cmd-99', 'test');
assertEq(state._pendingSelectId, 'cmd-99', 'pickCommand sets _pendingSelectId');
// Note: mock getElementById auto-creates elements, so verify via state change
// pickCommand calls releaseCurrentFocusTrap() then picker.remove()
// The key behavior is that _pendingSelectId is set correctly

// ── navigateCommand ──
console.log('navigateCommand tests');
assert(typeof navigateCommand === 'function', 'navigateCommand is a function');
assert(typeof navigatePrevCommand === 'function', 'navigatePrevCommand is a function');
assert(typeof navigateNextCommand === 'function', 'navigateNextCommand is a function');

// Test: navigateCommand with empty list
assert(() => { navigateCommand(1); }, 'navigateCommand with empty list does not throw');

// Test: navigateCommand with commands
_navCommands = [
    { instUrl: 'http://a.com', cmdId: 'c1', name: 'htop' },
    { instUrl: 'http://b.com', cmdId: 'c2', name: 'vim' },
    { instUrl: 'http://c.com', cmdId: 'c3', name: 'bash' },
];
let _lastSelectCmd = null;
let _lastSelectInstUrl = null;
const origSelectCommand = typeof selectCommand === 'function' ? selectCommand : null;
globalThis.selectCommand = function(instUrl, cmdId, name) {
    _lastSelectInstUrl = instUrl;
    _lastSelectCmd = cmdId;
};

state.selectedInstUrl = 'http://b.com';
state.selectedCmdId = 'c2';
state._focusedPanelId = null;
navigateCommand(1); // next from vim → bash
assertEq(_lastSelectCmd, 'c3', 'navigateCommand forward selects next');
assertEq(_lastSelectInstUrl, 'http://c.com', 'navigateCommand forward selects correct instance');

// backward test: set state to match forward navigation result
state.selectedInstUrl = 'http://c.com';
state.selectedCmdId = 'c3'; // forward landed on c3
navigateCommand(-1); // back from bash → vim (c3 at index 2, -1+2+3)%3 = 1)
assertEq(_lastSelectCmd, 'c2', 'navigateCommand backward selects prev');

// Test: wrap-around backward
state.selectedInstUrl = 'http://a.com';
state.selectedCmdId = 'c1';
state._focusedPanelId = null;
navigateCommand(-1); // wrap from htop to bash (c1 at index 0, -1+0+3)%3 = 2)
assertEq(_lastSelectCmd, 'c3', 'navigateCommand wraps backward');

// Test: wrap-around forward
state.selectedInstUrl = 'http://c.com';
state.selectedCmdId = 'c3';
navigateCommand(1); // wrap from bash to htop (c3 at index 2, 1+2+3)%3 = 0)
assertEq(_lastSelectCmd, 'c1', 'navigateCommand wraps forward');

// Test: no command selected → goes to first/last
_lastSelectCmd = null;
_lastSelectInstUrl = null;
state.selectedInstUrl = null;
state.selectedCmdId = null;
navigateCommand(1);
assertEq(_lastSelectCmd, 'c1', 'navigateCommand with no selection goes to first on forward');
_lastSelectCmd = null;
_lastSelectInstUrl = null;
state.selectedInstUrl = null;
state.selectedCmdId = null;
navigateCommand(-1);
assertEq(_lastSelectCmd, 'c3', 'navigateCommand with no selection goes to last on backward');

// Test: navigateCommand with empty list → no crash
_navCommands = [];
state.selectedInstUrl = null;
state.selectedCmdId = null;
_lastSelectCmd = null;
navigateCommand(1);
assertEq(_lastSelectCmd, null, 'navigateCommand empty list forward no-op');
navigateCommand(-1);
assertEq(_lastSelectCmd, null, 'navigateCommand empty list backward no-op');

// Restore
if (origSelectCommand) globalThis.selectCommand = origSelectCommand;

// ── navigatePrevCommand / navigateNextCommand ──
console.log('navigatePrevCommand/navigateNextCommand tests');
globalThis.selectCommand = function() {};
assert(() => { navigatePrevCommand(); }, 'navigatePrevCommand does not throw');
assert(() => { navigateNextCommand(); }, 'navigateNextCommand does not throw');

// ── loadCommands ──
console.log('loadCommands tests');
assert(typeof loadCommands === 'function', 'loadCommands is a function');

// Test: loadCommands with no connections
state.connections = [];
globalThis._buildSidebar = function() {};
_setFetchJson({ status: 'ok', data: [] });
assert(() => { loadCommands(); }, 'loadCommands with no connections does not throw');

// Test: loadCommands with connections
_resetFetch();
state.connections = [
    { url: 'http://localhost:9090', label: 'Local', token: '', reachable: undefined, _commands: [], _lastError: null },
];
_setFetchJson({ status: 'ok', data: [] });
assert(() => { loadCommands(); }, 'loadCommands with connections does not throw');

// Test: loadCommands updates reachability on failure
// Since api.js captures fetch at load time, we must use _setFetchError
_resetFetch();
_fetchCalls.length = 0;
_setFetchError(503, { status: 'error', data: [] });
state.connections[0].reachable = true;
// loadCommands is async — the reachability update happens after fetch resolves.
// We can't easily test async side-effects in sync tests without awaiting.
// For now, verify the function doesn't crash on error response.
assert(() => { loadCommands(); }, 'loadCommands handles fetch failure without crash');
// NOTE: Testing async reachability update requires _asyncTest pattern.
// This will be properly tested in a future async test file.

// ── loadCommands auto-select ──
console.log('loadCommands auto-select tests');
_resetFetch();
state.connections = [{ url: 'http://a.com', label: 'A', token: '', reachable: true, _commands: [{ id: 'x1', name: 'htop', alive: true }] }];
state.selectedCmdId = null;
state.panels = [];
const autoPanel = addPanelDirect();
autoPanel.selectedCmdId = null;
_setFetchJson({ status: 'ok', data: [] });
globalThis.loadVttyHttpForPanel = function() {};
globalThis.startPanelUpdateMode = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis.updateSidebarSelection = function() {};

// loadCommands will auto-select the first alive command
assert(() => { loadCommands(); }, 'loadCommands auto-select does not throw');
// NOTE: Async verification of auto-select requires _asyncTest pattern.

console.log('\n[commands-core.js] Tests complete');
