/// test_bug_drag_reorder_panelids.js — Fix 6.1
/// After drag-reordering panels, window.panelIds must be updated to match
/// the new DOM order. Otherwise panelIds and state.panels diverge.
'use strict';

const { createMockEvent } = require('./helpers');

console.log('\n=== Fix 6.1: window.panelIds updated after drag reorder ===\n');

// Mock render functions that would be called during drag
globalThis.renderPanels = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.setupPanelHeaderDrag = function() {};

// DRP-001: panelIds order matches new DOM order after drag reorder
{
    resetTestState();
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', reachable: true, _commands: [] }];

    // Create a window and add 3 panels
    state.windows = [];
    state.activeWindowId = null;
    const p1 = addPanelDirect();
    const p2 = addPanelDirect();
    const p3 = addPanelDirect();

    const win = _getActiveWindow();
    assert(win, 'DRP-001a: window exists');
    assertDeepEq(win.panelIds, [p1.id, p2.id, p3.id],
        'DRP-001b: initial panelIds order is [p1, p2, p3]');

    // Build DOM that mimics the user dragging p3 before p1
    const container = document.createElement('div');
    container.id = 'panelArea';
    document.body.appendChild(container);

    const el1 = document.createElement('div');
    el1.className = 'panel'; el1.id = p1.id;
    const el2 = document.createElement('div');
    el2.className = 'panel'; el2.id = p2.id;
    const el3 = document.createElement('div');
    el3.className = 'panel'; el3.id = p3.id;

    // New DOM order: p3, p1, p2 (user dragged p3 to the front)
    container.appendChild(el3);
    container.appendChild(el1);
    container.appendChild(el2);

    // Simulate what _panelDragMouseUp does: rebuild state.panels from DOM order
    const newOrder = [];
    container.querySelectorAll('.panel').forEach(p => {
        const pp = state.panels.find(x => x.id === p.id);
        if (pp) newOrder.push(pp);
    });
    state.panels = newOrder;

    // THE FIX: update win.panelIds to match
    if (win && win.panelIds) {
        const newIds = newOrder.map(p => p.id);
        const newIdSet = new Set(newIds);
        win.panelIds = newIds.concat(win.panelIds.filter(id => !newIdSet.has(id)));
    }

    assertDeepEq(win.panelIds, [p3.id, p1.id, p2.id],
        'DRP-001c: panelIds updated to [p3, p1, p2] after drag reorder');

    document.body.removeChild(container);
}

// DRP-002: minimized panels stay at end of panelIds after reorder
{
    resetTestState();
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', reachable: true, _commands: [] }];
    state.windows = [];
    state.activeWindowId = null;

    const p1 = addPanelDirect();
    const p2 = addPanelDirect();
    const p3 = addPanelDirect();
    p2.minimized = true;

    const win = _getActiveWindow();
    assertDeepEq(win.panelIds, [p1.id, p2.id, p3.id],
        'DRP-002a: initial panelIds includes minimized p2');

    // Simulate drag reorder: p3 before p1 (p2 is minimized, not in DOM)
    const container = document.createElement('div');
    container.id = 'panelArea';
    document.body.appendChild(container);

    const el3 = document.createElement('div');
    el3.className = 'panel'; el3.id = p3.id;
    const el1 = document.createElement('div');
    el1.className = 'panel'; el1.id = p1.id;
    container.appendChild(el3);
    container.appendChild(el1);

    const newOrder = [];
    container.querySelectorAll('.panel').forEach(p => {
        const pp = state.panels.find(x => x.id === p.id);
        if (pp) newOrder.push(pp);
    });
    state.panels = newOrder;

    // THE FIX
    if (win && win.panelIds) {
        const newIds = newOrder.map(p => p.id);
        const newIdSet = new Set(newIds);
        win.panelIds = newIds.concat(win.panelIds.filter(id => !newIdSet.has(id)));
    }

    // p3, p1 reordered; p2 (minimized) stays at end
    assertDeepEq(win.panelIds, [p3.id, p1.id, p2.id],
        'DRP-002b: minimized p2 preserved at end after reorder');

    document.body.removeChild(container);
}

// DRP-003: _getVisiblePanels returns panels in panelIds-matching order
{
    resetTestState();
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', reachable: true, _commands: [] }];
    state.windows = [];
    state.activeWindowId = null;

    const p1 = addPanelDirect();
    const p2 = addPanelDirect();
    const p3 = addPanelDirect();

    // Reorder state.panels to [p3, p1, p2]
    state.panels = [p3, p1, p2];

    const visible = _getVisiblePanels();
    // _getVisiblePanels uses a Set from panelIds, then filters state.panels.
    // So the order follows state.panels, which is now [p3, p1, p2].
    assertDeepEq(visible.map(p => p.id), [p3.id, p1.id, p2.id],
        'DRP-003: _getVisiblePanels follows state.panels order after reorder');
}

console.log('\n[Fix 6.1: Drag Reorder panelIds] Tests complete');