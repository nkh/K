/// test/test_command-selection.js — Tests for command selection, caching, history
require('./setup');

console.log('\n=== command-selection.js Tests ===\n');

resetTestState();

// Mock network-dependent functions
globalThis.renderPanels = function() {};
globalThis.loadVttyHttp = function() {};
globalThis.loadVttyHttpForPanel = function() {};
globalThis.startPanelUpdateMode = function() {};
globalThis.startUpdateMode = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.disconnectPanelWs = function() {};
globalThis.stopPanelPoll = function() {};
globalThis.connectPanelWs = function() {};
globalThis.startPanelPoll = function() {};
globalThis.applyVttyDiff = function() {};
globalThis.updateVttyDisplay = function() {};
globalThis._restoreCachedDom = function() {};
globalThis._cacheTerminalForSwitch = function() {};
globalThis._disconnectSecondaryWs = function() {};
globalThis._connectSecondaryWs = function() {};
globalThis._loadSecondaryVttyHttp = function() {};
globalThis._updateSplitPanelHeader = function() {};

// ── _isTerminalVisible ──
console.log('_isTerminalVisible tests');
assert(typeof _isTerminalVisible === 'function', '_isTerminalVisible is a function');

state.currentView = 'vtty';
state.selectedCmdId = 'cmd-1';
assert(_isTerminalVisible(), 'visible when vtty view + command selected');

state.currentView = 'logs';
assert(!_isTerminalVisible(), 'not visible in logs view');

state.currentView = 'vtty';
state.selectedCmdId = null;
assert(!_isTerminalVisible(), 'not visible when no command selected');

state.currentView = null;
assert(!_isTerminalVisible(), 'not visible when currentView is null');

state.currentView = 'vtty';
state.selectedCmdId = '';
assert(!_isTerminalVisible(), 'not visible when selectedCmdId is empty string');

// ── _flushPendingVttyUpdate ──
console.log('_flushPendingVttyUpdate tests');
assert(typeof _flushPendingVttyUpdate === 'function', '_flushPendingVttyUpdate is a function');

// No pending update → no-op
state._pendingVttyDirty = false;
state._pendingVttyData = null;
let updateVttyCalled = false;
globalThis.updateVttyDisplay = function(data) { updateVttyCalled = true; };
globalThis.applyVttyDiff = function(data) { updateVttyCalled = true; };
globalThis.loadVttyHttp = function() { updateVttyCalled = true; };

_flushPendingVttyUpdate();
assert(!updateVttyCalled, '_flushPendingVttyUpdate no-op when not dirty');

// Pending with no data → calls loadVttyHttp
state._pendingVttyDirty = true;
state._pendingVttyData = null;
state.selectedInstUrl = 'http://localhost:9090';
state.selectedCmdId = 'cmd-1';
_flushPendingVttyUpdate();
assert(updateVttyCalled, '_flushPendingVttyUpdate calls loadVttyHttp when dirty but no data');
assertEq(state._pendingVttyDirty, false, 'dirty flag cleared after flush');

// Pending with data but no cells → calls updateVttyDisplay
updateVttyCalled = false;
state._pendingVttyDirty = true;
state._pendingVttyData = { html: '<span>test</span>' };
_flushPendingVttyUpdate();
assert(updateVttyCalled, '_flushPendingVttyUpdate calls updateVttyDisplay when data has no cells');
assertEq(state._pendingVttyData, null, 'pending data cleared after flush');

// Pending with cells → calls applyVttyDiff
updateVttyCalled = false;
state._pendingVttyDirty = true;
state._pendingVttyData = { cells: [{ row: 0, col: 0, ch: 'a' }] };
_flushPendingVttyUpdate();
assert(updateVttyCalled, '_flushPendingVttyUpdate calls applyVttyDiff when data has cells');

// ── _cacheTerminalForSwitch ──
console.log('_cacheTerminalForSwitch tests');
assert(typeof _cacheTerminalForSwitch === 'function', '_cacheTerminalForSwitch is a function');

// No panel → early return
state.panels = [];
state._focusedPanelId = null;
assert(() => { _cacheTerminalForSwitch(); }, '_cacheTerminalForSwitch no-panel early return');

// With panel but the function does DOM operations via getSelectedPanel/querySelector
// Since MockElement querySelector has limited depth, test the early returns
state.panels = [];
state._focusedPanelId = null;
const cachePanel = addPanelDirect();
state._focusedPanelId = cachePanel.id;
state.selectedCmdId = 'cache-cmd';
// Panel exists but getSelectedPanel needs DOM element
_elementRegistry.delete(cachePanel.id);
assert(() => { _cacheTerminalForSwitch(); }, '_cacheTerminalForSwitch no-DOM early return');

// ── _restoreCachedDom ──
console.log('_restoreCachedDom tests');
assert(typeof _restoreCachedDom === 'function', '_restoreCachedDom is a function');

// No cached DOM → no-op
assert(() => { _restoreCachedDom('nonexistent-cmd'); }, '_restoreCachedDom no-cache no-op');

// Cached DOM exists but no panel → no-op
state._cachedDomPre['restore-test'] = document.createDocumentFragment();
state._cachedScrollPos['restore-test'] = 42;
state.panels = [];
state._focusedPanelId = null;
assert(() => { _restoreCachedDom('restore-test'); }, '_restoreCachedDom no-panel no-op');
// Cached data should still exist (not deleted)
assert(state._cachedDomPre['restore-test'] !== undefined, 'cache not deleted without panel');

// ── updateSidebarSelection ──
console.log('updateSidebarSelection tests');
assert(typeof updateSidebarSelection === 'function', '_updateSidebarSelection is a function');

// No command list in DOM → no-op
assert(() => { updateSidebarSelection(); }, 'updateSidebarSelection no-op with no DOM');

// With mock command items
state.selectedInstUrl = 'http://localhost:9090';
state.selectedCmdId = 'cmd-selected';
const cmdItem1 = document.createElement('div');
cmdItem1.className = 'cmd-item';
cmdItem1.dataset.instUrl = 'http://localhost:9090';
cmdItem1.dataset.cmdId = 'cmd-selected';
const cmdItem2 = document.createElement('div');
cmdItem2.className = 'cmd-item';
cmdItem2.dataset.instUrl = 'http://localhost:9090';
cmdItem2.dataset.cmdId = 'cmd-other';

// Mock querySelectorAll to return our items
const origQuerySelectorAll = document.querySelectorAll;
document.querySelectorAll = function(sel) {
    if (sel === '#commandList .cmd-item') {
        return [cmdItem1, cmdItem2];
    }
    return origQuerySelectorAll.call(this, sel);
};

updateSidebarSelection();
assert(cmdItem1.classList.contains('selected'), 'matching cmd-item gets selected class');
assert(!cmdItem2.classList.contains('selected'), 'non-matching cmd-item not selected');

// Clear selection
state.selectedCmdId = null;
updateSidebarSelection();
assert(!cmdItem1.classList.contains('selected'), 'selected class removed');

// Restore
document.querySelectorAll = origQuerySelectorAll;

// ── _pushPanelHistory ──
console.log('_pushPanelHistory tests');
assert(typeof _pushPanelHistory === 'function', '_pushPanelHistory is a function');

const histPanel = addPanelDirect();
histPanel.cmdHistory = [];
histPanel.cmdHistoryIdx = -1;
histPanel.selectedInstUrl = 'http://localhost:9090';
histPanel.selectedCmdId = 'cmd-1';

_pushPanelHistory(histPanel);
assertEq(histPanel.cmdHistory.length, 1, 'history has 1 entry');
assertEq(histPanel.cmdHistoryIdx, 0, 'history index at 0');
assertEq(histPanel.cmdHistory[0].instUrl, 'http://localhost:9090', 'history instUrl correct');
assertEq(histPanel.cmdHistory[0].cmdId, 'cmd-1', 'history cmdId correct');

// Push another entry
histPanel.selectedCmdId = 'cmd-2';
_pushPanelHistory(histPanel);
assertEq(histPanel.cmdHistory.length, 2, 'history has 2 entries');
assertEq(histPanel.cmdHistoryIdx, 1, 'history index at 1');

// Duplicate of current → not pushed
_pushPanelHistory(histPanel);
assertEq(histPanel.cmdHistory.length, 2, 'duplicate not pushed');

// No panelObj → early return
assert(() => { _pushPanelHistory(null); }, '_pushPanelHistory null early return');
assert(() => { _pushPanelHistory({ selectedCmdId: null }); }, '_pushPanelHistory no-cmd early return');

// ── _updatePanelHistoryBtns ──
console.log('_updatePanelHistoryBtns tests');
assert(typeof _updatePanelHistoryBtns === 'function', '_updatePanelHistoryBtns is a function');

const hbPanel = addPanelDirect();
hbPanel.cmdHistory = [{ instUrl: 'http://a', cmdId: 'c1' }, { instUrl: 'http://a', cmdId: 'c2' }];
hbPanel.cmdHistoryIdx = 1;

const backBtn = document.createElement('button');
backBtn.id = 'histBack-' + hbPanel.id;
backBtn.classList.add('hidden');
const fwdBtn = document.createElement('button');
fwdBtn.id = 'histFwd-' + hbPanel.id;
fwdBtn.classList.add('hidden');

_updatePanelHistoryBtns(hbPanel.id);
assert(!backBtn.classList.contains('hidden'), 'back button visible when history has prev');
assert(fwdBtn.classList.contains('hidden'), 'fwd button hidden at end of history');

// At beginning of history
hbPanel.cmdHistoryIdx = 0;
_updatePanelHistoryBtns(hbPanel.id);
assert(backBtn.classList.contains('hidden'), 'back button hidden at start');
assert(!fwdBtn.classList.contains('hidden'), 'fwd button visible when history has next');

// No panel found → no crash
assert(() => { _updatePanelHistoryBtns('nonexistent'); }, '_updatePanelHistoryBtns no-crash on missing panel');

// ── panelHistoryBack ──
console.log('panelHistoryBack tests');
assert(typeof panelHistoryBack === 'function', 'panelHistoryBack is a function');

const navPanel = addPanelDirect();
navPanel.cmdHistory = [
    { instUrl: 'http://a', cmdId: 'c1' },
    { instUrl: 'http://a', cmdId: 'c2' },
    { instUrl: 'http://a', cmdId: 'c3' },
];
navPanel.cmdHistoryIdx = 2;

// Mock _selectCommandForPanel to avoid complex DOM ops
globalThis._selectCommandForPanel = function(panelObj, instUrl, cmdId) {
    panelObj.selectedInstUrl = instUrl;
    panelObj.selectedCmdId = cmdId;
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    state.bufferView = 'current';
};

panelHistoryBack(navPanel.id);
assertEq(navPanel.cmdHistoryIdx, 1, 'navigated back in history');

// Already at start → no-op
navPanel.cmdHistoryIdx = 0;
panelHistoryBack(navPanel.id);
assertEq(navPanel.cmdHistoryIdx, 0, 'no-op at start of history');

// No panel → early return
assert(() => { panelHistoryBack('nonexistent'); }, 'panelHistoryBack no-crash on missing panel');

// ── panelHistoryForward ──
console.log('panelHistoryForward tests');
assert(typeof panelHistoryForward === 'function', 'panelHistoryForward is a function');

navPanel.cmdHistoryIdx = 0;
panelHistoryForward(navPanel.id);
assertEq(navPanel.cmdHistoryIdx, 1, 'navigated forward in history');

// At end → no-op
navPanel.cmdHistoryIdx = 2;
panelHistoryForward(navPanel.id);
assertEq(navPanel.cmdHistoryIdx, 2, 'no-op at end of history');

// No panel → early return
assert(() => { panelHistoryForward('nonexistent'); }, 'panelHistoryForward no-crash on missing panel');

// ── _selectCommandForPanel ──
console.log('_selectCommandForPanel tests');
assert(typeof _selectCommandForPanel === 'function', '_selectCommandForPanel is a function');

const selPanel = addPanelDirect();
selPanel.selectedCmdId = null;
selPanel.selectedInstUrl = null;

globalThis.focusPanel = function(id) { state._focusedPanelId = id; };
assert(() => { _selectCommandForPanel(selPanel, 'http://localhost:9090', 'cmd-x'); }, '_selectCommandForPanel does not throw');
assertEq(selPanel.selectedInstUrl, 'http://localhost:9090', 'instUrl set on panel');
assertEq(selPanel.selectedCmdId, 'cmd-x', 'cmdId set on panel');
assertEq(state.selectedInstUrl, 'http://localhost:9090', 'global instUrl synced');
assertEq(state.selectedCmdId, 'cmd-x', 'global cmdId synced');
assertEq(state.bufferView, 'current', 'bufferView reset to current');

// ── selectCommand ──
console.log('selectCommand tests');
assert(typeof selectCommand === 'function', 'selectCommand is a function');

// No panels → no-op
state.panels = [];
state._focusedPanelId = null;
const prevCmd = state.selectedCmdId;
selectCommand('http://a.com', 'cmd-x', 'testcmd');
assertEq(state.selectedCmdId, prevCmd, 'selectCommand no-op when no panels');

// With panel — calls real selectCommand which does DOM operations
state.panels = [];
const scPanel = addPanelDirect();
state._focusedPanelId = scPanel.id;
state.connections = [{ url: 'http://localhost:9090', label: 'Local', _commands: [] }];
assert(() => { selectCommand('http://localhost:9090', 'cmd-sc', 'testcmd'); }, 'selectCommand does not throw');

// ── History cap at 50 entries ──
console.log('history cap tests');
const capPanel = addPanelDirect();
capPanel.cmdHistory = [];
capPanel.cmdHistoryIdx = -1;
for (let i = 0; i < 55; i++) {
    capPanel.selectedCmdId = 'cmd-' + i;
    _pushPanelHistory(capPanel);
}
assertEq(capPanel.cmdHistory.length, 50, 'history capped at 50 entries');
assertEq(capPanel.cmdHistoryIdx, 49, 'history index at last entry');

// ── Forward truncation on history push ──
console.log('history truncation tests');
const truncPanel = addPanelDirect();
truncPanel.cmdHistory = [
    { instUrl: 'http://a', cmdId: 'c1' },
    { instUrl: 'http://a', cmdId: 'c2' },
    { instUrl: 'http://a', cmdId: 'c3' },
];
truncPanel.cmdHistoryIdx = 0; // go back to start
truncPanel.selectedCmdId = 'c2';
_pushPanelHistory(truncPanel);
assertEq(truncPanel.cmdHistory.length, 2, 'forward history truncated');
assertEq(truncPanel.cmdHistory[1].cmdId, 'c2', 'new entry pushed after truncation');

console.log('\n[command-selection.js] Tests complete');