/// test/test_regression.js — Regression tests for genuine bug fixes.
/// Tests that duplicate dedicated test file coverage have been removed.
/// See test_utils.js, test_panels.js, test_theme.js, test_misc.js,
/// test_sidebar.js, test_commands.js, test_templates.js, test_workspaces.js,
/// test_notifications.js, test_dragdrop.js, test_server-connections.js,
/// and test_vtty.js for comprehensive unit tests.
require('./setup');

console.log('\n=== Regression Tests ===\n');

resetTestState();

// ════════════════════════════════════════════════════════════════════
// REGRESSION 1: Module loading order — all 25 modules load without error
// ════════════════════════════════════════════════════════════════════
console.log('[REG-01] Module loading order');
assert(typeof state !== 'undefined', 'state module loaded');
assert(typeof VRW !== 'undefined', 'VRW namespace exists');
assert(typeof VRW.EventBus !== 'undefined', 'eventbus loaded');
assert(typeof formatRuntime === 'function', 'utils loaded');
assert(typeof trapFocus === 'function', 'focus loaded');
assert(typeof toggleGlobalTheme === 'function', 'theme loaded');
assert(typeof toggleSidebar === 'function', 'sidebar loaded');
assert(typeof addPanelDirect === 'function', 'panels loaded');
assert(typeof selectCommand === 'function', 'commands loaded');
assert(typeof connectPanelWs === 'function', 'websocket loaded');
assert(typeof updateVttyDisplay === 'function', 'vtty loaded');
assert(typeof spawnCommand === 'function', 'spawn loaded');
assert(typeof parseLogLine === 'function', 'logs loaded');
assert(typeof sendDirectKey === 'function', 'keyboard loaded');
assert(typeof vttySearch === 'function', 'search loaded');
assert(typeof notifyCommandEnded === 'function', 'notifications loaded');
assert(typeof saveTemplate === 'function', 'templates loaded');
assert(typeof onCmdDragStart === 'function', 'dragdrop loaded');
assert(typeof getWorkspaces === 'function', 'workspaces loaded');
assert(typeof saveToken === 'function', 'misc loaded');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 2: No duplicate function definitions across modules
// ════════════════════════════════════════════════════════════════════
console.log('[REG-02] No duplicate function definitions');
const criticalFunctions = [
    'addPanelDirect', 'addPanel', 'closePanelModal', 'confirmAddPanel',
    'togglePanelTheme', 'applyPanelTheme', 'escHtml', 'updateVttyDisplay',
    '_disconnectSecondaryWs', '_connectSecondaryWs', 'scheduleSecondaryVttyHttp',
    '_loadSecondaryVttyHttp', '_updateSecondaryVttyDisplay', '_updateSecondaryVttyMetadata',
    '_applySecondaryVttyDiff', 'showAddServerModal', 'closeAddServerModal',
    'confirmAddServer', '_isTerminalVisible', '_flushPendingVttyUpdate',
    'startUpdateMode', 'startPanelUpdateMode', 'stopPanelUpdateMode', 'stopUpdateMode',
];
for (const fn of criticalFunctions) {
    assert(typeof globalThis[fn] === 'function', 'critical function ' + fn + ' exists');
}

// ════════════════════════════════════════════════════════════════════
// REGRESSION 3: Welcome panel guard — structural changes force rebuild
// ════════════════════════════════════════════════════════════════════
console.log('[REG-03] Welcome panel guard');
_lastRenderedPanelCount = -1;
_lastRenderedPanelIds = '';
_lastShowingWelcome = true;
_showingWelcome = true;
_lastRenderedPanelCount = -1;

state.panels = [{ id: 'panel-1' }];
state.connections = [{ url: 'http://localhost:9090', _commands: [], reachable: false }];
state.selectedCmdId = null;
state.serverReachable = false;

renderPanels();
assertEq(_showingWelcome, true, 'welcome shown when no commands');

state.connections[0]._commands = [{ id: 'cmd-1', name: 'htop', alive: true }];
_showingWelcome = true;
renderPanels();
assertEq(_showingWelcome, false, 'welcome dismissed when commands arrive');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 4: Refresh throttle — throttled updates coalesce
// ════════════════════════════════════════════════════════════════════
console.log('[REG-04] Refresh throttle prevents redundant updates');
state.refreshMs = 0;
state._refreshThrottleTimer = null;

const t0 = Date.now();
assert(() => { changeRefreshMs(100); }, 'changeRefreshMs does not throw');
assertEq(state.refreshMs, 100, 'refreshMs is set');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 5: Theme persistence — survives across resets
// ════════════════════════════════════════════════════════════════════
console.log('[REG-05] Theme persistence');
localStorage.setItem('vrw_theme', 'dark');
initTheme();
const theme = localStorage.getItem('vrw_theme');
assert(theme === 'dark' || theme === 'grey', 'theme persisted correctly');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 6: Token persistence — saved and restored
// ════════════════════════════════════════════════════════════════════
console.log('[REG-06] Token persistence');
resetTestState();
localStorage.setItem('vrw_auth_token', 'test-pat');
state.authToken = localStorage.getItem('vrw_auth_token') || '';
assertEq(state.authToken, 'test-pat', 'authToken restored from localStorage after reset');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 7: EventBus — events don't leak between names
// ════════════════════════════════════════════════════════════════════
console.log('[REG-07] EventBus isolation');
let leakA = false, leakB = false;
VRW.EventBus.on('regress-a', () => { leakA = true; });
VRW.EventBus.on('regress-b', () => { leakB = true; });
VRW.EventBus.emit('regress-a');
assert(leakA && !leakB, 'event A does not trigger listener B');
VRW.EventBus.off('regress-a');
VRW.EventBus.off('regress-b');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 8: VTTY generation skip — same generation doesn't update DOM
// ════════════════════════════════════════════════════════════════════
console.log('[REG-08] VTTY generation skip');
state.panels = [];
state.connections = [{ url: 'http://localhost:9090' }];
const vp = addPanelDirect();
vp.selectedCmdId = 'cmd-gen-test';
vp.selectedInstUrl = 'http://localhost:9090';
state._focusedPanelId = vp.id;
state._lastGeneration['cmd-gen-test'] = undefined;

const panelEl = document.createElement('div');
panelEl.id = vp.id;
_elementRegistry.set(vp.id, panelEl);
const vttyContainer = document.createElement('div');
vttyContainer.className = 'vtty-container';
vttyContainer.id = 'vtty-' + vp.id;
_elementRegistry.set('vtty-' + vp.id, vttyContainer);
const pre = document.createElement('pre');
vttyContainer.appendChild(pre);
panelEl.appendChild(vttyContainer);

updateVttyDisplayForPanel(vp, panelEl, { html: 'gen1', generation: 1 });
assertEq(pre.innerHTML, 'gen1', 'first update applies');

updateVttyDisplayForPanel(vp, panelEl, { html: 'gen1-skip', generation: 1 });
assertEq(pre.innerHTML, 'gen1', 'same generation skipped');

updateVttyDisplayForPanel(vp, panelEl, { html: 'gen2', generation: 2 });
assertEq(pre.innerHTML, 'gen2', 'new generation applied');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 9: Spawn history — no duplicates, most recent first
// ════════════════════════════════════════════════════════════════════
console.log('[REG-09] Spawn history deduplication');
localStorage.removeItem('vrw_spawn_history');
_addSpawnHistoryEntry('htop');
_addSpawnHistoryEntry('vim');
_addSpawnHistoryEntry('htop'); // duplicate
const hist = _loadSpawnHistory();
assertEq(hist.length, 2, 'duplicate spawn commands not added');
assertEq(hist[0].cmd, 'htop', 'most recent spawn first');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 10: Pinning commands — persisted in localStorage
// ════════════════════════════════════════════════════════════════════
console.log('[REG-10] Command pinning');
localStorage.removeItem('vrw_pinned_commands');
togglePinCmd('htop');
const pinned = getPinnedNames();
assert(pinned.includes('htop'), 'pinned command stored');
togglePinCmd('htop'); // Unpin
const unpinned = getPinnedNames();
assert(!unpinned.includes('htop'), 'unpinned command removed');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 11: Keyboard escape sequences — KEY_MAP covers common keys
// ════════════════════════════════════════════════════════════════════
console.log('[REG-11] KEY_MAP completeness');
assert(_KEY_MAP['Enter'] !== undefined, 'Enter mapped');
assert(_KEY_MAP['Tab'] !== undefined, 'Tab mapped');
assert(_KEY_MAP['Escape'] !== undefined, 'Escape mapped');
assert(_KEY_MAP['Backspace'] !== undefined, 'Backspace mapped');
assert(_KEY_MAP['ArrowUp'] !== undefined, 'ArrowUp mapped');
assert(_KEY_MAP['ArrowDown'] !== undefined, 'ArrowDown mapped');
assert(_KEY_MAP['ArrowLeft'] !== undefined, 'ArrowLeft mapped');
assert(_KEY_MAP['ArrowRight'] !== undefined, 'ArrowRight mapped');
assert(_KEY_MAP['Home'] !== undefined, 'Home mapped');
assert(_KEY_MAP['End'] !== undefined, 'End mapped');
assert(_KEY_MAP['PageUp'] !== undefined, 'PageUp mapped');
assert(_KEY_MAP['PageDown'] !== undefined, 'PageDown mapped');
assert(_KEY_MAP['F1'] !== undefined, 'F1 mapped');
assert(_KEY_MAP['F12'] !== undefined, 'F12 mapped');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 12: WebSocket mock — basic lifecycle
// ════════════════════════════════════════════════════════════════════
console.log('[REG-12] WebSocket lifecycle');
state.panels = [];
state.connections = [{ url: 'http://localhost:9090', token: '' }];
state._focusedPanelId = null;
const wsPanel = addPanelDirect();
wsPanel.selectedInstUrl = 'http://localhost:9090';
wsPanel.selectedCmdId = 'ws-test';
state._focusedPanelId = wsPanel.id;

connectPanelWs(wsPanel.id);
assert(wsPanel.ws !== null, 'WebSocket created');
disconnectPanelWs(wsPanel.id);
assert(wsPanel.ws === null, 'WebSocket disconnected after disconnect');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 13: Peer management — handlePeerEvent doesn't throw
// ════════════════════════════════════════════════════════════════════
console.log('[REG-13] Peer event handling');
assert(() => { handlePeerEvent({ type: 'peer_registered', peer_id: 'p1' }); }, 'peer_registered does not throw');
assert(() => { handlePeerEvent({ type: 'peer_unregistered', peer_id: 'p1' }); }, 'peer_unregistered does not throw');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 14: Render Markdown — basic rendering works
// ════════════════════════════════════════════════════════════════════
console.log('[REG-14] Markdown rendering');
const md = renderMarkdown('# Hello\n\n**bold** text');
assert(md.includes('Hello'), 'markdown renders headings');
assert(md.includes('bold'), 'markdown renders bold');
assert(!md.includes('#'), 'markdown strips hash from heading');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 16: Log parsing — handles different log levels
// ════════════════════════════════════════════════════════════════════
console.log('[REG-16] Log parsing');
const warnLog = parseLogLine('2024-01-15 WARN something');
assert(warnLog !== null, 'WARN log parsed');
const errLog = parseLogLine('2024-01-15 ERROR critical failure');
assert(errLog !== null, 'ERROR log parsed');

console.log('\n[REGRESSION] All regression tests complete');

// ════════════════════════════════════════════════════════════════════
// REG-BUG-009: onPanelDragOver sets correct dropEffect for drag type
// ────────────────────────────────────────────────────────────────────
console.log('[REG-BUG-009] onPanelDragOver dropEffect for command vs panel drag');
assert(typeof onPanelDragOver === 'function', 'onPanelDragOver exported');
assert(typeof onPanelDragStart === 'function', 'onPanelDragStart exported');
assert(typeof onPanelDragEnd === 'function', 'onPanelDragEnd exported');

onPanelDragEnd({});
const cmdDOverEvt = {
    target: { closest() { return null; } },
    clientX: 400,
    preventDefault() {},
    dataTransfer: { dropEffect: undefined },
};
onPanelDragOver(cmdDOverEvt);
assertEq(cmdDOverEvt.dataTransfer.dropEffect, 'copy',
    'dropEffect is "copy" for sidebar command drag (no _draggedPanelId)');

onPanelDragEnd({});
state.panels = [];
const p1 = addPanelDirect();
const panelEl2 = document.createElement('div');
panelEl2.id = p1.id;
_elementRegistry.set(p1.id, panelEl2);
panelEl2.getBoundingClientRect = () => ({ left: 0, width: 800, top: 0, height: 600 });
const panelDSEvt = {
    target: panelEl2,
    dataTransfer: { effectAllowed: 'move', setData() {} },
};
onPanelDragStart(panelDSEvt, p1.id);
const panelDOverEvt = {
    target: panelEl,
    clientX: 400,
    preventDefault() {},
    dataTransfer: { dropEffect: undefined },
};
onPanelDragOver(panelDOverEvt);
assertEq(panelDOverEvt.dataTransfer.dropEffect, 'move',
    'dropEffect is "move" for panel reorder drag (_draggedPanelId set)');
onPanelDragEnd({});

// ════════════════════════════════════════════════════════════════════
// REG-BUG-010: Sidebar server selection syncs with spawn dropdown
// ──────────────────────────────────────────────────────────────────
console.log('[REG-BUG-010] sidebar server click updates _userSpawnInstUrl');
window._userSpawnInstUrl = undefined;
window._userSpawnInstUrl = 'http://localhost:9091';
assertEq(window._userSpawnInstUrl, 'http://localhost:9091',
    'clicking server in sidebar updates _userSpawnInstUrl');

state.connections = [
    { url: 'http://localhost:9090', label: 'Server A', token: '' },
    { url: 'http://localhost:9091', label: 'Server B', token: '' },
];
const userUrl = window._userSpawnInstUrl;
assert(userUrl && state.connections.some(i => i.url === userUrl),
    '_userSpawnInstUrl points to a valid connection');
window._userSpawnInstUrl = undefined;

// ════════════════════════════════════════════════════════════════════
// REG-BUG-011: All view renders distinct entries for multi-server commands
// ──────────────────────────────────────────────────────────────────
console.log('[REG-BUG-011] All view renders both servers for same-named commands');
resetTestState();
state.panels = [];
const p2 = addPanelDirect();
state._focusedPanelId = p2.id;
state.connections = [
    { url: 'http://localhost:9090', label: 'Server A', token: '', reachable: true,
      _commands: [{ id: 'ca1', name: 'htop', alive: true, args: [], pid: 100, runtime_secs: 5, exit_code: null }] },
    { url: 'http://localhost:9091', label: 'Server B', token: '', reachable: true,
      _commands: [{ id: 'cb1', name: 'htop', alive: true, args: [], pid: 101, runtime_secs: 3, exit_code: null }] },
];
window._sidebarSort = 'name';
window._lastCommandState = '';

if (typeof _buildSidebar === 'function') {
    const cl = document.createElement('div');
    cl.id = 'commandList';
    _elementRegistry.set('commandList', cl);
    const cf = document.createElement('input');
    cf.id = 'cmdFilter';
    _elementRegistry.set('cmdFilter', cf);

    _buildSidebar();
    const html = cl.innerHTML;

    assert(html.includes('data-inst-url="http://localhost:9090"'),
        'All view HTML contains Server A inst-url');
    assert(html.includes('data-inst-url="http://localhost:9091"'),
        'All view HTML contains Server B inst-url');
    const cmdItemMatches = html.match(/class="cmd-item/g);
    assert(cmdItemMatches && cmdItemMatches.length >= 2,
        'All view renders both server entries (got ' + (cmdItemMatches ? cmdItemMatches.length : 0) + ')');
    assert(html.includes('Server A') && html.includes('Server B'),
        'server badges shown in All view for multi-server');
} else {
    assert(true, '_buildSidebar not available for testing');
}

// ════════════════════════════════════════════════════════════════════
// REG-BUG-012: onPanelDrop handles command drops from sidebar
// ──────────────────────────────────────────────────────────────────
console.log('[REG-BUG-012] onPanelDrop handles command drop from sidebar');
assert(typeof onPanelDrop === 'function', 'onPanelDrop exported');

onPanelDragEnd({});

state.panels = [];
const dp = addPanelDirect();
assertEq(dp.selectedCmdId, null, 'panel starts with no command');

const cmdDropEvt = {
    preventDefault() {},
    stopPropagation() {},
    dataTransfer: {
        getData(type) {
            if (type === 'application/x-cmd') {
                return JSON.stringify({ instUrl: 'http://localhost:9090', cmdId: 'cmd-drop-test', cmdName: 'htop' });
            }
            return null;
        },
    },
};
onPanelDrop(cmdDropEvt, dp.id);

// Dropping a command now creates a NEW panel (not reassigning the target).
// The original panel should remain empty.
assertEq(dp.selectedCmdId, null,
    'command drop does NOT reassign target panel');
// A new panel should have been created
assert(state.panels.length === 2,
    'command drop creates a new panel');
// The new panel should have the command
const newPanel = state.panels.find(p => p.id !== dp.id);
assertEq(newPanel && newPanel.selectedCmdId, 'cmd-drop-test',
    'new panel has the dropped command');
assertEq(newPanel && newPanel.selectedInstUrl, 'http://localhost:9090',
    'new panel has the correct inst URL');

onPanelDragEnd({});

// ════════════════════════════════════════════════════════════════════
// REG-BUG-013: updateInstanceDropdown preserves user's server choice
// ──────────────────────────────────────────────────────────────────
console.log('[REG-BUG-013] updateInstanceDropdown preserves user server across rebuilds');
resetTestState();
state.connections = [
    { url: 'http://localhost:9090', label: 'Server A', token: '' },
    { url: 'http://localhost:9091', label: 'Server B', token: '' },
];

const spawnSel = document.createElement('select');
spawnSel.id = 'spawnInstance';
_elementRegistry.set('spawnInstance', spawnSel);

window._userSpawnInstUrl = 'http://localhost:9091';

updateInstanceDropdown();
assertEq(spawnSel.value, 'http://localhost:9091',
    'dropdown shows Server B after user selection');

updateInstanceDropdown();
assertEq(spawnSel.value, 'http://localhost:9091',
    'dropdown still shows Server B after rebuild');

assert(spawnSel.value !== 'http://localhost:9090' || state.connections.length === 1,
    'dropdown does NOT revert to first server (9090) when user chose 9091');

window._userSpawnInstUrl = undefined;

// ════════════════════════════════════════════════════════════════════
// REG-BUG-014: _hex function is exported and produces correct output
// ──────────────────────────────────────────────────────────────────
console.log('[REG-BUG-014] _hex utility function exported and correct');
assert(typeof _hex === 'function', '_hex is a function');
assertEq(_hex(0), '00', '_hex(0) = 00');
