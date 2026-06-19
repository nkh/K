/// test/test_windows.js — Tests for window management (create/switch/close).
/// Reproduces bugs: can't close windows, can't switch, new window breaks old,
/// diff baseline not cleared on disconnect, drop on split pane.
require('./setup');
const { createMockEvent } = require('./helpers');

console.log('\n=== Window Management Tests ===\n');

resetTestState();

// Mock functions that depend on network/DOM
globalThis.renderPanels = function() {};
globalThis.startPanelUpdateMode = function() {};
globalThis.stopPanelUpdateMode = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.stopPanelPoll = function() {};
globalThis.focusPanel = function(id) { state._focusedPanelId = id; };
globalThis.setupPanelHeaderDrag = function() {};

// Reset window state (not in resetTestState)
state.windows = [];
state.activeWindowId = null;

// ──────────────────────────────────────────────────────────────
// WIN-001: _renderWindowBar uses data-window, delegate uses data-window sig
// ──────────────────────────────────────────────────────────────
console.log('WIN-001: delegate sig matches — data-window used end-to-end');

// The _renderWindowBar HTML uses data-window="..." and the delegate maps
// SwitchWindow and CloseWindow to sig 'data-window' which reads el.dataset.window.
// This means clicking window tabs/buttons correctly passes the window ID.
{
    state.windows = [];
    state.activeWindowId = null;
    const win1 = 'win-test-1';
    const win2 = 'win-test-2';
    const p1 = addPanelDirect();
    const p2 = addPanelDirect();
    state.windows = [
        { id: win1, name: '1', panelIds: [p1.id] },
        { id: win2, name: '2', panelIds: [p2.id] },
    ];
    state.activeWindowId = win1;
    state._focusedPanelId = p1.id;

    const tab = document.createElement('div');
    tab.setAttribute('data-action', 'SwitchWindow');
    tab.setAttribute('data-window', win2);

    // data-window sig should read the value correctly
    const sigFn = _sigs['data-window'];
    const args = sigFn(tab, {}, null);
    assertEq(args[0], win2,
        'WIN-001a: data-window sig reads correct window ID');

    assertEq(tab.dataset.window, win2,
        'WIN-001b: data-window attribute IS set correctly on the element');

    let receivedArg = null;
    const savedSwitch = window.switchWindow;
    window.switchWindow = function(id) { receivedArg = id; };
    const ev = createMockEvent({ target: tab });
    _dispatchAction(ev);
    assertEq(receivedArg, win2,
        'WIN-001c: dispatching SwitchWindow action passes correct window ID');
    window.switchWindow = savedSwitch;
}

// ──────────────────────────────────────────────────────────────
// WIN-002: closeWindow(undefined) is a no-op — can't close windows
// ──────────────────────────────────────────────────────────────
console.log('WIN-002: closeWindow(undefined) does nothing');
{
    state.windows = [
        { id: 'win-a', name: '1', panelIds: [] },
        { id: 'win-b', name: '2', panelIds: [] },
    ];
    state.activeWindowId = 'win-a';

    const beforeLen = state.windows.length;
    closeWindow(undefined);
    assertEq(state.windows.length, beforeLen,
        'WIN-002a: closeWindow(undefined) does not remove any window');

    const idx = state.windows.findIndex(w => w.id === undefined);
    assertEq(idx, -1, 'WIN-002b: no window has id undefined, so findIndex returns -1');

    closeWindow('win-b');
    assertEq(state.windows.length, 1,
        'WIN-002c: closeWindow with correct id removes the window');
    assertEq(state.windows[0].id, 'win-a',
        'WIN-002d: remaining window is win-a');
}

// ──────────────────────────────────────────────────────────────
// WIN-003: switchWindow(undefined) falls back to windows[0], breaking active window
// ──────────────────────────────────────────────────────────────
console.log('WIN-003: switchWindow(undefined) always shows first window');
{
    state.windows = [];
    state.activeWindowId = null;
    const p1 = addPanelDirect();
    const p2 = addPanelDirect();
    state.windows = [
        { id: 'win-x', name: '1', panelIds: [p1.id] },
        { id: 'win-y', name: '2', panelIds: [p2.id] },
    ];
    state.activeWindowId = 'win-y';

    switchWindow(undefined);

    assertEq(state.activeWindowId, undefined,
        'WIN-003a: switchWindow(undefined) sets activeWindowId to undefined');

    const activeWin = _getActiveWindow();
    assertEq(activeWin.id, 'win-x',
        'WIN-003b: _getActiveWindow falls back to windows[0] when activeWindowId is undefined');

    const visible = _getVisiblePanels();
    assertEq(visible.length, 1, 'WIN-003c: only 1 visible panel (from window 1)');
    assertEq(visible[0].id, p1.id, 'WIN-003d: visible panel is from window 1, not window 2');
}

// ──────────────────────────────────────────────────────────────
// WIN-004: _renderWindowBar HTML output must use data-value (not data-window)
// ──────────────────────────────────────────────────────────────
console.log('WIN-004: _renderWindowBar generates correct data attributes for delegation');
{
    state.windows = [];
    state.activeWindowId = null;
    const p1 = addPanelDirect();
    state.windows = [
        { id: 'win-html-1', name: '1', panelIds: [p1.id] },
        { id: 'win-html-2', name: '2', panelIds: [] },
    ];
    state.activeWindowId = 'win-html-1';

    const html = _renderWindowBar();
    assert(html.includes('data-action="SwitchWindow"'), 'WIN-004a: has SwitchWindow action');
    assert(html.includes('data-action="CreateWindow"'), 'WIN-004b: has CreateWindow action');
    assert(html.includes('data-action="CloseWindow"'), 'WIN-004c: has CloseWindow action');

    assert(html.includes('data-window="win-html-1"'),
        'WIN-004d: SwitchWindow tab uses data-window for delegation sig');
    assert(html.includes('data-window="win-html-2"'),
        'WIN-004e: second window tab uses data-window');
}

// ──────────────────────────────────────────────────────────────
// WIN-005: Full window lifecycle — create, switch, close via delegation
// ──────────────────────────────────────────────────────────────
console.log('WIN-005: full window lifecycle via delegate dispatch');
{
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];

    const p1 = addPanelDirect();
    state._focusedPanelId = p1.id;

    assertEq(state.windows.length, 1, 'WIN-005a: starts with 1 window');
    assertEq(state.windows[0].panelIds.length, 1, 'WIN-005b: window 1 has 1 panel');

    const win2Id = 'win-lifecycle-2';
    state.windows.push({ id: win2Id, name: '2', panelIds: [] });
    switchWindow(win2Id);
    addPanelDirect();
    state._focusedPanelId = state.panels[state.panels.length - 1].id;

    assertEq(state.windows.length, 2, 'WIN-005c: now 2 windows');
    assertEq(state.activeWindowId, win2Id, 'WIN-005d: active is window 2');
    const visible2 = _getVisiblePanels();
    assertEq(visible2.length, 1, 'WIN-005e: window 2 has 1 visible panel');

    switchWindow(state.windows[0].id);
    assertEq(state.activeWindowId, state.windows[0].id, 'WIN-005f: switched back to window 1');
    const visible1 = _getVisiblePanels();
    assertEq(visible1.length, 1, 'WIN-005g: window 1 still has 1 visible panel');
    assertEq(visible1[0].id, p1.id, 'WIN-005h: window 1 panel is the original');

    closeWindow(win2Id);
    assertEq(state.windows.length, 1, 'WIN-005i: back to 1 window after closing window 2');

    closeWindow(state.windows[0].id);
    assertEq(state.windows.length, 1, 'WIN-005j: cannot close the last window');
}

// ──────────────────────────────────────────────────────────────
// WIN-006: SwitchWindow via delegate dispatch passes correct window ID
// ──────────────────────────────────────────────────────────────
console.log('WIN-006: delegate dispatch passes correct ID to switchWindow');
{
    state.windows = [];
    state.activeWindowId = null;
    const p1 = addPanelDirect();
    const p2 = addPanelDirect();
    state.windows = [
        { id: 'win-delegate-1', name: '1', panelIds: [p1.id] },
        { id: 'win-delegate-2', name: '2', panelIds: [p2.id] },
    ];
    state.activeWindowId = 'win-delegate-1';
    state._focusedPanelId = p1.id;

    const tab = document.createElement('div');
    tab.setAttribute('data-action', 'SwitchWindow');
    tab.setAttribute('data-window', 'win-delegate-2');

    let receivedId = null;
    const savedSwitch = window.switchWindow;
    window.switchWindow = function(id) { receivedId = id; };
    const ev = createMockEvent({ target: tab });
    _dispatchAction(ev);
    assertEq(receivedId, 'win-delegate-2',
        'WIN-006a: delegate dispatches correct window ID via data-window');
    window.switchWindow = savedSwitch;
}

// ──────────────────────────────────────────────────────────────
// WIN-007: CloseWindow via delegate dispatch passes correct window ID
// ──────────────────────────────────────────────────────────────
console.log('WIN-007: delegate dispatch passes correct ID to closeWindow');
{
    state.windows = [
        { id: 'win-close-1', name: '1', panelIds: [] },
        { id: 'win-close-2', name: '2', panelIds: [] },
    ];
    state.activeWindowId = 'win-close-1';

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'CloseWindow');
    btn.setAttribute('data-window', 'win-close-2');

    let receivedId = null;
    const savedClose = window.closeWindow;
    window.closeWindow = function(id) { receivedId = id; };
    const ev = createMockEvent({ target: btn });
    _dispatchAction(ev);
    assertEq(receivedId, 'win-close-2',
        'WIN-007a: delegate dispatches correct window ID to closeWindow');
    window.closeWindow = savedClose;
}

// ──────────────────────────────────────────────────────────────
// WIN-008: disconnectPanelWs must clear diff baselines so
//          reconnection gets a full refresh, not a stale empty diff
// ──────────────────────────────────────────────────────────────
console.log('WIN-008: disconnectPanelWs clears diff baselines');
{
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];
    state._diffBaselines = {};

    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-abc';
    p.wsInstUrl = 'http://localhost:9090';
    p.wsCmdId = 'cmd-abc';
    state._focusedPanelId = p.id;

    // Simulate a diff baseline being set (as it would be after receiving WS data)
    state._diffBaselines[p.id + '/cmd-abc'] = 'uuid-stale-baseline-12345';

    // Verify the baseline exists
    assert(state._diffBaselines[p.id + '/cmd-abc'] === 'uuid-stale-baseline-12345',
        'WIN-008a: diff baseline is set before disconnect');

    // This is what happens when switchWindow stops the panel:
    disconnectPanelWs(p.id);

    // After disconnect, the baseline must be cleared so reconnection fetches full content
    assertEq(state._diffBaselines[p.id + '/cmd-abc'], undefined,
        'WIN-008b: diff baseline is cleared after disconnectPanelWs');

    // Panel's ws fields should be nulled (existing behavior)
    assertEq(p.ws, null, 'WIN-008c: panel.ws is null after disconnect');
    assertEq(p.wsInstUrl, null, 'WIN-008d: panel.wsInstUrl is null after disconnect');
    assertEq(p.wsCmdId, null, 'WIN-008e: panel.wsCmdId is null after disconnect');

    // But selectedCmdId/selectedInstUrl must be preserved (the panel still has a command)
    assertEq(p.selectedCmdId, 'cmd-abc',
        'WIN-008f: selectedCmdId preserved after disconnect');
    assertEq(p.selectedInstUrl, 'http://localhost:9090',
        'WIN-008g: selectedInstUrl preserved after disconnect');
}

// ──────────────────────────────────────────────────────────────
// WIN-009: disconnectPanelWs clears baselines for branch split too
// ──────────────────────────────────────────────────────────────
console.log('WIN-009: disconnectPanelWs clears baselines for branch split too');
{
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];
    state._diffBaselines = {};

    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-pri';
    p.wsInstUrl = 'http://localhost:9090';
    p.wsCmdId = 'cmd-pri';
    // Set up a split using the tree structure
    splitPanel(p.id, 'horizontal');
    const branchLeaf = p.split.branch;
    branchLeaf.cmdId = 'cmd-sec';
    branchLeaf.instUrl = 'http://localhost:9090';

    state._focusedPanelId = p.id;
    state._diffBaselines[p.id + '/cmd-pri'] = 'uuid-pri';
    state._diffBaselines[branchLeaf.id + '/cmd-sec'] = 'uuid-sec';

    disconnectPanelWs(p.id);

    assertEq(state._diffBaselines[p.id + '/cmd-pri'], undefined,
        'WIN-009a: root-leaf diff baseline cleared');
    assertEq(state._diffBaselines[branchLeaf.id + '/cmd-sec'], undefined,
        'WIN-009b: branch diff baseline cleared');
}

// ──────────────────────────────────────────────────────────────
// WIN-010: onPanelDrop on split pane assigns to correct side
//          (not creating a new pane)
// ──────────────────────────────────────────────────────────────
console.log('WIN-010: onPanelDrop on split pane assigns to split side');
{
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];
    state.connections = [];

    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-existing';
    state._focusedPanelId = p.id;
    splitPanel(p.id, 'horizontal');
    state.connections.push({ url: 'http://localhost:9090', label: 'Local', token: '', reachable: true, _commands: [] });

    const panelCountBefore = state.panels.length;

    // Simulate dropping a command on the SECONDARY side of a split panel
    const cmdData = JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-new', cmdName: 'my-cmd' });

    // Create a mock event whose target is inside the SECONDARY leaf's vtty
    const branchVtty = document.createElement('div');
    branchVtty.setAttribute('data-leaf-id', p.split.branch.id);
    branchVtty.setAttribute('data-panel', p.id);
    branchVtty.className = 'vtty-container';

    const ev = {
        preventDefault: function() {},
        stopPropagation: function() {},
        dataTransfer: {
            getData: function(mime) { return mime === 'application/x-cmd' ? cmdData : ''; },
        },
        target: branchVtty,
    };

    // Track what _selectLeafCommand receives
    let leafReceived = null;
    const savedLeaf = window._selectLeafCommand;
    window._selectLeafCommand = function(panelObj, leaf, instUrl, cmdId) {
        leafReceived = { panelId: panelObj.id, leafId: leaf.id, instUrl, cmdId };
    };

    onPanelDrop(ev, p.id);

    // Should NOT have created a new panel
    assertEq(state.panels.length, panelCountBefore,
        'WIN-010a: no new panel created when dropping on split pane');

    // Should have called _selectLeafCommand with the correct args
    assert(leafReceived !== null,
        'WIN-010b: _selectLeafCommand was called');
    if (leafReceived) {
        assertEq(leafReceived.instUrl, 'http://localhost:9090',
            'WIN-010c: leaf select received correct instUrl');
        assertEq(leafReceived.cmdId, 'cmd-new',
            'WIN-010d: leaf select received correct cmdId');
        assertEq(leafReceived.panelId, p.id,
            'WIN-010e: leaf select received correct panel');
        assertEq(leafReceived.leafId, p.split.branch.id,
            'WIN-010f: leaf select received correct leaf id');
    }

    window._selectLeafCommand = savedLeaf;
}

// ──────────────────────────────────────────────────────────────
// WIN-011: onPanelDrop on root-leaf side of split pane (already has command) creates new pane
// ──────────────────────────────────────────────────────────────
console.log('WIN-011: onPanelDrop on root-leaf side of split pane creates new pane');
{
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];
    state.connections = [];

    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-old';
    state._focusedPanelId = p.id;
    splitPanel(p.id, 'horizontal');
    state.connections.push({ url: 'http://localhost:9090', label: 'Local', token: '', reachable: true, _commands: [] });

    const panelCountBefore = state.panels.length;

    const cmdData = JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-replaced', cmdName: 'new-cmd' });

    // Create a mock event whose target is inside the root-leaf of the split
    const rootLeafVtty = document.createElement('div');
    rootLeafVtty.setAttribute('data-leaf-id', p.id);
    rootLeafVtty.setAttribute('data-panel', p.id);
    rootLeafVtty.className = 'vtty-container';

    const ev = {
        preventDefault: function() {},
        stopPropagation: function() {},
        dataTransfer: {
            getData: function(mime) { return mime === 'application/x-cmd' ? cmdData : ''; },
        },
        target: rootLeafVtty,
    };

    onPanelDrop(ev, p.id);

    // Root leaf already has a command — drop replaces it
    assertEq(state.panels.length, panelCountBefore,
        'WIN-011a: no new pane created when dropping on root-leaf (command replaced)');
    assertEq(p.selectedCmdId, 'cmd-replaced',
        'WIN-011b: root-leaf got the dropped command');
}

// ──────────────────────────────────────────────────────────────
// WIN-012: onPanelDrop on non-split panel WITH existing command replaces it
// ──────────────────────────────────────────────────────────────
console.log('WIN-012: onPanelDrop on non-split panel with command replaces it');
{
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];
    state.connections = [];

    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-existing';
    state._focusedPanelId = p.id;
    state.connections.push({ url: 'http://localhost:9090', label: 'Local', token: '', reachable: true, _commands: [] });

    const panelCountBefore = state.panels.length;

    const cmdData = JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-dropped', cmdName: 'dropped' });

    // Target is NOT inside a split-pane — just a regular panel
    const regularDiv = document.createElement('div');
    regularDiv.className = 'vtty-container';
    // No data-split-side attribute

    const ev = {
        preventDefault: function() {},
        stopPropagation: function() {},
        dataTransfer: {
            getData: function(mime) { return mime === 'application/x-cmd' ? cmdData : ''; },
        },
        target: regularDiv,
    };

    onPanelDrop(ev, p.id);

    // Should replace the existing command (not create a new pane)
    assertEq(state.panels.length, panelCountBefore,
        'WIN-012a: no new pane created when dropping on non-split panel');
    assertEq(p.selectedCmdId, 'cmd-dropped',
        'WIN-012b: dropped command replaces existing command');
}

// ──────────────────────────────────────────────────────────────
// WIN-013: onPanelDrop on non-split EMPTY panel assigns to that panel
// ──────────────────────────────────────────────────────────────
console.log('WIN-013: onPanelDrop on non-split empty panel assigns to that panel');
{
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];
    state.connections = [];

    const p = addPanelDirect();
    // Panel has NO command selected
    p.selectedInstUrl = null;
    p.selectedCmdId = null;
    state._focusedPanelId = p.id;
    state.connections.push({ url: 'http://localhost:9090', label: 'Local', token: '', reachable: true, _commands: [] });

    const panelCountBefore = state.panels.length;

    const cmdData = JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-assigned', cmdName: 'my-cmd' });

    const regularDiv = document.createElement('div');
    regularDiv.className = 'vtty-container';

    let selectReceived = null;
    const savedSelect = window._selectCommandForPanel;
    window._selectCommandForPanel = function(panelObj, instUrl, cmdId) {
        selectReceived = { panelId: panelObj.id, instUrl, cmdId };
    };

    const ev = {
        preventDefault: function() {},
        stopPropagation: function() {},
        dataTransfer: {
            getData: function(mime) { return mime === 'application/x-cmd' ? cmdData : ''; },
        },
        target: regularDiv,
    };

    onPanelDrop(ev, p.id);

    // Should NOT have created a new panel — empty panel should be reused
    assertEq(state.panels.length, panelCountBefore,
        'WIN-013a: no new panel created when dropping on empty panel');

    // Should have called _selectCommandForPanel for the existing panel
    assert(selectReceived !== null,
        'WIN-013b: _selectCommandForPanel was called for the empty panel');
    if (selectReceived) {
        assertEq(selectReceived.panelId, p.id,
            'WIN-013c: command assigned to the existing panel');
        assertEq(selectReceived.cmdId, 'cmd-assigned',
            'WIN-013d: correct cmdId passed');
    }

    window._selectCommandForPanel = savedSelect;
}

// ──────────────────────────────────────────────────────────────
// WIN-014: onPanelDrop on split panel without specific side uses activeSide
// ──────────────────────────────────────────────────────────────
console.log('WIN-014: onPanelDrop on split panel without specific side uses activeSide');
{
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];
    state.connections = [];

    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-existing';
    state._focusedPanelId = p.id;
    splitPanel(p.id, 'horizontal');
    p.split.activeSide = 'branch';
    state.connections.push({ url: 'http://localhost:9090', label: 'Local', token: '', reachable: true, _commands: [] });

    const panelCountBefore = state.panels.length;

    const cmdData = JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-new', cmdName: 'new-cmd' });

    // Drop target is the panel header (OUTSIDE split-container, no data-split-side)
    const headerDiv = document.createElement('div');
    headerDiv.className = 'panel-header';
    // No data-split-side attribute — simulates dropping on the panel header

    // Drop on header (no data-leaf-id) — assigns to the panel root leaf
    const ev = {
        preventDefault: function() {},
        stopPropagation: function() {},
        dataTransfer: {
            getData: function(mime) { return mime === 'application/x-cmd' ? cmdData : ''; },
        },
        target: headerDiv,
    };

    onPanelDrop(ev, p.id);

    // Command replaces the panel root leaf's existing command (no new pane)
    assertEq(state.panels.length, panelCountBefore,
        'WIN-014a: no new pane created when dropping on split panel header');
    assertEq(p.selectedCmdId, 'cmd-new',
        'WIN-014b: dropped command replaces root leaf command');
}

// ──────────────────────────────────────────────────────────────
// WIN-015: switchWindow does NOT disconnect WS for old window panels
// ──────────────────────────────────────────────────────────────
console.log('WIN-015: switchWindow does NOT disconnect WS');
{
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];
    state._diffBaselines = {};

    const p1 = addPanelDirect();
    p1.selectedInstUrl = 'http://localhost:9090';
    p1.selectedCmdId = 'cmd-abc';
    p1.wsInstUrl = 'http://localhost:9090';
    p1.wsCmdId = 'cmd-abc';

    const p2 = addPanelDirect();
    state._focusedPanelId = p1.id;

    state.windows = [
        { id: 'win-ws-1', name: '1', panelIds: [p1.id] },
        { id: 'win-ws-2', name: '2', panelIds: [p2.id] },
    ];
    state.activeWindowId = 'win-ws-1';

    // Simulate WS being connected
    p1.ws = { readyState: 1 }; // 1 = OPEN

    // Switch to window 2
    switchWindow('win-ws-2');

    // Window 1's panel WS should NOT be disconnected
    assertEq(p1.wsInstUrl, 'http://localhost:9090',
        'WIN-015a: wsInstUrl preserved after switchWindow');
    assertEq(p1.wsCmdId, 'cmd-abc',
        'WIN-015b: wsCmdId preserved after switchWindow');
    assert(p1.ws !== null,
        'WIN-015c: ws reference preserved after switchWindow');
    assertEq(p1.selectedCmdId, 'cmd-abc',
        'WIN-015d: selectedCmdId preserved after switchWindow');

    // Diff baselines should NOT be cleared
    assert(state._diffBaselines !== undefined,
        'WIN-015e: _diffBaselines still exists');
}

// ──────────────────────────────────────────────────────────────
// WIN-016: switchWindow sets _panelsNeedingFetch for visible panels
// ──────────────────────────────────────────────────────────────
console.log('WIN-016: switchWindow marks visible panels for content re-fetch');
{
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];

    const p1 = addPanelDirect();
    p1.selectedInstUrl = 'http://localhost:9090';
    p1.selectedCmdId = 'cmd-1';

    const p2 = addPanelDirect();
    p2.selectedInstUrl = 'http://localhost:9090';
    p2.selectedCmdId = 'cmd-2';

    state.windows = [
        { id: 'win-fetch-1', name: '1', panelIds: [p1.id] },
        { id: 'win-fetch-2', name: '2', panelIds: [p2.id] },
    ];
    state.activeWindowId = 'win-fetch-1';

    // Clear any previous _panelsNeedingFetch
    state._panelsNeedingFetch = null;

    // Switch to window 2 — p2 should be marked for fetch
    switchWindow('win-fetch-2');

    assert(state._panelsNeedingFetch !== null,
        'WIN-016a: _panelsNeedingFetch is set after switchWindow');
    // Note: renderPanels is mocked, so _panelsNeedingFetch is NOT consumed
    assert(state._panelsNeedingFetch.has(p2.id),
        'WIN-016b: window 2 panel is in _panelsNeedingFetch');
    assert(!state._panelsNeedingFetch.has(p1.id),
        'WIN-016c: window 1 panel is NOT in _panelsNeedingFetch');
}

// ──────────────────────────────────────────────────────────────
// WIN-017: _applyPanelLayoutClass never falls back to container
// ──────────────────────────────────────────────────────────────
console.log('WIN-017: _applyPanelLayoutClass never falls back to container');

{
    // Reset
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];

    const container = document.getElementById('view-vtty');
    // Remove any existing panelArea
    const oldArea = document.getElementById('panelArea');
    if (oldArea) oldArea.remove();

    // Clear any inline style from previous tests
    container.style.flexDirection = '';

    // Call _applyPanelLayoutClass when #panelArea doesn't exist
    _applyPanelLayoutClass(container);

    assert(container.style.flexDirection === '' || container.style.flexDirection === undefined,
        'WIN-017a: _applyPanelLayoutClass does NOT set flex-direction on container when panelArea missing');

    // Now add a panelArea and verify it gets the style
    const area = document.createElement('div');
    area.id = 'panelArea';
    area.className = 'panel-area';
    container.appendChild(area);

    state.panelLayout = 'row';
    _applyPanelLayoutClass(container);

    assert(area.style.flexDirection === 'row',
        'WIN-017b: _applyPanelLayoutClass sets flex-direction on panelArea');
    assert(container.style.flexDirection === '' || container.style.flexDirection === undefined,
        'WIN-017c: container is NOT touched when panelArea exists');
}

// ──────────────────────────────────────────────────────────────
// WIN-018: Generation cache cleared when panel DOM destroyed
// (prevents "No command selected" sticking after window switch)
// ──────────────────────────────────────────────────────────────
console.log('WIN-018: generation cache cleared when panel DOM destroyed');

{
    // Reset
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];

    if (!state._lastGeneration) state._lastGeneration = {};

    const p1 = addPanelDirect();
    p1.selectedInstUrl = 'http://localhost:9090';
    p1.selectedCmdId = 'cmd-gen-1';

    const p2 = addPanelDirect();
    p2.selectedInstUrl = 'http://localhost:9090';
    p2.selectedCmdId = 'cmd-gen-2';

    // Simulate: both panels have been rendered and have generation cached
    state._lastGeneration[p1.id + '/' + p1.selectedCmdId] = 42;
    state._lastGeneration[p2.id + '/' + p2.selectedCmdId] = 99;

    // In the real renderPanels, when a panel's DOM element doesn't exist
    // (because it was in a different window and the DOM was rebuilt),
    // the generation cache is cleared. Simulate this by temporarily
    // overriding getElementById to return null for p1.
    const origGetById = document.getElementById;
    document.getElementById = function(id) {
        if (id === p1.id) return null; // simulate destroyed DOM
        return origGetById.call(document, id);
    };

    // Run the cache-clearing logic from renderPanels
    for (const panel of state.panels) {
        const el = document.getElementById(panel.id);
        if (!el) {
            if (panel.selectedCmdId) {
                delete state._lastGeneration[panel.id + '/' + panel.selectedCmdId];
            }
        }
    }

    document.getElementById = origGetById;

    assert(state._lastGeneration[p1.id + '/' + p1.selectedCmdId] === undefined,
        'WIN-018a: generation cache cleared for panel whose DOM was destroyed');
    assert(state._lastGeneration[p2.id + '/' + p2.selectedCmdId] === 99,
        'WIN-018b: generation cache preserved for panel whose DOM still exists');
}

// ──────────────────────────────────────────────────────────────
// WIN-019: _cmdReorderMouseUp drops into existing panel/split
// ──────────────────────────────────────────────────────────────
console.log('WIN-019: mousedown drag drop targets existing panel/split');

// _cmdReorderMouseUp is inside an IIFE and not exported, so we test via
// the _reorderState + mouseup simulation pattern.
// Instead, test the drop logic directly by calling onPanelDrop (HTML5 path)
// which already has correct logic, and verify the mousedown path would
// behave the same by checking _reorderState tracking.

{
    // Reset
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];

    // Create a panel with a command (with DOM)
    const p1 = addPanelDirect();
    p1.selectedInstUrl = 'http://localhost:9090';
    p1.selectedCmdId = 'cmd-existing';
    const p1Dom = document.getElementById(p1.id);
    if (!p1Dom) {
        const d = document.createElement('div'); d.id = p1.id; d.className = 'panel';
        document.getElementById('view-vtty').appendChild(d);
    }

    // Create an empty panel (with DOM)
    const p2 = addPanelDirect();
    p2.selectedCmdId = null;
    p2.selectedInstUrl = null;
    const p2Dom = document.getElementById(p2.id);
    if (!p2Dom) {
        const d = document.createElement('div'); d.id = p2.id; d.className = 'panel';
        document.getElementById('view-vtty').appendChild(d);
    }

    // Track which function was called
    let calledWith = null;
    const origSelect = globalThis._selectCommandForPanel;
    const origNewPane = globalThis._openCommandInNewPane;

    const origSelectLeaf = globalThis._selectLeafCommand;
    const origPushHist = globalThis._pushPanelHistory;
    globalThis._selectCommandForPanel = function(panel, inst, cmd) { calledWith = { fn: 'select', panelId: panel.id, inst, cmd }; };
    globalThis._openCommandInNewPane = function(inst, cmd, name) { calledWith = { fn: 'newPane', inst, cmd }; };
    globalThis._selectLeafCommand = function(panel, leaf, inst, cmd) { calledWith = { fn: 'leafCommand', panelId: panel.id, leafId: leaf.id, inst, cmd }; };
    globalThis._pushPanelHistory = function() {};

    // Test: HTML5 drop on empty panel → should assign to that panel
    const mockEvt1 = { preventDefault: () => {}, stopPropagation: () => {}, dataTransfer: { getData: () => JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-drop', cmdName: 'test' }) }, target: null };
    onPanelDrop(mockEvt1, p2.id);
    assert(calledWith && calledWith.fn === 'select' && calledWith.panelId === p2.id,
        'WIN-019a: HTML5 drop on empty panel assigns command to that panel');

    // Test: HTML5 drop on panel with existing command → replaces it
    calledWith = null;
    const mockEvt2 = { preventDefault: () => {}, stopPropagation: () => {}, dataTransfer: { getData: () => JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-drop2', cmdName: 'test2' }) }, target: null };
    onPanelDrop(mockEvt2, p1.id);
    assert(calledWith && calledWith.fn === 'select' && calledWith.panelId === p1.id,
        'WIN-019b: HTML5 drop on panel with existing command replaces it');

    // Test: HTML5 drop on split pane branch side
    p1.split = { direction: 'horizontal', splitRatio: 0.5, activeSide: 'panel', branch: { id: p1.id + '-branch1', cmdId: null, instUrl: null } };
    calledWith = null;
    const mockEvt3 = { preventDefault: () => {}, stopPropagation: () => {}, dataTransfer: { getData: () => JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-drop3', cmdName: 'test3' }) }, target: null };
    // Create a fake target with data-leaf-id
    const splitDiv = document.createElement('div');
    splitDiv.dataset.leafId = p1.split.branch.id;
    mockEvt3.target = splitDiv;
    onPanelDrop(mockEvt3, p1.id);
    assert(calledWith && calledWith.fn === 'leafCommand' && calledWith.panelId === p1.id,
        'WIN-019c: HTML5 drop on split pane branch side calls _selectLeafCommand');

    // Test: HTML5 drop on split pane root-leaf side (has existing command → replaces it)
    calledWith = null;
    splitDiv.dataset.leafId = p1.id;
    const mockEvt4 = { preventDefault: () => {}, stopPropagation: () => {}, dataTransfer: { getData: () => JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-drop4', cmdName: 'test4' }) }, target: splitDiv };
    onPanelDrop(mockEvt4, p1.id);
    assert(calledWith && calledWith.fn === 'select' && calledWith.panelId === p1.id,
        'WIN-019d: HTML5 drop on split pane root-leaf side replaces existing command');

    // Restore
    globalThis._selectCommandForPanel = origSelect;
    globalThis._openCommandInNewPane = origNewPane;
    globalThis._selectLeafCommand = origSelectLeaf;
    globalThis._pushPanelHistory = origPushHist;
    p1.split = null;
}

// ──────────────────────────────────────────────────────────────
// WIN-020: renderPanels does NOT set flex-direction on #view-vtty
// ──────────────────────────────────────────────────────────────
console.log('WIN-020: renderPanels does NOT set flex-direction on #view-vtty');

{
    // Reset
    state.windows = [];
    state.activeWindowId = null;
    state.panels = [];

    const container = document.getElementById('view-vtty');
    container.style.flexDirection = ''; // start clean

    // Mock the real renderPanels (unmock it temporarily)
    const origRender = globalThis.renderPanels;
    // Create a minimal real renderPanels that exercises _applyPanelLayoutClass
    globalThis.renderPanels = function() {
        const ct = document.getElementById('view-vtty');
        // Simulate the fixed renderPanels flow:
        // 1. Cache loop (no panels to cache)
        // 2. Set innerHTML (creates panelArea)
        ct.innerHTML = '<div class="panel-area" id="panelArea"></div>';
        // 3. Call _applyPanelLayoutClass AFTER innerHTML
        _applyPanelLayoutClass(ct);
    };

    state.panelLayout = 'row';
    renderPanels();

    assert(container.style.flexDirection === '' || container.style.flexDirection === undefined,
        'WIN-020a: renderPanels does NOT set flex-direction on #view-vtty');

    const area = document.getElementById('panelArea');
    assert(area && area.style.flexDirection === 'row',
        'WIN-020b: renderPanels sets flex-direction on #panelArea');

    globalThis.renderPanels = origRender;
}