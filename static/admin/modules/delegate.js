/// delegate.js — Event delegation system.
/// Replaces inline onclick handlers with data-action attributes + delegated listeners.
/// Set up ONCE at init on stable container elements. Never rebuilt.
///
/// Usage in HTML:
///   <button data-action="ToggleSidebar">☰</button>
///   <button data-action="ChangeFontSize" data-delta="-1">A-</button>
///
/// The dispatcher reads data-action, looks up the handler, and calls it.
/// Handlers are resolved lazily from window.* at dispatch time, so modules
/// can register them in any load order as long as they set window.<name>.
///
/// For actions needing arguments, use data-* attributes:
///   <button data-action="FreezeCommand" data-inst-url="..." data-cmd-id="...">⏸</button>
///   <button data-action="ChangeFontSize" data-delta="-1">A-</button>
///   <button data-action="SwitchSidebarTab" data-tab="spawn">Spawn</button>
///   <button data-action="ApplyLayoutPreset" data-preset="grid-2x2">2x2</button>
///
/// Special cases:
///   data-action-placeholder="..." — element exists in HTML but action not yet migrated; ignored safely.

'use strict';

// ── Argument builder signatures ──
// Each signature is a function(el, event, panelId) that returns an args array.
// The handler is then called as: handler(...args).
const _sigs = {
    // No arguments
    'none':          function()    { return []; },

    // Pass the original event
    'event':         function(el, ev) { return [ev]; },

    // Pass (tab, element) for sidebar tabs
    'tab-el':        function(el) { return [el.dataset.tab, el]; },

    // Pass panelId (from getActivePanelId())
    'panelId':       function(el, ev, pid) { return [pid]; },

    // Pass (panelId, delta) for font size changes
    'panelId-delta': function(el, ev, pid) {
        return [pid, el.dataset.delta !== undefined ? parseInt(el.dataset.delta, 10) : undefined];
    },

    // Pass data-preset
    'preset':        function(el) { return [el.dataset.preset]; },

    // Pass data-delta
    'delta':         function(el) {
        return [el.dataset.delta !== undefined ? parseInt(el.dataset.delta, 10) : 0];
    },

    // Pass (panelId, element.value) for buffer switch
    'panelId-value': function(el, ev, pid) {
        return [pid, el.value || el.dataset.value || 'current'];
    },

    // Pass element.value for select change handlers
    'value':         function(el) { return [el.value || el.dataset.value]; },

    // Pass (instUrl, cmdId, cmdName) from data-* attributes on command items
    'cmd-select':    function(el) {
        return [el.dataset.instUrl, el.dataset.cmdId, el.dataset.cmdName];
    },

    // Pass (instUrl, cmdId) from data-* attributes
    'cmd-id':        function(el) {
        return [el.dataset.instUrl, el.dataset.cmdId];
    },

    // Pass single string from data-value attribute
    'data-value':    function(el) {
        return [el.dataset.value];
    },

    // Pass (value) from data-window attribute (window management)
    'data-window':    function(el) {
        return [el.dataset.window];
    },

    // Pass (panelId) from data-panel-id attribute (for dynamically generated panel buttons)
    'el-panelId':    function(el) {
        return [el.dataset.panelId];
    },

    // Pass the element itself
    'element':       function(el) { return [el]; },

    // Pass (instUrl) from data-inst-url attribute
    'inst-url':      function(el) { return [el.dataset.instUrl]; },

    // Pass (cmdName) from data-cmd-name attribute
    'cmd-name':      function(el) { return [el.dataset.cmdName]; },

    // Pass (index) from data-index attribute (numeric)
    'index':         function(el) {
        return [el.dataset.index !== undefined ? parseInt(el.dataset.index, 10) : -1];
    },

    // Pass (value) from data-value attribute (string)
    'value-str':     function(el) { return [el.dataset.value]; },

    // Pass (name) from data-name attribute (string)
    'name':          function(el) { return [el.dataset.name]; },

    // Pass (name, cmdName) for group+cmd operations
    'name-index':    function(el) {
        return [el.dataset.name, el.dataset.cmdName || ''];
    },

    // Pass (instUrl, cmdId, cmdName, alive, retained) for context menu
    'cmd-context':   function(el) {
        return [el.dataset.instUrl, el.dataset.cmdId, el.dataset.cmdName,
            el.dataset.cmdAlive === 'true', el.dataset.cmdRetained === 'true'];
    },

    // Pass (panelId) from data-panel attribute (for dynamically rendered panel elements)
    'data-panel':    function(el) { return [el.dataset.panel]; },

    // Pass (panelId, leafId) from data-panel and data-leaf attributes
    'data-panel+leaf': function(el) { return [el.dataset.panel, el.dataset.leaf]; },
};

// ── Action registry ──
// Maps action name → { handler: 'windowFuncName', sig: 'signatureKey' }
// If sig is omitted, defaults to 'none' (no args).
const _actions = {
    // ── Topbar ──
    'ToggleSidebar':            { handler: 'toggleSidebar' },
    'NavigatePrevCommand':      { handler: 'navigatePrevCommand' },
    'NavigateNextCommand':      { handler: 'navigateNextCommand' },
    'OpenGlobalSearch':         { handler: 'openGlobalSearch' },
    'ToggleGlobalTheme':        { handler: 'toggleGlobalTheme' },
    'ToggleSoundNotifications': { handler: 'toggleSoundNotifications' },
    'ToggleLogsView':           { handler: 'toggleLogsView' },
    'ToggleBottombar':          { handler: 'toggleBottombar' },
    'SaveToken':                { handler: 'saveToken' },
    'ShowDocs':                 { handler: 'showDocs' },
    'ShowShortcuts':            { handler: 'showShortcuts' },

    // ── Sidebar ──
    'SwitchSidebarTab':         { handler: 'switchSidebarTab', sig: 'tab-el' },
    'ShowAddServerModal':       { handler: 'showAddServerModal' },
    'KillAllCommands':          { handler: 'killAllCommands' },
    'FreezeAllCommands':        { handler: 'freezeAllCommands' },
    'LoadCommands':             { handler: 'loadCommands' },
    'SpawnCommand':             { handler: 'spawnCommand' },
    'SpawnOnServer':             { handler: '_spawnOnServer', sig: 'inst-url' },
    'ShowSpawnModal':            { handler: '_showSpawnModal' },
    'AutofitTerminalSize':      { handler: 'autofitTerminalSize' },
    'ShowAddTemplateForm':      { handler: 'showAddTemplateForm' },
    'SaveTemplate':             { handler: 'saveTemplate' },
    'HideAddTemplateForm':      { handler: 'hideAddTemplateForm' },
    'CreateCmdGroup':           { handler: 'createCmdGroup' },
    'RenderCmdManagerList':     { handler: 'renderCmdManagerList' },

    // ── Sidebar: dynamic command list ──
    'DisconnectServer':         { handler: 'disconnectServer', sig: 'inst-url', stop: true },
    'FreezeThawServer':         { handler: '_freezeThawServer', sig: 'inst-url', stop: true },
    'SortSidebarBy':            { handler: '_sortSidebarBy', sig: 'data-value' },
    'ToggleKeepCmd':            { handler: 'toggleKeepCmd', sig: 'cmd-id', stop: true },
    'TogglePauseRunByIdx':      { handler: 'togglePauseRunPanelByIdx', sig: 'cmd-id', stop: true },
    'TogglePinCmd':             { handler: 'togglePinCmd', sig: 'cmd-name', stop: true },
    'SelectCommand':            { handler: 'selectCommand', sig: 'cmd-select' },

    // ── Sidebar: command context menu ──
    'ShowCmdContextMenu':       { handler: 'showCmdContextMenu', sig: 'cmd-context' },

    // ── Panels: dynamic ──
    'ClosePanelContent':        { handler: 'closePanelContent', sig: 'data-panel', stop: true },
    'PanelHistoryBack':         { handler: 'panelHistoryBack', sig: 'data-panel', stop: true },
    'PanelHistoryForward':      { handler: 'panelHistoryForward', sig: 'data-panel', stop: true },
    'StartRenamePanel':         { handler: 'startRenamePanel', sig: 'data-panel', stop: true },
    'UnsplitPanel':             { handler: 'unsplitPanel', sig: 'data-panel', stop: true },
    'UnsplitLeaf':              { handler: 'unsplitLeaf', sig: 'data-panel+leaf', stop: true },
    'ToggleMinimizePanel':      { handler: 'toggleMinimizePanel', sig: 'data-panel' },
    'FocusPanel':               { handler: 'focusPanel', sig: 'data-panel' },
    'ScrollTerminalBottom':     { handler: 'scrollTerminalBottom', sig: 'data-panel' },
    'VttySearchNext':           { handler: 'vttySearchNext', sig: 'data-panel' },
    'VttySearchPrev':           { handler: 'vttySearchPrev', sig: 'data-panel' },
    'VttySearchClose':          { handler: 'vttySearchClose', sig: 'data-panel' },

    // ── Special keys modal ──
    'CloseSpecialKeysModal':    { handler: 'closeSpecialKeysModal' },

    // ── Search: dynamic ──
    'RestartCommandById':       { handler: 'restartCommandById', sig: 'cmd-id', stop: true },
    'KillCommand':              { handler: 'killCommand', sig: 'cmd-id', stop: true },
    'SelectAndViewCmd':         { handler: '_selectAndViewCmd', sig: 'cmd-select', stop: true },
    'OnSearchResultClick':      { handler: 'onSearchResultClick', sig: 'cmd-select' },

    // ── Templates: dynamic ──
    'SpawnServerTemplate':      { handler: 'spawnServerTemplate', sig: 'index' },
    'SpawnUserTemplate':        { handler: 'spawnUserTemplate', sig: 'index' },
    'DeleteUserTemplate':       { handler: 'deleteUserTemplate', sig: 'index', stop: true },

    // ── Workspaces: dynamic ──
    'ActivateEnvironment':      { handler: 'activateEnvironment', sig: 'name' },
    'ToggleGroupCollapse':      { handler: 'toggleGroupCollapse', sig: 'name' },
    'RenameCmdGroup':           { handler: 'renameCmdGroup', sig: 'name', stop: true },
    'DeleteCmdGroup':           { handler: 'deleteCmdGroup', sig: 'name', stop: true },
    'ToggleCmdInGroup':         { handler: '_toggleCmdInGroupAndRender', sig: 'name-index', stop: true },
    'LoadWorkspace':            { handler: 'loadWorkspace', sig: 'name' },
    'DeleteWorkspace':          { handler: 'deleteWorkspace', sig: 'name', stop: true },

    // ── Command picker ──
    'CloseCmdPicker':           { handler: 'closeCmdPicker' },
    'PickCommand':              { handler: 'pickCommand', sig: 'cmd-select' },
    'CloseWorkspaceManage':     { handler: 'closeWorkspaceManage' },
    'CloseSpawnModal':          { handler: '_closeSpawnModal' },
    'ShowAddServerModal':       { handler: 'showAddServerModal', stop: true },

    // ── Shared toolbar ──
    'RestartCommand':           { handler: 'restartCommand', sig: 'panelId' },
    'ToggleResources':          { handler: 'toggleResources' },
    'ChangePanelFontSize':      { handler: 'changePanelFontSize', sig: 'panelId-delta' },
    'ResizeTerminalPanel':      { handler: 'resizeTerminalPanel', sig: 'panelId' },
    'ToggleMaxFit':             { handler: 'toggleMaxFit', sig: 'panelId' },
    'ToggleMaxFont':            { handler: 'toggleMaxFont', sig: 'panelId' },
    'SwitchBufferPanel':        { handler: 'switchBufferPanel', sig: 'panelId-value' },
    'ChangeRefreshMs':          { handler: 'changeRefreshMs', sig: 'delta' },
    'SendKeysToPanel':          { handler: 'sendKeysToPanel', sig: 'panelId' },
    'ShowSpecialKeysHelp':      { handler: 'showSpecialKeysHelp' },
    'TogglePanelLayout':        { handler: 'togglePanelLayout' },
    'ToggleLayoutPresetMenu':   { handler: 'toggleLayoutPresetMenu', sig: 'event' },
    'ApplyLayoutPreset':        { handler: 'applyLayoutPreset', sig: 'preset' },
    'ToggleSelectionMode':      { handler: 'toggleSelectionMode', sig: 'panelId' },
    'TogglePauseRunPanel':      { handler: 'togglePauseRunPanel', sig: 'panelId' },
    'CopyTerminalSelection':    { handler: 'copyTerminalSelection', sig: 'panelId' },
    'ExportTerminal':           { handler: 'exportTerminal', sig: 'panelId' },
    'ScreenshotPanel':          { handler: 'screenshotPanel', sig: 'panelId' },
    'TogglePanelTheme':         { handler: 'togglePanelTheme', sig: 'panelId' },
    'ToggleBufferDropdown':     { handler: null, sig: '__builtin__' },  // handled inline

    // ── Log viewer ──
    'SearchLogs':               { handler: 'searchLogs' },
    'ClearLogSearch':           { handler: 'clearLogSearch' },
    'LoadLog':                  { handler: 'loadLog' },

    // ── Global search modal ──
    'ExecuteGlobalSearch':      { handler: 'executeGlobalSearch' },
    'CloseGlobalSearch':        { handler: 'closeGlobalSearch' },
    'ToggleSearchFreeze':       { handler: '_toggleSearchFreezeCommands' },

    // ── Add server modal ──
    'CloseAddServerModal':      { handler: 'closeAddServerModal' },
    'ConfirmAddServer':         { handler: 'confirmAddServer' },

    // ── Windows ──
    'SwitchWindow':             { handler: 'switchWindow', sig: 'data-window' },
    'CreateWindow':             { handler: 'createWindow' },
    'CloseWindow':              { handler: 'closeWindow', sig: 'data-window', stop: true },

    // ── Command manager modal ──
    'CloseCmdManager':          { handler: 'closeCmdManager' },
    'CmdManagerKillAll':        { handler: 'cmdManagerKillAll' },

    // ── Bottombar ──
    'SwitchUpdateMode':         { handler: 'switchUpdateMode', sig: 'value' },
    'ApplyPollInterval':        { handler: 'applyPollInterval' },
    'ApplyRefreshMs':           { handler: 'applyRefreshMs' },
};

// Backward-compatible flat map: action name → handler function name
// Used by tests and for quick lookups
const _actionMap = {};
for (const [action, def] of Object.entries(_actions)) {
    _actionMap[action] = def.handler;
}

// ── Dispatcher ──
// Finds the [data-action] element, looks up the handler, and calls it.
// Returns true if an action was dispatched, false otherwise.
function _dispatchAction(event) {
    const el = event.target.closest('[data-action]');
    if (!el) return false;

    const action = el.dataset.action;
    if (!action || action === '') return false;

    // data-action-placeholder means "this will be migrated later" — ignore safely
    if (el.hasAttribute('data-action-placeholder')) return false;

    const def = _actions[action];
    if (!def) return false;

    // Built-in handlers (no window function)
    if (def.sig === '__builtin__') {
        if (action === 'ToggleBufferDropdown') {
            const select = document.getElementById('stBufferSelect');
            if (select) select.classList.toggle('hidden');
            return true;
        }
        return false;
    }

    const handler = window[def.handler];
    if (typeof handler !== 'function') {
        console.warn('[delegate] handler not found for action "' + action + '" → window.' + def.handler);
        return false;
    }

    // Get active panel ID once (used by 'panelId*' signatures)
    const panelId = (def.sig && def.sig.startsWith('panelId') && typeof window.getActivePanelId === 'function')
        ? window.getActivePanelId()
        : null;

    // Build args using the signature
    const sigKey = def.sig || 'none';
    const sigFn = _sigs[sigKey] || _sigs['none'];
    const args = sigFn(el, event, panelId);

    // If this action has stop:true, prevent event from bubbling to parent data-action
    if (def.stop) event.stopPropagation();

    handler.apply(null, args);
    return true;
}

// ── Close-on-backdrop pattern ──
// For modals: clicking the overlay (not the modal content) closes the modal.
function _dispatchModalBackdrop(event) {
    // Only fire if clicking directly on the overlay, not its children
    if (event.target !== event.currentTarget) return false;

    const closeAction = event.currentTarget.dataset.closeAction;
    if (!closeAction) return false;

    const def = _actions[closeAction];
    const handlerName = def ? def.handler : closeAction;
    const handler = window[handlerName];
    if (typeof handler === 'function') {
        handler();
        return true;
    }
    return false;
}

// ── Init ──
// Call once at startup. Attaches delegation listeners to stable containers.
function initDelegation() {
    // Click delegation on the whole document
    // This catches ALL [data-action] clicks regardless of where they are
    // Skip <select> and <input> — they dispatch on 'change', not 'click'
    document.addEventListener('click', function(event) {
        const tag = event.target && event.target.tagName;
        if (tag === 'SELECT' || tag === 'INPUT') return;
        _dispatchAction(event);
    });

    // Change delegation for <select> and <input> elements with data-action
    // These fire 'change' events (not 'click') when the user selects a new value
    document.addEventListener('change', function(event) {
        const el = event.target;
        if (!el || !el.dataset || !el.dataset.action) return;
        _dispatchAction(event);
    });

    // Contextmenu delegation for [data-action] elements
    document.addEventListener('contextmenu', function(event) {
        const el = event.target.closest('[data-action]');
        if (!el) return;
        const action = el.dataset.action;
        const def = _actions[action];
        if (!def) return;
        // Only dispatch contextmenu for actions that explicitly handle it
        if (action !== 'ShowCmdContextMenu' && action !== 'ShowPanelContextMenu') return;
        event.preventDefault();
        const handler = window[def.handler];
        if (typeof handler !== 'function') return;
        const sigKey = def.sig || 'none';
        const sigFn = _sigs[sigKey] || _sigs['none'];
        const args = sigFn(el, event, null);
        handler.apply(null, args);
    });

    // Modal backdrop close: each modal overlay has data-close-action
    const modals = document.querySelectorAll('[data-close-action]');
    for (const modal of modals) {
        modal.addEventListener('click', _dispatchModalBackdrop);
    }
}

// ── Exports ──
window.initDelegation = initDelegation;
window._actionMap = _actionMap;
window._actions = _actions;
window._dispatchAction = _dispatchAction;
window._dispatchModalBackdrop = _dispatchModalBackdrop;
window._sigs = _sigs;