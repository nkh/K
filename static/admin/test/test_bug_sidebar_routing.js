/// test/test_bug_sidebar_routing.js — Tests for Bug 2:
///   "Clicking a command in the sidebar routes it to the NON-SELECTED pane"
///
/// Key insight: functions defined inside IIFEs use closure-local references.
/// Monkey-patching window._selectActiveLeafCommand etc. does NOT intercept
/// calls from within the IIFE. We must verify behavior through STATE changes.
require('./setup');

console.log('\n=== Bug 2: Sidebar Command Routing Tests ===\n');

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
// BUG2-001: After split, _focusedLeafId should be initialized
//           to panel.id (root leaf) so selectCommand has a valid target
// ──────────────────────────────────────────────────────────────
console.log('BUG2-001: _focusedLeafId initialized after split');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-1', name: 'top', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    p.selectedCmdId = 'cmd-1';
    p.selectedInstUrl = 'http://localhost:9090';

    splitPanel(p.id, 'horizontal');
    assert(p.split !== null, 'BUG2-001a: split created');

    // After split, _focusedLeafId MUST be initialized, not null.
    // splitPanel intentionally focuses the NEW branch pane so the
    // user can immediately select a command for it.
    const branchLeaf = p.split.branch;
    assert(p._focusedLeafId === branchLeaf.id,
        'BUG2-001b: _focusedLeafId initialized to branch after split');
}

// ──────────────────────────────────────────────────────────────
// BUG2-002: selectCommand routes to branch leaf when branch is focused
//           Exact scenario: user splits, clicks right pane, clicks sidebar
// ──────────────────────────────────────────────────────────────
console.log('BUG2-002: selectCommand routes to focused branch leaf');
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

    // Simulate user clicking on the branch (right) pane
    _setActiveSideForLeaf(p, branchLeaf.id);
    assertEq(p._focusedLeafId, branchLeaf.id,
        'BUG2-002a: _focusedLeafId is branch after click');

    // User clicks "htop" in the sidebar
    selectCommand('http://localhost:9090', 'cmd-B', 'htop');

    // CRITICAL: cmd-B MUST be in the BRANCH (right) pane
    assertEq(branchLeaf.cmdId, 'cmd-B',
        'BUG2-002b: branch leaf has cmd-B (htop)');
    assertEq(branchLeaf.instUrl, 'http://localhost:9090',
        'BUG2-002c: branch leaf has correct instUrl');
    // Root must be UNCHANGED
    assertEq(p.selectedCmdId, 'cmd-A',
        'BUG2-002d: root leaf still has cmd-A (unchanged)');
}

// ──────────────────────────────────────────────────────────────
// BUG2-003: selectCommand routes to root leaf when root is focused
// ──────────────────────────────────────────────────────────────
console.log('BUG2-003: selectCommand routes to root when root is focused');
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

    // User clicks on root (left) pane
    _setActiveSideForLeaf(p, p.id);

    // User clicks "htop" in the sidebar
    selectCommand('http://localhost:9090', 'cmd-B', 'htop');

    // Root leaf should have cmd-B
    assertEq(p.selectedCmdId, 'cmd-B',
        'BUG2-003a: root leaf has cmd-B');
    assertEq(p.selectedInstUrl, 'http://localhost:9090',
        'BUG2-003b: root leaf has correct instUrl');
    // Branch should be UNCHANGED
    assertEq(branchLeaf.cmdId, null,
        'BUG2-003c: branch leaf still null (unchanged)');
}

// ──────────────────────────────────────────────────────────────
// BUG2-004: focusPanel does NOT reset _focusedLeafId (regression)
// ──────────────────────────────────────────────────────────────
console.log('BUG2-004: focusPanel preserves _focusedLeafId');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-1', name: 'top', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    splitPanel(p.id, 'horizontal');
    const branchLeaf = p.split.branch;

    _setActiveSideForLeaf(p, branchLeaf.id);
    assertEq(p._focusedLeafId, branchLeaf.id, 'BUG2-004-pre: branch focused');

    // focusPanel should NOT change _focusedLeafId
    focusPanel(p.id);
    assertEq(p._focusedLeafId, branchLeaf.id,
        'BUG2-004a: _focusedLeafId still branch after focusPanel');
}

// ──────────────────────────────────────────────────────────────
// BUG2-005: Full real-world flow
//   1. Create panel, put cmd-A on it
//   2. Split (right-click → Split)
//   3. Click on right pane (branch)
//   4. Click cmd-B in sidebar
//   5. Verify cmd-B is in the RIGHT pane
// ──────────────────────────────────────────────────────────────
console.log('BUG2-005: full real-world flow');
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

    // Step 1: panel with cmd-A
    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    p.selectedCmdId = 'cmd-A';
    p.selectedInstUrl = 'http://localhost:9090';

    // Step 2: split
    splitPanel(p.id, 'horizontal');
    const branchLeaf = p.split.branch;

    // Step 3: click on right pane
    _setActiveSideForLeaf(p, branchLeaf.id);

    // Step 4: click cmd-B in sidebar
    selectCommand('http://localhost:9090', 'cmd-B', 'htop');

    // Step 5: verify
    assertEq(branchLeaf.cmdId, 'cmd-B',
        'BUG2-005a: RIGHT pane has cmd-B');
    assertEq(p.selectedCmdId, 'cmd-A',
        'BUG2-005b: LEFT pane still has cmd-A');
}

// ──────────────────────────────────────────────────────────────
// BUG2-006: _getFocusedLeafId returns correct leaf after split
// ──────────────────────────────────────────────────────────────
console.log('BUG2-006: _getFocusedLeafId returns correct leaf');
{
    state.panels = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    const branchLeaf = p.split.branch;

    // splitPanel focuses the newly created branch pane after split.
    let focusedId = _getFocusedLeafId(p);
    assertEq(focusedId, branchLeaf.id,
        'BUG2-006a: default focused leaf is branch (new pane)');

    // After clicking branch, activeSide is 'branch'
    _setActiveSideForLeaf(p, branchLeaf.id);
    focusedId = _getFocusedLeafId(p);
    assertEq(focusedId, branchLeaf.id,
        'BUG2-006b: after branch click, focused leaf is branch');

    // After clicking root, activeSide is 'panel'
    _setActiveSideForLeaf(p, p.id);
    focusedId = _getFocusedLeafId(p);
    assertEq(focusedId, p.id,
        'BUG2-006c: after root click, focused leaf is root');
}

console.log('\n[Bug 2: Sidebar Routing] Tests complete');
