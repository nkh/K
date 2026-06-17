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
// WIN-001: _renderWindowBar uses data-window, delegate uses data-value sig
// ──────────────────────────────────────────────────────────────
console.log('WIN-001: delegate sig mismatch — SwitchWindow/CloseWindow get undefined');

// The _renderWindowBar HTML uses data-window="..." but the delegate maps
// SwitchWindow and CloseWindow to sig 'data-value' which reads el.dataset.value.
// This means clicking window tabs/buttons always passes undefined.
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

    const sigFn = _sigs['data-value'];
    const args = sigFn(tab, {}, null);
    assertEq(args[0], undefined,
        'WIN-001a: data-value sig reads undefined when HTML has data-window (confirms bug)');

    assertEq(tab.dataset.window, win2,
        'WIN-001b: data-window attribute IS set correctly on the element');

    let receivedArg = '__sentinel__';
    const savedSwitch = window.switchWindow;
    window.switchWindow = function(id) { receivedArg = id; };
    const ev = createMockEvent({ target: tab });
    _dispatchAction(ev);
    assertEq(receivedArg, undefined,
        'WIN-001c: dispatching SwitchWindow action passes undefined (confirms bug)');
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

    assert(html.includes('data-value="win-html-1"'),
        'WIN-004d: SwitchWindow tab uses data-value for delegation sig');
    assert(html.includes('data-value="win-html-2"'),
        'WIN-004e: second window tab uses data-value');
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
    tab.setAttribute('data-value', 'win-delegate-2');

    let receivedId = null;
    const savedSwitch = window.switchWindow;
    window.switchWindow = function(id) { receivedId = id; };
    const ev = createMockEvent({ target: tab });
    _dispatchAction(ev);
    assertEq(receivedId, 'win-delegate-2',
        'WIN-006a: delegate dispatches correct window ID via data-value');
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
    btn.setAttribute('data-value', 'win-close-2');

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
// WIN-009: disconnectPanelWs clears baselines for secondary split too
// ──────────────────────────────────────────────────────────────
console.log('WIN-009: disconnectPanelWs clears secondary diff baselines');
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
    // Set up a split
    splitPanel(p.id, 'horizontal');
    p.split.secondaryInstUrl = 'http://localhost:9090';
    p.split.secondaryCmdId = 'cmd-sec';

    state._focusedPanelId = p.id;
    state._diffBaselines[p.id + '/cmd-pri'] = 'uuid-pri';
    state._diffBaselines[p.id + '/cmd-sec'] = 'uuid-sec';

    disconnectPanelWs(p.id);

    assertEq(state._diffBaselines[p.id + '/cmd-pri'], undefined,
        'WIN-009a: primary diff baseline cleared');
    assertEq(state._diffBaselines[p.id + '/cmd-sec'], undefined,
        'WIN-009b: secondary diff baseline cleared');
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

    // Create a mock event whose target is inside the secondary split pane
    const secondaryVtty = document.createElement('div');
    secondaryVtty.setAttribute('data-split-side', 'secondary');
    secondaryVtty.setAttribute('data-panel', p.id);
    secondaryVtty.className = 'vtty-container';

    const ev = {
        preventDefault: function() {},
        stopPropagation: function() {},
        dataTransfer: {
            getData: function(mime) { return mime === 'application/x-cmd' ? cmdData : ''; },
        },
        target: secondaryVtty,
    };

    // Track what _handleSecondarySelect receives
    let secondaryReceived = null;
    const savedSecondary = window._handleSecondarySelect;
    window._handleSecondarySelect = function(panelObj, instUrl, cmdId) {
        secondaryReceived = { panelId: panelObj.id, instUrl, cmdId };
    };

    onPanelDrop(ev, p.id);

    // Should NOT have created a new panel
    assertEq(state.panels.length, panelCountBefore,
        'WIN-010a: no new panel created when dropping on split pane');

    // Should have called _handleSecondarySelect with the correct args
    assert(secondaryReceived !== null,
        'WIN-010b: _handleSecondarySelect was called');
    if (secondaryReceived) {
        assertEq(secondaryReceived.instUrl, 'http://localhost:9090',
            'WIN-010c: secondary select received correct instUrl');
        assertEq(secondaryReceived.cmdId, 'cmd-new',
            'WIN-010d: secondary select received correct cmdId');
        assertEq(secondaryReceived.panelId, p.id,
            'WIN-010e: secondary select received correct panel');
    }

    window._handleSecondarySelect = savedSecondary;
}

// ──────────────────────────────────────────────────────────────
// WIN-011: onPanelDrop on PRIMARY side of split pane assigns to primary
// ──────────────────────────────────────────────────────────────
console.log('WIN-011: onPanelDrop on primary side of split pane assigns to primary');
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

    // Create a mock event whose target is inside the PRIMARY split pane
    const primaryVtty = document.createElement('div');
    primaryVtty.setAttribute('data-split-side', 'primary');
    primaryVtty.setAttribute('data-panel', p.id);
    primaryVtty.className = 'vtty-container';

    const ev = {
        preventDefault: function() {},
        stopPropagation: function() {},
        dataTransfer: {
            getData: function(mime) { return mime === 'application/x-cmd' ? cmdData : ''; },
        },
        target: primaryVtty,
    };

    // Track what _selectCommandForPanel receives
    let primaryReceived = null;
    const savedSelect = window._selectCommandForPanel;
    window._selectCommandForPanel = function(panelObj, instUrl, cmdId) {
        primaryReceived = { panelId: panelObj.id, instUrl, cmdId };
    };

    onPanelDrop(ev, p.id);

    assertEq(state.panels.length, panelCountBefore,
        'WIN-011a: no new panel created when dropping on primary split side');
    assert(primaryReceived !== null,
        'WIN-011b: _selectCommandForPanel was called');
    if (primaryReceived) {
        assertEq(primaryReceived.cmdId, 'cmd-replaced',
            'WIN-011c: primary select received correct cmdId');
    }

    window._selectCommandForPanel = savedSelect;
}

// ──────────────────────────────────────────────────────────────
// WIN-012: onPanelDrop on non-split panel still creates new pane
// ──────────────────────────────────────────────────────────────
console.log('WIN-012: onPanelDrop on non-split panel creates new pane (existing behavior)');
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

    // Should have created a new panel (existing behavior for non-split panels)
    assertEq(state.panels.length, panelCountBefore + 1,
        'WIN-012a: new pane created when dropping on non-split panel');
}