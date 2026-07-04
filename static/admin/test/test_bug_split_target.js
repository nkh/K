/// test/test_bug_split_target.js — Tests for:
///   "Splitting splits the last pane, it must split the selected pane"
///
/// Root cause: splitPanel had a "Bug 1a fix" that redirected leafId from
/// the root (panel.id) to the branch when panel.split already existed.
/// This meant the root pane could never be split after the first split.
/// Fix: removed the redirect; when splitting the root pane and panel.split
/// exists, create panel._rootSplit (parallel to branch.split for branches).
require('./setup');

console.log('\n=== Bug: Split Targets Selected Pane ===\n');

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

function assertEq(a, b, msg) {
    if (a !== b) { console.log('  FAIL: ' + msg + ' — got ' + JSON.stringify(a) + ', expected ' + JSON.stringify(b)); process.exitCode = 1; }
    else console.log('  ok: ' + msg);
}
function assertIncludes(str, sub, msg) {
    if (!str.includes(sub)) { console.log('  FAIL: ' + msg); process.exitCode = 1; }
    else console.log('  ok: ' + msg);
}
function assert(cond, msg) {
    if (!cond) { console.log('  FAIL: ' + msg); process.exitCode = 1; }
    else console.log('  ok: ' + msg);
}

// ──────────────────────────────────────────────────────────────
// SPT-001: Splitting the root pane (selected) should split the root,
//           NOT redirect to the branch (last) pane.
// ──────────────────────────────────────────────────────────────
console.log('SPT-001: Split root pane splits root, not branch');
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

    // First split: root + branch1
    splitPanel(p.id, 'horizontal');
    assert(p.split !== null, 'SPT-001a: first split created');
    const branch1 = p.split.branch;
    assert(branch1 !== null, 'SPT-001b: first split has branch');
    const branch1Id = branch1.id;

    // User clicks the ROOT pane header — focus stays on root
    p._focusedLeafId = p.id;

    // Split the root pane — must create _rootSplit, NOT redirect to branch
    splitPanel(p.id, 'vertical', p.id);
    assert(p._rootSplit !== null, 'SPT-001c: _rootSplit created (root was split)');
    assert(p.split !== null, 'SPT-001d: top-level split still exists');
    assert(p.split.branch.id === branch1Id, 'SPT-001e: original branch unchanged');
    const rootBranch = p._rootSplit.branch;
    assert(rootBranch !== null, 'SPT-001f: _rootSplit has its own branch');

    // Verify _focusedLeafId moved to the newly created branch (intentional: user can immediately select a command)
    assertEq(p._focusedLeafId, rootBranch.id, 'SPT-001g: focus moved to root branch after splitting root');

    // Verify branch1 was NOT split (it should be untouched)
    assert(branch1.split === null, 'SPT-001h: branch1 was not split');
}

// ──────────────────────────────────────────────────────────────
// SPT-002: Splitting the branch pane (selected) should split the branch,
//           NOT the root.
// ──────────────────────────────────────────────────────────────
console.log('SPT-002: Split branch pane splits branch, not root');
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

    // First split: root + branch1
    splitPanel(p.id, 'horizontal');
    const branch1 = p.split.branch;
    const branch1Id = branch1.id;

    // User clicks the BRANCH pane header — focus moves to branch
    p._focusedLeafId = branch1Id;

    // Split the branch pane
    splitPanel(p.id, 'vertical', branch1Id);
    assert(branch1.split !== null, 'SPT-002a: branch1 now has a split');
    assert(!p._rootSplit, 'SPT-002b: _rootSplit was NOT created');
    assert(p.split !== null, 'SPT-002c: top-level split still exists');

    const branch1Sub = branch1.split.branch;
    assert(branch1Sub !== null, 'SPT-002d: branch1 split has sub-branch');

    // Verify focus moved to the newly created sub-branch (intentional: user can select a command)
    assertEq(p._focusedLeafId, branch1Sub.id, 'SPT-002e: focus moved to branch1 sub-branch');
}

// ──────────────────────────────────────────────────────────────
// SPT-003: Keyboard split (no leafId) splits the focused leaf,
//           not the last/branch pane.
// ──────────────────────────────────────────────────────────────
console.log('SPT-003: Keyboard split targets focused leaf');
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

    // First split: root + branch1
    splitPanel(p.id, 'horizontal');
    const branch1 = p.split.branch;
    const branch1Id = branch1.id;

    // Focus on root
    p._focusedLeafId = p.id;

    // Keyboard shortcut calls splitPanel without leafId — should use _focusedLeafId
    splitPanel(p.id, 'vertical');
    assert(p._rootSplit !== null, 'SPT-003a: _rootSplit created (root was focused)');
    assert(branch1.split === null, 'SPT-003b: branch1 not split (root was focused)');
}

// ──────────────────────────────────────────────────────────────
// SPT-004: Context menu split uses the leafId from the right-clicked header,
//           not a fallback to the branch.
// ──────────────────────────────────────────────────────────────
console.log('SPT-004: Context menu split uses correct leafId');
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

    // First split
    splitPanel(p.id, 'horizontal');
    const branch1 = p.split.branch;
    const branch1Id = branch1.id;

    // Context menu is opened on the root header: leafId = p.id
    // Simulate: showPanelContextMenu(e, p.id, p.id) → splitPanel(p.id, 'horizontal', p.id)
    splitPanel(p.id, 'horizontal', p.id);
    assert(p._rootSplit !== null, 'SPT-004a: root split via context menu (leafId=p.id)');
    assert(branch1.split === null, 'SPT-004b: branch untouched');
}

// ──────────────────────────────────────────────────────────────
// SPT-005: _findLeafState finds leaves in _rootSplit tree
// ──────────────────────────────────────────────────────────────
console.log('SPT-005: _findLeafState finds _rootSplit leaves');
{
    state.panels = [];
    state.connections = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    // First split
    splitPanel(p.id, 'horizontal');
    const branch1 = p.split.branch;
    const branch1Id = branch1.id;

    // Split root
    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;
    const rootBranchId = rootBranch.id;

    // Find root branch via _findLeafState
    const found1 = _findLeafState(p, rootBranchId);
    assert(found1 !== null && found1.leaf !== null, 'SPT-005a: found root branch');
    assertEq(found1.leaf.id, rootBranchId, 'SPT-005b: correct root branch found');

    // Find top-level branch via _findLeafState
    const found2 = _findLeafState(p, branch1Id);
    assert(found2 !== null && found2.leaf !== null, 'SPT-005c: found top-level branch');
    assertEq(found2.leaf.id, branch1Id, 'SPT-005d: correct top-level branch found');

    // Find root itself
    const found3 = _findLeafState(p, p.id);
    assert(found3 !== null && found3.leaf === p, 'SPT-005e: root panel found as itself');
}

// ──────────────────────────────────────────────────────────────
// SPT-006: _getAllLeaves includes _rootSplit leaves
// ──────────────────────────────────────────────────────────────
console.log('SPT-006: _getAllLeaves includes _rootSplit leaves');
{
    state.panels = [];
    state.connections = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);

    const leaves = _getAllLeaves(p);
    // Should have: root (panel), rootBranch (from _rootSplit), branch1 (from top-level split)
    assertEq(leaves.length, 3, 'SPT-006a: 3 leaves total');
    assert(leaves.some(l => l.leaf.id === p.id), 'SPT-006b: root in leaves');
    assert(leaves.some(l => l.leaf.id === p._rootSplit.branch.id), 'SPT-006c: root branch in leaves');
    assert(leaves.some(l => l.leaf.id === p.split.branch.id), 'SPT-006d: top-level branch in leaves');
}

// ──────────────────────────────────────────────────────────────
// SPT-007: Unsplit leaf within _rootSplit works correctly
// ──────────────────────────────────────────────────────────────
console.log('SPT-007: Unsplit _rootSplit leaf');
{
    state.panels = [];
    state.connections = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);
    const rootBranchId = p._rootSplit.branch.id;
    assert(p._rootSplit !== null, 'SPT-007a: _rootSplit exists');

    // Unsplit the root branch
    unsplitPanel(p.id, rootBranchId);
    assert(!p._rootSplit, 'SPT-007b: _rootSplit removed after unsplit');
    assert(p.split !== null, 'SPT-007c: top-level split still exists');
}

// ──────────────────────────────────────────────────────────────
// SPT-008: Full unsplit (no leafId) removes both _rootSplit and split
// ──────────────────────────────────────────────────────────────
console.log('SPT-008: Full unsplit removes all splits');
{
    state.panels = [];
    state.connections = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);
    assert(!!p._rootSplit, 'SPT-008a: _rootSplit exists');
    assert(p.split !== null, 'SPT-008b: split exists');

    unsplitPanel(p.id);
    assert(!p._rootSplit, 'SPT-008c: _rootSplit removed');
    assert(!p.split, 'SPT-008d: split removed');
}

console.log('\n[Bug: Split Targets Selected Pane] Tests complete\n');
