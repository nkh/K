/// test_fixes_af5902e.js — Tests for fixes in the af5902e follow-up commit
require('./setup');

console.log('\n=== Fixes for af5902e follow-up ===\n');

resetTestState();

// Mock functions that touch DOM
globalThis.renderPanels = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.disconnectPanelWs = function() {};
globalThis.stopPanelPoll = function() {};

// ── 1. Split independence (no command inheritance) ──
console.log('Split independence tests');
state.panels = [];
const sp = addPanelDirect();
sp.selectedCmdId = 'cmd-abc';
sp.selectedInstUrl = 'http://localhost:9090';

splitPanel(sp.id, 'vertical');
assert(sp.split !== null, 'split created');
assertEq(sp.split.secondaryCmdId, null, 'split secondary has NO inherited cmdId');
assertEq(sp.split.secondaryInstUrl, null, 'split secondary has NO inherited instUrl');
// Parent command should remain unchanged
assertEq(sp.selectedCmdId, 'cmd-abc', 'parent cmdId unchanged after split');
assertEq(sp.selectedInstUrl, 'http://localhost:9090', 'parent instUrl unchanged after split');

// Split with parent that has no command → still null
state.panels = [];
const sp2 = addPanelDirect();
splitPanel(sp2.id, 'horizontal');
assertEq(sp2.split.secondaryCmdId, null, 'split from empty panel has null cmdId');
assertEq(sp2.split.secondaryInstUrl, null, 'split from empty panel has null instUrl');

// ── 2. Keyboard shortcuts: _defaultShortcuts and rebuild system ──
console.log('Keyboard shortcut system tests');
assert(typeof _defaultShortcuts === 'object' && Array.isArray(_defaultShortcuts), '_defaultShortcuts is array');
assert(typeof _rebuildShortcuts === 'function', '_rebuildShortcuts is a function');
assert(typeof _loadCustomShortcuts === 'function', '_loadCustomShortcuts is a function');
assert(typeof _saveCustomShortcut === 'function', '_saveCustomShortcut is a function');

// Verify specific new shortcuts exist in defaults
const splitV = _defaultShortcuts.find(s => s.id === 'split-vertical');
assert(splitV !== undefined, 'split-vertical shortcut defined');
assertEq(splitV.key, '|', 'split-vertical uses | key');
assertEq(splitV.alt, true, 'split-vertical requires alt');

const splitH = _defaultShortcuts.find(s => s.id === 'split-horizontal');
assert(splitH !== undefined, 'split-horizontal shortcut defined');
assertEq(splitH.key, '-', 'split-horizontal uses - key');
assertEq(splitH.alt, true, 'split-horizontal requires alt');

const newWin = _defaultShortcuts.find(s => s.id === 'new-window');
assert(newWin !== undefined, 'new-window shortcut defined');
assertEq(newWin.key, 'w', 'new-window uses w key');
assertEq(newWin.alt, true, 'new-window requires alt');

const unsplit = _defaultShortcuts.find(s => s.id === 'unsplit');
assert(unsplit !== undefined, 'unsplit shortcut defined');
assertEq(unsplit.key, 'u', 'unsplit uses u key');

// Verify context-menu shortcut uses string shift (alternative key, not modifier)
const ctxMenu = _defaultShortcuts.find(s => s.id === 'context-menu');
assert(ctxMenu !== undefined, 'context-menu shortcut defined');
assertEq(ctxMenu.shift, 'F10', 'context-menu shift is string F10 (alternative key)');

// Verify Alt+1..9 window switch shortcuts
for (let i = 1; i <= 9; i++) {
    const ws = _defaultShortcuts.find(s => s.id === 'win-' + i);
    assert(ws !== undefined, 'win-' + i + ' shortcut defined');
    assertEq(ws.key, String(i), 'win-' + i + ' uses key ' + i);
    assertEq(ws.alt, true, 'win-' + i + ' requires alt');
}

// ── 3. Window system: addPanelDirect adds panel to window ──
console.log('Window system tests');
assert(typeof state.windows !== 'undefined', 'state.windows exists');
assert(Array.isArray(state.windows), 'state.windows is array');

// After addPanelDirect, the panel should be in a window's panelIds
state.panels = [];
state.windows = [];
state.activeWindowId = null;
const wp = addPanelDirect();
const activeWin = _getActiveWindow();
assert(activeWin !== undefined, '_getActiveWindow returns a window');
assert(Array.isArray(activeWin.panelIds), 'window has panelIds array');
assert(activeWin.panelIds.includes(wp.id), 'new panel is in window panelIds after addPanelDirect');

// Multiple panels should all be in the window
const wp2 = addPanelDirect();
assert(activeWin.panelIds.includes(wp2.id), 'second panel is in window panelIds');
assertEq(activeWin.panelIds.length, 2, 'window has 2 panelIds');

// removePanel should clean up window panelIds
const wp2Id = wp2.id;
removePanel(wp2Id);
assert(!activeWin.panelIds.includes(wp2Id), 'removed panel cleaned from window panelIds');
assertEq(activeWin.panelIds.length, 1, 'window has 1 panelId after removal');

// ── 4. Window management functions ──
console.log('Window management function tests');
assert(typeof createWindow === 'function', 'createWindow is a function');
assert(typeof closeWindow === 'function', 'closeWindow is a function');
assert(typeof switchWindow === 'function', 'switchWindow is a function');
assert(typeof _renderWindowBar === 'function', '_renderWindowBar is a function');

// createWindow should create a new window and switch to it
state.panels = [];
state.windows = [];
state.activeWindowId = null;
addPanelDirect(); // initial panel in window 0
const winCountBefore = state.windows.length;
createWindow();
assertEq(state.windows.length, winCountBefore + 1, 'createWindow adds a window');
// Should have switched to new window
assertEq(state.activeWindowId, state.windows[state.windows.length - 1].id, 'createWindow switches to new window');

// closeWindow should remove window and its panels
const winIdToDelete = state.windows[state.windows.length - 1].id;
closeWindow(winIdToDelete);
assert(!state.windows.find(w => w.id === winIdToDelete), 'closed window removed from state');

// closeWindow on last window should be a no-op
state.windows = [{ id: 'only-win', name: '1', panelIds: [] }];
state.activeWindowId = 'only-win';
closeWindow('only-win');
assertEq(state.windows.length, 1, 'cannot close the last window');

// switchWindow should change activeWindowId
state.windows = [
    { id: 'w1', name: '1', panelIds: [] },
    { id: 'w2', name: '2', panelIds: [] },
];
state.activeWindowId = 'w1';
switchWindow('w2');
assertEq(state.activeWindowId, 'w2', 'switchWindow changes activeWindowId');
// Switching to same window should be a no-op
switchWindow('w2');
assertEq(state.activeWindowId, 'w2', 'switchWindow same window is no-op');

// ── 5. _renderWindowBar ──
console.log('_renderWindowBar tests');
state.windows = [{ id: 'w1', name: '1', panelIds: [] }];
state.activeWindowId = 'w1';
const bar1 = _renderWindowBar();
assertEq(bar1, '', '_renderWindowBar returns empty for single window');

state.windows = [
    { id: 'w1', name: '1', panelIds: [] },
    { id: 'w2', name: '2', panelIds: [] },
];
state.activeWindowId = 'w2';
const bar2 = _renderWindowBar();
assert(bar2.includes('window-bar'), 'window bar rendered for multiple windows');
assert(bar2.includes('data-action="SwitchWindow"'), 'window bar has SwitchWindow actions');
assert(bar2.includes('data-action="CreateWindow"'), 'window bar has CreateWindow button');
assert(bar2.includes('active'), 'window bar marks active window');
assert(bar2.includes('data-action="CloseWindow"'), 'window bar has CloseWindow for multi-window');

// ── 6. Spawn modal is sidebar (not modal overlay) ──
console.log('Spawn sidebar tests');
// The spawn element should exist in the DOM
const spawnEl = document.getElementById('tab-spawn');
assert(spawnEl !== null, 'tab-spawn element exists');
// It should NOT have fixed positioning (modal)
assert(!spawnEl.style.position || spawnEl.style.position !== 'fixed',
    'tab-spawn is not position:fixed (not a modal)');

// _showSpawnModal hides tab-servers, shows tab-spawn
_showSpawnModal();
assert(!spawnEl.classList.contains('hidden'), '_showSpawnModal reveals tab-spawn');
const serversEl = document.getElementById('tab-servers');
assert(serversEl.classList.contains('hidden'), '_showSpawnModal hides tab-servers');

// _closeSpawnModal reverses
_closeSpawnModal();
assert(spawnEl.classList.contains('hidden'), '_closeSpawnModal hides tab-spawn');
assert(!serversEl.classList.contains('hidden'), '_closeSpawnModal reveals tab-servers');

// _spawnOnServer with URL sets the spawn server and shows spawn
_spawnOnServer('http://prod:8080');
assertEq(window._userSpawnInstUrl, 'http://prod:8080', '_spawnOnServer sets _userSpawnInstUrl');
assert(!spawnEl.classList.contains('hidden'), '_spawnOnServer reveals tab-spawn');
assert(serversEl.classList.contains('hidden'), '_spawnOnServer hides tab-servers');
_closeSpawnModal();

// ── 7. User-definable shortcuts: save/reset cycle ──
console.log('User-definable shortcuts save/reset tests');
// Save a custom shortcut and verify it's applied
_saveCustomShortcut('split-vertical', { key: '\\', alt: true });
_rebuildShortcuts();
// After _rebuildShortcuts, _defaultShortcuts still has the original
// but _shortcuts (internal) has the override
const customCheck = _loadCustomShortcuts();
assert(customCheck['split-vertical'] !== undefined, 'custom shortcut saved to localStorage');
assertEq(customCheck['split-vertical'].key, '\\', 'custom shortcut key is \\' );

// Reset the custom shortcut
_saveCustomShortcut('split-vertical', null);
_rebuildShortcuts();
const resetCheck = _loadCustomShortcuts();
assert(resetCheck['split-vertical'] === undefined, 'custom shortcut removed after reset');

// ── Summary ──
console.log('\n[Fixes af5902e follow-up] Tests complete');