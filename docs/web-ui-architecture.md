# vrw Web UI — Architecture Document

> **Version:** Post-refactor (20 modules + entry point)
> **Date:** 2025
> **Author:** Automated analysis of source code

---

## 1. Overview

**vrw** (Virtual Run Window) is a web-based terminal administration panel for managing, monitoring, and interacting with terminal commands (processes) running on one or more remote vrw server instances. It provides real-time ANSI-rendered terminal output, keyboard/mouse forwarding to PTY, command lifecycle management (spawn, kill, restart, freeze/thaw), log streaming, and a multi-panel workspace system.

### Tech Stack

| Layer | Technology |
|-------|-----------|
| **Backend** | Rust + Axum HTTP framework |
| **Frontend** | Vanilla JavaScript (no build system, no npm, no framework) |
| **Communication** | REST API (`fetch`) + WebSocket (per-panel push + poll fallback) |
| **State Persistence** | `localStorage` + `sessionStorage` |
| **Styling** | CSS custom properties (3 themes: auto/light/dark) + per-panel themes |
| **Testing** | Node.js test harness with `require('./setup')` + custom `assert`/`assertEq` macros |
| **Delivery** | Static files served by the Rust backend; `<script>` tags loaded via `index.html` |

### Design Philosophy

- **No build step.** All modules are vanilla JS files loaded in order via `<script>` tags. No transpilation, no bundler, no package manager.
- **IIFE module pattern.** Each module wraps its contents in `(function() { 'use strict'; ... })()` to avoid polluting the global scope, then selectively exposes functions on `window.*` or `window.VRW.*`.
- **Global singleton state.** A single `state` object on `window.VRW.state` holds all mutable application state. Module-level `let`/`const` variables are exposed via `window.VRW.*` for cross-module access.
- **DOM-first rendering.** HTML is generated as string concatenation and applied via `innerHTML`. An incremental cell-level diff system (Level 3) patches individual `<span>` elements without full DOM rebuilds.
- **Three-level VTTY update optimization.** Level 1 (scroll preservation), Level 2 (generation-based skip), Level 3 (cell grid incremental diff).

---

## 2. Architecture Diagram

```
┌──────────────────────────────── Module Dependency Graph ─────────────────────────────────┐
│                                                                                          │
│  state.js ─────────────────────────────────────────────────────────────────────────────  │
│     ↑                                                                                   │
│  eventbus.js ───────────────────────────────────────────────────────────────────────────  │
│     ↑                                                                                   │
│  utils.js ───────────────────────────────────────────────────────────────────────────────  │
│     ↑       ↑       ↑       ↑       ↑       ↑       ↑       ↑       ↑       ↑         │
│     │       │       │       │       │       │       │       │       │       │         │
│  focus.js   │   theme.js  │       │       │       │       │       │       │         │
│     ↑       │       ↑       │       │       │       │       │       │       │         │
│     │       │       │       │       │       │       │       │       │       │         │
│  sidebar.js │       │       │       │       │       │       │       │       │         │
│     ↑       │       │       │       │       │       │       │       │       │         │
│  panels.js  │       │       │       │       │       │       │       │       │         │
│     ↑       │       │       │       │       │       │       │       │       │         │
│  commands.js│       │       │       │       │       │       │       │       │         │
│     ↑       │       │       │       │       │       │       │       │       │         │
│  websocket.js      │       │       │       │       │       │       │       │         │
│     ↑               │       │       │       │       │       │       │       │         │
│  vtty.js            │       │       │       │       │       │       │       │         │
│     ↑               │       │       │       │       │       │       │       │         │
│  spawn.js           │       │       │       │       │       │       │       │         │
│     ↑               │       │       │       │       │       │       │       │         │
│  logs.js            │       │       │       │       │       │       │       │         │
│     ↑               │       │       │       │       │       │       │       │         │
│  keyboard.js        │       │       │       │       │       │       │       │         │
│     ↑               │       │       │       │       │       │       │       │         │
│  search.js          │       │       │       │       │       │       │       │         │
│     ↑               │       │       │       │       │       │       │       │         │
│  notifications.js    │       │       │       │       │       │       │       │         │
│     ↑               │       │       │       │       │       │       │       │         │
│  onboarding.js      │       │       │       │       │       │       │       │         │
│     ↑               │       │       │       │       │       │       │       │         │
│  templates.js       │       │       │       │       │       │       │       │         │
│     ↑               │       │       │       │       │       │       │       │         │
│  dragdrop.js        │       │       │       │       │       │       │       │         │
│     ↑               │       │       │       │       │       │       │       │         │
│  workspaces.js      │       │       │       │       │       │       │       │         │
│     ↑               │       │       │       │       │       │       │       │         │
│  misc.js ─────────────────────────────────────────────────────────────────────────────── │
│     ↑                                                                                   │
│  app.js ────────────────────────────────────────────────────────────────────────────── │
│                                                                                          │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

### Layered Architecture

```
┌──────────────────────────────────────────────────────────┐
│                     app.js (Entry Point)                  │
│           Initialization, event wiring, URL routing        │
├──────────────────────────────────────────────────────────┤
│                    Feature Modules                        │
│  workspaces │ templates │ dragdrop │ onboarding │ misc    │
├──────────────────────────────────────────────────────────┤
│                   Interaction Modules                     │
│    keyboard │ search │ notifications │ logs │ spawn        │
├──────────────────────────────────────────────────────────┤
│                    Core Data Modules                       │
│    commands │ panels │ sidebar │ websocket │ vtty          │
├──────────────────────────────────────────────────────────┤
│                    Infrastructure                        │
│    state │ eventbus │ utils │ focus │ theme                │
└──────────────────────────────────────────────────────────┘
```

---

## 3. Module Inventory

### 3.1 `state.js` (130 lines)

**Purpose:** Defines the global application state singleton and exposes module-level private variables to the `VRW` namespace for cross-module access.

**Key exports:** `VRW.state`, `VRW._lastCommandState`, `VRW._navCommands`, `VRW._showingWelcome`, `VRW._sidebarSort`, `VRW._searchFrozenPanelIds`, `VRW._searchFrozenCmdIds`, `VRW._lastRenderedPanelCount`, `VRW._lastRenderedPanelIds`, `VRW._lastSplitState`, `VRW._lastShowingWelcome`

**Dependencies:** None (leaf module).

**State reads/writes:**
- **Defines:** `state` object with ~50 fields covering panels, connections, selection, WebSocket, VTTY caching, resource cache, layout, theme, auth, polling config, and scroll state.
- **Reads:** `localStorage` for initial auth token, font size, update mode, poll interval, panel layout, resource toggle, sound toggle.

---

### 3.2 `eventbus.js` (13 lines)

**Purpose:** Central publish/subscribe event emitter for cross-module communication. Currently defined but **not widely used** — most inter-module communication happens via direct `window.*` function calls.

**Key exports:** `VRW.EventBus` with `.on(event, fn)`, `.off(event, fn)`, `.emit(event, ...args)`, `.once(event, fn)`.

**Dependencies:** None (leaf module).

**State reads/writes:** None.

---

### 3.3 `utils.js` (147 lines)

**Purpose:** Pure utility functions used across all modules. HTML escaping, URL construction, auth header generation, spawn argument parsing, runtime formatting.

**Key exports:** `formatRuntime`, `getBaseUrl`, `authHeaders`, `authHeadersForInstance`, `apiUrl`, `escHtml`, `parseSpawnArgs`, `parseSpawnEnvVars`

**Dependencies:** `state` (via `state.connections`, `state.authToken`).

**State reads/writes:**
- **Reads:** `state.connections[0].url`, `state.authToken`.

---

### 3.4 `focus.js` (93 lines)

**Purpose:** Focus trap management for modal dialogs. Traps Tab/Shift+Tab within a container and restores focus on release. Used by command picker, onboarding, shortcuts panel, and all modals.

**Key exports:** `trapFocus(container)`, `releaseCurrentFocusTrap()`

**Dependencies:** None (leaf module).

**State reads/writes:** None.

---

### 3.5 `theme.js` (86 lines)

**Purpose:** Global theme cycling (Auto → Grey → Dark) and per-panel theme toggling (inherit → light → dark). Themes are stored in `localStorage` and applied via `data-theme` / `data-panel-theme` attributes.

**Key exports:** `initTheme`, `toggleGlobalTheme`, `updateThemeButton`, `togglePanelTheme`, `applyPanelTheme`

**Dependencies:** `state` (for `state.panels` to find panel objects), `utils` (implicitly via `getActivePanelId`).

**State reads/writes:**
- **Reads:** `localStorage.getItem('vrw_theme')`, `state.panels` (for theme lookup).
- **Writes:** `localStorage.setItem('vrw_theme', ...)`, `localStorage.setItem('vrw_panel_theme_' + panelId, ...)`, `panelObj.theme`.

---

### 3.6 `sidebar.js` (448 lines)

**Purpose:** Sidebar rendering, command list building, filtering, pinning/favorites, sort mode management, tab switching, resource toggle, bottom bar toggle, logs view toggle, and disconnected UI updates. Contains the critical `_buildSidebar()` function that is the main sidebar render path.

**Key exports:** `toggleSidebar`, `switchSidebarTab`, `updateSidebarTabsVisibility`, `updateCmdToolbarVisibility`, `updateDisconnectedUI`, `updateSidebarBanner`, `updateTerminalDisconnectedOverlay`, `_buildSidebar`, `getPinnedNames`, `setPinnedNames`, `togglePinCmd`, `rearrangePinnedCommands`, `toggleResources`, `toggleBottombar`, `initBottombar`, `toggleLogsView`

**Dependencies:** `utils` (`escHtml`, `formatRuntime`, `getBaseUrl`), `state` (extensively: `state.connections`, `state.selectedInstUrl`, `state.selectedCmdId`, `state._resourceCache`, `state.showResources`, `state.connections`), `commands` (calls `selectCommand`, `updatePanelCommandInfo`, `scheduleVttyHttp`), `websocket` (calls `connectLogWs`, `disconnectLogWs`).

**State reads/writes:**
- **Reads:** `state.connections`, `state.selectedInstUrl`, `state.selectedCmdId`, `state._resourceCache`, `state.showResources`, `state.serverReachable`, `VRW._sidebarSort`, `VRW._lastCommandState`, `VRW._showingWelcome`.
- **Writes:** `VRW._lastCommandState`, `VRW._navCommands`, `VRW._showingWelcome`, `state.showResources`.

---

### 3.7 `panels.js` (~1,595 lines)

**Purpose:** Multi-panel management — creation, removal, splitting, layout (row/column/grid presets), minimization, focus tracking, rendering, shared toolbar synchronization, send-keys, special keys help, copy/export/screenshot, max-fit/max-font, and panel rename. This is the **largest module**.

**Key exports:** `addPanelDirect`, `addPanel`, `removePanel`, `toggleMinimizePanel`, `splitPanel`, `unsplitPanel`, `renderPanels`, `focusPanel`, `updateSharedToolbar`, `sendKeysToPanel`, `showSpecialKeysHelp`, `getActivePanelId`, `getSelectedPanel`, `copyTerminalSelection`, `exportTerminal`, `takeScreenshot`, `toggleSelectionMode`, `autoFitActiveTerminal`, `startRenamePanel`, `finishRenamePanel`, `togglePanelLayout`, `toggleLayoutPresetMenu`, `applyLayoutPreset`, `showPanelContextMenu`, `closeContextMenu`

**Dependencies:** `utils` (`escHtml`, `getBaseUrl`, `authHeaders`, `apiUrl`), `state` (extensively), `focus` (`trapFocus`, `releaseCurrentFocusTrap`).

**State reads/writes:**
- **Reads:** `state.panels`, `state._focusedPanelId`, `state.selectedInstUrl`, `state.selectedCmdId`, `state.connections`, `state.fontSize`, `state.panelLayout`, `state._lastRenderedPanelCount`, `state._lastRenderedPanelIds`, `state._lastSplitState`, `state._lastShowingWelcome`, `state._resourceCache`, `state.showResources`, `state.bufferView`, `state.refreshMs`, `state.updateMode`, `state.serverReachable`.
- **Writes:** `state.panels`, `state._focusedPanelId`, `state.panelLayout`, `state.selectedInstUrl`, `state.selectedCmdId`, `_lastRenderedPanelCount`, `_lastRenderedPanelIds`, `_lastSplitState`, `_lastShowingWelcome`, `_showingWelcome`, `_maxFitState`, `_maxFontState`.

---

### 3.8 `commands.js` (~1,084 lines)

**Purpose:** Command lifecycle management — loading, selecting, navigating (prev/next), caching terminal DOM for instant switch-back, command history (back/forward per panel), pause/run (freeze/thaw), server config fetching, update mode switching, certificate loading, instance connection management, restart, welcome panel logic, and snapshot loading. Also contains peer event handling and auto-restart logic.

**Key exports:** `lookupAndSelectCommand`, `navigateCommand`, `loadCommands`, `selectCommand`, `updatePanelCommandInfo`, `updateBottomBarLabel`, `getSelectedPanel`, `getActivePanelId`, `togglePauseRun`, `togglePauseRunPanel`, `fetchServerConfig`, `applyUpdateModeUI`, `switchUpdateMode`, `applyPollInterval`, `loadCertificates`, `updateCertDropdown`, `addConnection`, `removeConnection`, `disconnectServer`, `restartCommand`, `restartCommandById`, `showAddServerModal`, `closeAddServerModal`, `confirmAddServer`, `autofitTerminalSize`, `updateInstanceDropdown`, `_cacheTerminalForSwitch`, `_restoreCachedDom`, `updateSidebarSelection`, `_pushPanelHistory`, `panelHistoryBack`, `panelHistoryForward`, `startUpdateMode`, `stopUpdateMode`, `startPanelUpdateMode`, `stopPanelUpdateMode`, `_isTerminalVisible`, `_flushPendingVttyUpdate`, `handlePeerEvent`, `fetchPeers`, `addDiscoveredPeer`, `savePeersToStorage`, `loadSnapshot`

**Dependencies:** `utils` (`escHtml`, `formatRuntime`, `getBaseUrl`, `authHeaders`, `authHeadersForInstance`, `apiUrl`), `state`, `panels` (`focusPanel`, `renderPanels`, `addPanelDirect`), `focus` (`trapFocus`, `releaseCurrentFocusTrap`), `sidebar` (`_buildSidebar`, `updateDisconnectedUI`), `websocket` (`disconnectPanelWs`, `stopPanelPoll`, `connectPanelWs`, `startPanelPoll`), `vtty` (`loadVttyHttp`, `loadVttyHttpForPanel`, `scheduleVttyHttp`, `scheduleVttyHttpForPanel`, `updateVttyDisplay`, `updateVttyDisplayForPanel`), `spawn` (`loadCertificates`, `fetchServerTemplates`), `notifications` (`notifyCommandEnded`).

**State reads/writes:**
- **Reads:** `state.connections`, `state.selectedInstUrl`, `state.selectedCmdId`, `state.panels`, `state.serverReachable`, `state.updateMode`, `state.pollInterval`, `state.bufferView`, `state._cachedDomPre`, `state._cachedScrollPos`, `state._lastGeneration`, `state._pendingVttyData`, `state._pendingVttyDirty`, `state._level3Enabled`, `state.authToken`, `VRW._lastCommandState`, `VRW._showingWelcome`, `VRW._navCommands`.
- **Writes:** `state.connections`, `state.selectedInstUrl`, `state.selectedCmdId`, `state.panels`, `state.serverReachable`, `state.updateMode`, `state.pollInterval`, `state.bufferView`, `state._cachedDomPre`, `state._cachedScrollPos`, `state._lastGeneration`, `state._pendingVttyData`, `state._pendingVttyDirty`, `VRW._lastCommandState`, `VRW._showingWelcome`, `VRW._navCommands`, `_snapshotLoaded`.

---

### 3.9 `websocket.js` (780 lines)

**Purpose:** WebSocket connection management for VTTY push updates. Includes legacy global WS (`connectVttyWs`), per-panel WS (`connectPanelWs`), secondary pane WS for split panels, poll mode (`startPanelPoll`, `pollOncePanel`), connection quality indicator (ping/pong latency), auto-reconnect with exponential backoff (capped at 5 attempts), and dirty signal handling.

**Key exports:** `connectVttyWs`, `disconnectVttyWs`, `connectPanelWs`, `disconnectPanelWs`, `disconnectAllPanelWs`, `updateWsQualityIndicator`, `startPoll`, `startPanelPoll`, `stopPoll`, `stopPanelPoll`, `pollOnce`, `pollOncePanel`, `_connectSecondaryWs`, `_disconnectSecondaryWs`, `scheduleSecondaryVttyHttp`, `_loadSecondaryVttyHttp`, `_updateSecondaryVttyDisplay`, `_updateSecondaryVttyMetadata`, `_applySecondaryVttyDiff`, `updateVttyDisplay` (also defined here), `_throttleRefresh`

**Dependencies:** `utils` (`apiUrl`, `authHeaders`, `authHeadersForInstance`), `state`, `commands` (`getActivePanelId`, `getSelectedPanel`, `handlePeerEvent`), `vtty` (`updateVttyDisplayForPanel`, `applyVttyDiffForPanel`, `scheduleVttyHttpForPanel`, `loadVttyHttpForPanel`), `notifications` (`notifyCommandEnded`).

**State reads/writes:**
- **Reads:** `state.panels`, `state._focusedPanelId`, `state.selectedCmdId`, `state.authToken`, `state.connections`, `state._lastGeneration`, `state.bufferView`, `state._userScrolling`, `state._userAtBottom`, `state._pendingVttyData`, `state._pendingVttyDirty`, `state._level3Enabled`, `state._cellGrids`, `state.refreshMs`, `state.updateMode`, `state.pollInterval`.
- **Writes:** `state.vttyWs`, `state.vttyWsUrl`, `state.vttyWsCmdId`, `state._wsPingInterval`, `state._wsPingSendTime`, `state._wsLatency`, `state._wsReconnectCount`, `state._vttyHttpTimer`, `state._lastGeneration`, `state._cellGrids`, `state._pendingVttyData`, `state._pendingVttyDirty`.

---

### 3.10 `vtty.js` (799 lines)

**Purpose:** VTTY display rendering and diff application. Handles both global (legacy) and per-panel VTTY display updates, Level 3 cell grid construction and incremental diff patching, HTML fetch/prefetch with generation-based dedup, scroll position preservation, buffer switching (main/alt/current), cursor/dimension/mouse metadata updates, and cell style generation matching the server's VttyRenderer output format.

**Key exports:** `updateVttyDisplay`, `updateVttyDisplayForPanel`, `updateVttyMetadataForPanel`, `applyVttyDiffForPanel`, `scheduleVttyHttpForPanel`, `loadVttyHttpForPanel`, `buildCellGrid`, `applyVttyDiff`, `scheduleVttyHttp`, `_prefetchVttyHtml`, `loadVttyHttp`, `updateVttyMetadata`, `updateVttyMetadataFromHttp`, `switchBuffer`

**Dependencies:** `utils` (`apiUrl`, `authHeadersForInstance`), `state` (for generation cache, cell grids, cached DOM, scroll state, user scrolling state).

**State reads/writes:**
- **Reads:** `state.panels`, `state._focusedPanelId`, `state.selectedInstUrl`, `state.selectedCmdId`, `state._lastGeneration`, `state._cellGrids`, `state._cachedDomPre`, `state._cachedScrollPos`, `state._level3Enabled`, `state._userScrolling`, `state._pendingVttyData`, `state._pendingVttyDirty`, `state._userAtBottom`, `state.bufferView`, `state._termRows`, `state._termCols`, `state.fontSize`.
- **Writes:** `state._lastGeneration`, `state._cellGrids`, `state._cachedDomPre`, `state._cachedScrollPos`, `state._userAtBottom`, `state._pendingVttyData`, `state._pendingVttyDirty`, `state._termRows`, `state._termCols`.

---

### 3.11 `spawn.js` (510 lines)

**Purpose:** Command spawning, tab completion for the spawn command input, spawn history (saved in localStorage), kill/purge commands, kill-all with filter, send-keys, terminal resize, buffer switching, keep/unkeep, and working directory support.

**Key exports:** `spawnCmdTabComplete`, `_resetSpawnCompletion`, `spawnCommand`, `toggleKeepCmd`, `killCommand`, `purgeCommand`, `killAllCommands`, `sendKeys`, `resizeTerminal`, `resizeTerminalPanel`, `switchBufferPanel`

**Dependencies:** `utils` (`escHtml`, `parseSpawnArgs`, `parseSpawnEnvVars`, `getBaseUrl`, `authHeadersForInstance`, `apiUrl`, `formatRuntime`), `state` (for panels, connections, selected command), `commands` (`addPanelDirect`, `focusPanel`, `_cacheTerminalForSwitch`, `getSelectedPanel`, `loadCommands`, `loadVttyHttp`, `disconnectPanelWs`, `togglePauseRun`).

**State reads/writes:**
- **Reads:** `state.panels`, `state._focusedPanelId`, `state.selectedCmdId`, `state.selectedInstUrl`, `state.connections`.
- **Writes:** `state.selectedInstUrl`, `state._pendingSelectId`, `_lastCommandState`, `localStorage` (spawn history).

---

### 3.12 `logs.js` (234 lines)

**Purpose:** Log viewer — WebSocket log streaming, HTTP log loading with search, log line parsing and formatting, auto-scroll, transport mode indicator (WS vs HTTP), and exponential backoff reconnection.

**Key exports:** `connectLogWs`, `disconnectLogWs`, `loadLog`, `searchLogs`, `clearLogSearch`, `_updateLogTransportIndicator`, `_scheduleLogWsReconnect`

**Dependencies:** `utils` (`escHtml`, `getBaseUrl`, `authHeaders`, `apiUrl`), `state` (for log WS state, current view, reconnection tracking).

**State reads/writes:**
- **Reads:** `state.logWs`, `state.authToken`, `state.connections`, `state.currentView`.
- **Writes:** `state.logWs`, `state._logWsReconnectAttempts`, `state._logWsPingTimer`, `state.logWsReconnectTimer`, `state._logSearchReconnectTimer`.

---

### 3.13 `keyboard.js` (550 lines)

**Purpose:** Global keyboard and mouse event listeners. Direct terminal key sending (with full escape sequence mapping for special keys, Ctrl/Alt/Meta combinations), click-to-focus terminal, mouse wheel scrollback navigation (with rAF coalescing), mouse event forwarding to PTY (when mouse tracking is enabled), keyboard shortcuts (Ctrl+F, Ctrl+Shift+C, Ctrl+Shift+R, Alt+S, Alt+N, Alt+T, ?, Escape), and context menu keyboard triggers.

**Key exports:** None (all handlers are attached via `document.addEventListener` — no `window.*` exports).

**Dependencies:** `utils` (`apiUrl`, `authHeadersForInstance`), `state` (for panels, selected command, scroll state), `search` (`vttySearchClose`), `onboarding` (`closeShortcuts`), `focus` (`trapFocus`), `panels` (`closePanelModal`, `toggleSelectionMode`, `copyTerminalSelection`, `exportTerminal`, `restartCommand`, `addPanel`, `sendDirectKey`), `commands` (`navigatePrevCommand`, `navigateNextCommand`, `closeContextMenu`), `theme` (`togglePanelTheme`), `vtty` (`scheduleVttyHttp`, `loadVttyHttp`, `loadVttyHttpForPanel`).

**State reads/writes:**
- **Reads:** `state.panels`, `state._focusedPanelId`, `state.selectedCmdId`, `state.selectedInstUrl`, `state.currentView`, `state.updateMode`, `state.fontSize`.
- **Writes:** `panelObj.focused`, `panelObj.mouseTracking`, `panelObj.mouseSgr`, `panelObj.scrollbackOffset`, `sessionStorage` (scrollback positions).

---

### 3.14 `search.js` (529 lines)

**Purpose:** Terminal search (within-panel Ctrl+F), global search (across all commands' VTTY text), command manager dialog (sortable/filterable table of all commands), scroll-to-bottom, and panel freeze/thaw for search stability. The global search freezes all panel updates and optionally SIGSTOPs commands so text doesn't shift during search.

**Key exports:** `vttySearch`, `vttyApplyHighlights`, `vttyRemoveHighlights`, `vttySearchClose`, `vttySearchNext`, `vttySearchPrev`, `scrollTerminalBottom`, `openGlobalSearch`, `closeGlobalSearch`, `executeGlobalSearch`, `onSearchResultClick`, `updateFrozenIndicator`, `_toggleSearchFreezeCommands`, `openCmdManager`, `closeCmdManager`, `renderCmdManagerList`, `cmdManagerKillAll`

**Dependencies:** `utils` (`escHtml`, `formatRuntime`, `getBaseUrl`, `authHeadersForInstance`, `apiUrl`), `state` (for panels, connections, resources), `focus` (`trapFocus`, `releaseCurrentFocusTrap`), `commands` (`getPinnedNames`, `getActivePanelId`, `selectCommand`, `startPanelUpdateMode`, `stopPanelUpdateMode`, `restartCommandById`), `websocket` (`stopPanelUpdateMode`, `startPanelUpdateMode`, `_loadSecondaryVttyHttp`).

**State reads/writes:**
- **Reads:** `state.panels`, `state._focusedPanelId`, `state.selectedCmdId`, `state.connections`, `state._resourceCache`.
- **Writes:** `VRW._searchFrozenPanelIds`, `VRW._searchFrozenCmdIds`, `sessionStorage` (scrollback).

---

### 3.15 `notifications.js` (205 lines)

**Purpose:** Browser notification on command exit (via Notification API), sound notification (Web Audio API oscillator), auto-restart of pinned commands with debounce, resource polling (CPU/memory for all alive commands), and sidebar resource text updates.

**Key exports:** `pollResources`, `updateSidebarResourceText`, `checkForExitedCommands`, `notifyCommandEnded`, `initSoundToggle`, `toggleSoundNotifications`, `playExitSound`

**Dependencies:** `utils` (`getBaseUrl`, `authHeaders`, `authHeadersForInstance`, `apiUrl`), `state` (for connections, resources, sound toggle), `commands` (`getPinnedNames`, `restartCommandById`).

**State reads/writes:**
- **Reads:** `state.connections`, `state.authToken`, `state.soundEnabled`, `state._resourceCache`.
- **Writes:** `state._resourceCache`, `state.soundEnabled`, `localStorage`.

---

### 3.16 `onboarding.js` (147 lines)

**Purpose:** First-run tutorial overlay with 7 steps spotlighting UI elements, keyboard shortcuts help panel, and onboarding state persistence in localStorage.

**Key exports:** `checkOnboarding`, `openOnboarding`, `closeOnboarding`, `nextOnboardingStep`, `showShortcuts`, `closeShortcuts`

**Dependencies:** `utils` (`escHtml`), `focus` (`trapFocus`, `releaseCurrentFocusTrap`).

**State reads/writes:**
- **Reads:** `localStorage.getItem('vrw_onboarding_done')`.
- **Writes:** `localStorage.setItem('vrw_onboarding_done', '1')`.

---

### 3.17 `templates.js` (203 lines)

**Purpose:** Command templates — server-side templates (fetched from `/api/templates` from vrw config) and user-defined templates (stored in localStorage). Templates provide one-click command spawning with pre-configured arguments, working directory, and certificates.

**Key exports:** `fetchServerTemplates`, `getServerTemplates`, `getUserTemplates`, `saveUserTemplates`, `renderTemplates`, `spawnServerTemplate`, `spawnUserTemplate`, `deleteUserTemplate`, `showAddTemplateForm`, `hideAddTemplateForm`, `saveTemplate`

**Dependencies:** `utils` (`escHtml`, `getBaseUrl`, `authHeaders`, `authHeadersForInstance`, `apiUrl`), `state` (for connections, spawn state), `commands` (`loadCommands`, `_cacheTerminalForSwitch`, `switchSidebarTab`).

**State reads/writes:**
- **Reads:** `state.selectedInstUrl`, `state.connections`.
- **Writes:** `state.selectedInstUrl`, `state._pendingSelectId`, `_serverTemplates` (module-level), `localStorage`.

---

### 3.18 `dragdrop.js` (303 lines)

**Purpose:** Drag-and-drop support — sidebar command drag-to-panel (HTML5 DnD API), sidebar command reorder via custom mousedown/mousemove/mouseup handler (avoids nested DnD anti-pattern), and drop-to-open-command-in-new-pane. Custom order persisted in localStorage.

**Key exports:** `onCmdDragStart`, `getCmdOrder`, `setCmdOrder`, `getOrderedCmds`, `_openCommandInNewPane`

**Dependencies:** `utils` (`escHtml`), `state` (for panels, connections), `panels` (`addPanelDirect`, `focusPanel`), `commands` (`_cacheTerminalForSwitch`, `updatePanelCommandInfo`, `updateTerminalDisconnectedOverlay`, `updateSidebarSelection`, `loadVttyHttpForPanel`, `startPanelUpdateMode`).

**State reads/writes:**
- **Reads:** `state.panels`, `state.connections`.
- **Writes:** `state.selectedInstUrl`, `state.selectedCmdId`, `state._pendingVttyData`, `state._pendingVttyDirty`, `state.bufferView`, `VRW._lastCommandState`, `localStorage`.

---

### 3.19 `workspaces.js` (782 lines)

**Purpose:** Documentation viewer (markdown→HTML), workspace environments (server-defined from config + user-defined in localStorage), command groups (named sets of commands), and workspace save/restore (panel configurations stored in localStorage).

**Key exports:** `showDocs`, `loadDocs`, `renderMarkdown`, `renderEmbeddedDocs`, `fetchEnvironments`, `renderEnvironments`, `activateEnvironment`, `getCmdGroups`, `saveCmdGroups`, `getGroupCollapsedState`, `saveGroupCollapsedState`, `createCmdGroup`, `deleteCmdGroup`, `renameCmdGroup`, `toggleCmdInGroup`, `toggleGroupCollapse`, `renderGroups`, `getWorkspaces`, `saveWorkspaces`, `toggleWorkspaceDropdown`, `renderWorkspaceList`, `saveCurrentWorkspace`, `loadWorkspace`, `deleteWorkspace`, `openWorkspaceManage`

**Dependencies:** `utils` (`escHtml`, `getBaseUrl`, `authHeaders`, `authHeadersForInstance`, `apiUrl`), `state`, `panels` (`addPanelDirect`, `renderPanels`, `focusPanel`, `disconnectPanelWs`, `stopPanelPoll`), `commands` (`loadCommands`, `loadCertificates`, `addConnection`, `selectCommand`, `disconnectLogWs`), `focus` (`trapFocus`, `releaseCurrentFocusTrap`), `logs` (`disconnectLogWs`).

**State reads/writes:**
- **Reads:** `state.panels`, `state.connections`, `state.currentView`, `state.panelLayout`, `state._lastRenderedPanelCount`.
- **Writes:** `state.panels`, `state.panelLayout`, `state._focusedPanelId`, `state.currentView`, `_lastRenderedPanelCount`, `_serverEnvironments` (module-level), `localStorage`.

---

### 3.20 `misc.js` (165 lines)

**Purpose:** Miscellaneous UI controls — token management, font size (global and per-panel), refresh throttle (0–2000ms), selection mode toggle, and the main refresh loop (triggers `loadSnapshot` on first call, then periodic `loadCommands` + `checkForExitedCommands`). Also re-exports connection helpers from commands.js.

**Key exports:** `saveToken`, `changeFontSize`, `applyFontSize`, `changePanelFontSize`, `changeRefreshMs`, `applyRefreshMs`, `toggleSelectionMode`, `startRefresh`, `handlePeerEvent`, `fetchPeers`, `addDiscoveredPeer`, `savePeersToStorage`, `addConnection`, `removeConnection`

**Dependencies:** All modules (re-exports shared functions and provides the refresh loop orchestration).

**State reads/writes:**
- **Reads:** `state.authToken`, `state.fontSize`, `state.panels`, `state.selectedInstUrl`, `state.selectedCmdId`, `state.refreshMs`, `state._refreshThrottleTimer`.
- **Writes:** `state.authToken`, `state.fontSize`, `state.refreshMs`, `localStorage`, `state.refreshInterval`, `state._resourceInterval`.

---

### 3.21 `app.js` (169 lines)

**Purpose:** Entry point / bootstrap. Initializes the application: parses URL arguments for multi-instance configuration, creates initial connections and panels, sets up scroll detection (pauses VTTY DOM updates during scrolling), starts the refresh loop, loads certificates/templates/environments/server config/peers, triggers onboarding, handles mobile layout detection, window resize handling, sidebar resize handle, command-name URL routing (`/admin/my-cmd` → auto-selects that command), and event delegation for command list kill buttons.

**Dependencies:** All modules.

**State reads/writes:**
- **Reads:** URL query parameters, `state.connections`, `window.innerWidth`.
- **Writes:** `state.connections`, `state._mobileTabbedLayout`.

---

## 4. Data Flow

### 4.1 WebSocket Push Mode (Primary Path)

```
Server VTTY Buffer Change
        │
        ▼
WebSocket: { type: "vtty_full" | "vtty_diff" | "vtty_dirty", data: {...} }
        │
        ▼
websocket.js: onmessage handler
        │  • Guards: discard if cmd_id doesn't match selectedCmdId
        │  • If vtty_full → vtty.js: updateVttyDisplayForPanel()
        │  • If vtty_diff → vtty.js: applyVttyDiffForPanel()
        │  • If vtty_dirty → scheduleVttyHttpForPanel() (legacy fallback)
        │  • If command_ended → disconnect + notify
        │  • If pong → update latency indicator
        │
        ▼
vtty.js: Generation check (Level 2)
        │  • If generation unchanged → updateVttyMetadataForPanel() only
        │  • If generation changed → continue
        │
        ▼
vtty.js: User scrolling check
        │  • If userScrolling → buffer as _pendingVttyData
        │  • If not scrolling → continue
        │
        ▼
vtty.js: Refresh throttle check
        │  • If refreshMs > 0 → buffer, apply after throttle window
        │  • If refreshMs = 0 → continue
        │
        ▼
vtty.js: Full HTML replacement OR incremental diff
        │  • Level 1: Save scroll position → pre.innerHTML = data.html → restore scroll
        │  • Level 3: Build cell grid from DOM, then apply cell-level patches
        │
        ▼
vtty.js: updateVttyMetadataForPanel()
        │  • Cursor position indicator
        │  • Terminal dimensions
        │  • Mouse tracking state
        │  • Bottom bar labels
        │  • Toolbar resize inputs
        │
        ▼
DOM updated → user sees new terminal output
```

### 4.2 Command Selection Flow

```
User clicks command in sidebar
        │
        ▼
commands.js: selectCommand(instUrl, cmdId, name)
        │
        ├─── Push current command to panel history
        ├─── _cacheTerminalForSwitch() → detach <pre> children to DocumentFragment
        ├─── disconnectPanelWs(panelId)
        ├─── Update panelObj.selectedInstUrl / selectedCmdId
        ├─── focusPanel(panelId) → update global state, toolbar
        ├─── _restoreCachedDom(cmdId) → instant display if cached
        ├─── updatePanelCommandInfo() → header text, badges, toolbar
        ├─── updateSidebarSelection() → toggle .selected class
        ├─── loadVttyHttpForPanel() → fetch HTML from server
        └─── startPanelUpdateMode() → connectPanelWs() or startPanelPoll()
```

### 4.3 Direct Key Sending Flow

```
User presses key while terminal is focused
        │
        ▼
keyboard.js: document keydown handler
        │
        ├─── Is panel focused + command selected?
        ├─── Is key in a search input? → let input handle
        ├─── Is Escape? → close modals/search/shortcuts
        ├─── Is Ctrl+F? → open search
        ├─── Otherwise: sendDirectKey(e, panelObj)
        │
        ▼
keyboard.js: sendDirectKey()
        │
        ├─── Map key to escape sequence (xterm sequences)
        ├─── POST /api/commands/:id/keys { keys: "\x1b[A" }
        │
        ▼
keyboard.js: scheduleVttyHttp() → trigger refresh
        │
        ▼
Server echoes keystroke → VTTY buffer updated → WS push → vtty.js render
```

---

## 5. State Management

### 5.1 Global State Object (`VRW.state`)

The `state` object is a mutable singleton. There is no immutability enforcement — any module can read and write to any field.

| Category | Fields | Persistence |
|----------|--------|-------------|
| **Auth** | `authToken` | `localStorage('vrw_auth_token')` |
| **Selection** | `selectedInstUrl`, `selectedCmdId`, `_focusedPanelId` | Volatile (session-only) |
| **Panels** | `panels[]` (array of panel objects) | Volatile (reconstructed on load) |
| **Connections** | `connections[]` (array of server connections) | Volatile (from URL params or auto-detect) |
| **VTTY Cache** | `_cachedDomPre{}`, `_cachedScrollPos{}`, `_cellGrids{}`, `_lastGeneration{}` | Volatile (performance caches) |
| **Scroll** | `_userScrolling`, `_userAtBottom`, `_pendingVttyData`, `_pendingVttyDirty` | Volatile |
| **Update Mode** | `updateMode`, `pollInterval`, `_level3Enabled` | `localStorage` |
| **Layout** | `panelLayout`, `fontSize`, `bufferView` | `localStorage` |
| **Server Config** | `serverUpdateMode`, `serverPollMs`, `serverDirtyMs`, `serverReachable` | Fetched from `/api/info` |
| **WebSocket** | `vttyWs` (deprecated), `logWs`, ping/latency/reconnect fields | Volatile |
| **Resources** | `_resourceCache{}` (cpu_percent, memory_mb per cmdId) | Poll every 2s |
| **UI Preferences** | `showResources`, `soundEnabled`, `_mobileTabbedLayout` | `localStorage` |

### 5.2 Per-Panel State

Each panel object in `state.panels[]` carries its own state:

| Field | Purpose |
|-------|---------|
| `id` | Unique DOM ID (e.g., `"panel-1719..."`) |
| `selectedInstUrl`, `selectedCmdId` | Which command this panel displays |
| `ws`, `wsCmdId`, `wsInstUrl` | Per-panel WebSocket connection |
| `pollTimer` | Per-panel poll interval timer |
| `fontSize`, `theme`, `customTitle` | Per-panel display preferences |
| `selectionMode`, `mouseTracking`, `mouseSgr` | Interaction state |
| `scrollbackOffset`, `focused`, `minimized` | Scroll and focus state |
| `cmdHistory[]`, `cmdHistoryIdx` | Browser-like back/forward history |
| `split` (optional) | Split pane config with secondary WS/poll |

### 5.3 Module-Level Variables

Private variables at module scope are exposed via `window.VRW.*`:

| Variable | Module | Purpose |
|----------|--------|---------|
| `_lastCommandState` | state.js | Sidebar fingerprint for skip-if-unchanged |
| `_navCommands[]` | state.js | Flat list for prev/next navigation |
| `_showingWelcome` | state.js | Welcome panel visibility flag |
| `_sidebarSort` | state.js | Sidebar sort mode |
| `_searchFrozenPanelIds` | state.js | Panels frozen during global search |
| `_searchFrozenCmdIds` | state.js | Commands SIGSTOP'd during search |
| `_lastRenderedPanelCount` | state.js | Panel count for structural skip |
| `_lastRenderedPanelIds` | state.js | Panel ID fingerprint for skip |
| `_lastSplitState` | state.js | Split state fingerprint for skip |
| `_lastShowingWelcome` | state.js | Welcome state fingerprint for skip |
| `_snapshotLoaded` | commands.js | Whether initial snapshot was fetched |
| `_userSpawnInstUrl` | commands.js | User's explicit spawn instance choice |
| `_spawnCompletions[]` | spawn.js | Tab completion matches |
| `_serverTemplates[]` | templates.js | Cached server templates |
| `_serverEnvironments[]` | workspaces.js | Cached server environments |
| `_draggedCmd` | dragdrop.js | Currently dragged command |
| `_reorderState` | dragdrop.js | Sidebar reorder drag state |

### 5.4 localStorage Keys

| Key | Module | Purpose |
|-----|--------|---------|
| `vrw_auth_token` | state | Authentication token |
| `vrw_font_size` | state | Global font size |
| `vrw_theme` | theme | Global theme (empty/grey/dark) |
| `vrw_update_mode` | state | Push or poll |
| `vrw_poll_interval` | state | Poll interval (ms) |
| `vrw_panel_layout` | state | Row/column/grid layout |
| `vrw_panel_font_<id>` | panels | Per-panel font size |
| `vrw_panel_sel_<id>` | panels | Per-panel selection mode |
| `vrw_panel_theme_<id>` | theme | Per-panel theme |
| `vrw_panel_title_<id>` | panels | Per-panel custom title |
| `vrw_show_resources` | state | Resource badge visibility |
| `vrw_sound` | notifications | Sound notification toggle |
| `vrw_refresh_ms` | misc | Refresh throttle (ms) |
| `vrw_pinned_cmds` | sidebar | Pinned command names |
| `vrw_cmd_order` | dragdrop | Custom command reorder |
| `vrw_spawn_history` | spawn | Last 20 spawn commands |
| `vrw_templates` | templates | User-defined templates |
| `vrw_workspaces` | workspaces | Saved workspace configs |
| `vrw_environments` | workspaces | User-defined environments |
| `vrw_cmd_groups` | workspaces | Command groups |
| `vrw_group_collapsed` | workspaces | Collapsed group state |
| `vrw_onboarding_done` | onboarding | Tutorial completed |
| `vrw_bottombar_hidden` | sidebar | Bottom bar visibility |
| `vrw_scrollback_<cmdId>` | sessionStorage | Per-command scrollback offset |

---

## 6. Load Order

The 21 `<script>` tags in `index.html` (lines 407–426) **must** load in this exact order:

```
 1. state.js        ← Defines VRW.state; zero dependencies
 2. eventbus.js     ← Defines VRW.EventBus; zero dependencies
 3. utils.js        ← Pure functions; reads state.connections
 4. focus.js        ← Focus trap; zero dependencies
 5. theme.js        ← Reads state.panels for panel theme toggle
 6. sidebar.js      ← Reads utils, state; calls commands/search functions
 7. panels.js       ← Reads utils, state; calls focus.trapFocus
 8. commands.js     ← Reads utils, state; calls panels, sidebar, websocket, vtty
 9. websocket.js    ← Reads utils, state; calls vtty, notifications, commands
10. vtty.js         ← Reads utils, state; pure rendering, minimal cross-calls
11. spawn.js        ← Reads utils, state; calls commands, panels
12. logs.js         ← Reads utils, state; standalone log viewer
13. keyboard.js     ← Reads utils, state, focus; calls search, panels, commands
14. search.js       ← Reads utils, state, focus; calls commands, websocket
15. notifications.js ← Reads utils, state; calls commands
16. onboarding.js    ← Reads utils, focus
17. templates.js    ← Reads utils, state; calls commands
18. dragdrop.js     ← Reads utils, state; calls panels, commands
19. workspaces.js   ← Reads utils, state, focus; calls panels, commands, logs
20. misc.js         ← Re-exports from commands.js; orchestrates refresh loop
21. app.js          ← Bootstrap; calls init functions from all modules
```

### Why Order Matters

1. **State must load first** — all modules reference `state` at load time (inside IIFEs).
2. **Utils before consumers** — `escHtml`, `formatRuntime`, `apiUrl`, etc. are called inside IIFEs during module initialization (e.g., `localStorage.getItem` in state.js runs before utils.js is loaded, but state.js doesn't call utils functions — it only defines data).
3. **Focus before panels/modals** — `trapFocus`/`releaseCurrentFocusTrap` are used by panels.js, commands.js, search.js, onboarding.js, workspaces.js, and misc.js.
4. **Panels before commands** — `commands.js` calls `focusPanel()`, `renderPanels()`, `addPanelDirect()` from panels.js.
5. **Commands before websocket/vtty** — websocket.js and vtty.js call `getActivePanelId()`, `getSelectedPanel()`, `updateVttyDisplayForPanel()` from commands.js.
6. **Feature modules before misc** — misc.js re-exports connection helpers from commands.js.
7. **app.js last** — it invokes initialization functions from all other modules.

---

## 7. Test Coverage

### Test Infrastructure

- **Location:** `/static/admin/test/`
- **Runner:** Node.js with `require('./setup')` which provides a DOM shim (`jsdom` or similar) and custom `assert`/`assertEq` macros.
- **Pattern:** Each test file follows `test_<module>.js` naming.

### Test Files (21 files)

| Test File | Module Under Test | Test Count (est.) |
|----------|-----------------|-------------------|
| `test_state.js` | state.js | 10 |
| `test_eventbus.js` | eventbus.js | ~5 |
| `test_utils.js` | utils.js | ~25 |
| `test_focus.js` | focus.js | ~10 |
| `test_theme.js` | theme.js | ~8 |
| `test_sidebar.js` | sidebar.js | ~15 |
| `test_panels.js` | panels.js | ~15 |
| `test_commands.js` | commands.js | ~20 |
| `test_websocket.js` | websocket.js | ~15 |
| `test_vtty.js` | vtty.js | ~20 |
| `test_spawn.js` | spawn.js | ~15 |
| `test_logs.js` | logs.js | ~10 |
| `test_keyboard.js` | keyboard.js | ~10 |
| `test_search.js` | search.js | ~15 |
| `test_notifications.js` | notifications.js | ~10 |
| `test_onboarding.js` | onboarding.js | ~8 |
| `test_templates.js` | templates.js | ~10 |
| `test_dragdrop.js` | dragdrop.js | ~10 |
| `test_workspaces.js` | workspaces.js | ~15 |
| `test_misc.js` | misc.js | ~10 |
| `test_refresh.js` | refresh integration | ~5 |
| `test_welcome_guard.js` | welcome panel guard | ~5 |

**Estimated total:** ~250 assertions across 21 test files.

### Coverage Summary

| Area | Coverage |
|------|----------|
| State initialization & defaults | ✅ Good |
| Pure utility functions | ✅ Good |
| Focus trap | ✅ Good |
| Panel rendering | ⚠️ Limited (DOM-dependent, hard to unit test) |
| VTTY diff application | ⚠️ Limited (requires cell grid DOM) |
| WebSocket message routing | ⚠️ Limited (requires WebSocket mock) |
| Keyboard/mouse handling | ❌ Not tested (event-driven, browser-only) |
| Drag-and-drop | ❌ Not tested (mouse event simulation) |
| End-to-end flows | ❌ No integration tests |

---

## 8. Known Issues & Technical Debt

### 8.1 Global State Mutation

All modules share a single mutable `state` object with no change notification system. Any module can write to any field at any time, making it difficult to reason about data flow and impossible to implement undo/redo or time-travel debugging.

### 8.2 Window.* Global Scope Pollution

Despite the IIFE pattern, every module selectively exports functions to `window.*` (global scope). There are **~200+ globally exposed functions**. This creates:
- Risk of naming collisions with browser extensions or third-party scripts.
- No tree-shaking or dead-code elimination.
- Difficulty in determining which module a function belongs to.

### 8.3 Duplicate Function Definitions

Several functions are defined in multiple modules (both `websocket.js` and `vtty.js` define `updateVttyDisplay`; both `commands.js` and `misc.js` define `addConnection`/`removeConnection`). The last-loaded definition wins. This is fragile and error-prone.

### 8.4 DOM-String Concatenation Rendering

HTML is built via string concatenation with `escHtml()` for XSS protection. This is:
- **Fragile** — missing an `escHtml()` call is an XSS vulnerability.
- **Slow** for large DOMs — full `innerHTML` replacement triggers HTML parsing.
- **Not composable** — no component model, no slots, no declarative templates.

The Level 3 cell-grid diff partially mitigates the performance concern for terminal updates, but sidebar re-renders still do full `innerHTML` replacement.

### 5. No Debounce/Throttle Utility

Debouncing is implemented ad-hoc with `setTimeout`/`clearTimeout` in multiple modules (scroll detection, VTTY HTTP fetch, resize, log WS reconnect, mouse move throttle). There is no shared debounce utility.

### 8.6 Tight Coupling Between Panels and Commands

`panels.js` is 1,595 lines — far too large. It handles layout, rendering, context menus, copy/export, screenshots, max-fit, rename, and shared toolbar. It should be further decomposed.

### 8.7 Legacy Global WebSocket (`vttyWs`)

The per-panel WebSocket system (`panel.ws`) was added later, but the legacy global `state.vttyWs` still exists and is referenced in a few places. The `state.connections` array (not defined in state.js but used everywhere) is defined dynamically in `app.js`.

### 8.8 No Error Boundaries

WebSocket errors, HTTP fetch failures, and JSON parse errors are silently caught with `catch (e) { /* ignore */ }` or `console.error(...)`. There is no centralized error reporting or user-facing error UI for failed operations.

### 8.9 Missing `state.connections` Definition

`state.connections` is not defined in `state.js` (130 lines). It is created dynamically in `app.js` via `state.connections = [...]`. This means any module that reads `state.connections` before `app.js` runs will get `undefined` (though in practice, modules only define functions at load time — they don't call them until after `app.js` runs).

### 8.10 Mobile Support is Incomplete

Mobile detection (`window.innerWidth <= 768`) triggers a tabbed layout, but the touch event handling, responsive breakpoints, and mobile-specific UX are minimal.

---

## 9. Future Improvements

### 9.1 Short-Term (Low Risk)

1. **Deduplicate global exports.** Move shared functions (`addConnection`, `removeConnection`, `updateVttyDisplay`) to a single owner module and import from there. Eliminate the "last writer wins" pattern.

2. **Add a debounce/throttle utility to utils.js.** Replace all ad-hoc `setTimeout`/`clearTimeout` patterns with a shared `debounce(fn, ms)` and `throttle(fn, ms)`.

3. **Define `state.connections` in state.js.** Add `connections: []` to the state object so it's always defined, even before `app.js` runs.

4. **Add error boundary.** Create a `showError(msg)` function and replace all `/* ignore */` catch blocks with at minimum a console warning and optionally a user-facing toast notification.

### 9.2 Medium-Term (Moderate Risk)

5. **Decompose panels.js.** Split into `panels-layout.js` (render, layout presets, resize handles), `panels-toolbar.js` (shared toolbar, font/theme controls, max-fit), `panels-context.js` (context menus, copy/export/screenshot), and `panels-rename.js` (double-click rename).

6. **Introduce a proper event system.** Leverage the existing `VRW.EventBus` (currently unused) or adopt a more structured approach where state mutations emit events. This would enable reactive UI updates without tight coupling.

7. **Add integration tests.** Create browser-based integration tests using Playwright or Puppeteer to test real user flows: spawn a command, select it, verify terminal output renders, kill it, verify exit notification.

8. **Replace innerHTML with a lightweight templating approach.** Even without a framework, tagged template literals with auto-escaping would reduce XSS risk and improve readability:
   ```js
   const html = `<div class="cmd-item">${esc(cmdName)}</div>`;
   ```

### 9.3 Long-Term (High Risk)

9. **Adopt ES Modules.** Replace IIFE + `window.*` with `import`/`export`. This enables:
   - Static analysis of dependency graph
   - Tree-shaking (if a bundler is ever added)
   - Named imports (no more global namespace pollution)
   - TypeScript migration path

10. **Consider a component framework.** The panel rendering, sidebar rendering, and modal system are essentially a component tree. Even a lightweight library (Preact ~3KB, Lit ~5KB) would eliminate the string-concatenation rendering pattern and provide proper lifecycle management.

11. **Implement a state management library.** The 50+ field mutable state object could be replaced with a reactive store (e.g., a simple `createStore` with subscribers) to make data flow explicit and auditable.

12. **WebSocket multiplexing.** Currently each panel opens its own WebSocket. For users with many panels, this creates N WebSocket connections. A multiplexed approach (one WS, server-side fan-out per command) would reduce connection overhead.

---

## Appendix A: API Endpoints Used

| Method | Endpoint | Used By |
|--------|----------|---------|
| GET | `/api/info` | commands.js (server config) |
| GET | `/api/commands` | commands.js (load commands) |
| POST | `/api/commands` | spawn.js, templates.js |
| GET | `/api/commands/:id/vtty/html` | vtty.js (fetch HTML) |
| GET | `/api/commands/:id/vtty/changed` | websocket.js (poll mode) |
| GET | `/api/commands/:id/vtty/text` | search.js (global search) |
| GET | `/api/commands/:id/vtty/buffer` | vtty.js (buffer switch) |
| POST | `/api/commands/:id/keys` | keyboard.js, spawn.js, panels.js |
| POST | `/api/commands/:id/kill` | spawn.js, search.js |
| POST | `/api/commands/:id/freeze` | commands.js, search.js |
| POST | `/api/commands/:id/thaw` | commands.js, search.js |
| POST | `/api/commands/:id/unkeep` | spawn.js |
| POST | `/api/commands/:id/keep` | spawn.js |
| POST | `/api/commands/:id/restart` | commands.js, search.js |
| POST | `/api/commands/:id/resize` | spawn.js |
| POST | `/api/commands/:id/mouse` | keyboard.js |
| GET | `/api/commands/:id/resources` | notifications.js |
| DELETE | `/api/commands/:id` | search.js |
| GET | `/api/commands/:id/handles` | (API reference) |
| GET | `/api/commands/lookup/:name` | commands.js (URL routing) |
| GET | `/api/certificates` | commands.js |
| GET | `/api/templates` | templates.js |
| GET | `/api/environments` | workspaces.js |
| GET | `/api/log` | logs.js |
| GET | `/api/completions` | spawn.js (tab completion) |
| WS  | `/api/commands/:id/ws` | websocket.js (per-panel push) |
| WS  | `/api/ws/logs` | logs.js (log streaming) |

---

## Appendix B: File Size Summary

| Module | Lines | Size Category |
|--------|-------|---------------|
| state.js | 130 | Tiny |
| eventbus.js | 13 | Tiny |
| utils.js | 147 | Tiny |
| focus.js | 93 | Tiny |
| theme.js | 86 | Tiny |
| onboarding.js | 147 | Tiny |
| notifications.js | 205 | Small |
| misc.js | 165 | Small |
| logs.js | 234 | Small |
| sidebar.js | 448 | Medium |
| keyboard.js | 550 | Medium |
| search.js | 529 | Medium |
| dragdrop.js | 303 | Medium |
| spawn.js | 510 | Medium |
| websocket.js | 780 | Large |
| vtty.js | 799 | Large |
| commands.js | ~1,084 | Large |
| workspaces.js | 782 | Large |
| panels.js | ~1,595 | XL |
| app.js | 169 | Small |
| **Total** | **~9,264** | **—** |

The refactored 21-file structure totals ~9,264 lines, compared to the original single-file monolith of 8,833 lines. The ~400-line increase is due to module headers, IIFE wrappers, and `window.*` export statements added during the refactor — the actual logic is unchanged.

---

*End of document.*
