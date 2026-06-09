/// test/test_command-ui.js — Tests for command UI (panel header, bottom bar, terminal auto-fit)
require('./setup');

console.log('\n=== command-ui.js Tests ===\n');

resetTestState();

// Mock functions
globalThis.renderPanels = function() {};
globalThis.loadCommands = function() { return Promise.resolve(); };
globalThis.updateSharedToolbar = function() {};
globalThis.disconnectPanelWs = function() {};
globalThis.stopPanelPoll = function() {};
globalThis.connectPanelWs = function() {};
globalThis.loadVttyHttp = function() {};
globalThis.scheduleVttyHttp = function() {};
globalThis.trapFocus = function() {};
globalThis.releaseCurrentFocusTrap = function() {};

// ── getSelectedPanel ──
console.log('getSelectedPanel tests');
assert(typeof getSelectedPanel === 'function', 'getSelectedPanel is a function');

// No panels → null
state.panels = [];
state._focusedPanelId = null;
assert(getSelectedPanel() === null, 'getSelectedPanel returns null when no panels');

// With panel but no DOM element registered → returns null gracefully
state.panels = [];
const gsPanel = addPanelDirect();
state._focusedPanelId = gsPanel.id;
// Panel exists but no DOM element registered
_elementRegistry.delete(gsPanel.id);
assert(getSelectedPanel() === null, 'getSelectedPanel returns null when no DOM element');

// Register DOM element for panel
const gsPanelEl = document.createElement('div');
gsPanelEl.id = gsPanel.id;
const gsVtty = document.createElement('div');
gsVtty.className = 'vtty-container';
gsPanelEl.appendChild(gsVtty);
const result = getSelectedPanel();
assert(result !== null, 'getSelectedPanel returns element when panel and DOM exist');
assertEq(result.id, gsPanelEl.id, 'getSelectedPanel returns correct panel element');

// ── getActivePanelId ──
console.log('getActivePanelId tests');
assert(typeof getActivePanelId === 'function', 'getActivePanelId is a function');

// Focused panel
state._focusedPanelId = 'panel-x';
assertEq(getActivePanelId(), 'panel-x', 'returns focused panel id');

// No focus, has panels → returns first panel
state._focusedPanelId = null;
state.panels = [addPanelDirect()];
assertEq(getActivePanelId(), state.panels[0].id, 'returns first panel when no focus');

// No panels → null
state.panels = [];
state._focusedPanelId = null;
assertEq(getActivePanelId(), null, 'returns null when no panels');

// Syncs global state from panel
state.panels = [];
const syncP = addPanelDirect();
syncP.selectedInstUrl = 'http://my-server';
syncP.selectedCmdId = 'cmd-sync';
state._focusedPanelId = syncP.id;
const syncEl = document.createElement('div');
syncEl.id = syncP.id;
_elementRegistry.delete(syncP.id);
getActivePanelId(); // triggers sync
assertEq(state.selectedInstUrl, 'http://my-server', 'syncs selectedInstUrl from focused panel');
assertEq(state.selectedCmdId, 'cmd-sync', 'syncs selectedCmdId from focused panel');

// ── updatePanelCommandInfo ──
console.log('updatePanelCommandInfo tests');
assert(typeof updatePanelCommandInfo === 'function', 'updatePanelCommandInfo is a function');

// No selected command → early return
state.selectedInstUrl = null;
state.selectedCmdId = null;
assert(() => { updatePanelCommandInfo(); }, 'updatePanelCommandInfo no-selection early return');

// With selected command but no panel → early return
state.selectedInstUrl = 'http://localhost:9090';
state.selectedCmdId = 'cmd-1';
state.panels = [];
assert(() => { updatePanelCommandInfo(); }, 'updatePanelCommandInfo no-panel early return');

// With panel + command → updates name
state.panels = [];
const uiPanel = addPanelDirect();
state._focusedPanelId = uiPanel.id;
state.connections = [{ url: 'http://localhost:9090', label: 'Local', _commands: [
    { id: 'cmd-ui-1', name: 'htop', args: ['-s', '1'], alive: true, pid: 123, runtime_secs: 300, frozen: false, exit_code: null }
] }];

// Create DOM structure
const panelEl = document.createElement('div');
panelEl.id = uiPanel.id;
const headerEl = document.createElement('div');
headerEl.className = 'panel-header';
panelEl.appendChild(headerEl);
const nameEl = document.createElement('span');
nameEl.className = 'cmd-fullname';
headerEl.appendChild(nameEl);
const argsEl =.createElement('span');
argsEl.className = 'cmd-args';
headerEl.appendChild(argsEl);
const bottomBarLabel = document.createElement('span');
bottomBarLabel.id = 'cmdLabel';

assert(() => { updatePanelCommandInfo(); }, 'updatePanelCommandInfo does not throw');

// Test with custom title
uiPanel.customTitle = 'My Custom';
assert(() => { updatePanelCommandInfo(); }, 'updatePanelCommandInfo with custom title does not throw');
uiPanel.customTitle = '';

// Test with dead command
state.connections[0]._commands[0].alive = false;
state.connections[0]._commands[0].exit_code = 1;
assert(() => { updatePanelCommandInfo(); }, 'updatePanelCommandInfo with dead command does not throw');

// Test with frozen command
state.connections[0]._commands[0].alive = true;
state.connections[0]._commands[0].frozen = true;
assert(() => { updatePanelCommandInfo(); }, 'updatePanelCommandInfo with frozen command does not throw');

// ── updateBottomBarLabel ──
console.log('updateBottomBarLabel tests');
assert(typeof updateBottomBarLabel === 'function', 'updateBottomBarLabel is a function');

// Null command → clears
assert(() => { updateBottomBarLabel(null); }, 'updateBottomBarLabel null no throw');
assertEq(bottomBarLabel.innerHTML, '', 'bottom bar cleared with null cmd');

// Command with name only
assert(() => { updateBottomBarLabel({ name: 'htop', args: [], pid: null, runtime_secs: null }); }, 'updateBottomBarLabel name-only no throw');

// Command with all fields
assert(() => {
    updateBottomBarLabel({ name: 'bash', args: ['-c', 'echo hi'], pid: 456, runtime_secs: 125 });
}, 'updateBottomBarLabel full command no throw');

// Command with short runtime
assert(() => { updateBottomBarLabel({ name: 'quick', args: [], pid: 1, runtime_secs: 30 }); }, 'updateBottomBarLabel short runtime no throw');

// Command with long runtime
assert(() => { updateBottomBarLabel({ name: 'long', args: [], pid: 1, runtime_secs: 7384 }); }, 'updateBottomBarLabel long runtime no throw');

// Command with id fallback (no name)
assert(() => { updateBottomBarLabel({ id: 'abc-123', args: [], pid: null, runtime_secs: null }); }, 'updateBottomBarLabel id fallback no throw');

// ── autofitTerminalSize ──
console.log('autofitTerminalSize tests');
assert(typeof autofitTerminalSize === 'function', 'autofitTerminalSize is a function');

// No panel → shows hint
state.panels = [];
const autofitHint = document.createElement('span');
autofitHint.id = 'autofitHint';
assert(() => { autofitTerminalSize(); }, 'autofitTerminalSize no throw');
assertEq(autofitHint.textContent, 'No panel visible to measure', 'hint when no panel');

// Panel without vtty-container → shows hint
const fitPanel = addPanelDirect();
state._focusedPanelId = fitPanel.id;
const fitPanelEl = document.createElement('div');
fitPanelEl.id = fitPanel.id;
autofitTerminalSize();
assertEq(autofitHint.textContent, 'No terminal container found', 'hint when no vtty container');

// Panel with vtty-container → calculates size
const fitVtty = document.createElement('div');
fitVtty.className = 'vtty-container';
fitPanelEl.appendChild(fitVtty);

const spawnRows = document.createElement('input');
spawnRows.id = 'spawnRows';
const spawnCols = document.createElement('input');
spawnCols.id = 'spawnCols';

state.fontSize = 10;
assert(() => { autofitTerminalSize(); }, 'autofitTerminalSize with container no throw');
assert(spawnRows.value > 0, 'rows calculated');
assert(spawnCols.value > 0, 'cols calculated');
assert(autofitHint.textContent.includes('rows'), 'hint includes rows');
assert(autofitHint.textContent.includes('cols'), 'hint includes cols');

console.log('\n[command-ui.js] Tests complete');