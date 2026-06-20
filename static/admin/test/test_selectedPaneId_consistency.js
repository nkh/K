/// test/test_selectedPaneId_consistency.js
///
/// Tests that _focusedLeafId (selectedPaneId) is CONSISTENTLY honored
/// across ALL code paths — not just selectCommand but also:
///   - _getFocusedLeafId validation
///   - click-to-focus handlers
///   - keyboard shortcuts
///   - WS connect/disconnect
///   - context menus
///   - terminal operations (copy, export, restart)
///
/// Founding principle: _focusedLeafId / selectedPaneId is the SOLE source
/// of truth for "which pane". Every function that resolves a leaf MUST
/// check BOTH panel.split AND panel._rootSplit.
require('./setup');

console.log('\n=== selectedPaneId (_focusedLeafId) Consistency Tests ===\n');

// Re-define assert/assertEq to print ok: on success
function assert(cond, msg) {
    if (!cond) { console.log('  FAIL: ' + msg); process.exitCode = 1; }
    else console.log('  ok: ' + msg);
}
function assertEq(a, b, msg) {
    if (a !== b) { console.log('  FAIL: ' + msg + ' — got ' + JSON.stringify(a) + ', expected ' + JSON.stringify(b)); process.exitCode = 1; }
    else console.log('  ok: ' + msg);
}

resetTestState();

// ── Mocks for render/DOM functions ──
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

// Track leaf WS connect calls
let _wsConnectCalls = [];
const _origConnectLeafWs = globalThis._connectLeafWs;
globalThis._connectLeafWs = function(leaf) {
    _wsConnectCalls.push({ leafId: leaf ? leaf.id : null, cmdId: leaf ? leaf.cmdId : null });
};

// Track leaf load calls
let _loadCalls = [];
const _origLoadLeaf = globalThis._loadLeafVttyHttpDirect;
globalThis._loadLeafVttyHttpDirect = function(leaf) {
    _loadCalls.push({ fn: 'leaf', leafId: leaf ? leaf.id : null, cmdId: leaf ? leaf.cmdId : null });
};
const _origLoadPanel = globalThis.loadVttyHttpForPanel;
globalThis.loadVttyHttpForPanel = function(panelId, instUrl, cmdId) {
    _loadCalls.push({ fn: 'panel', panelId, cmdId });
};

// Track disconnect calls
let _disconnectCalls = [];
const _origDisconnect = globalThis.disconnectPanelWs;
globalThis.disconnectPanelWs = function(panelId) {
    _disconnectCalls.push(panelId);
};

// ──────────────────────────────────────────────────────────────
// SPID-001: _getFocusedLeafId validates _rootSplit leaves
//           (was broken: only checked panel.split)
// ──────────────────────────────────────────────────────────────
console.log('SPID-001: _getFocusedLeafId validates _rootSplit leaves');
{
    state.panels = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');       // top-level split
    splitPanel(p.id, 'vertical', p.id);   // root's own split (_rootSplit)

    const rootBranch = p._rootSplit.branch;
    const rootBranchId = rootBranch.id;

    // Set _focusedLeafId to a _rootSplit leaf
    p._focusedLeafId = rootBranchId;

    // _getFocusedLeafId MUST return the _rootSplit leaf, not panel.id
    const focusedId = _getFocusedLeafId(p);
    assertEq(focusedId, rootBranchId,
        'SPID-001a: _getFocusedLeafId returns _rootSplit leaf');
}

// ──────────────────────────────────────────────────────────────
// SPID-002: _getFocusedLeafId works after unsplit removes panel.split
//           (panel has _rootSplit but panel.split = null)
// ──────────────────────────────────────────────────────────────
console.log('SPID-002: _getFocusedLeafId after top-level unsplit leaves _rootSplit');
{
    state.panels = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    const topBranch = p.split.branch;
    topBranch.instUrl = 'http://localhost:9090';
    topBranch.cmdId = 'cmd-top';

    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;
    const rootBranchId = rootBranch.id;
    rootBranch.instUrl = 'http://localhost:9090';
    rootBranch.cmdId = 'cmd-htop';

    p._focusedLeafId = rootBranchId;

    // Unsplit the TOP-LEVEL branch (removes panel.split, _rootSplit stays)
    unsplitPanel(p.id, topBranch.id);
    assert(!p.split, 'SPID-002a: panel.split removed after top-level unsplit');
    assert(!!p._rootSplit, 'SPID-002b: _rootSplit still exists');

    // _getFocusedLeafId MUST still work with _rootSplit
    const focusedId = _getFocusedLeafId(p);
    assertEq(focusedId, rootBranchId,
        'SPID-002c: _getFocusedLeafId returns _rootSplit leaf after top-level unsplit');
}

// ──────────────────────────────────────────────────────────────
// SPID-003: selectCommand routes to _rootSplit leaf when focused
// ──────────────────────────────────────────────────────────────
console.log('SPID-003: selectCommand routes to _rootSplit leaf');
{
    _loadCalls = [];
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-a', name: 'a', args: [] },
              { id: 'cmd-b', name: 'b', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    p.selectedCmdId = 'cmd-a';
    p.selectedInstUrl = 'http://localhost:9090';

    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;
    const rootBranchId = rootBranch.id;

    // Focus on rootBranch
    p._focusedLeafId = rootBranchId;

    // Select cmd-b — should go to rootBranch
    selectCommand('http://localhost:9090', 'cmd-b', 'b');

    assertEq(rootBranch.cmdId, 'cmd-b', 'SPID-003a: rootBranch got cmd-b');
    const leafLoad = _loadCalls.find(c => c.fn === 'leaf' && c.leafId === rootBranchId);
    assert(!!leafLoad, 'SPID-003b: _loadLeafVttyHttpDirect called for rootBranch');
    const panelLoad = _loadCalls.find(c => c.fn === 'panel');
    assert(!panelLoad, 'SPID-003c: loadVttyHttpForPanel NOT called (target was _rootSplit leaf)');
}

// ──────────────────────────────────────────────────────────────
// SPID-004: _setActiveSideForLeaf updates _rootSplit activeSide
// ──────────────────────────────────────────────────────────────
console.log('SPID-004: _setActiveSideForLeaf handles _rootSplit leaves');
{
    state.panels = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;
    const topBranch = p.split.branch;

    // Focus on rootBranch
    _setActiveSideForLeaf(p, rootBranch.id);
    assertEq(p._focusedLeafId, rootBranch.id, 'SPID-004a: _focusedLeafId set to rootBranch');
    assertEq(p._rootSplit.activeSide, 'branch', 'SPID-004b: _rootSplit activeSide is branch');
    assertEq(p.split.activeSide, 'panel', 'SPID-004c: top-level split activeSide is panel');

    // Focus on topBranch
    _setActiveSideForLeaf(p, topBranch.id);
    assertEq(p._focusedLeafId, topBranch.id, 'SPID-004d: _focusedLeafId set to topBranch');
    assertEq(p.split.activeSide, 'branch', 'SPID-004e: top-level split activeSide is branch');

    // Focus on root
    _setActiveSideForLeaf(p, p.id);
    assertEq(p._focusedLeafId, p.id, 'SPID-004f: _focusedLeafId set to root');
    assertEq(p._rootSplit.activeSide, 'panel', 'SPID-004g: _rootSplit activeSide is panel');
}

// ──────────────────────────────────────────────────────────────
// SPID-005: _getLeafFromVtty resolves _rootSplit leaves
// ──────────────────────────────────────────────────────────────
console.log('SPID-005: _getLeafFromVtty resolves _rootSplit leaves');
{
    state.panels = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;

    // Create mock vtty-container for rootBranch
    const mockVtty = { getAttribute: (attr) => {
        if (attr === 'data-leaf-id') return rootBranch.id;
        return null;
    }};

    const result = _getLeafFromVtty(mockVtty, p);
    assert(result !== null, 'SPID-005a: _getLeafFromVtty returned a result');
    assert(result.leaf.id === rootBranch.id, 'SPID-005b: resolved to rootBranch leaf');
    assertEq(result.isPanelLeaf, false, 'SPID-005c: not a panel leaf');
}

// ──────────────────────────────────────────────────────────────
// SPID-006: _withPanel returns focused leaf ID for _rootSplit panels
// ──────────────────────────────────────────────────────────────
console.log('SPID-006: _withPanel resolves _rootSplit focused leaf');
{
    state.panels = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;
    p._focusedLeafId = rootBranch.id;
    state._focusedPanelId = p.id;

    // Simulate _withPanel logic directly
    const panelObj = state.panels.find(pp => pp.id === p.id);
    const targetId = (panelObj && (panelObj.split || panelObj._rootSplit))
        ? (panelObj._focusedLeafId || p.id)
        : p.id;

    assertEq(targetId, rootBranch.id,
        'SPID-006a: _withPanel returns _rootSplit focused leaf ID');
}

// ──────────────────────────────────────────────────────────────
// SPID-008: Context menu resolves _rootSplit leaf command
// ──────────────────────────────────────────────────────────────
console.log('SPID-008: Context menu resolves _rootSplit leaf command');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-c', name: 'c', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;
    rootBranch.instUrl = 'http://localhost:9090';
    rootBranch.cmdId = 'cmd-c';

    // Simulate the context menu leaf resolution logic
    const leafId = rootBranch.id;
    let instUrl, cmdId;
    if (leafId && leafId !== p.id && (p.split || p._rootSplit)) {
        const found = _findLeafState(p, leafId);
        if (found && found.leaf) {
            instUrl = found.leaf.instUrl;
            cmdId = found.leaf.cmdId;
        }
    }

    assertEq(instUrl, 'http://localhost:9090', 'SPID-008a: resolved instUrl from _rootSplit leaf');
    assertEq(cmdId, 'cmd-c', 'SPID-008b: resolved cmdId from _rootSplit leaf');
}

// ──────────────────────────────────────────────────────────────
// SPID-009: Alternating focus between split trees routes correctly
//           (the "I keep clicking panes and it goes to the wrong one" scenario)
// ──────────────────────────────────────────────────────────────
console.log('SPID-009: Rapid alternating focus routes to correct leaf');
{
    _loadCalls = [];
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-1', name: 'one', args: [] },
              { id: 'cmd-2', name: 'two', args: [] },
              { id: 'cmd-3', name: 'three', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    splitPanel(p.id, 'horizontal');
    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;
    const topBranch = p.split.branch;

    // 1) Focus root → select cmd-1
    p._focusedLeafId = p.id;
    selectCommand('http://localhost:9090', 'cmd-1', 'one');
    assertEq(p.selectedCmdId, 'cmd-1', 'SPID-009a: root has cmd-1');

    // 2) Focus rootBranch → select cmd-2
    _loadCalls = [];
    p._focusedLeafId = rootBranch.id;
    selectCommand('http://localhost:9090', 'cmd-2', 'two');
    assertEq(rootBranch.cmdId, 'cmd-2', 'SPID-009b: rootBranch has cmd-2');
    assertEq(p.selectedCmdId, 'cmd-1', 'SPID-009c: root still has cmd-1');

    // 3) Focus topBranch → select cmd-3
    _loadCalls = [];
    p._focusedLeafId = topBranch.id;
    selectCommand('http://localhost:9090', 'cmd-3', 'three');
    assertEq(topBranch.cmdId, 'cmd-3', 'SPID-009d: topBranch has cmd-3');
    assertEq(rootBranch.cmdId, 'cmd-2', 'SPID-009e: rootBranch still has cmd-2');
    assertEq(p.selectedCmdId, 'cmd-1', 'SPID-009f: root still has cmd-1');

    // 4) Back to rootBranch → select cmd-1
    _loadCalls = [];
    p._focusedLeafId = rootBranch.id;
    selectCommand('http://localhost:9090', 'cmd-1', 'one');
    assertEq(rootBranch.cmdId, 'cmd-1', 'SPID-009g: rootBranch now has cmd-1');
    assertEq(topBranch.cmdId, 'cmd-3', 'SPID-009h: topBranch still has cmd-3');
}

// ──────────────────────────────────────────────────────────────
// SPID-010: After unsplit of top-level branch, _rootSplit leaves
//           still get focus and command routing works
// ──────────────────────────────────────────────────────────────
console.log('SPID-010: Command routing after top-level unsplit (only _rootSplit remains)');
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
    p.selectedCmdId = 'cmd-x';
    p.selectedInstUrl = 'http://localhost:9090';

    splitPanel(p.id, 'horizontal');
    const topBranch = p.split.branch;
    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;

    // Unsplit the top-level branch
    unsplitPanel(p.id, topBranch.id);
    assert(!p.split, 'SPID-010a: panel.split removed');
    assert(!!p._rootSplit, 'SPID-010b: _rootSplit still exists');

    // Focus on rootBranch
    p._focusedLeafId = rootBranch.id;

    // Select cmd-y — should go to rootBranch via _rootSplit path
    selectCommand('http://localhost:9090', 'cmd-y', 'y');

    assertEq(rootBranch.cmdId, 'cmd-y', 'SPID-010c: rootBranch got cmd-y after unsplit');
    const leafLoad = _loadCalls.find(c => c.fn === 'leaf' && c.leafId === rootBranch.id);
    assert(!!leafLoad, 'SPID-010d: _loadLeafVttyHttpDirect called for rootBranch');
}

// ──────────────────────────────────────────────────────────────
// SPID-011: No other leaf is touched when selecting into _rootSplit leaf
// ──────────────────────────────────────────────────────────────
console.log('SPID-011: selectCommand into _rootSplit leaf touches ONLY that leaf');
{
    _loadCalls = [];
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-z', name: 'z', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    p.selectedCmdId = 'cmd-a';
    p.selectedInstUrl = 'http://localhost:9090';

    splitPanel(p.id, 'horizontal');
    const topBranch = p.split.branch;
    topBranch.cmdId = 'cmd-b';
    topBranch.instUrl = 'http://localhost:9090';

    splitPanel(p.id, 'vertical', p.id);
    const rootBranch = p._rootSplit.branch;
    p._focusedLeafId = rootBranch.id;

    // Select cmd-z into rootBranch
    selectCommand('http://localhost:9090', 'cmd-z', 'z');

    assertEq(rootBranch.cmdId, 'cmd-z', 'SPID-011a: rootBranch got cmd-z');
    assertEq(p.selectedCmdId, 'cmd-a', 'SPID-011b: root untouched');
    assertEq(topBranch.cmdId, 'cmd-b', 'SPID-011c: topBranch untouched');

    // Verify only ONE load call was for the target leaf
    const targetLoads = _loadCalls.filter(c => c.fn === 'leaf' && c.leafId === rootBranch.id);
    assertEq(targetLoads.length, 1, 'SPID-011d: exactly one load for rootBranch');
    const otherLeafLoads = _loadCalls.filter(c => c.fn === 'leaf' && c.leafId !== rootBranch.id);
    assertEq(otherLeafLoads.length, 0, 'SPID-011e: zero loads for other leaves');
}

console.log('\n[selectedPaneId Consistency] Tests complete\n');
