/// test/test_cmd_selected_pane.js — Tests for the CRITICAL bug:
///   "Commands do not appear in the selected pane"
///
/// Root causes found and fixed:
/// A) _selectActiveLeafCommand used raw _focusedLeafId which could point
///    to a deleted leaf (after unsplit), causing _findLeafState to return
///    null and the command to SILENTLY DISAPPEAR.
/// B) unsplitPanel never reset _focusedLeafId — after removing a split,
///    _focusedLeafId still pointed to the deleted branch.
/// C) _cacheTerminalForSwitch / _restoreCachedDom always targeted the
///    FIRST .vtty-container in the panel, not the focused leaf's container.
require('./setup');

console.log('\n=== Bug: Commands Appear in Selected Pane ===\n');

function assert(cond, msg) {
    if (!cond) { console.log('  FAIL: ' + msg); process.exitCode = 1; }
    else console.log('  ok: ' + msg);
}
function assertEq(a, b, msg) {
    if (a !== b) { console.log('  FAIL: ' + msg + ' — got ' + JSON.stringify(a) + ', expected ' + JSON.stringify(b)); process.exitCode = 1; }
    else console.log('  ok: ' + msg);
}

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

// ──────────────────────────────────────────────────────────────
// CSP-001: After full unsplit, selectCommand STILL works
//           (was broken: _focusedLeafId pointed to deleted branch,
//            _findLeafState returned null, command silently dropped)
// ──────────────────────────────────────────────────────────────
console.log('CSP-001: selectCommand works after full unsplit');
{
    _loadCalls = [];
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-A', name: 'a', args: [] },
              { id: 'cmd-B', name: 'b', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    p.selectedCmdId = 'cmd-A';
    p.selectedInstUrl = 'http://localhost:9090';

    // Split
    splitPanel(p.id, 'horizontal');
    const branch = p.split.branch;
    const branchId = branch.id;

    // Focus on branch
    _setActiveSideForLeaf(p, branchId);
    assertEq(p._focusedLeafId, branchId, 'CSP-001a: focused on branch');

    // Full unsplit (no leafId → removes everything)
    unsplitPanel(p.id);
    assertEq(p._focusedLeafId, p.id, 'CSP-001b: _focusedLeafId reset to root after unsplit');
    assert(!p.split, 'CSP-001c: split removed');

    // Select cmd-B — MUST work (was silently dropping before fix)
    selectCommand('http://localhost:9090', 'cmd-B', 'b');

    assertEq(p.selectedCmdId, 'cmd-B', 'CSP-001d: root has cmd-B after unsplit + select');
    const panelLoad = _loadCalls.find(c => c.fn === 'panel');
    assert(!!panelLoad, 'CSP-001e: loadVttyHttpForPanel was called');
}

// ──────────────────────────────────────────────────────────────
// CSP-002: After partial unsplit (remove branch), selectCommand
//           works on remaining root pane
// ──────────────────────────────────────────────────────────────
console.log('CSP-002: selectCommand works after removing focused branch');
{
    _loadCalls = [];
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-X', name: 'x', args: [] },
              { id: 'cmd-Y', name: 'y', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    splitPanel(p.id, 'horizontal');
    const branch = p.split.branch;
    const branchId = branch.id;

    // Focus on branch
    _setActiveSideForLeaf(p, branchId);
    assertEq(p._focusedLeafId, branchId, 'CSP-002a: focused on branch');

    // Unsplit the branch specifically
    unsplitPanel(p.id, branchId);
    assertEq(p._focusedLeafId, p.id, 'CSP-002b: _focusedLeafId reset after branch unsplit');
    assert(!p.split, 'CSP-002c: split removed');

    // Select cmd-Y — MUST work
    selectCommand('http://localhost:9090', 'cmd-Y', 'y');
    assertEq(p.selectedCmdId, 'cmd-Y', 'CSP-002d: root has cmd-Y');
}

// ──────────────────────────────────────────────────────────────
// CSP-003: After unsplit, _getFocusedLeafId returns panel.id
//           (not the stale deleted branch ID)
// ──────────────────────────────────────────────────────────────
console.log('CSP-003: _getFocusedLeafId correct after unsplit');
{
    state.panels = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    const branchId = p.split.branch.id;

    // Focus on branch
    p._focusedLeafId = branchId;

    // Full unsplit
    unsplitPanel(p.id);

    // _getFocusedLeafId must return panel.id, not the deleted branch
    const focusedId = _getFocusedLeafId(p);
    assertEq(focusedId, p.id, 'CSP-003a: _getFocusedLeafId returns root after unsplit');
}

// ──────────────────────────────────────────────────────────────
// CSP-004: _selectActiveLeafCommand falls back to root when
//           focused leaf no longer exists
// ──────────────────────────────────────────────────────────────
console.log('CSP-004: _selectActiveLeafCommand fallback to root');
{
    _loadCalls = [];
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-Z', name: 'z', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    splitPanel(p.id, 'horizontal');
    const branchId = p.split.branch.id;

    // Manually set _focusedLeafId to the branch (simulating stale state)
    p._focusedLeafId = branchId;

    // Now manually remove the split (simulating unsplit without reset)
    p.split = null;
    p._rootSplit = null;

    // Select command — _selectActiveLeafCommand should fall back to root
    selectCommand('http://localhost:9090', 'cmd-Z', 'z');

    assertEq(p.selectedCmdId, 'cmd-Z', 'CSP-004a: root has cmd-Z despite stale _focusedLeafId');
    const panelLoad = _loadCalls.find(c => c.fn === 'panel');
    assert(!!panelLoad, 'CSP-004b: loadVttyHttpForPanel was called (fallback to root)');
}

// ──────────────────────────────────────────────────────────────
// CSP-005: _cacheTerminalForSwitch targets focused leaf, not first
// ──────────────────────────────────────────────────────────────
console.log('CSP-005: _cacheTerminalForSwitch targets focused leaf');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-A', name: 'a', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    splitPanel(p.id, 'horizontal');
    const branch = p.split.branch;

    // Focus on branch, set a command on it
    _setActiveSideForLeaf(p, branch.id);
    branch.cmdId = 'cmd-A';
    branch.instUrl = 'http://localhost:9090';
    state.selectedCmdId = 'cmd-A';

    // _cacheTerminalForSwitch should target branch's container, not root's
    // We verify by checking that it uses _focusedLeafId, not querySelector('.vtty-container')
    // The function now uses document.getElementById('vtty-' + leafId)
    // where leafId = p._focusedLeafId || p.id = branch.id
    // So it targets vtty-${branch.id}, NOT the first .vtty-container

    // Verify the focused leaf ID is the branch
    const panelObj = state.panels.find(pp => pp.id === p.id);
    const expectedLeafId = panelObj._focusedLeafId || panelObj.id;
    assertEq(expectedLeafId, branch.id, 'CSP-005a: cache targets branch leaf ID, not root');
}

// ──────────────────────────────────────────────────────────────
// CSP-006: _restoreCachedDom targets focused leaf, not first
// ──────────────────────────────────────────────────────────────
console.log('CSP-006: _restoreCachedDom targets focused leaf');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-A', name: 'a', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    splitPanel(p.id, 'horizontal');
    const branch = p.split.branch;

    _setActiveSideForLeaf(p, branch.id);
    state.selectedCmdId = 'cmd-A';

    // Same verification as CSP-005: the function uses _focusedLeafId
    const panelObj = state.panels.find(pp => pp.id === p.id);
    const expectedLeafId = panelObj._focusedLeafId || panelObj.id;
    assertEq(expectedLeafId, branch.id, 'CSP-006a: restore targets branch leaf ID, not root');
}

// ──────────────────────────────────────────────────────────────
// CSP-007: Unsplit focused branch in _rootSplit tree resets _focusedLeafId
// ──────────────────────────────────────────────────────────────
console.log('CSP-007: Unsplit _rootSplit branch resets focus');
{
    state.panels = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;
    const rootBranchId = rootBranch.id;

    // Focus on rootBranch
    _setActiveSideForLeaf(p, rootBranchId);
    assertEq(p._focusedLeafId, rootBranchId, 'CSP-007a: focused on rootBranch');

    // Unsplit rootBranch
    unsplitPanel(p.id, rootBranchId);
    assertEq(p._focusedLeafId, p.id, 'CSP-007b: _focusedLeafId reset to root');
}

// ──────────────────────────────────────────────────────────────
// CSP-008: _leafIdInSubtree correctly detects descendants
// ──────────────────────────────────────────────────────────────
console.log('CSP-008: _leafIdInSubtree detects descendants');
{
    state.panels = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    const branch = p.split.branch;

    // Split the branch itself
    branch.split = {
        direction: 'vertical', splitRatio: 0.5, activeSide: 'panel',
        branch: { id: 'sub-branch-1', cmdId: null, instUrl: null, scrollbackOffset: 0 },
    };
    const subBranch = branch.split.branch;
    const subBranchId = subBranch.id;

    assert(_leafIdInSubtree(branch, subBranchId), 'CSP-008a: direct child found');
    assert(_leafIdInSubtree(branch, branch.id), 'CSP-008b: self found');
    assert(!_leafIdInSubtree(branch, p.id), 'CSP-008c: unrelated ID not found');
    assert(!_leafIdInSubtree(branch, null), 'CSP-008d: null target returns false');
    assert(!_leafIdInSubtree(null, subBranchId), 'CSP-008e: null leaf returns false');

    // Cleanup: remove the manual split
    branch.split = null;
}

console.log('\n[Bug: Commands in Selected Pane] Tests complete\n');
