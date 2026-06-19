/// test/test_bug_dragdrop.js — Tests for Bug 3:
///   "Drag-dropping a command creates a new pane instead of assigning to
///    the existing selected pane"
///
/// Note: _cmdReorderMouseUp is not directly testable from outside its IIFE
/// because _reorderState is a closure-local variable. We test the exported
/// onPanelDrop function instead, which has the same routing logic.
require('./setup');
const { createMockEvent } = require('./helpers');

console.log('\n=== Bug 3: Drag-Drop Routing Tests ===\n');

resetTestState();

// ── Mocks for render/DOM functions ──
globalThis.renderPanels = function() {};
globalThis.startPanelUpdateMode = function() {};
globalThis.stopPanelUpdateMode = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.stopPanelPoll = function() {};
globalThis.setupPanelHeaderDrag = function() {};
globalThis.updateSidebarSelection = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis.disconnectPanelWs = function() {};
globalThis._connectLeafWs = function() {};
globalThis._loadLeafVttyHttpDirect = function() {};
globalThis.loadVttyHttpForPanel = function() {};
globalThis._cacheTerminalForSwitch = function() {};
globalThis._restoreCachedDom = function() {};

// ──────────────────────────────────────────────────────────────
// BUG3-001: onPanelDrop on non-split pane WITH command must assign
//           (not create new pane or split)
// ──────────────────────────────────────────────────────────────
console.log('BUG3-001: onPanelDrop on occupied non-split pane assigns');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-A', name: 'top', args: [] },
              { id: 'cmd-B', name: 'htop', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    p.selectedCmdId = 'cmd-A';
    p.selectedInstUrl = 'http://localhost:9090';

    const initialPanelCount = state.panels.length;

    // Create mock event and DOM
    const panelEl = document.createElement('div');
    panelEl.className = 'panel';
    panelEl.id = p.id;
    document.body.appendChild(panelEl);

    const target = document.createElement('div');
    target.className = 'vtty-container';
    target.setAttribute('data-leaf-id', p.id);
    panelEl.appendChild(target);

    const dt = { getData: function(mime) {
        if (mime === 'application/x-cmd') return JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-B', cmdName: 'htop' });
        return '';
    }};
    const e = { preventDefault: function(){}, stopPropagation: function(){}, target: target, dataTransfer: dt };

    onPanelDrop(e, p.id);

    assertEq(state.panels.length, initialPanelCount,
        'BUG3-001a: no new pane created');
    assertEq(p.selectedCmdId, 'cmd-B',
        'BUG3-001b: pane now has cmd-B');

    document.body.removeChild(panelEl);
}

// ──────────────────────────────────────────────────────────────
// BUG3-002: onPanelDrop on split pane branch leaf assigns to it
// ──────────────────────────────────────────────────────────────
console.log('BUG3-002: onPanelDrop on split branch leaf assigns');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-A', name: 'top', args: [] },
              { id: 'cmd-B', name: 'htop', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    p.selectedCmdId = 'cmd-A';
    p.selectedInstUrl = 'http://localhost:9090';

    splitPanel(p.id, 'horizontal');
    const branchLeaf = p.split.branch;

    const panelEl = document.createElement('div');
    panelEl.className = 'panel';
    panelEl.id = p.id;
    document.body.appendChild(panelEl);

    const branchVtty = document.createElement('div');
    branchVtty.className = 'vtty-container';
    branchVtty.setAttribute('data-leaf-id', branchLeaf.id);
    panelEl.appendChild(branchVtty);

    const dt = { getData: function(mime) {
        if (mime === 'application/x-cmd') return JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-B', cmdName: 'htop' });
        return '';
    }};
    const e = { preventDefault: function(){}, stopPropagation: function(){}, target: branchVtty, dataTransfer: dt };

    onPanelDrop(e, p.id);

    assertEq(branchLeaf.cmdId, 'cmd-B',
        'BUG3-002a: branch leaf has cmd-B');
    assertEq(p.selectedCmdId, 'cmd-A',
        'BUG3-002b: root leaf still has cmd-A');

    document.body.removeChild(panelEl);
}

// ──────────────────────────────────────────────────────────────
// BUG3-003: onPanelAreaDrop assigns to focused panel
// ──────────────────────────────────────────────────────────────
console.log('BUG3-003: onPanelAreaDrop assigns to focused panel');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-A', name: 'top', args: [] },
              { id: 'cmd-B', name: 'htop', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    p.selectedCmdId = 'cmd-A';
    p.selectedInstUrl = 'http://localhost:9090';

    const dt = { getData: function(mime) {
        if (mime === 'application/x-cmd') return JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-B', cmdName: 'htop' });
        return '';
    }};
    const e = { preventDefault: function(){}, dataTransfer: dt };

    onPanelAreaDrop(e);

    assertEq(p.selectedCmdId, 'cmd-B',
        'BUG3-003a: focused pane has cmd-B');
}

console.log('\n[Bug 3: Drag-Drop Routing] Tests complete');
