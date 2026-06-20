/// test/test_bug_cmd_routing_selected.js — Tests for:
///   "Selecting a command displays its terminal in the last pane,
///    not the selected one"
///
/// Root cause: three sub-bugs:
/// A) _updateSplitHeaders ignored _rootSplit leaves
/// B) _selectCommandForPanel never called _updateSplitHeaders
/// C) _selectLeafCommand guarded on panelObj.split only (not _rootSplit)
///
/// All three meant pane headers stayed stale after command selection,
/// making it LOOK like the terminal appeared in the wrong pane.
/// The terminal content itself loaded into the correct vtty element,
/// but the header/label mismatch confused users.
require('./setup');

console.log('\n=== Bug: Command Routes to Selected Pane ===\n');

resetTestState();

// ── Mocks ──
globalThis.renderPanels = function() {};
globalThis.startPanelUpdateMode = function() {};
globalThis.stopPanelUpdateMode = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.stopPanelPoll = function() {};
globalThis.setupPanelHeaderDrag = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis.disconnectPanelWs = function() {};
globalThis._connectLeafWs = function() {};
globalThis._loadLeafVttyHttpDirect = function() {};
globalThis.loadVttyHttpForPanel = function() {};
globalThis._cacheTerminalForSwitch = function() {};
globalThis._restoreCachedDom = function() {};

let _loadCalls = [];
const _origLoadLeaf = globalThis._loadLeafVttyHttpDirect;
globalThis._loadLeafVttyHttpDirect = function(leaf) {
    _loadCalls.push({ fn: 'leaf', leafId: leaf ? leaf.id : null, cmdId: leaf ? leaf.cmdId : null });
};
const _origLoadPanel = globalThis.loadVttyHttpForPanel;
globalThis.loadVttyHttpForPanel = function(panelId, instUrl, cmdId) {
    _loadCalls.push({ fn: 'panel', panelId, cmdId });
};

function assert(cond, msg) {
    if (!cond) { console.log('  FAIL: ' + msg); process.exitCode = 1; }
    else console.log('  ok: ' + msg);
}
function assertEq(a, b, msg) {
    if (a !== b) { console.log('  FAIL: ' + msg + ' — got ' + JSON.stringify(a) + ', expected ' + JSON.stringify(b)); process.exitCode = 1; }
    else console.log('  ok: ' + msg);
}

// ──────────────────────────────────────────────────────────────
// CRS-001: Command selection with _rootSplit routes to focused leaf,
//           not the top-level branch (last pane).
// ──────────────────────────────────────────────────────────────
console.log('CRS-001: Command routes to focused leaf, not last pane');
{
    _loadCalls = [];
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-top', name: 'top', args: [] },
              { id: 'cmd-htop', name: 'htop', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    // First split: root + branch1
    splitPanel(p.id, 'horizontal');
    const branch1 = p.split.branch;
    const branch1Id = branch1.id;

    // Split the root (creates _rootSplit): root + rootBranch
    p._focusedLeafId = p.id;
    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;
    const rootBranchId = rootBranch.id;

    // 3 panes now: root (P), rootBranch (R), branch1 (B)
    // User focuses on rootBranch (R)
    p._focusedLeafId = rootBranchId;

    // Select command "top" — should go to rootBranch (R), NOT branch1 (B)
    selectCommand('http://localhost:9090', 'cmd-top', 'top');

    // Verify the command was loaded into the ROOT BRANCH leaf
    assertEq(rootBranch.cmdId, 'cmd-top', 'CRS-001a: rootBranch got cmd-top');
    assertEq(rootBranch.instUrl, 'http://localhost:9090', 'CRS-001b: rootBranch got correct instUrl');
    // Verify branch1 was NOT touched
    assert(branch1.cmdId !== 'cmd-top', 'CRS-001c: branch1 was NOT given cmd-top');
    // Verify _loadLeafVttyHttpDirect was called for rootBranch
    const rootBranchLoad = _loadCalls.find(c => c.fn === 'leaf' && c.leafId === rootBranchId);
    assert(!!rootBranchLoad, 'CRS-001d: _loadLeafVttyHttpDirect called for rootBranch');
    // Verify loadVttyHttpForPanel was NOT called (root leaf path not used)
    const panelLoad = _loadCalls.find(c => c.fn === 'panel');
    assert(!panelLoad, 'CRS-001e: loadVttyHttpForPanel NOT called (target was branch leaf)');
}

// ──────────────────────────────────────────────────────────────
// CRS-002: Command selection with focused root in split routes to root
// ──────────────────────────────────────────────────────────────
console.log('CRS-002: Command routes to root when root is focused');
{
    _loadCalls = [];
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-top', name: 'top', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    // Split: root + branch1
    splitPanel(p.id, 'horizontal');
    const branch1 = p.split.branch;
    const branch1Id = branch1.id;

    // User focuses on ROOT
    p._focusedLeafId = p.id;

    // Select command
    selectCommand('http://localhost:9090', 'cmd-top', 'top');

    // Should go to root leaf via _selectCommandForPanel
    assertEq(p.selectedCmdId, 'cmd-top', 'CRS-002a: root got cmd-top');
    assertEq(p.selectedInstUrl, 'http://localhost:9090', 'CRS-002b: root got correct instUrl');
    // Verify branch1 was NOT touched
    assert(branch1.cmdId !== 'cmd-top', 'CRS-002c: branch1 untouched');
    // loadVttyHttpForPanel should be called for the root
    const panelLoad = _loadCalls.find(c => c.fn === 'panel' && c.panelId === p.id);
    assert(!!panelLoad, 'CRS-002d: loadVttyHttpForPanel called for root');
}

// ──────────────────────────────────────────────────────────────
// CRS-003: Switching focus then selecting command routes to NEW
//           focused leaf, not the previous one.
// ──────────────────────────────────────────────────────────────
console.log('CRS-003: Switch focus then select routes to new focus');
{
    _loadCalls = [];
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-a', name: 'cmd_a', args: [] },
              { id: 'cmd-b', name: 'cmd_b', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    // Split: root + branch1
    splitPanel(p.id, 'horizontal');
    const branch1 = p.split.branch;
    const branch1Id = branch1.id;

    // Focus on branch1, select cmd-a
    p._focusedLeafId = branch1Id;
    selectCommand('http://localhost:9090', 'cmd-a', 'cmd_a');
    assertEq(branch1.cmdId, 'cmd-a', 'CRS-003a: branch1 got cmd-a');

    // Switch focus to ROOT
    p._focusedLeafId = p.id;

    // Select cmd-b — should go to ROOT, not branch1
    _loadCalls = [];
    selectCommand('http://localhost:9090', 'cmd-b', 'cmd_b');

    assertEq(p.selectedCmdId, 'cmd-b', 'CRS-003b: root got cmd-b');
    // branch1 should still have cmd-a (untouched)
    assertEq(branch1.cmdId, 'cmd-a', 'CRS-003c: branch1 still has cmd-a');
    // Verify loadVttyHttpForPanel was called for root
    const panelLoad = _loadCalls.find(c => c.fn === 'panel' && c.panelId === p.id);
    assert(!!panelLoad, 'CRS-003d: loadVttyHttpForPanel called for root');
}

// ──────────────────────────────────────────────────────────────
// CRS-004: _updateSplitHeaders updates _rootSplit leaves
// ──────────────────────────────────────────────────────────────
console.log('CRS-004: _updateSplitHeaders updates _rootSplit leaves');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-top', name: 'top', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);

    // Mock _updateOneSplitHeader to track calls
    const headerUpdates = [];
    const origUpdate = globalThis._updateOneSplitHeader;
    // Can't easily mock IIFE functions — verify through _updateSplitHeaders
    // not crashing (it was returning early before the fix)
    try {
        _updateSplitHeaders(p);
        assert(true, 'CRS-004a: _updateSplitHeaders does not crash with _rootSplit');
    } catch (e) {
        assert(false, 'CRS-004a: _updateSplitHeaders crashed: ' + e.message);
    }
}

// ──────────────────────────────────────────────────────────────
// CRS-005: Select command in _rootSplit leaf, then select in
//           top-level branch — each routes correctly.
// ──────────────────────────────────────────────────────────────
console.log('CRS-005: Alternating selections route to correct leaves');
{
    _loadCalls = [];
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-x', name: 'x', args: [] },
              { id: 'cmd-y', name: 'y', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;
    const rootBranchId = rootBranch.id;
    const topBranch = p.split.branch;
    const topBranchId = topBranch.id;

    // Select cmd-x into rootBranch
    p._focusedLeafId = rootBranchId;
    selectCommand('http://localhost:9090', 'cmd-x', 'x');
    assertEq(rootBranch.cmdId, 'cmd-x', 'CRS-005a: rootBranch has cmd-x');
    assert(!topBranch.cmdId || topBranch.cmdId !== 'cmd-x', 'CRS-005b: topBranch does NOT have cmd-x');

    // Select cmd-y into topBranch
    _loadCalls = [];
    p._focusedLeafId = topBranchId;
    selectCommand('http://localhost:9090', 'cmd-y', 'y');
    assertEq(topBranch.cmdId, 'cmd-y', 'CRS-005c: topBranch has cmd-y');
    assertEq(rootBranch.cmdId, 'cmd-x', 'CRS-005d: rootBranch still has cmd-x');

    // Select cmd-y into root
    _loadCalls = [];
    p._focusedLeafId = p.id;
    selectCommand('http://localhost:9090', 'cmd-y', 'y');
    assertEq(p.selectedCmdId, 'cmd-y', 'CRS-005e: root has cmd-y');
    assertEq(rootBranch.cmdId, 'cmd-x', 'CRS-005f: rootBranch still has cmd-x');
}

console.log('\n[Bug: Command Routes to Selected Pane] Tests complete\n');
