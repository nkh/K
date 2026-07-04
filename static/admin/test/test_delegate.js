/// test_delegate.js — Tests for event delegation system (delegate.js).
'use strict';

const { createMockEvent } = require('./helpers');
require('./setup');

console.log('\n=== delegate.js ===\n');

// ── Module structure ──
assertType(window._actionMap, 'object', '_actionMap is an object');
assertType(window._actions, 'object', '_actions registry is an object');
assertType(window._sigs, 'object', '_sigs (signatures) is an object');
assertType(window.initDelegation, 'function', 'initDelegation is a function');
assertType(window._dispatchAction, 'function', '_dispatchAction is a function');
assertType(window._dispatchModalBackdrop, 'function', '_dispatchModalBackdrop is a function');

// ── Action map completeness ──
const expectedActions = [
    'ToggleSidebar', 'NavigatePrevCommand', 'NavigateNextCommand',
    'OpenGlobalSearch', 'ToggleGlobalTheme', 'ToggleSoundNotifications',
    'ToggleLogsView', 'ToggleBottombar', 'SaveToken', 'ShowDocs', 'ShowShortcuts',
    'SwitchSidebarTab', 'ShowAddServerModal', 'KillAllCommands', 'FreezeAllCommands',
    'LoadCommands', 'SpawnCommand', 'AutofitTerminalSize',
    'ShowAddTemplateForm', 'SaveTemplate', 'HideAddTemplateForm',
    'CreateCmdGroup', 'RenderCmdManagerList',
    // Phase 3: dynamic onclick migrations
    'DisconnectServer', 'SortSidebarBy', 'ToggleKeepCmd', 'TogglePauseRunByIdx',
    'TogglePinCmd', 'SelectCommand', 'ShowCmdContextMenu',
    'ClosePanelContent', 'PanelHistoryBack', 'PanelHistoryForward',
    'StartRenamePanel', 'SplitPaneHorizontal', 'SplitPaneVertical', 'UnsplitPanel', 'UnsplitLeaf', 'UnsplitPane', 'ToggleMinimizePanel', 'FocusPanel',
    'ScrollTerminalBottom', 'VttySearchNext', 'VttySearchPrev', 'VttySearchClose',
    'CloseSpecialKeysModal',
    'RestartCommandById', 'KillCommand', 'SelectAndViewCmd', 'OnSearchResultClick',
    'SpawnServerTemplate', 'SpawnUserTemplate', 'DeleteUserTemplate',
    'ActivateEnvironment', 'ToggleGroupCollapse', 'RenameCmdGroup', 'DeleteCmdGroup',
    'ToggleCmdInGroup', 'LoadWorkspace', 'DeleteWorkspace',
    'CloseCmdPicker', 'PickCommand', 'CloseWorkspaceManage',
    'FreezeThawServer', 'SpawnOnServer', 'ShowSpawnModal', 'CloseSpawnModal',
    // Shared toolbar
    'RestartCommand', 'ToggleResources', 'ChangePanelFontSize',
    'ResizeTerminalPanel', 'ToggleMaxFit', 'ToggleMaxFont',
    'SwitchBufferPanel', 'ChangeRefreshMs', 'SendKeysToPanel',
    'ShowSpecialKeysHelp', 'TogglePanelLayout', 'ToggleLayoutPresetMenu',
    'ApplyLayoutPreset', 'ToggleSelectionMode', 'TogglePauseRunPanel', 'TogglePauseRunLeaf',
    'CopyTerminalSelection', 'ExportTerminal', 'ScreenshotPanel',
    'TogglePanelTheme', 'ToggleBufferDropdown',
    'SearchLogs', 'ClearLogSearch', 'LoadLog',
    'ExecuteGlobalSearch', 'CloseGlobalSearch', 'ToggleSearchFreeze',
    'CloseAddServerModal', 'ConfirmAddServer',
    'CloseCmdManager', 'CmdManagerKillAll',
    'SwitchUpdateMode', 'ApplyPollInterval', 'ApplyRefreshMs',
    // Window management
    'SwitchWindow', 'CreateWindow', 'CloseWindow',
];

for (const action of expectedActions) {
    assertProperty(window._actions, action, 'actions registry has ' + action);
}
assertEq(Object.keys(window._actions).length, expectedActions.length,
    'action count matches expected (' + expectedActions.length + ')');
console.log('  (' + Object.keys(window._actions).length + ' actions registered)');

// ── Signature registry ──
const expectedSigs = ['none', 'event', 'tab-el', 'panelId', 'panelId-delta', 'preset', 'delta', 'panelId-value', 'value',
    'cmd-select', 'cmd-id', 'data-value', 'el-panelId', 'element',
    'inst-url', 'cmd-name', 'index', 'value-str', 'name', 'name-index', 'cmd-context', 'data-panel',
    'data-panel+leaf', 'data-window', 'focusedLeaf'];
for (const sig of expectedSigs) {
    assertProperty(window._sigs, sig, 'sigs has ' + sig);
    assertType(window._sigs[sig], 'function', 'sigs.' + sig + ' is a function');
}

// ── _dispatchAction basics ──

// 1. Returns false for element without data-action
{
    const div = document.createElement('div');
    const ev = createMockEvent({ target: div });
    assertEq(_dispatchAction(ev), false, 'no data-action → returns false');
}

// 2. Returns false for data-action-placeholder
{
    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'Foo');
    btn.setAttribute('data-action-placeholder', '');
    const ev = createMockEvent({ target: btn });
    assertEq(_dispatchAction(ev), false, 'data-action-placeholder → returns false');
}

// 3. Returns false for unknown action
{
    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'NonExistentAction');
    const ev = createMockEvent({ target: btn });
    assertEq(_dispatchAction(ev), false, 'unknown action → returns false');
}

// 4. Warns and returns false for action with no window handler
{
    const originalWarn = console.warn;
    let warnCalled = false;
    console.warn = function() { warnCalled = true; };
    // Temporarily add a fake action pointing to a non-existent window function
    window._actions._testMissing = { handler: '_nonExistentWindowFn' };
    window._actionMap._testMissing = '_nonExistentWindowFn';
    const btn = document.createElement('button');
    btn.setAttribute('data-action', '_testMissing');
    const ev = createMockEvent({ target: btn });
    assertEq(_dispatchAction(ev), false, 'missing window handler → returns false');
    assert(warnCalled, 'missing handler triggers console.warn');
    console.warn = originalWarn;
    delete window._actions._testMissing;
    delete window._actionMap._testMissing;
}

// 5. Calls a no-arg handler (sig: 'none')
{
    let called = false;
    const saved = window.toggleSidebar;
    window.toggleSidebar = function() { called = true; };
    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'ToggleSidebar');
    const ev = createMockEvent({ target: btn });
    assertEq(_dispatchAction(ev), true, 'no-arg handler dispatched');
    assert(called, 'no-arg handler was called');
    window.toggleSidebar = saved;
}

// 6. Calls a handler with panelId (sig: 'panelId')
{
    let receivedPanelId = null;
    const savedHandler = window.restartCommand;
    const savedGetActivePanelId = window.getActivePanelId;
    window.restartCommand = function(pid) { receivedPanelId = pid; };
    window.getActivePanelId = function() { return 'panel-1'; };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'RestartCommand');
    const ev = createMockEvent({ target: btn });
    _dispatchAction(ev);
    assertEq(receivedPanelId, 'panel-1', 'panelId handler receives active panel id');

    window.restartCommand = savedHandler;
    window.getActivePanelId = savedGetActivePanelId;
}

// 7. Calls a handler with (panelId, delta) (sig: 'panelId-delta')
{
    let receivedArgs = null;
    const savedHandler = window.changePanelFontSize;
    const savedGetActivePanelId = window.getActivePanelId;
    window.changePanelFontSize = function(pid, delta) { receivedArgs = [pid, delta]; };
    window.getActivePanelId = function() { return 'p2'; };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'ChangePanelFontSize');
    btn.setAttribute('data-delta', '-1');
    const ev = createMockEvent({ target: btn });
    _dispatchAction(ev);
    assertDeepEq(receivedArgs, ['p2', -1], 'panelId-delta handler receives panel id + parsed delta');

    window.changePanelFontSize = savedHandler;
    window.getActivePanelId = savedGetActivePanelId;
}

// 8. Calls a handler with data-preset (sig: 'preset')
{
    let receivedPreset = null;
    const savedHandler = window.applyLayoutPreset;
    window.applyLayoutPreset = function(p) { receivedPreset = p; };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'ApplyLayoutPreset');
    btn.setAttribute('data-preset', 'grid-2x2');
    const ev = createMockEvent({ target: btn });
    _dispatchAction(ev);
    assertEq(receivedPreset, 'grid-2x2', 'preset handler receives data-preset');

    window.applyLayoutPreset = savedHandler;
}

// 9. Calls a handler with event (sig: 'event')
{
    let receivedEvent = null;
    const savedHandler = window.toggleLayoutPresetMenu;
    window.toggleLayoutPresetMenu = function(e) { receivedEvent = e; };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'ToggleLayoutPresetMenu');
    const ev = createMockEvent({ target: btn, type: 'click' });
    _dispatchAction(ev);
    assert(receivedEvent === ev, 'event handler receives the original event');

    window.toggleLayoutPresetMenu = savedHandler;
}

// 10. Calls SwitchSidebarTab with (tab, element) (sig: 'tab-el')
{
    let receivedArgs = null;
    const savedHandler = window.switchSidebarTab;
    window.switchSidebarTab = function(tab, el) { receivedArgs = [tab, el]; };

    const div = document.createElement('div');
    div.setAttribute('data-action', 'SwitchSidebarTab');
    div.setAttribute('data-tab', 'templates');
    const ev = createMockEvent({ target: div });
    _dispatchAction(ev);
    assertDeepEq(receivedArgs, ['templates', div], 'tab-el handler receives (tab, element)');

    window.switchSidebarTab = savedHandler;
}

// 11. ToggleBufferDropdown (builtin handler) toggles select visibility
{
    const select = document.getElementById('stBufferSelect');
    select.classList.add('hidden');

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'ToggleBufferDropdown');
    const ev = createMockEvent({ target: btn });
    _dispatchAction(ev);
    assert(!select.classList.contains('hidden'), 'buffer dropdown: toggles from none to visible');

    _dispatchAction(ev);
    assert(select.classList.contains('hidden'), 'buffer dropdown: toggles back to none');

    select.classList.remove('hidden');
}

// 12. closest() traversal — action on child element finds parent [data-action]
{
    let called = false;
    const savedHandler = window.toggleSidebar;
    window.toggleSidebar = function() { called = true; };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'ToggleSidebar');
    const span = document.createElement('span');
    const icon = document.createElement('i');
    span.appendChild(icon);
    btn.appendChild(span);

    // Click on the <i> inside the button
    const ev = createMockEvent({ target: icon });
    _dispatchAction(ev);
    assert(called, 'closest() finds data-action on ancestor button');

    window.toggleSidebar = savedHandler;
}

// 13. delta signature (ChangeRefreshMs pattern)
{
    let receivedDelta = null;
    const savedHandler = window.changeRefreshMs;
    window.changeRefreshMs = function(d) { receivedDelta = d; };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'ChangeRefreshMs');
    btn.setAttribute('data-delta', '100');
    const ev = createMockEvent({ target: btn });
    _dispatchAction(ev);
    assertEq(receivedDelta, 100, 'delta handler receives parsed delta');

    // No data-delta defaults to 0
    const btn2 = document.createElement('button');
    btn2.setAttribute('data-action', 'ChangeRefreshMs');
    const ev2 = createMockEvent({ target: btn2 });
    _dispatchAction(ev2);
    assertEq(receivedDelta, 0, 'delta handler defaults to 0 when no data-delta');

    window.changeRefreshMs = savedHandler;
}

// 14. value signature (SwitchUpdateMode pattern)
{
    let receivedValue = null;
    const savedHandler = window.switchUpdateMode;
    window.switchUpdateMode = function(v) { receivedValue = v; };

    const select = document.createElement('select');
    select.setAttribute('data-action', 'SwitchUpdateMode');
    select.value = 'poll';
    const ev = createMockEvent({ target: select });
    _dispatchAction(ev);
    assertEq(receivedValue, 'poll', 'value handler receives element.value');

    window.switchUpdateMode = savedHandler;
}

// 15. panelId-value signature (SwitchBufferPanel pattern)
{
    let receivedArgs = null;
    const savedHandler = window.switchBufferPanel;
    const savedGetActivePanelId = window.getActivePanelId;
    window.switchBufferPanel = function(pid, val) { receivedArgs = [pid, val]; };
    window.getActivePanelId = function() { return 'p3'; };

    const select = document.createElement('select');
    select.setAttribute('data-action', 'SwitchBufferPanel');
    select.value = 'alt';
    const ev = createMockEvent({ target: select });
    _dispatchAction(ev);
    assertDeepEq(receivedArgs, ['p3', 'alt'], 'panelId-value handler receives (panelId, value)');

    window.switchBufferPanel = savedHandler;
    window.getActivePanelId = savedGetActivePanelId;
}

// ── initDelegation ──
// 16. initDelegation sets up click delegation — verify by dispatching through it
{
    initDelegation();

    let called = false;
    const savedHandler = window.toggleSidebar;
    window.toggleSidebar = function() { called = true; };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'ToggleSidebar');
    document.body.appendChild(btn);

    // Dispatch through globalThis.dispatchEvent (fires global addEventListener handlers)
    const ev = createMockEvent({ target: btn, type: 'click' });
    globalThis.dispatchEvent(ev);

    assert(called, 'click on [data-action] fires handler through delegation');

    document.body.removeChild(btn);
    window.toggleSidebar = savedHandler;
}

// ── Modal backdrop dispatch ──
// 17. _dispatchModalBackdrop: clicking overlay directly (target === currentTarget) closes
{
    let called = false;
    const savedClose = window.closeAddServerModal;
    window.closeAddServerModal = function() { called = true; };

    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.setAttribute('data-close-action', 'CloseAddServerModal');

    // Click the overlay directly (target === currentTarget)
    const ev1 = createMockEvent({ target: overlay, currentTarget: overlay });
    const result1 = _dispatchModalBackdrop(ev1);
    assertEq(result1, true, 'backdrop click dispatches close action');
    assert(called, 'close modal handler was called');

    // Clicking a child should NOT close
    called = false;
    const child = document.createElement('div');
    const ev2 = createMockEvent({ target: child, currentTarget: overlay });
    const result2 = _dispatchModalBackdrop(ev2);
    assertEq(result2, false, 'child click does not close modal');
    assert(!called, 'close handler not called for child click');

    window.closeAddServerModal = savedClose;
}

// 18. _dispatchModalBackdrop: uses fallback handler name when action not in registry
{
    let called = false;
    window._testFallbackClose = function() { called = true; };

    const overlay = document.createElement('div');
    overlay.setAttribute('data-close-action', '_testFallbackClose');

    const ev = createMockEvent({ target: overlay, currentTarget: overlay });
    const result = _dispatchModalBackdrop(ev);
    assertEq(result, true, 'fallback handler name dispatched');
    assert(called, 'fallback handler was called');

    delete window._testFallbackClose;
}

// SpawnOnServer uses inst-url sig — passes data-inst-url to handler
{
    let receivedArgs = null;
    const savedHandler = window._spawnOnServer;
    window._spawnOnServer = function(instUrl) { receivedArgs = [instUrl]; };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'SpawnOnServer');
    btn.setAttribute('data-inst-url', 'http://prod:8080');
    const ev = createMockEvent({ target: btn });
    _dispatchAction(ev);
    assertDeepEq(receivedArgs, ['http://prod:8080'], 'SpawnOnServer passes inst-url from data attribute');

    window._spawnOnServer = savedHandler;
}

// ShowSpawnModal calls handler with no args
{
    let called = false;
    const savedHandler = window._showSpawnModal;
    window._showSpawnModal = function() { called = true; };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'ShowSpawnModal');
    const ev = createMockEvent({ target: btn });
    _dispatchAction(ev);
    assert(called, 'ShowSpawnModal dispatches to _showSpawnModal');

    window._showSpawnModal = savedHandler;
}

console.log('\n  [delegate] all dispatch, signatures, and modal backdrop verified');