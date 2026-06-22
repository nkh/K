/// test/test_bug_click_focus.js — Tests for Bug 7:
///   "Must click twice to select a pane"
///
/// The old click handler toggled focused=true/false. The fix always focuses.

require('./setup');

console.log('\n=== Bug 7: Click-to-Focus Tests ===\n');

// NOTE: Do NOT call resetTestState() — it clears _listeners, removing
// the click/keydown handlers registered by keyboard.js during setup.

// ── Mocks (set before any state changes) ──
const _saved = saveMock('renderPanels', 'startPanelUpdateMode', 'updateSharedToolbar',
    'setupPanelHeaderDrag', 'updateSidebarSelection', 'updatePanelCommandInfo',
    'updateTerminalDisconnectedOverlay', 'disconnectPanelWs', 'loadVttyHttpForPanel');
globalThis.renderPanels = function() {};
globalThis.startPanelUpdateMode = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.setupPanelHeaderDrag = function() {};
globalThis.updateSidebarSelection = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis.disconnectPanelWs = function() {};
globalThis.loadVttyHttpForPanel = function() {};

// ──────────────────────────────────────────────────────────────
// BUG7-001: First click sets focused=true (not toggle)
// ──────────────────────────────────────────────────────────────
console.log('BUG7-001: Single click focuses terminal');
{
    state.panels = [{ id: 'test-panel', focused: false, selectedCmdId: 'cmd-1',
        selectedInstUrl: 'http://localhost', split: null, _focusedLeafId: null }];
    state._focusedPanelId = 'test-panel';
    state.currentView = 'vtty';
    state.selectedCmdId = 'cmd-1';
    state.connections = [{ url: 'http://localhost', label: 'L', token: '', reachable: true, _commands: [] }];
    state.windows = [];
    state.activeWindowId = null;

    const panel = document.createElement('div');
    panel.id = 'test-panel';
    panel.className = 'panel';
    const vtty = document.createElement('div');
    vtty.className = 'vtty-container';
    vtty.setAttribute('data-leaf-id', 'test-panel');
    panel.appendChild(vtty);
    document.body.appendChild(panel);

    emitEvent({ type: 'click', target: vtty, preventDefault(){}, stopPropagation(){} });

    assert(state.panels[0].focused === true,
        'BUG7-001a: panel focused after single click');
}

// ──────────────────────────────────────────────────────────────
// BUG7-002: Second click STILL keeps focused=true (no toggle)
// ──────────────────────────────────────────────────────────────
console.log('BUG7-002: Second click keeps focus (no toggle)');
{
    state.panels = [{ id: 'test-panel-2', focused: true, selectedCmdId: 'cmd-1',
        selectedInstUrl: 'http://localhost', split: null, _focusedLeafId: null }];
    state._focusedPanelId = 'test-panel-2';
    state.currentView = 'vtty';
    state.selectedCmdId = 'cmd-1';

    const panel = document.createElement('div');
    panel.id = 'test-panel-2';
    panel.className = 'panel';
    const vtty = document.createElement('div');
    vtty.className = 'vtty-container';
    vtty.setAttribute('data-leaf-id', 'test-panel-2');
    panel.appendChild(vtty);
    document.body.appendChild(panel);

    emitEvent({ type: 'click', target: vtty, preventDefault(){}, stopPropagation(){} });

    assert(state.panels[0].focused === true,
        'BUG7-002a: panel STILL focused after second click');
}

// ──────────────────────────────────────────────────────────────
// BUG7-003: Click outside vtty unfocuses
// ──────────────────────────────────────────────────────────────
console.log('BUG7-003: Click outside unfocuses');
{
    state.panels = [{ id: 'test-panel-3', focused: true, selectedCmdId: 'cmd-1',
        selectedInstUrl: 'http://localhost', split: null, _focusedLeafId: null }];
    state._focusedPanelId = 'test-panel-3';
    state.currentView = 'vtty';
    state.selectedCmdId = 'cmd-1';

    const outsideEl = { closest() { return null; } };
    emitEvent({ type: 'click', target: outsideEl, preventDefault(){}, stopPropagation(){} });

    assert(state.panels[0].focused === false,
        'BUG7-003a: panel unfocused after clicking outside');
}

// Restore mocks
restoreMock(_saved);

console.log('\n[Bug 7: Click-to-Focus] Tests complete');
