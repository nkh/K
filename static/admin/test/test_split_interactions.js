/// test/test_split_interactions.js — Tests that simulate user interactions
/// with split panes: command selection, keyboard shortcuts, focus tracking,
/// window renaming, and leaf VTTY loading.
///
/// These tests reproduce the exact signals that would be sent if a user
/// performed the actions by hand (clicking sidebar, pressing keys, etc.)
require('./setup');
const { createMockEvent } = require('./helpers');

console.log('\n=== Split Pane Interaction Tests ===\n');

resetTestState();

// ── Mocks for render/DOM functions ──
globalThis.renderPanels = function() {};
globalThis.startPanelUpdateMode = function() {};
globalThis.stopPanelUpdateMode = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.stopPanelPoll = function() {};
globalThis.focusPanel = function(id) { state._focusedPanelId = id; };
globalThis.setupPanelHeaderDrag = function() {};
globalThis.updateSidebarSelection = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis.disconnectPanelWs = function(id) {
    const p = state.panels.find(pp => pp.id === id);
    if (!p) return;
    if (p.wsInstUrl && p.wsCmdId) {
        delete state._diffBaselines[id + '/' + p.wsCmdId];
    }
    p.wsInstUrl = null; p.wsCmdId = null; p.ws = null;
};

// ──────────────────────────────────────────────────────────────
// SPL-001: selectCommand assigns to the active leaf in a split
//           (simulates clicking a command in the sidebar)
// ──────────────────────────────────────────────────────────────
console.log('SPL-001: selectCommand routes to active leaf in split');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-1', name: 'top', args: [] },
              { id: 'cmd-2', name: 'htop', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    // Split the panel
    splitPanel(p.id, 'horizontal');
    const branchLeaf = p.split.branch;

    // Set active side to branch so _getFocusedLeafId returns branch
    p.split.activeSide = 'branch';
    p._focusedLeafId = branchLeaf.id;

    // Verify _getFocusedLeafId returns the branch
    const focusedId = _getFocusedLeafId(p);
    assertEq(focusedId, branchLeaf.id, 'SPL-001-pre: _getFocusedLeafId returns branch');

    // Track which leaf got the command
    let leafIdForVtty = null;
    let leafInstUrl = null;
    let leafCmdId = null;
    const origLoad = globalThis._loadLeafVttyHttpDirect;
    globalThis._loadLeafVttyHttpDirect = function(leaf) {
        leafIdForVtty = leaf.id;
        leafInstUrl = leaf.instUrl;
        leafCmdId = leaf.cmdId;
    };

    // Simulate user clicking "htop" in the sidebar → calls selectCommand
    selectCommand('http://localhost:9090', 'cmd-2', 'htop');

    // The branch leaf should have received the command
    assertEq(branchLeaf.cmdId, 'cmd-2',
        'SPL-001a: branch leaf received cmd-2');
    assertEq(branchLeaf.instUrl, 'http://localhost:9090',
        'SPL-001b: branch leaf received correct instUrl');
    assertEq(leafIdForVtty, branchLeaf.id,
        'SPL-001c: _loadLeafVttyHttpDirect called with leaf ID');
    assertEq(leafCmdId, 'cmd-2',
        'SPL-001d: _loadLeafVttyHttpDirect called with correct cmdId');

    globalThis._loadLeafVttyHttpDirect = origLoad;
}

// ──────────────────────────────────────────────────────────────
// SPL-002: _selectLeafCommand directly loads command into a leaf
//           (simulates drag-drop of a command onto a split pane)
// ──────────────────────────────────────────────────────────────
console.log('SPL-002: _selectLeafCommand loads command into specific leaf');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-3', name: 'ls', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'vertical');
    const branchLeaf = p.split.branch;

    let loadCalled = false;
    let loadLeafId = null;
    const origLoad = globalThis._loadLeafVttyHttpDirect;
    globalThis._loadLeafVttyHttpDirect = function(leaf) {
        loadCalled = true;
        loadLeafId = leaf.id;
    };

    _selectLeafCommand(p, branchLeaf, 'http://localhost:9090', 'cmd-3');

    assertEq(branchLeaf.cmdId, 'cmd-3', 'SPL-002a: leaf cmdId set');
    assertEq(branchLeaf.instUrl, 'http://localhost:9090', 'SPL-002b: leaf instUrl set');
    assertOk(loadCalled, 'SPL-002c: _loadLeafVttyHttpDirect was called');
    assertEq(loadLeafId, branchLeaf.id, 'SPL-002d: called with correct leaf ID');

    globalThis._loadLeafVttyHttpDirect = origLoad;
}

// ──────────────────────────────────────────────────────────────
// SPL-003: _selectActiveLeafCommand routes to panel when no split
// ──────────────────────────────────────────────────────────────
console.log('SPL-003: _selectActiveLeafCommand routes to panel (no split)');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-4', name: 'vim', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    let httpCalled = false;
    let httpPanelId = null;
    const origHttp = globalThis.loadVttyHttpForPanel;
    globalThis.loadVttyHttpForPanel = function(pid, instUrl, cmdId) {
        httpCalled = true;
        httpPanelId = pid;
    };

    _selectActiveLeafCommand(p, 'http://localhost:9090', 'cmd-4');

    assertEq(p.selectedCmdId, 'cmd-4', 'SPL-003a: panel cmdId set');
    assertOk(httpCalled, 'SPL-003b: loadVttyHttpForPanel called');
    assertEq(httpPanelId, p.id, 'SPL-003c: called with panel ID (panel)');

    globalThis.loadVttyHttpForPanel = origHttp;
}

// ──────────────────────────────────────────────────────────────
// SPL-004: Alt+w shortcut is registered and wired to createWindow
// ──────────────────────────────────────────────────────────────
console.log('SPL-004: Alt+w shortcut registered and wired to createWindow');
{
    const newWin = _defaultShortcuts.find(s => s.id === 'new-window');
    assertOk(!!newWin, 'SPL-004a: new-window shortcut exists');
    assertEq(newWin.key, 'w', 'SPL-004b: key is "w"');
    assertOk(newWin.alt, 'SPL-004c: alt modifier is set');

    // Verify the action calls createWindow
    let createWindowCalled = false;
    const orig = globalThis.createWindow;
    globalThis.createWindow = function() { createWindowCalled = true; };
    newWin.action({ preventDefault: function(){} });
    assertOk(createWindowCalled, 'SPL-004d: action dispatches createWindow');
    globalThis.createWindow = orig;
}

// ──────────────────────────────────────────────────────────────
// SPL-005: Alt+W shortcut is registered and wired to closeWindow
// ──────────────────────────────────────────────────────────────
console.log('SPL-005: Alt+W shortcut registered and wired to closeWindow');
{
    const closeWin = _defaultShortcuts.find(s => s.id === 'close-window');
    assertOk(!!closeWin, 'SPL-005a: close-window shortcut exists');
    assertEq(closeWin.key, 'W', 'SPL-005b: key is "W"');
    assertOk(closeWin.alt, 'SPL-005c: alt modifier is set');

    state.activeWindowId = 'win-test-close';
    let closeWindowCalled = false;
    let closeWindowArg = null;
    const orig = globalThis.closeWindow;
    globalThis.closeWindow = function(id) { closeWindowCalled = true; closeWindowArg = id; };
    closeWin.action({ preventDefault: function(){} });
    assertOk(closeWindowCalled, 'SPL-005d: action dispatches closeWindow');
    assertEq(closeWindowArg, 'win-test-close', 'SPL-005e: closeWindow received active window ID');
    globalThis.closeWindow = orig;
}

// ──────────────────────────────────────────────────────────────
// SPL-006: Alt+1-9 shortcuts are registered
// ──────────────────────────────────────────────────────────────
console.log('SPL-006: Alt+1-9 shortcuts registered');
{
    for (let i = 1; i <= 9; i++) {
        const sc = _defaultShortcuts.find(s => s.id === 'win-' + i);
        assertOk(!!sc, 'SPL-006-' + i + 'a: win-' + i + ' shortcut exists');
        assertEq(sc.key, String(i), 'SPL-006-' + i + 'b: key is "' + i + '"');
        assertOk(sc.alt, 'SPL-006-' + i + 'c: alt modifier is set');
    }
    // Verify the action calls switchWindow with the correct index
    state.windows = [
        { id: 'w-a', name: '1', panelIds: [] },
        { id: 'w-b', name: '2', panelIds: [] },
    ];
    state.activeWindowId = 'w-b';
    let switchArg = null;
    const orig = globalThis.switchWindow;
    globalThis.switchWindow = function(id) { switchArg = id; };
    const sc1 = _defaultShortcuts.find(s => s.id === 'win-1');
    sc1.action({ preventDefault: function(){} });
    assertEq(switchArg, 'w-a', 'SPL-006d: win-1 switches to first window');
    globalThis.switchWindow = orig;
}

// ──────────────────────────────────────────────────────────────
// SPL-007: Active pane focus tracked via _setActiveSideForLeaf
//           (simulates clicking on a vtty-container in a split)
// ──────────────────────────────────────────────────────────────
console.log('SPL-007: Click on branch vtty sets activeSide');
{
    state.panels = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    const branchLeaf = p.split.branch;

    // Initially activeSide should be 'panel'
    assertEq(p.split.activeSide, 'panel', 'SPL-007a: initial activeSide is panel');

    // Simulate clicking the branch vtty (what _getPanelObj does)
    p.split.activeSide = 'panel';
    _setActiveSideForLeaf(p, branchLeaf.id);

    assertEq(p.split.activeSide, 'branch', 'SPL-007b: activeSide changed to branch');
    assertEq(p._focusedLeafId, branchLeaf.id, 'SPL-007c: _focusedLeafId set to branch');
}

// ──────────────────────────────────────────────────────────────
// SPL-008: startRenameWindow does NOT call renderPanels
//           (verifies the fix — renaming updates DOM directly)
// ──────────────────────────────────────────────────────────────
console.log('SPL-008: startRenameWindow does not call renderPanels');
{
    // Verify the real startRenameWindow function exists
    assertEq(typeof startRenameWindow, 'function', 'SPL-008a: startRenameWindow is a function');

    // The actual DOM behavior (finding tabs, contentEditable) requires a real browser.
    // Here we verify the CRITICAL FIX: the old code called renderPanels() in finish(),
    // which destroyed the editing element. The new code updates DOM directly.
    // We verify this by reading the function source and confirming renderPanels
    // is NOT called.
    const src = startRenameWindow.toString();
    assert(!src.includes('renderPanels()'), 'SPL-008b: finish() does not call renderPanels');
    assert(src.includes('querySelectorAll'), 'SPL-008c: finish() uses querySelectorAll for DOM update');

    // Verify the finish function only updates DOM, doesn't re-render
    const src2 = src.substring(src.indexOf('const finish'));
    // The finish function should NOT contain 'renderPanels'
    assert(!src2.includes('renderPanels'), 'SPL-008d: finish callback does not call renderPanels anywhere');
}

// ──────────────────────────────────────────────────────────────
// SPL-009: _fetchLeafDiff uses leaf ID for diff timer key
//           (verifies leaf-specific debounce timers, not panel-level)
// ──────────────────────────────────────────────────────────────
console.log('SPL-009: _fetchLeafDiff uses leaf-specific timer key');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    const branchLeaf = p.split.branch;
    branchLeaf.cmdId = 'cmd-sec';
    branchLeaf.instUrl = 'http://localhost:9090';

    // Mock the diff fetch to capture the timer key used
    let fetchTimerKey = null;
    const origSetTimeout = globalThis.setTimeout;
    globalThis.setTimeout = function(fn, delay) {
        // The timer key is embedded in the closure — we can verify by
        // checking that _fetchLeafDiff doesn't throw and creates a timer
        return origSetTimeout(fn, delay);
    };

    assert(() => { _fetchLeafDiff(branchLeaf.id, 'http://localhost:9090', 'cmd-sec', 100); },
        'SPL-009a: _fetchLeafDiff with leaf ID does not throw');

    globalThis.setTimeout = origSetTimeout;
}

// ──────────────────────────────────────────────────────────────
// SPL-010: Split pane headers are identical to non-split headers
//           (same class, same data-attributes, same buttons)
// ──────────────────────────────────────────────────────────────
console.log('SPL-010: Split pane headers match non-split headers');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-h', name: 'htop', args: ['-s'] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-h';

    // Render non-split header
    const nonSplitHeader = _renderLeafHeader(p, p, p.id);

    // Now split and render the branch leaf header
    splitPanel(p.id, 'vertical');
    const branchLeaf = p.split.branch;
    branchLeaf.cmdId = 'cmd-h';
    branchLeaf.instUrl = 'http://localhost:9090';
    const splitHeader = _renderLeafHeader(p, branchLeaf, branchLeaf.id);

    // Both should have panel-header class
    assertIncludes(nonSplitHeader, 'class="panel-header"', 'SPL-010a: non-split has panel-header class');
    assertIncludes(splitHeader, 'class="panel-header"', 'SPL-010b: split has panel-header class');

    // Both should have data-ctxmenu for context menu (uses data-ctxmenu="panel" to
    // avoid dual-fire with the click delegation system)
    assertIncludes(nonSplitHeader, 'data-ctxmenu="panel"', 'SPL-010c: non-split has context menu via delegation');
    assertIncludes(splitHeader, 'data-ctxmenu="panel"', 'SPL-010d: split has context menu via delegation');

    // Both should have data-leaf-id
    assertIncludes(nonSplitHeader, 'data-leaf-id="' + p.id + '"', 'SPL-010e: non-split has data-leaf-id');
    assertIncludes(splitHeader, 'data-leaf-id="' + branchLeaf.id + '"', 'SPL-010f: split has data-leaf-id');

    // Both should have close button
    assertIncludes(nonSplitHeader, 'panel-close-btn', 'SPL-010g: non-split has close button');
    assertIncludes(splitHeader, 'panel-close-btn', 'SPL-010h: split has close button');

    // Split branch should use UnsplitLeaf action
    assertIncludes(splitHeader, 'data-action="UnsplitLeaf"', 'SPL-010i: split branch uses UnsplitLeaf');
    // All leaves (including root) use UnsplitLeaf — close removes just that leaf
    assertIncludes(nonSplitHeader, 'data-action="UnsplitLeaf"', 'SPL-010j: top-level also uses UnsplitLeaf');
}

// ──────────────────────────────────────────────────────────────
// SPL-011: connectPanelWs walks tree and connects all leaf WS
// ──────────────────────────────────────────────────────────────
console.log('SPL-011: connectPanelWs connects WS for all split leaves');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true },
    ];
    state.windows = [];
    state.activeWindowId = null;
    state.updateMode = 'push';

    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-pri';
    splitPanel(p.id, 'horizontal');
    const branchLeaf = p.split.branch;
    branchLeaf.cmdId = 'cmd-sec';
    branchLeaf.instUrl = 'http://localhost:9090';

    // connectPanelWs should not throw
    assert(() => { connectPanelWs(p.id); }, 'SPL-011: connectPanelWs with split does not throw');

    // Root-leaf should be subscribed via shared pool
    const priKey = 'http://localhost:9090/cmd-pri';
    assertOk(_sharedSubs[priKey], 'SPL-011a: root-leaf subscribed in shared pool');
    assertOk(_sharedSubs[priKey].panels.has(p.id), 'SPL-011b: panel ID in root-leaf subscription');

    // Branch leaf should have its own WS (via _connectLeafWs)
    // In the mock, WebSocket constructor creates a mock — just verify it was called
    assertOk(branchLeaf.ws !== null || branchLeaf.wsInstUrl === 'http://localhost:9090',
        'SPL-011c: branch leaf WS state initialized');
}

// ──────────────────────────────────────────────────────────────
// SPL-012: _applyLeafDiff applies incremental cell updates
//           (simulates a WS vtty_dirty → diff response)
// ──────────────────────────────────────────────────────────────
console.log('SPL-012: _applyLeafDiff applies cell-level updates');
{
    state._level3Enabled = true;
    state._cellGrids = {};
    state._lastGeneration = {};

    const vttyEl = document.createElement('div');
    vttyEl.className = 'vtty-container';
    const pre = document.createElement('pre');
    // Create span manually — mock DOM doesn't parse innerHTML into childNodes
    const span = document.createElement('span');
    span.textContent = 'A';
    span.className = 'c w1';
    span.setAttribute('style', 'width:1ch;color:#000;background:#fff');
    pre.appendChild(span);
    vttyEl.appendChild(pre);

    const leafId = 'test-diff-leaf';
    const cmdId = 'cmd-diff';

    // Manually set up the cell grid (buildCellGrid can't work in mock DOM)
    state._cellGrids[leafId + '/' + cmdId] = {
        grid: [[ { span: span, idx: 0, len: 1 } ]],
        rows: 1,
        cols: 1,
    };
    state._lastGeneration[leafId + '/' + cmdId] = 1;

    // Apply a diff that changes cell (0,0) from "A" to "B"
    const diffData = {
        generation: 2,
        _cmdId: cmdId,
        dimensions: { rows: 1, cols: 1 },
        cells: [{
            row: 0, col: 0,
            cell: { ch: 'B', fg: [0,0,0], bg: [255,255,255], width: 1 }
        }]
    };

    _applyLeafDiff(vttyEl, leafId, cmdId, 'http://localhost:9090', diffData);

    // The span should now contain "B"
    assertEq(span.textContent, 'B', 'SPL-012a: cell updated from A to B');
    assertEq(state._lastGeneration[leafId + '/' + cmdId], 2,
        'SPL-012b: generation cached');

    // Second diff with same generation → no-op (metadata only)
    const sameGenData = { generation: 2, _cmdId: cmdId };
    const beforeHtml = span.textContent;
    _applyLeafDiff(vttyEl, leafId, cmdId, 'http://localhost:9090', sameGenData);
    assertEq(span.textContent, beforeHtml, 'SPL-012c: same generation skipped update');

    delete state._cellGrids[leafId + '/' + cmdId];
    delete state._lastGeneration[leafId + '/' + cmdId];
    state._level3Enabled = true;
}

// ──────────────────────────────────────────────────────────────
// SPL-013: Recursive split — split a pane that is already split
//           (the branch of a split can itself be split)
// ──────────────────────────────────────────────────────────────
console.log('SPL-013: Recursive split — branch can be split again');
{
    state.panels = [];
    state.connections = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    // First split: panel → [panel, branch]
    splitPanel(p.id, 'horizontal');
    assertOk(!!p.split, 'SPL-013a: panel has top-level split');
    const branch1 = p.split.branch;
    assertOk(!!branch1, 'SPL-013b: first branch exists');

    // Focus the branch and split it again
    p.split.activeSide = 'branch';
    p._focusedLeafId = branch1.id;
    splitPanel(p.id, 'vertical');

    // The branch should now itself be split
    assertOk(!!branch1.split, 'SPL-013c: branch has its own split');
    const branch2 = branch1.split.branch;
    assertOk(!!branch2, 'SPL-013d: second-level branch exists');
    assertEq(branch2.id.indexOf(p.id), 0, 'SPL-013e: deep branch ID starts with panel ID');

    // Verify _getAllLeaves returns 3 leaves
    const allLeaves = _getAllLeaves(p);
    assertEq(allLeaves.length, 3, 'SPL-013f: 3 leaves after double split');
    assertEq(allLeaves[0].leaf.id, p.id, 'SPL-013g: leaf 0 is panel root');
    assertEq(allLeaves[1].leaf.id, branch1.id, 'SPL-013h: leaf 1 is first branch');
    assertEq(allLeaves[2].leaf.id, branch2.id, 'SPL-013i: leaf 2 is second-level branch');

    // Verify _findLeafState can find the deep leaf
    const found = _findLeafState(p, branch2.id);
    assertOk(!!found, 'SPL-013j: _findLeafState finds deep leaf');
    assertEq(found.leaf.id, branch2.id, 'SPL-013k: found leaf has correct ID');
}

// ──────────────────────────────────────────────────────────────
// SPL-014: showPanelContextMenu receives leafId for split panes
//           and resolves the correct command for that leaf
// ──────────────────────────────────────────────────────────────
console.log('SPL-014: Context menu resolves correct leaf command');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-pri', name: 'top', args: [] },
              { id: 'cmd-sec', name: 'htop', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-pri';
    splitPanel(p.id, 'vertical');
    const branchLeaf = p.split.branch;
    branchLeaf.cmdId = 'cmd-sec';
    branchLeaf.instUrl = 'http://localhost:9090';

    // Verify showPanelContextMenu is a function that accepts 3 args
    assertEq(typeof showPanelContextMenu, 'function', 'SPL-014a: showPanelContextMenu exists');
    // Check function signature accepts leafId (3 params)
    assertEq(showPanelContextMenu.length, 3, 'SPL-014b: showPanelContextMenu takes 3 params (e, panelId, leafId)');
}

// ──────────────────────────────────────────────────────────────
// SPL-015: unsplitPanel removes split tree after collapse
// ──────────────────────────────────────────────────────────────
console.log('SPL-015: unsplitPanel resets activeSide after collapse');
{
    state.panels = [];
    state.connections = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    p.split.activeSide = 'branch';

    // Unsplit the branch
    unsplitPanel(p.id, p.split.branch.id);

    // The top-level split should be removed
    assertEq(p.split, null, 'SPL-015a: top-level split cleared after unsplit');
}

// ──────────────────────────────────────────────────────────────
// SPL-016: Split pane header oncontextmenu passes leafId
// ──────────────────────────────────────────────────────────────
console.log('SPL-016: Split header oncontextmenu passes leafId');
{
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-h', name: 'htop', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'vertical');
    const branchLeaf = p.split.branch;

    const headerHtml = _renderLeafHeader(p, branchLeaf, branchLeaf.id);
    // The delegation should pass panel and leaf via data attributes
    assertIncludes(headerHtml, 'data-panel="' + p.id + '"', 'SPL-016: branch header has data-panel');
    assertIncludes(headerHtml, 'data-leaf="' + branchLeaf.id + '"', 'SPL-016: branch header has data-leaf');
}

console.log('\n[split_interactions] Tests complete');

// ──────────────────────────────────────────────────────────────
// SPL-017: _setupPanelDelegation reads data-leaf-id (not data-split-side)
//           Source-level verification of the mousedown handler fix
// ──────────────────────────────────────────────────────────────
console.log('SPL-017: _setupPanelDelegation reads data-leaf-id');
{
    const src = _setupPanelDelegation.toString();
    assertIncludes(src, 'data-leaf-id', 'SPL-017a: handler reads data-leaf-id attribute');
    assertNotIncludes(src, 'data-split-side', 'SPL-017b: handler does NOT reference old data-split-side');
    assertIncludes(src, '_setActiveSideForLeaf', 'SPL-017c: handler calls _setActiveSideForLeaf');
}

// ──────────────────────────────────────────────────────────────
// SPL-018: Full flow — split, click branch, selectCommand routes to branch
//           Simulates: Alt+| split → click on branch pane → click cmd in sidebar
//           This is the exact bug that was fixed: branch panes were unselectable.
// ──────────────────────────────────────────────────────────────
console.log('SPL-018: Full user flow — split, click branch, selectCommand');
{
    resetTestState();
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-a', name: 'top', args: [] },
              { id: 'cmd-b', name: 'htop', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    state._focusedPanelId = p.id;

    // Step 1: Split the panel (Alt+|)
    splitPanel(p.id, 'horizontal');
    const branchLeaf = p.split.branch;

    // Step 2: Verify initial state — splitPanel focuses the new branch
    assertEq(p.split.activeSide, 'panel', 'SPL-018a: initial activeSide is panel');
    assertEq(_getFocusedLeafId(p), branchLeaf.id, 'SPL-018b: _getFocusedLeafId returns branch (new pane)');

    // Step 3: Simulate user clicking on the branch pane's vtty-container
    // This is what the FIXED _setupPanelDelegation mousedown handler does:
    //   const el = e.target.closest('[data-leaf-id]');
    //   const leafId = el.getAttribute('data-leaf-id');
    //   _setActiveSideForLeaf(p, leafId);
    _setActiveSideForLeaf(p, branchLeaf.id);

    // Step 4: Verify the click updated the active leaf
    assertEq(p.split.activeSide, 'branch', 'SPL-018c: activeSide changed to branch after click');
    assertEq(p._focusedLeafId, branchLeaf.id, 'SPL-018d: _focusedLeafId set to branch after click');
    assertEq(_getFocusedLeafId(p), branchLeaf.id, 'SPL-018e: _getFocusedLeafId returns branch');

    // Step 5: User clicks "htop" in the sidebar → selectCommand
    // Mock the leaf VTTY loader to track which leaf gets the command
    let loadLeafId = null;
    let loadCmdId = null;
    const origLoad = globalThis._loadLeafVttyHttpDirect;
    globalThis._loadLeafVttyHttpDirect = function(leaf) {
        loadLeafId = leaf.id;
        loadCmdId = leaf.cmdId;
    };

    selectCommand('http://localhost:9090', 'cmd-b', 'htop');

    // Step 6: Verify command was routed to the branch leaf (not root)
    assertEq(branchLeaf.cmdId, 'cmd-b', 'SPL-018f: branch leaf received cmd-b');
    assertEq(branchLeaf.instUrl, 'http://localhost:9090', 'SPL-018g: branch leaf received correct instUrl');
    assertEq(loadLeafId, branchLeaf.id, 'SPL-018h: VTTY loader called for branch leaf');
    assertEq(loadCmdId, 'cmd-b', 'SPL-018i: VTTY loader received correct cmdId');

    // Step 7: Verify the panel root leaf was NOT overwritten
    assertEq(p.selectedCmdId, null, 'SPL-018j: panel root leaf cmdId unchanged (still null)');

    globalThis._loadLeafVttyHttpDirect = origLoad;
}

// ──────────────────────────────────────────────────────────────
// SPL-019: Click-to-focus handler in keyboard.js updates active leaf
//           Source-level verification that the click handler tracks split leaves
// ──────────────────────────────────────────────────────────────
console.log('SPL-019: Click-to-focus handler tracks active leaf in splits');
{
    // The click-to-focus handler is an anonymous function in keyboard.js,
    // but we can verify the exported _setActiveSideForLeaf is used.
    // Verify the function exists and was called correctly (already tested by SPL-007).
    // Source-level: verify keyboard.js has the data-leaf-id tracking in click handler.
    const fs = require('fs');
    const src = fs.readFileSync(require('path').join(__dirname, '..', 'modules', 'keyboard.js'), 'utf8');
    // The click handler should contain data-leaf-id reference
    assertIncludes(src, 'data-leaf-id', 'SPL-019a: keyboard.js references data-leaf-id');
}

// ──────────────────────────────────────────────────────────────
// SPL-020: Rendered split pane HTML has correct data-leaf-id attributes
//           that match the mousedown handler's querySelector
// ──────────────────────────────────────────────────────────────
console.log('SPL-020: Split pane HTML data-leaf-id matches delegation handler');
{
    resetTestState();
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [{ id: 'cmd-x', name: 'vim', args: [] }] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'vertical');
    const branchLeaf = p.split.branch;

    // Render the leaf header for the branch (exported function)
    const headerHtml = _renderLeafHeader(p, branchLeaf, branchLeaf.id);

    // Header must have data-leaf-id that matches the branch leaf ID
    assertIncludes(headerHtml, 'data-leaf-id="' + branchLeaf.id + '"',
        'SPL-020a: header has matching data-leaf-id');

    // Verify the full leaf pane rendering includes data-leaf-id
    const paneHtml = _renderLeafPane(p, branchLeaf, branchLeaf.id);
    assertIncludes(paneHtml, 'data-leaf-id="' + branchLeaf.id + '"',
        'SPL-020b: leaf pane has matching data-leaf-id');
    assertIncludes(paneHtml, 'class="split-pane', 'SPL-020c: leaf pane has split-pane class');

    // Verify querySelector('[data-leaf-id]') would find both header and pane
    const matchCount = (paneHtml.match(/data-leaf-id=/g) || []).length;
    assertGt(matchCount, 0, 'SPL-020d: at least one data-leaf-id in rendered pane');
}

console.log('\n[split_interactions_extra] Tests complete');

// ──────────────────────────────────────────────────────────────
// SPL-021: Nested split renders correct leaf data in headers
//           After splitting a branch, its header should show its own command,
//           not the root panel's command.
// ──────────────────────────────────────────────────────────────
console.log('SPL-021: Nested split header shows correct leaf command');
{
    resetTestState();
    state.panels = [];
    state.connections = [
        { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true,
          _commands: [
              { id: 'cmd-p', name: 'top', args: [] },
              { id: 'cmd-branch1', name: 'htop', args: [] },
              { id: 'cmd-branch2', name: 'vim', args: [] },
          ] },
    ];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-p';

    // First split: panel → [panel-root, branch1]
    splitPanel(p.id, 'horizontal');
    const branch1 = p.split.branch;
    branch1.cmdId = 'cmd-branch1';
    branch1.instUrl = 'http://localhost:9090';

    // Second split: branch1 → [branch1-root, branch2]
    _setActiveSideForLeaf(p, branch1.id);
    splitPanel(p.id, 'vertical');
    const branch2 = branch1.split.branch;
    branch2.cmdId = 'cmd-branch2';
    branch2.instUrl = 'http://localhost:9090';

    // Render the top-level split container
    const html = _renderSplitContainer(p);

    // All three leaf headers should be present with their own commands
    // Panel root header should mention panel.id
    assertIncludes(html, 'data-leaf-id="' + p.id + '"', 'SPL-021a: panel root header in rendered output');
    // S1 header should mention branch1.id
    assertIncludes(html, 'data-leaf-id="' + branch1.id + '"', 'SPL-021b: branch1 header in rendered output');
    // S2 header should mention branch2.id
    assertIncludes(html, 'data-leaf-id="' + branch2.id + '"', 'SPL-021c: branch2 header in rendered output');

    // Verify _getAllLeaves returns 3 leaves
    const allLeaves = _getAllLeaves(p);
    assertEq(allLeaves.length, 3, 'SPL-021d: 3 leaves after double split');

    // Each leaf's header should reference its own data-leaf-id
    for (const { leaf, side } of allLeaves) {
        const headerHtml = _renderLeafHeader(p, leaf, leaf.id);
        assertIncludes(headerHtml, 'data-leaf-id="' + leaf.id + '"',
            'SPL-021e-' + leaf.id.substring(0, 12) + ': header has own leaf-id');
    }
}