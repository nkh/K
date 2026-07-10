#!/usr/bin/env python3
"""Extract app.js (8833 lines) into 12 modular files in modules/ directory."""
import os

SRC = '/home/z/K/static/admin/app.js'
DST = '/home/z/K/static/admin/modules'

with open(SRC, 'r') as f:
    lines = f.readlines()

def extract(start, end):
    """Extract lines (1-indexed, inclusive) as a string."""
    return ''.join(lines[start-1:end])

def write_module(name, content):
    """Write module file."""
    path = os.path.join(DST, name)
    with open(path, 'w') as f:
        f.write(content)
    print(f"  {name}: {len(content.splitlines())} lines")

os.makedirs(DST, exist_ok=True)

# ─── 1. state.js ───
# Lines 1-116: module-level vars + state object
write_module('state.js', extract(1, 116) + """
// Expose module-level vars to VRW namespace for cross-module access
window.VRW = window.VRW || {};
VRW.state = state;
VRW._lastCommandState = _lastCommandState;
VRW._navCommands = _navCommands;
VRW._showingWelcome = _showingWelcome;
VRW._sidebarSort = _sidebarSort;
VRW._searchFrozenPanelIds = _searchFrozenPanelIds;
VRW._searchFrozenCmdIds = _searchFrozenCmdIds;
VRW._lastRenderedPanelCount = _lastRenderedPanelCount;
VRW._lastRenderedPanelIds = _lastRenderedPanelIds;
VRW._lastSplitState = _lastSplitState;
VRW._lastShowingWelcome = _lastShowingWelcome;
""")

# ─── 2. eventbus.js ───
write_module('eventbus.js', """// ─── Event Bus ───
// Central event emitter for cross-module communication.
window.VRW = window.VRW || {};
VRW.EventBus = {
    _listeners: {},
    on(event, fn) { (this._listeners[event] = this._listeners[event] || []).push(fn); },
    off(event, fn) { if (this._listeners[event]) this._listeners[event] = this._listeners[event].filter(f => f !== fn); },
    emit(event, ...args) { (this._listeners[event] || []).forEach(fn => fn(...args)); },
    once(event, fn) {
        const wrapper = (...args) => { fn(...args); this.off(event, wrapper); };
        this.on(event, wrapper);
    }
};
""")

# ─── 3. utils.js ───
# Pure utility functions used everywhere. Must load before other modules.
utils_content = """// ─── Utilities ───
// Pure utility functions used across all modules.
(function() {
    'use strict';

"""
utils_content += extract(488, 497) + "\n"   # formatRuntime
utils_content += extract(499, 517) + "\n"   # getBaseUrl, authHeaders, authHeadersForInstance, apiUrl
utils_content += extract(5450, 5456) + "\n" # escHtml
utils_content += extract(2809, 2831) + "\n" # _hex, _htmlEscapeChar
utils_content += extract(3194, 3260) + "\n" # parseSpawnArgs, parseSpawnEnvVars
utils_content += """
    // Expose to global scope
    window.formatRuntime = formatRuntime;
    window.getBaseUrl = getBaseUrl;
    window.authHeaders = authHeaders;
    window.authHeadersForInstance = authHeadersForInstance;
    window.apiUrl = apiUrl;
    window.escHtml = escHtml;
    window.parseSpawnArgs = parseSpawnArgs;
    window.parseSpawnEnvVars = parseSpawnEnvVars;
})();
"""
write_module('utils.js', utils_content)

# ─── 4. theme.js ───
# Lines 118-159 (theme) + 622-653 (panel theme)
theme_content = """// ─── Theme ───
(function() {
    'use strict';
"""
theme_content += extract(118, 159) + "\n"  # initTheme, toggleGlobalTheme, updateThemeButton
theme_content += extract(622, 653) + "\n"  # togglePanelTheme, applyPanelTheme
theme_content += """
    window.initTheme = initTheme;
    window.toggleGlobalTheme = toggleGlobalTheme;
    window.updateThemeButton = updateThemeButton;
    window.togglePanelTheme = togglePanelTheme;
    window.applyPanelTheme = applyPanelTheme;
})();
"""
write_module('theme.js', theme_content)

# ─── 5. focus.js ───
# Lines 161-245
focus_content = """// ─── Focus Management ───
(function() {
    'use strict';
"""
focus_content += extract(161, 245) + "\n"
focus_content += """
    window.trapFocus = trapFocus;
    window.releaseCurrentFocusTrap = releaseCurrentFocusTrap;
})();
"""
write_module('focus.js', focus_content)

# ─── 6. sidebar.js ───
# Lines 679-810 (sidebar toggles, tab switching, disconnected state)
# Lines 1127-1347 (_buildSidebar)
# Lines 7301-7375 (command pinning)
sidebar_content = """// ─── Sidebar ───
(function() {
    'use strict';
"""
sidebar_content += extract(679, 810) + "\n"   # sidebar toggles, tabs, disconnected
sidebar_content += extract(1127, 1347) + "\n"  # _buildSidebar
sidebar_content += extract(7301, 7375) + "\n"  # pinning
sidebar_content += """
    window.toggleSidebar = toggleSidebar;
    window.switchSidebarTab = switchSidebarTab;
    window.updateDisconnectedUI = updateDisconnectedUI;
    window.updateSidebarBanner = updateSidebarBanner;
    window.updateTerminalDisconnectedOverlay = updateTerminalDisconnectedOverlay;
    window._buildSidebar = _buildSidebar;
    window.getPinnedNames = getPinnedNames;
    window.setPinnedNames = setPinnedNames;
    window.togglePinCmd = togglePinCmd;
    window.rearrangePinnedCommands = rearrangePinnedCommands;
})();
"""
write_module('sidebar.js', sidebar_content)

# ─── 7. panels.js ───
# Lines 3828-3915 (addPanelDirect, addPanel)
# Lines 4026-4051 (removePanel)
# Lines 4052-4069 (toggleMinimizePanel)
# Lines 4070-4477 (split panel)
# Lines 4478-4583 (renderPanels helpers, layout)
# Lines 4584-4845 (renderPanels - the big one)
# Lines 4846-5053 (focusPanel, updateSharedToolbar, sendKeysToPanel, showSpecialKeysHelp)
# Lines 6297-6334 (panel resize IIFE)
# Lines 6336-6700 (export, screenshot, context menu)
# Lines 6700-6907 (rename, autofit, max-fit, max-font, resize helper)
# Lines 7560-7638 (drag-and-drop panel reorder)
panels_content = """// ─── Panels ───
(function() {
    'use strict';
"""
panels_content += extract(3828, 3915) + "\n"  # addPanelDirect, addPanel
panels_content += extract(4026, 4069) + "\n"  # removePanel, toggleMinimizePanel
panels_content += extract(4070, 4477) + "\n"  # split panel functions
panels_content += extract(4478, 4845) + "\n"  # renderPanels and helpers
panels_content += extract(4846, 5053) + "\n"  # focusPanel, updateSharedToolbar, sendKeysToPanel, showSpecialKeysHelp
panels_content += extract(6297, 6907) + "\n"  # resize, export, screenshot, context menu, rename, max-fit, max-font
panels_content += extract(7560, 7638) + "\n"  # drag-and-drop panel reorder
panels_content += """
    // Expose all public functions
    const publics = ['addPanelDirect', 'addPanel', 'closePanelModal', 'confirmAddPanel',
        'removePanel', 'toggleMinimizePanel', 'splitPanel', 'unsplitPanel',
        'renderPanels', 'focusPanel', 'updateSharedToolbar', 'sendKeysToPanel',
        'showSpecialKeysHelp', 'getActivePanelId', 'getSelectedPanel',
        'togglePanelLayout', 'toggleLayoutPresetMenu', 'applyLayoutPreset',
        'copyTerminalSelection', 'exportTerminal', 'screenshotPanel',
        'closeContextMenu', 'showCmdContextMenu', 'showPanelContextMenu',
        'startRenamePanel', 'finishRenamePanel', 'copyCommandUrl',
        'togglePauseCmd', 'autoFitActiveTerminal', 'toggleMaxFit', 'toggleMaxFont',
        'onPanelDragStart', 'onPanelDragOver', 'onPanelDragLeave', 'onPanelDrop', 'onPanelDragEnd',
        'initPanelDropTargets'];
    for (const name of publics) {
        if (typeof window[name] === 'undefined' && typeof eval(name) === 'function') {
            window[name] = eval(name);
        }
    }
})();
"""
write_module('panels.js', panels_content)

# ─── 8. commands.js ───
# Lines 956-1126 (loadSnapshot - incomplete)
# Lines 971-987 (navigateCommand)
# Lines 1127-1347 (already in sidebar.js as _buildSidebar, skip)
# Lines 1348-1394 (sidebar rebuild optimization)
# Lines 1395-1503 (DOM caching, sidebar selection, panel history)
# Lines 1504-1627 (selectCommand internals)
# Lines 1628-1726 (updatePanelCommandInfo)
# Lines 1726-1800 (updateBottomBarLabel, autofitTerminalSize, getSelectedPanel, getActivePanelId)
# Lines 409-486 (lookupAndSelectCommand, showCommandPicker, pickCommand)
# Lines 1801-1836 (togglePauseRun, togglePauseRunPanel)
# Lines 1837-1978 (fetchServerConfig, update mode)
# Lines 1920-1977 (_isTerminalVisible, flush, start/stop update mode)
# Lines 3747-3746 (certificates)
# Lines 3916-4025 (addConnection, removeConnection, disconnectServer, showAddServerModal, etc)
# Lines 3959-4025 (add server modal)
commands_content = """// ─── Commands ───
(function() {
    'use strict';
"""
commands_content += extract(409, 486) + "\n"   # lookupAndSelectCommand, showCommandPicker, pickCommand
commands_content += extract(956, 970) + "\n"   # loadSnapshot (start)
commands_content += extract(971, 987) + "\n"   # navigateCommand
commands_content += extract(1348, 1503) + "\n" # sidebar rebuild, DOM caching, panel history
commands_content += extract(1504, 1627) + "\n" # selectCommand internals
commands_content += extract(1628, 1774) + "\n" # updatePanelCommandInfo, updateBottomBarLabel, autofitTerminalSize
commands_content += extract(1775, 1836) + "\n" # getSelectedPanel, getActivePanelId, togglePauseRun
commands_content += extract(1837, 1978) + "\n" # fetchServerConfig, update mode
commands_content += extract(1920, 1978) + "\n" # VTTY visibility helpers
commands_content += extract(3747, 3915) + "\n" # certificates, panels create
commands_content += extract(3916, 4025) + "\n" # connections
commands_content += extract(3959, 4025) + "\n" # add server modal
commands_content += extract(7229, 7271) + "\n" # restart command
commands_content += extract(7272, 7300) + "\n" # welcome panel spawn
commands_content += """
    window.lookupAndSelectCommand = lookupAndSelectCommand;
    window.showCommandPicker = showCommandPicker;
    window.pickCommand = pickCommand;
    window.loadSnapshot = loadSnapshot;
    window.navigateCommand = navigateCommand;
    window.navigatePrevCommand = navigatePrevCommand;
    window.navigateNextCommand = navigateNextCommand;
    window.loadCommands = loadCommands;
    window.selectCommand = selectCommand;
    window.updatePanelCommandInfo = updatePanelCommandInfo;
    window.updateBottomBarLabel = updateBottomBarLabel;
    window.autofitTerminalSize = autofitTerminalSize;
    window.getSelectedPanel = getSelectedPanel;
    window.getActivePanelId = getActivePanelId;
    window.togglePauseRun = togglePauseRun;
    window.togglePauseRunPanel = togglePauseRunPanel;
    window.fetchServerConfig = fetchServerConfig;
    window.applyUpdateModeUI = applyUpdateModeUI;
    window.switchUpdateMode = switchUpdateMode;
    window.applyPollInterval = applyPollInterval;
    window._isTerminalVisible = _isTerminalVisible;
    window._flushPendingVttyUpdate = _flushPendingVttyUpdate;
    window.startUpdateMode = startUpdateMode;
    window.startPanelUpdateMode = startPanelUpdateMode;
    window.stopUpdateMode = stopUpdateMode;
    window.stopPanelUpdateMode = stopPanelUpdateMode;
    window.loadCertificates = loadCertificates;
    window.updateCertDropdown = updateCertDropdown;
    window.updateInstanceDropdown = updateInstanceDropdown;
    window.addConnection = addConnection;
    window.removeConnection = removeConnection;
    window.disconnectServer = disconnectServer;
    window.showAddServerModal = showAddServerModal;
    window.closeAddServerModal = closeAddServerModal;
    window.confirmAddServer = confirmAddServer;
    window.restartCommand = restartCommand;
    window.restartCommandById = restartCommandById;
    window.spawnFromWelcome = spawnFromWelcome;
    window.updateSidebarSelection = updateSidebarSelection;
    window._cacheTerminalForSwitch = _cacheTerminalForSwitch;
    window._restoreCachedDom = _restoreCachedDom;
    window._pushPanelHistory = _pushPanelHistory;
    window._updatePanelHistoryBtns = _updatePanelHistoryBtns;
    window.panelHistoryBack = panelHistoryBack;
    window.panelHistoryForward = panelHistoryForward;
    window._selectCommandForPanel = _selectCommandForPanel;
})();
"""
write_module('commands.js', commands_content)

# ─── 9. vtty.js ───
# Lines 2415-2998 (per-panel VTTY display + cell grid + diff)
# Lines 3003-3193 (debounced HTTP fetch + loadVttyHttp)
vtty_content = """// ─── VTTY Display ───
(function() {
    'use strict';
"""
vtty_content += extract(2415, 3193) + "\n"
vtty_content += """
    window.updateVttyDisplay = updateVttyDisplay;
    window.updateVttyDisplayForPanel = updateVttyDisplayForPanel;
    window.updateVttyMetadataForPanel = updateVttyMetadataForPanel;
    window.applyVttyDiffForPanel = applyVttyDiffForPanel;
    window.scheduleVttyHttpForPanel = scheduleVttyHttpForPanel;
    window.loadVttyHttpForPanel = loadVttyHttpForPanel;
    window.buildCellGrid = buildCellGrid;
    window.applyVttyDiff = applyVttyDiff;
    window.scheduleVttyHttp = scheduleVttyHttp;
    window._prefetchVttyHtml = _prefetchVttyHtml;
    window.loadVttyHttp = loadVttyHttp;
    window.updateVttyMetadata = updateVttyMetadata;
    window.updateVttyMetadataFromHttp = updateVttyMetadataFromHttp;
    window.switchBuffer = switchBuffer;
})();
"""
write_module('vtty.js', vtty_content)

# ─── 10. websocket.js ───
# Lines 1979-2328 (push mode WS)
# Lines 2329-2363 (WS quality)
# Lines 2364-2463 (poll mode)
# Lines 4114-4382 (split panel secondary WS/VTTY)
ws_content = """// ─── WebSocket Management ───
(function() {
    'use strict';
"""
ws_content += extract(1979, 2463) + "\n"   # push mode WS, quality, poll mode
ws_content += extract(4114, 4382) + "\n"   # split panel secondary WS
ws_content += """
    window.connectVttyWs = connectVttyWs;
    window.disconnectVttyWs = disconnectVttyWs;
    window.connectPanelWs = connectPanelWs;
    window.disconnectPanelWs = disconnectPanelWs;
    window.disconnectAllPanelWs = disconnectAllPanelWs;
    window.updateWsQualityIndicator = updateWsQualityIndicator;
    window.startPoll = startPoll;
    window.startPanelPoll = startPanelPoll;
    window.stopPoll = stopPoll;
    window.stopPanelPoll = stopPanelPoll;
    window.pollOnce = pollOnce;
    window.pollOncePanel = pollOncePanel;
    window._connectSecondaryWs = _connectSecondaryWs;
    window._disconnectSecondaryWs = _disconnectSecondaryWs;
    window.scheduleSecondaryVttyHttp = scheduleSecondaryVttyHttp;
    window._loadSecondaryVttyHttp = _loadSecondaryVttyHttp;
    window._updateSecondaryVttyDisplay = _updateSecondaryVttyDisplay;
    window._updateSecondaryVttyMetadata = _updateSecondaryVttyMetadata;
    window._applySecondaryVttyDiff = _applySecondaryVttyDiff;
})();
"""
write_module('websocket.js', ws_content)

# ─── 11. spawn.js ───
# Lines 3262-3746 (tab completion, spawn history, spawn, kill, keep, resize, switch buffer, pause)
spawn_content = """// ─── Spawn & Command Management ───
(function() {
    'use strict';
"""
spawn_content += extract(3262, 3746) + "\n"
spawn_content += """
    window.spawnCmdTabComplete = spawnCmdTabComplete;
    window._resetSpawnCompletion = _resetSpawnCompletion;
    window._loadSpawnHistory = _loadSpawnHistory;
    window._saveSpawnHistory = _saveSpawnHistory;
    window._addSpawnHistoryEntry = _addSpawnHistoryEntry;
    window._renderSpawnHistoryDropdown = _renderSpawnHistoryDropdown;
    window._removeSpawnHistoryDropdown = _removeSpawnHistoryDropdown;
    window._applySpawnHistoryEntry = _applySpawnHistoryEntry;
    window._onSpawnCmdFocus = _onSpawnCmdFocus;
    window._onSpawnCmdKeydownForHistory = _onSpawnCmdKeydownForHistory;
    window.spawnCommand = spawnCommand;
    window.toggleKeepCmd = toggleKeepCmd;
    window.killCommand = killCommand;
    window.purgeCommand = purgeCommand;
    window.killAllCommands = killAllCommands;
    window.sendKeys = sendKeys;
    window.resizeTerminal = resizeTerminal;
    window.resizeTerminalPanel = resizeTerminalPanel;
    window.switchBufferPanel = switchBufferPanel;
})();
"""
write_module('spawn.js', spawn_content)

# ─── 12. misc.js ───
# Everything else: logs, docs, search, sound, onboarding, shortcuts,
# resource polling, notifications, global search, cmd manager,
# workspaces, environments, groups, templates, connections, refresh loop,
# keyboard handling, mouse events, drag-and-drop sidebar reorder
misc_content = """// ─── Miscellaneous ───
// Logs, Docs, Search, Sound, Onboarding, Shortcuts, Resources,
// Notifications, Global Search, Command Manager, Workspaces, Environments,
// Groups, Templates, Refresh Loop, Keyboard/Mouse handling
(function() {
    'use strict';
"""
misc_content += extract(5054, 5457) + "\n"  # logs, docs
misc_content += extract(5458, 5673) + "\n"  # refresh loop
misc_content += extract(5674, 6014) + "\n"  # keyboard handling
misc_content += extract(6015, 6175) + "\n"  # mouse events
misc_content += extract(6176, 6296) + "\n"  # terminal search, scroll to bottom, notifications
misc_content += extract(7021, 7154) + "\n"  # onboarding, shortcuts
misc_content += extract(7155, 7228) + "\n"  # resource polling
misc_content += extract(7376, 7559) + "\n"  # templates
misc_content += extract(7640, 7694) + "\n"  # sidebar cmd drag to panels (start)
misc_content += extract(7695, 7928) + "\n"  # sidebar cmd reorder + open in new pane
misc_content += extract(7929, 8227) + "\n"  # global search, cmd manager
misc_content += extract(8228, 8833) + "\n"  # sound, environments, groups, workspaces
misc_content += """
    // Logs
    window.connectLogWs = connectLogWs;
    window.disconnectLogWs = disconnectLogWs;
    window.loadLog = loadLog;
    window.searchLogs = searchLogs;
    window.clearLogSearch = clearLogSearch;
    window._updateLogTransportIndicator = _updateLogTransportIndicator;
    window._scheduleLogWsReconnect = _scheduleLogWsReconnect;
    // Docs
    window.showDocs = showDocs;
    // Refresh loop
    window.startRefresh = startRefresh;
    window.pollResources = pollResources;
    window.updateSidebarResourceText = updateSidebarResourceText;
    window.checkForExitedCommands = checkForExitedCommands;
    window.notifyCommandEnded = notifyCommandEnded;
    // Terminal search
    window.vttySearch = vttySearch;
    window.vttyApplyHighlights = vttyApplyHighlights;
    window.vttyRemoveHighlights = vttyRemoveHighlights;
    window.vttySearchClose = vttySearchClose;
    window.vttySearchNext = vttySearchNext;
    window.vttySearchPrev = vttySearchPrev;
    window.scrollTerminalBottom = scrollTerminalBottom;
    // Sound
    window.initSoundToggle = initSoundToggle;
    window.toggleSoundNotifications = toggleSoundNotifications;
    window.playExitSound = playExitSound;
    // Onboarding
    window.checkOnboarding = checkOnboarding;
    window.openOnboarding = openOnboarding;
    window.closeOnboarding = closeOnboarding;
    window.nextOnboardingStep = nextOnboardingStep;
    // Shortcuts
    window.showShortcuts = showShortcuts;
    window.closeShortcuts = closeShortcuts;
    // Global search
    window.openGlobalSearch = openGlobalSearch;
    window.closeGlobalSearch = closeGlobalSearch;
    window.executeGlobalSearch = executeGlobalSearch;
    window.onSearchResultClick = onSearchResultClick;
    window.updateFrozenIndicator = updateFrozenIndicator;
    window._toggleSearchFreezeCommands = _toggleSearchFreezeCommands;
    // Command manager
    window.openCmdManager = openCmdManager;
    window.closeCmdManager = closeCmdManager;
    window.renderCmdManagerList = renderCmdManagerList;
    window.cmdManagerKillAll = cmdManagerKillAll;
    // Templates
    window.fetchServerTemplates = fetchServerTemplates;
    window.getServerTemplates = getServerTemplates;
    window.getUserTemplates = getUserTemplates;
    window.saveUserTemplates = saveUserTemplates;
    window.renderTemplates = renderTemplates;
    window.spawnServerTemplate = spawnServerTemplate;
    window.spawnUserTemplate = spawnUserTemplate;
    window.deleteUserTemplate = deleteUserTemplate;
    window.showAddTemplateForm = showAddTemplateForm;
    window.hideAddTemplateForm = hideAddTemplateForm;
    window.saveTemplate = saveTemplate;
    // Workspaces
    window.getWorkspaces = getWorkspaces;
    window.saveWorkspaces = saveWorkspaces;
    window.toggleWorkspaceDropdown = toggleWorkspaceDropdown;
    window.renderWorkspaceList = renderWorkspaceList;
    window.saveCurrentWorkspace = saveCurrentWorkspace;
    window.loadWorkspace = loadWorkspace;
    window.deleteWorkspace = deleteWorkspace;
    window.openWorkspaceManage = openWorkspaceManage;
    // Environments
    window.fetchEnvironments = fetchEnvironments;
    window.renderEnvironments = renderEnvironments;
    window.activateEnvironment = activateEnvironment;
    // Groups
    window.getCmdGroups = getCmdGroups;
    window.saveCmdGroups = saveCmdGroups;
    window.getGroupCollapsedState = getGroupCollapsedState;
    window.saveGroupCollapsedState = saveGroupCollapsedState;
    window.createCmdGroup = createCmdGroup;
    window.deleteCmdGroup = deleteCmdGroup;
    window.renameCmdGroup = renameCmdGroup;
    window.toggleCmdInGroup = toggleCmdInGroup;
    window.toggleGroupCollapse = toggleGroupCollapse;
    window.renderGroups = renderGroups;
    // Sidebar drag
    window.onCmdDragStart = onCmdDragStart;
    window.getCmdOrder = getCmdOrder;
    window.setCmdOrder = setCmdOrder;
    window.getOrderedCmds = getOrderedCmds;
    window._openCommandInNewPane = _openCommandInNewPane;
    // Connections (also in commands.js — just exposing again is fine)
    window.handlePeerEvent = handlePeerEvent;
    window.fetchPeers = fetchPeers;
    window.addDiscoveredPeer = addDiscoveredPeer;
    window.savePeersToStorage = savePeersToStorage;
    window.addConnection = addConnection;
    window.removeConnection = removeConnection;
})();
"""
write_module('misc.js', misc_content)

print("\nAll 12 modules created in", DST)
print("Total lines extracted:", sum(len(open(os.path.join(DST, f)).readlines()) for f in os.listdir(DST) if f.endswith('.js')))
