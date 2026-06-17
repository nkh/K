/// test/test_windows.js — Tests for window management (create/switch/close).
/// Reproduces bugs: can't close windows, can't switch, new window breaks old.
require('./setup');
const { createMockEvent } = require('./helpers');

console.log('\n=== Window Management Tests ===\n');

resetTestState();

// Mock functions that depend on network/DOM
globalThis.renderPanels = function() {};
globalThis.startPanelUpdateMode = function() {};
globalThis.stopPanelUpdateMode = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.disconnectPanelWs = function() {};
globalThis.stopPanelPoll = function() {};
globalThis.focusPanel = function(id) { state._focusedPanelId = id; };
globalThis.setupPanelHeaderDrag = function() {};
globalThis._disconnectSecondaryWs = function() {};

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
    // Create two windows with panels
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

    // Simulate clicking a SwitchWindow tab — the HTML uses data-window="win-test-2"
    // but the delegate sig 'data-value' reads el.dataset.value which is undefined
    const tab = document.createElement('div');
    tab.setAttribute('data-action', 'SwitchWindow');
    tab.setAttribute('data-window', win2);  // This is what _renderWindowBar generates
    // Note: NO data-value attribute set

    const sigFn = _sigs['data-value'];
    const args = sigFn(tab, {}, null);
    assertEq(args[0], undefined,
        'WIN-001a: data-value sig reads undefined when HTML has data-window (confirms bug)');

    // Verify what the correct value should be
    assertEq(tab.dataset.window, win2,
        'WIN-001b: data-window attribute IS set correctly on the element');

    // Now test the actual dispatch: clicking SwitchWindow tab passes undefined to switchWindow
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

    // Calling closeWindow(undefined) — what happens when the button is clicked
    const beforeLen = state.windows.length;
    closeWindow(undefined);
    assertEq(state.windows.length, beforeLen,
        'WIN-002a: closeWindow(undefined) does not remove any window');

    // findIndex returns -1 for undefined id
    const idx = state.windows.findIndex(w => w.id === undefined);
    assertEq(idx, -1, 'WIN-002b: no window has id undefined, so findIndex returns -1');

    // The correct call works
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
    state.activeWindowId = 'win-y';  // Start on window 2

    // switchWindow(undefined) should NOT switch (id === activeWindowId is false though)
    switchWindow(undefined);

    // After switchWindow(undefined), activeWindowId is set to undefined
    assertEq(state.activeWindowId, undefined,
        'WIN-003a: switchWindow(undefined) sets activeWindowId to undefined');

    // _getActiveWindow falls back to windows[0]
    const activeWin = _getActiveWindow();
    assertEq(activeWin.id, 'win-x',
        'WIN-003b: _getActiveWindow falls back to windows[0] when activeWindowId is undefined');

    // The visible panels are now window 1's panels, not window 2's
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

    // After fix: should use data-value (not data-window) so the delegate 'data-value' sig works
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

    // Start with one window, one panel (addPanelDirect calls _initWindows internally)
    const p1 = addPanelDirect();
    state._focusedPanelId = p1.id;

    assertEq(state.windows.length, 1, 'WIN-005a: starts with 1 window');
    assertEq(state.windows[0].panelIds.length, 1, 'WIN-005b: window 1 has 1 panel');

    // Create a second window via keyboard (direct function call, not delegate)
    const win2Id = 'win-lifecycle-2';
    state.windows.push({ id: win2Id, name: '2', panelIds: [] });
    switchWindow(win2Id);
    addPanelDirect();
    state._focusedPanelId = state.panels[state.panels.length - 1].id;

    assertEq(state.windows.length, 2, 'WIN-005c: now 2 windows');
    assertEq(state.activeWindowId, win2Id, 'WIN-005d: active is window 2');
    const visible2 = _getVisiblePanels();
    assertEq(visible2.length, 1, 'WIN-005e: window 2 has 1 visible panel');

    // Switch back to window 1 via direct call (simulating keyboard Alt+1)
    switchWindow(state.windows[0].id);
    assertEq(state.activeWindowId, state.windows[0].id, 'WIN-005f: switched back to window 1');
    const visible1 = _getVisiblePanels();
    assertEq(visible1.length, 1, 'WIN-005g: window 1 still has 1 visible panel');
    assertEq(visible1[0].id, p1.id, 'WIN-005h: window 1 panel is the original');

    // Close window 2 via direct call
    closeWindow(win2Id);
    assertEq(state.windows.length, 1, 'WIN-005i: back to 1 window after closing window 2');

    // Can't close last window
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

    // Create a tab element with data-value (the FIXED attribute)
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