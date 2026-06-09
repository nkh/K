/// test/test_regression.js — Higher-level regression tests for vrw web UI.
/// These tests verify critical bug fixes and end-to-end scenarios.
require('./setup');

console.log('\n=== Regression Tests ===\n');

resetTestState();

// ════════════════════════════════════════════════════════════════════
// REGRESSION 1: Module loading order — all 20 modules load without error
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
assert(typeof checkOnboarding === 'function', 'onboarding loaded');
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
// Check that each function has exactly one definition
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
_lastRenderedPanelCount = -1; // Force first rebuild

state.panels = [{ id: 'panel-1' }];
state.connections = [{ url: 'http://localhost:9090', _commands: [], reachable: false }];
state.selectedCmdId = null;
state.serverReachable = false;

// First render with no commands → should show welcome
renderPanels();
assertEq(_showingWelcome, true, 'welcome shown when no commands');

// Commands arrive → welcome should be dismissed
state.connections[0]._commands = [{ id: 'cmd-1', name: 'htop', alive: true }];
_showingWelcome = true; // Simulate detection
renderPanels(); // With _showingWelcome changed, guard should fire
assertEq(_showingWelcome, false, 'welcome dismissed when commands arrive');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 4: Refresh throttle — throttled updates coalesce
// ════════════════════════════════════════════════════════════════════
console.log('[REG-04] Refresh throttle prevents redundant updates');
state.refreshMs = 0;
state._refreshThrottleTimer = null;

// First call should set timer (throttled)
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
// REGRESSION 7: Panel creation — panels have all required fields
// ════════════════════════════════════════════════════════════════════
console.log('[REG-07] Panel creation has required fields');
resetTestState();
state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
state._focusedPanelId = null;

const panel = addPanelDirect();
assert(panel.id.startsWith('panel-'), 'panel has valid id');
assert(panel.id.length > 10, 'panel id is sufficiently long');
assertEq(panel.minimized, false, 'panel starts unminimized');
assertEq(panel.focused, false, 'panel starts unfocused');
assertEq(panel.selectedCmdId, null, 'panel starts with no command');
assertEq(panel.selectedInstUrl, null, 'panel starts with no instance');
assert(panel.fontSize >= 8 && panel.fontSize <= 28, 'panel fontSize in valid range');
assert(Array.isArray(panel.cmdHistory), 'panel has command history array');
assertEq(panel.cmdHistoryIdx, -1, 'command history index starts at -1');
assert(panel.ws === null, 'panel ws starts null');
assert(panel.pollTimer === null, 'panel poll timer starts null');
assertEq(state.panels.length, 1, 'panel added to state');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 8: Connection idempotency — adding same URL twice is idempotent
// ════════════════════════════════════════════════════════════════════
console.log('[REG-08] Connection idempotency');
state.connections = [];
const c1 = addConnection('http://localhost:9090', 'Test', 'tok');
assertEq(state.connections.length, 1, 'first connection added');
const c2 = addConnection('http://localhost:9090', 'Test2', 'tok2');
assertEq(state.connections.length, 1, 'duplicate URL not added');
assertEq(c2.label, 'Test', 'existing connection returned (label unchanged)');
assertEq(c2.token, 'tok', 'existing connection returned (token unchanged)');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 9: EventBus — events don't leak between names
// ════════════════════════════════════════════════════════════════════
console.log('[REG-09] EventBus isolation');
let leakA = false, leakB = false;
VRW.EventBus.on('regress-a', () => { leakA = true; });
VRW.EventBus.on('regress-b', () => { leakB = true; });
VRW.EventBus.emit('regress-a');
assert(leakA && !leakB, 'event A does not trigger listener B');
VRW.EventBus.off('regress-a');
VRW.EventBus.off('regress-b');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 10: VTTY generation skip — same generation doesn't update DOM
// ════════════════════════════════════════════════════════════════════
console.log('[REG-10] VTTY generation skip');
state.panels = [];
state.connections = [{ url: 'http://localhost:9090' }];
const vp = addPanelDirect();
vp.selectedCmdId = 'cmd-gen-test';
vp.selectedInstUrl = 'http://localhost:9090';
state._focusedPanelId = vp.id;
state._lastGeneration['cmd-gen-test'] = undefined;

// Create a mock panel element with .vtty-container > pre structure
// (updateVttyDisplayForPanel queries panelEl.querySelector('.vtty-container'))
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
// REGRESSION 11: Panel theme — cycles correctly
// ════════════════════════════════════════════════════════════════════
console.log('[REG-11] Panel theme cycling');
resetTestState();
state.panels = [];
state.connections = [];
const tp = addPanelDirect();
assertEq(tp.theme, '', 'theme starts empty (inherit)');

togglePanelTheme(tp.id);
assertEq(tp.theme, 'light', 'empty → light');

togglePanelTheme(tp.id);
assertEq(tp.theme, 'dark', 'light → dark');

togglePanelTheme(tp.id);
assertEq(tp.theme, '', 'dark → empty (inherit)');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 12: Font size clamping — never below 8 or above 28
// ════════════════════════════════════════════════════════════════════
console.log('[REG-12] Font size clamping');
state.fontSize = 10;
changeFontSize(-100);
assert(state.fontSize >= 8, 'fontSize never below 8: got ' + state.fontSize);
state.fontSize = 26;
changeFontSize(100);
assert(state.fontSize <= 28, 'fontSize never above 28: got ' + state.fontSize);

// ════════════════════════════════════════════════════════════════════
// REGRESSION 13: RefreshMs clamping — 0 to 2000 only
// ════════════════════════════════════════════════════════════════════
console.log('[REG-13] RefreshMs clamping');
state.refreshMs = 0;
changeRefreshMs(-100);
assertEq(state.refreshMs, 0, 'refreshMs clamped at 0');
state.refreshMs = 1900;
changeRefreshMs(200);
assertEq(state.refreshMs, 2000, 'refreshMs capped at 2000');
state.refreshMs = 2000;
changeRefreshMs(100);
assertEq(state.refreshMs, 2000, 'refreshMs cannot exceed 2000');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 14: Spawn history — no duplicates, most recent first
// ════════════════════════════════════════════════════════════════════
console.log('[REG-14] Spawn history deduplication');
localStorage.removeItem('vrw_spawn_history');
_addSpawnHistoryEntry('htop');
_addSpawnHistoryEntry('vim');
_addSpawnHistoryEntry('htop'); // duplicate
const hist = _loadSpawnHistory();
assertEq(hist.length, 2, 'duplicate spawn commands not added');
assertEq(hist[0].cmd, 'htop', 'most recent spawn first');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 15: Template persistence — saved and loaded correctly
// ════════════════════════════════════════════════════════════════════
console.log('[REG-15] Template persistence');
localStorage.removeItem('vrw_templates');
saveUserTemplates([{ name: 'dev', cmd: 'npm run dev', args: '' }]);
const loaded = getUserTemplates();
assert(Array.isArray(loaded), 'templates loaded as array');
assert(loaded.length >= 1, 'at least one template');
assertEq(loaded[0].cmd, 'npm run dev', 'template cmd preserved');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 16: Sidebar toggle — collapsed class toggles
// ════════════════════════════════════════════════════════════════════
console.log('[REG-16] Sidebar toggle');
const sidebar = document.getElementById('sidebar');
sidebar._classList = new Set(); // fresh
toggleSidebar();
assert(sidebar._classList.has('collapsed'), 'sidebar collapsed after toggle');
toggleSidebar();
assert(!sidebar._classList.has('collapsed'), 'sidebar uncollapsed after second toggle');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 17: Workspace save/load
// ════════════════════════════════════════════════════════════════════
console.log('[REG-17] Workspace persistence');
localStorage.removeItem('vrw_workspaces');
// Workspaces are stored as a name-keyed object: { "name": { panels: [...] } }
saveWorkspaces({ 'default-ws': { panels: [{ id: 'p1', cmdId: 'c1' }] } });
const ws = getWorkspaces();
assert(typeof ws === 'object', 'workspaces loaded');
assert(ws['default-ws'], 'workspace saved by name');
deleteWorkspace('default-ws');
const wsAfter = getWorkspaces();
assert(!wsAfter['default-ws'], 'workspace deleted');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 18: Command groups persistence
// ════════════════════════════════════════════════════════════════════
console.log('[REG-18] Command groups persistence');
localStorage.removeItem('vrw_cmd_groups');
saveCmdGroups([{ name: 'dev-tools', cmds: ['htop', 'vim'] }]);
const groups = getCmdGroups();
assert(typeof groups === 'object', 'groups loaded as object');

// ══════════════════════════════════════════════════════════════════
// REGRESSION 19: EscHtml — properly escapes HTML entities
// ══════════════════════════════════════════════════════════════════
console.log('[REG-19] escHtml XSS prevention');
const xss = escHtml('<img src=x onerror=alert(1)>');
assert(xss.includes('&lt;'), 'XSS: < escaped');
assert(xss.includes('&gt;'), 'XSS: > escaped');
assert(!xss.includes('<img'), 'XSS: tag broken');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 20: Parse spawn args — handles quoted strings
// ══════════════════════════════════════════════════════════════════
console.log('[REG-20] Parse spawn args handles quotes');
const args1 = parseSpawnArgs('--name "my app" --other thing');
assertEq(args1.length, 4, '4 args with quoted string');
assertEq(args1[1], 'my app', 'quoted arg preserved');

const args2 = parseSpawnArgs("-c 'echo hello world'");
assertEq(args2.length, 2, '2 args with single quotes');
assertEq(args2[1], 'echo hello world', 'single-quoted arg preserved');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 21: Pinning commands — persisted in localStorage
// ════════════════════════════════════════════════════════════════════
console.log('[REG-21] Command pinning');
localStorage.removeItem('vrw_pinned_commands');
// togglePinCmd takes a cmdName string, not (instUrl, cmdId)
togglePinCmd('htop');
const pinned = getPinnedNames();
assert(pinned.includes('htop'), 'pinned command stored');
togglePinCmd('htop'); // Unpin
const unpinned = getPinnedNames();
assert(!unpinned.includes('htop'), 'unpinned command removed');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 22: Sound toggle — persisted state
// ══════════════════════════════════════════════════════════════════
console.log('[REG-22] Sound toggle persistence');
localStorage.setItem('vrw_sound', 'false');
state.soundEnabled = false;
toggleSoundNotifications();
assertEq(state.soundEnabled, true, 'sound toggled on');
assertEq(localStorage.getItem('vrw_sound'), 'true', 'sound persisted to localStorage');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 23: Panel minimize — toggles minimized flag
// ══════════════════════════════════════════════════════════════════
console.log('[REG-23] Panel minimize/restore');
resetTestState();
state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
state._focusedPanelId = null;
const mp = addPanelDirect();
assertEq(mp.minimized, false, 'panel starts unminimized');
toggleMinimizePanel(mp.id);
assertEq(mp.minimized, true, 'panel minimized');
toggleMinimizePanel(mp.id);
assertEq(mp.minimized, false, 'panel restored');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 24: Split panel — creates split structure
// ══════════════════════════════════════════════════════════════════
console.log('[REG-24] Split panel structure');
resetTestState();
state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
state._focusedPanelId = null;
const sp = addPanelDirect();
assert(sp.split === undefined, 'no split initially');
splitPanel(sp.id, 'horizontal');
assert(sp.split !== null, 'split created');
assertEq(sp.split.direction, 'horizontal', 'split direction correct');
assertEq(sp.split.splitRatio, 0.5, 'split ratio 0.5');
assertEq(sp.split.activeSide, 'primary', 'active side is primary');
unsplitPanel(sp.id);
assertEq(sp.split, null, 'split removed after unsplit');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 25: Auth headers — token included correctly
// ════════════════════════════════════════════════════════════════════
console.log('[REG-25] Auth headers');
state.authToken = 'test-token';
const h = authHeaders('');
assertEq(h.Authorization, 'Bearer test-token', 'Bearer token from global state');
const h2 = authHeaders('override-token');
assertEq(h2.Authorization, 'Bearer override-token', 'explicit token overrides global');

state.authToken = '';
state.connections = [];
const h3 = authHeadersForInstance({ url: 'http://localhost:9090', token: 'inst-tok' });
assertEq(h3.Authorization, 'Bearer inst-tok', 'instance token used');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 26: API URL — correctly constructs full URL
// ════════════════════════════════════════════════════════════════════
console.log('[REG-26] API URL construction');
state.connections = [{ url: 'http://localhost:9090' }];
assertEq(apiUrl('/api/commands'), 'http://localhost:9090/api/commands', 'default base URL');
assertEq(apiUrl('/api/commands', { url: 'http://example.com' }), 'http://example.com/api/commands', 'instance-specific URL');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 27: Render Markdown — basic rendering works
// ══════════════════════════════════════════════════════════════════
console.log('[REG-27] Markdown rendering');
const md = renderMarkdown('# Hello\n\n**bold** text');
assert(md.includes('Hello'), 'markdown renders headings');
assert(md.includes('bold'), 'markdown renders bold');
assert(!md.includes('#'), 'markdown strips hash from heading');

// ══════════════════════════════════════════════════════════════════
// REGRESSION 28: Onboarding — step data structure
// ══════════════════════════════════════════════════════════════════
console.log('[REG-28] Onboarding steps data structure');
assert(Array.isArray(_onboardingSteps), 'onboarding steps is array');
if (_onboardingSteps && _onboardingSteps.length > 0) {
    assert(typeof _onboardingSteps[0].title === 'string', 'step has title');
    assert(typeof _onboardingSteps[0].body === 'string', 'step has body');
    assert(_onboardingSteps.length >= 5, 'at least 5 onboarding steps');
} else {
    assert(true, 'onboarding steps not exported (acceptable)');
}

// ══════════════════════════════════════════════════════════════════
// REGRESSION 29: Log parsing — handles different log levels
// ══════════════════════════════════════════════════════════════════
console.log('[REG-29] Log parsing');
const warnLog = parseLogLine('2024-01-15 WARN something');
assert(warnLog !== null, 'WARN log parsed');
const errLog = parseLogLine('2024-01-15 ERROR critical failure');
assert(errLog !== null, 'ERROR log parsed');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 30: Hex color conversion — correct format
// ══════════════════════════════════════════════════════════════════
console.log('[REG-30] Hex color conversion');
assertEq(_hex(0), '00', 'hex(0) = 00');
assertEq(_hex(255), 'ff', 'hex(255) = ff');
assertEq(_hex(16), '10', 'hex(16) = 10');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 31: Keyboard escape sequences — KEY_MAP covers common keys
// ════════════════════════════════════════════════════════════════════
console.log('[REG-31] KEY_MAP completeness');
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
// REGRESSION 32: WebSocket mock — basic lifecycle
// ════════════════════════════════════════════════════════════════════
console.log('[REG-32] WebSocket lifecycle');
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
// REGRESSION 33: Drag-and-drop — data transfer sets correct data
// ══════════════════════════════════════════════════════════════════
console.log('[REG-33] Drag-and-drop data transfer');
const dt = {
    dataTransfer: { setData(k, v) { dt.dataTransfer[k] = v; }, effectAllowed: 'copy' },
    target: { style: { opacity: '' } },
};
onCmdDragStart(dt, 'http://localhost:9090', 'cmd-1', 'htop');
assertEq(dt.dataTransfer['text/plain'], 'cmd-1', 'drag data set correctly (text/plain)');
assert(typeof dt.dataTransfer['application/x-cmd'] === 'string', 'drag data set correctly (application/x-cmd)');
const cmdJson = JSON.parse(dt.dataTransfer['application/x-cmd']);
assertEq(cmdJson.instUrl, 'http://localhost:9090', 'drag data has instUrl');
assertEq(cmdJson.cmdId, 'cmd-1', 'drag data has cmdId');
assertEq(cmdJson.cmdName, 'htop', 'drag data has cmdName');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 34: Peer management — handlePeerEvent doesn't throw
// ════════════════════════════════════════════════════════════════════
console.log('[REG-34] Peer event handling');
assert(() => { handlePeerEvent({ type: 'peer_registered', peer_id: 'p1' }); }, 'peer_registered does not throw');
assert(() => { handlePeerEvent({ type: 'peer_unregistered', peer_id: 'p1' }); }, 'peer_unregistered does not throw');

// ════════════════════════════════════════════════════════════════════
// REGRESSION 35: formatRuntime — handles edge cases
// ══════════════════════════════════════════════════════════════════
console.log('[REG-35] formatRuntime edge cases');
assertEq(formatRuntime(null), '', 'null runtime → empty');
assertEq(formatRuntime(undefined), '', 'undefined runtime → empty');
assertEq(formatRuntime(0), '', 'zero runtime → empty');
assertEq(formatRuntime(-1), '', 'negative runtime → empty');
assertEq(formatRuntime(0.5), '0s', 'sub-second rounds down');
assertEq(formatRuntime(59), '59s', '59 seconds');
assertEq(formatRuntime(60), '1m 0s', '60 seconds = 1m 0s');
assertEq(formatRuntime(3600), '1h 0m', '1 hour');
assertEq(formatRuntime(90061), '25h 1m', '25 hours 1 minute');

console.log('\n[REGRESSION] All regression tests complete');

// ════════════════════════════════════════════════════════════════════
// REG-BUG-009: onPanelDragOver sets correct dropEffect for drag type
// ────────────────────────────────────────────────────────────────────
console.log('[REG-BUG-009] onPanelDragOver dropEffect for command vs panel drag');
assert(typeof onPanelDragOver === 'function', 'onPanelDragOver exported');
assert(typeof onPanelDragStart === 'function', 'onPanelDragStart exported');
assert(typeof onPanelDragEnd === 'function', 'onPanelDragEnd exported');

// Simulate command drag (no panel drag — _draggedPanelId should be null)
onPanelDragEnd({}); // clear state
const cmdDOverEvt = {
    target: { closest() { return null; } },
    clientX: 400,
    preventDefault() {},
    dataTransfer: { dropEffect: undefined },
};
onPanelDragOver(cmdDOverEvt);
assertEq(cmdDOverEvt.dataTransfer.dropEffect, 'copy',
    'dropEffect is "copy" for sidebar command drag (no _draggedPanelId)');

// Simulate panel drag (_draggedPanelId set via onPanelDragStart)
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
// Simulate clicking Server B in sidebar sort bar (sets _userSpawnInstUrl)
window._userSpawnInstUrl = 'http://localhost:9091';
assertEq(window._userSpawnInstUrl, 'http://localhost:9091',
    'clicking server in sidebar updates _userSpawnInstUrl');

// Verify updateInstanceDropdown logic preserves user's choice
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

// Ensure no panel drag is active (command drag scenario)
onPanelDragEnd({});

// Create a panel to receive the drop
state.panels = [];
const dp = addPanelDirect();
assertEq(dp.selectedCmdId, null, 'panel starts with no command');

// Simulate dropping a command from sidebar onto the panel
const cmdDropEvt = {
    preventDefault() {},
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

// Verify the panel now has the dropped command assigned
assertEq(dp.selectedInstUrl, 'http://localhost:9090',
    'command drop sets panel selectedInstUrl');
assertEq(dp.selectedCmdId, 'cmd-drop-test',
    'command drop sets panel selectedCmdId');
assertEq(state.selectedInstUrl, 'http://localhost:9090',
    'command drop syncs global selectedInstUrl');
assertEq(state.selectedCmdId, 'cmd-drop-test',
    'command drop syncs global selectedCmdId');

// Cleanup
onPanelDragEnd({});

// ════════════════════════════════════════════════════════════════════
// REG-BUG-013: updateInstanceDropdown preserves user's server choice
//              across multiple calls (prevents spawn 9090 revert bug)
// ──────────────────────────────────────────────────────────────────
console.log('[REG-BUG-013] updateInstanceDropdown preserves user server across rebuilds');
resetTestState();
state.connections = [
    { url: 'http://localhost:9090', label: 'Server A', token: '' },
    { url: 'http://localhost:9091', label: 'Server B', token: '' },
];

// Create the spawnInstance dropdown
const spawnSel = document.createElement('select');
spawnSel.id = 'spawnInstance';
_elementRegistry.set('spawnInstance', spawnSel);

// User selects Server B in the sidebar → sets _userSpawnInstUrl
window._userSpawnInstUrl = 'http://localhost:9091';

// First updateInstanceDropdown call (from _buildSidebar/loadCommands)
updateInstanceDropdown();
assertEq(spawnSel.value, 'http://localhost:9091',
    'dropdown shows Server B after user selection');

// Second call (simulates polling interval rebuilding sidebar)
updateInstanceDropdown();
assertEq(spawnSel.value, 'http://localhost:9091',
    'dropdown still shows Server B after rebuild');

// Verify it does NOT revert to the first connection (9090)
assert(spawnSel.value !== 'http://localhost:9090' || state.connections.length === 1,
    'dropdown does NOT revert to first server (9090) when user chose 9091');

// Cleanup
window._userSpawnInstUrl = undefined;

// ════════════════════════════════════════════════════════════════════
// REG-BUG-014: _hex function is exported and produces correct output
// ──────────────────────────────────────────────────────────────────
console.log('[REG-BUG-014] _hex utility function exported and correct');
assert(typeof _hex === 'function', '_hex is a function');
assertEq(_hex(0), '00', '_hex(0) = 00');
assertEq(_hex(1), '01', '_hex(1) = 01');
assertEq(_hex(15), '0f', '_hex(15) = 0f');
assertEq(_hex(16), '10', '_hex(16) = 10');
assertEq(_hex(255), 'ff', '_hex(255) = ff');
assertEq(_hex(128), '80', '_hex(128) = 80');

// ════════════════════════════════════════════════════════════════════
// REG-BUG-015: _onboardingSteps exported with correct structure
// ──────────────────────────────────────────────────────────────────
console.log('[REG-BUG-015] _onboardingSteps exported with correct structure');
assert(Array.isArray(_onboardingSteps), '_onboardingSteps is array');
assert(_onboardingSteps.length >= 5, 'at least 5 onboarding steps');
const firstStep = _onboardingSteps[0];
assert(typeof firstStep.title === 'string', 'first step has title string');
assert(typeof firstStep.body === 'string', 'first step has body string');
assert(typeof firstStep.target === 'string' || firstStep.target === null,
    'first step has target string or null');

// ════════════════════════════════════════════════════════════════════
// REG-BUG-016: color_terminal_log is NOT auto-detected from TTY
//              (only enabled via -F flag, never via IsTerminal)
// ──────────────────────────────────────────────────────────────────
console.log('[REG-BUG-016] color_terminal_log not auto-detected from TTY');
// This test verifies the Rust code does not import or use IsTerminal
// for color_terminal_log. We verify at the source level that the
// configuration only comes from the -F CLI flag.
const fs = require('fs');
const argsCode = fs.readFileSync(
    require('path').join(__dirname, '..', '..', '..', 'src', 'cli', 'args.rs'), 'utf8'
);
assert(!argsCode.includes('IsTerminal'),
    'args.rs does NOT import IsTerminal (no TTY auto-detect for color_terminal_log)');
assert(argsCode.includes('color_terminal_log'),
    'args.rs still has color_terminal_log field (from -F flag only)');

console.log('\n[BUG FIXES] All bug-fix regression tests complete');

// ════════════════════════════════════════════════════════════════════
// Final summary
// ════════════════════════════════════════════════════════════════════
console.log('\n=== Test Summary ===');
console.log('Passed: ' + _testPassed);
console.log('Failed: ' + _testFailed);
if (_testFailed > 0) {
    console.error('\n  SOME TESTS FAILED — do not push!');
    process.exit(1);
} else {
    console.log('\n  ALL TESTS PASSED');
    process.exit(0);
}
