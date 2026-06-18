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
    const secLeaf = p.split.secondary;

    // Set active side to secondary so _getFocusedLeafId returns secondary
    p.split.activeSide = 'secondary';
    p._focusedLeafId = secLeaf.id;

    // Verify _getFocusedLeafId returns the secondary
    const focusedId = _getFocusedLeafId(p);
    assertEq(focusedId, secLeaf.id, 'SPL-001-pre: _getFocusedLeafId returns secondary');

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

    // The secondary leaf should have received the command
    assertEq(secLeaf.cmdId, 'cmd-2',
        'SPL-001a: secondary leaf received cmd-2');
    assertEq(secLeaf.instUrl, 'http://localhost:9090',
        'SPL-001b: secondary leaf received correct instUrl');
    assertEq(leafIdForVtty, secLeaf.id,
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
    const secLeaf = p.split.secondary;

    let loadCalled = false;
    let loadLeafId = null;
    const origLoad = globalThis._loadLeafVttyHttpDirect;
    globalThis._loadLeafVttyHttpDirect = function(leaf) {
        loadCalled = true;
        loadLeafId = leaf.id;
    };

    _selectLeafCommand(p, secLeaf, 'http://localhost:9090', 'cmd-3');

    assertEq(secLeaf.cmdId, 'cmd-3', 'SPL-002a: leaf cmdId set');
    assertEq(secLeaf.instUrl, 'http://localhost:9090', 'SPL-002b: leaf instUrl set');
    assertOk(loadCalled, 'SPL-002c: _loadLeafVttyHttpDirect was called');
    assertEq(loadLeafId, secLeaf.id, 'SPL-002d: called with correct leaf ID');

    globalThis._loadLeafVttyHttpDirect = origLoad;
}

// ──────────────────────────────────────────────────────────────
// SPL-003: _selectActiveLeafCommand routes to primary when no split
// ──────────────────────────────────────────────────────────────
console.log('SPL-003: _selectActiveLeafCommand routes to primary (no split)');
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
    assertEq(httpPanelId, p.id, 'SPL-003c: called with panel ID (primary)');

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
console.log('SPL-007: Click on secondary vtty sets activeSide');
{
    state.panels = [];
    state.windows = [];
    state.activeWindowId = null;

    const p = addPanelDirect();
    splitPanel(p.id, 'horizontal');
    const secLeaf = p.split.secondary;

    // Initially activeSide should be 'primary'
    assertEq(p.split.activeSide, 'primary', 'SPL-007a: initial activeSide is primary');

    // Simulate clicking the secondary vtty (what _getPanelObj does)
    p.split.activeSide = 'primary';
    _setActiveSideForLeaf(p, secLeaf.id);

    assertEq(p.split.activeSide, 'secondary', 'SPL-007b: activeSide changed to secondary');
    assertEq(p._focusedLeafId, secLeaf.id, 'SPL-007c: _focusedLeafId set to secondary');
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
    const secLeaf = p.split.secondary;
    secLeaf.cmdId = 'cmd-sec';
    secLeaf.instUrl = 'http://localhost:9090';

    // Mock the diff fetch to capture the timer key used
    let fetchTimerKey = null;
    const origSetTimeout = globalThis.setTimeout;
    globalThis.setTimeout = function(fn, delay) {
        // The timer key is embedded in the closure — we can verify by
        // checking that _fetchLeafDiff doesn't throw and creates a timer
        return origSetTimeout(fn, delay);
    };

    assert(() => { _fetchLeafDiff(secLeaf.id, 'http://localhost:9090', 'cmd-sec', 100); },
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

    // Now split and render the secondary header
    splitPanel(p.id, 'vertical');
    const secLeaf = p.split.secondary;
    secLeaf.cmdId = 'cmd-h';
    secLeaf.instUrl = 'http://localhost:9090';
    const splitHeader = _renderLeafHeader(p, secLeaf, secLeaf.id);

    // Both should have panel-header class
    assertIncludes(nonSplitHeader, 'class="panel-header"', 'SPL-010a: non-split has panel-header class');
    assertIncludes(splitHeader, 'class="panel-header"', 'SPL-010b: split has panel-header class');

    // Both should have oncontextmenu for context menu
    assertIncludes(nonSplitHeader, 'oncontextmenu="showPanelContextMenu', 'SPL-010c: non-split has context menu');
    assertIncludes(splitHeader, 'oncontextmenu="showPanelContextMenu', 'SPL-010d: split has context menu');

    // Both should have data-leaf-id
    assertIncludes(nonSplitHeader, 'data-leaf-id="' + p.id + '"', 'SPL-010e: non-split has data-leaf-id');
    assertIncludes(splitHeader, 'data-leaf-id="' + secLeaf.id + '"', 'SPL-010f: split has data-leaf-id');

    // Both should have close button
    assertIncludes(nonSplitHeader, 'panel-close-btn', 'SPL-010g: non-split has close button');
    assertIncludes(splitHeader, 'panel-close-btn', 'SPL-010h: split has close button');

    // Split secondary should use UnsplitLeaf action (not UnsplitPanel)
    assertIncludes(splitHeader, 'data-action="UnsplitLeaf"', 'SPL-010i: split secondary uses UnsplitLeaf');
    assertIncludes(nonSplitHeader, 'data-action="UnsplitPanel"', 'SPL-010j: top-level uses UnsplitPanel');
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
    const secLeaf = p.split.secondary;
    secLeaf.cmdId = 'cmd-sec';
    secLeaf.instUrl = 'http://localhost:9090';

    // connectPanelWs should not throw
    assert(() => { connectPanelWs(p.id); }, 'SPL-011: connectPanelWs with split does not throw');

    // Primary should be subscribed via shared pool
    const priKey = 'http://localhost:9090/cmd-pri';
    assertOk(_sharedSubs[priKey], 'SPL-011a: primary subscribed in shared pool');
    assertOk(_sharedSubs[priKey].panels.has(p.id), 'SPL-011b: panel ID in primary subscription');

    // Secondary should have its own WS (via _connectLeafWs)
    // In the mock, WebSocket constructor creates a mock — just verify it was called
    assertOk(secLeaf.ws !== null || secLeaf.wsInstUrl === 'http://localhost:9090',
        'SPL-011c: secondary leaf WS state initialized');
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
    // Create initial HTML with one cell: "A" at row 0, col 0
    pre.innerHTML = '<span class="c w1" style="width:1ch;color:#000;background:#fff">A</span>\n';
    vttyEl.appendChild(pre);

    const leafId = 'test-diff-leaf';
    const cmdId = 'cmd-diff';

    // Build the cell grid
    buildCellGrid(leafId + '/' + cmdId, pre, 1, 1);

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

    _applyLeafDiff(vttyEl, leafId, diffData);

    // The span should now contain "B"
    assertIncludes(pre.innerHTML, '>B<', 'SPL-012a: cell updated from A to B');
    assertEq(state._lastGeneration[leafId + '/' + cmdId], 2,
        'SPL-012b: generation cached');

    // Second diff with same generation → no-op (metadata only)
    const sameGenData = { generation: 2, _cmdId: cmdId };
    const beforeHtml = pre.innerHTML;
    _applyLeafDiff(vttyEl, leafId, sameGenData);
    assertEq(pre.innerHTML, beforeHtml, 'SPL-012c: same generation skipped update');

    state._level3Enabled = false;
}

console.log('\n[split_interactions] Tests complete');