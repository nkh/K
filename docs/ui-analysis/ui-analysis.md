# rvw Admin Web UI — Code Analysis

## 1. Overview

| Metric | Value |
|--------|-------|
| Total JS modules | 23 |
| Total JS lines | ~10,700 |
| HTML lines | 389 |
| CSS lines | 724 |
| Extract script (Python) | ~100 lines |
| Exported functions | ~150 |
| Functions over 50 lines | 42 |
| Functions over 100 lines | 5 |

## 2. Architecture Problems

### 2.1 Dead Code: EventBus (eventbus.js)

`eventbus.js` (13 lines) implements a complete pub/sub system (`on`, `off`, `emit`, `once`) on `VRW.EventBus`. **It is never called by any other module.** All cross-module communication uses raw `window.*` globals. This module should be deleted, or the codebase should be migrated to use it instead of direct global calls.

### 2.2 No Module Encapsulation — Everything Is a Global

Every module is an IIFE that assigns functions to `window.*`. There is no import/require system. Cross-module calls are direct function references:
```
// sidebar.js calls:
renderPanels();            // panels.js
loadCommands();            // commands-core.js
selectCommand(url, id);    // command-selection.js
```

This creates **implicit bidirectional dependencies**. For example:
- `sidebar.js` → calls `renderPanels()` (panels.js)
- `panels.js` → calls `_buildSidebar()` (sidebar.js)
- `panels.js` → calls `selectCommand()` (command-selection.js)
- `command-selection.js` → calls `renderPanels()` (panels.js)
- `command-selection.js` → calls `_buildSidebar()` (sidebar.js)

This is a **circular dependency graph**: `panels ↔ sidebar ↔ command-selection ↔ panels`.

### 2.3 State Is a God Object

`state.js` defines a single `state` object with **54+ properties**, plus **9 module-level `let` variables** that are also state but live outside the object. Properties include:

- **Legacy globals** (never cleaned up from pre-panel era): `vttyWs`, `vttyWsUrl`, `vttyWsCmdId`, `selectedInstUrl`, `selectedCmdId`
- **Per-panel data mixed with global data**: `panels[]` (array of panel objects) coexists with `selectedCmdId` (global selection)
- **UI transient state**: `_userAtBottom`, `_userScrolling`, `_userScrollTimer`, `_pendingVttyData`, `_pendingVttyDirty`
- **Cache state**: `_cellGrids`, `_cachedDomPre`, `_cachedScrollPos`, `_resourceCache`
- **Server config**: `refreshMs`, `pollInterval`, `updateMode`, `serverPollMs`, `serverDirtyMs`, `serverScreenshotFontSize`, `serverScreenshotFontName`

Every module reads and writes directly to `state.*` with no validation, no change notification, no encapsulation.

### 2.4 Global → Per-Panel Migration Half-Done

The codebase was originally single-pane and is being migrated to multi-panel. This migration is incomplete and has created **parallel code paths** for everything:

| Concept | Global (legacy, still active) | Per-panel (new) |
|---------|------------------------------|-----------------|
| WebSocket | `connectVttyWs()` 157 lines | `connectPanelWs()` 137 lines |
| HTTP poll | `scheduleVttyHttp()` | `scheduleVttyHttpForPanel()` |
| VTTY display | `updateVttyDisplay()` | `updateVttyDisplayForPanel()` |
| VTTY diff | `applyVttyDiff()` 88 lines | `applyVttyDiffForPanel()` 94 lines |
| VTTY metadata | `updateVttyMetadata()` | `updateVttyMetadataForPanel()` |
| VTTY HTTP load | `loadVttyHttp()` 79 lines | `loadVttyHttpForPanel()` |
| Update mode | `startUpdateMode()` / `stopUpdateMode()` | `startPanelUpdateMode()` / `stopPanelUpdateMode()` |
| Poll | `startPoll()` / `stopPoll()` | `startPanelPoll()` / `stopPanelPoll()` |

That's **14 functions duplicated** (~800 lines of near-identical code). The global versions should be deleted.

### 2.5 "Secondary Pane" — Another Full Clone

`websocket.js` contains an entire "secondary pane" subsystem:
- `_connectSecondaryWs()` — 111 lines
- `_disconnectSecondaryWs()` — 22 lines
- `_updateSecondaryVttyDisplay()` — 30 lines
- `_updateSecondaryVttyMetadata()` — 31 lines
- `_applySecondaryVttyDiff()` — 38 lines
- `_loadSecondaryVttyHttp()` — 26 lines
- `scheduleSecondaryVttyHttp()` — 13 lines

Total: **~270 lines** duplicating the primary pane logic. This appears to be a split-pane feature that was implemented by copy-pasting the entire WS/VTTY pipeline instead of parameterizing the existing per-panel code.

## 3. Function Complexity — The Worst Offenders

### Top 10 Largest Functions

| # | Function | Module | Lines | Problem |
|---|----------|--------|-------|---------|
| 1 | `renderPanels()` | panels.js | **302** | Builds entire panel UI as one HTML string. Contains: layout detection, fast-path vs full-rebuild branching, panel container HTML, panel header HTML (with all buttons), split containers, minimized panels, mobile tabs, shared toolbar, WS reconnection loop. Must be split into at least 8 sub-functions. |
| 2 | keydown handler (anonymous) | keyboard.js | **198** | Single anonymous function with deeply nested if/else chains handling all keyboard shortcuts. No lookup table, no key→action map. |
| 3 | `selectCommand()` | command-selection.js | **101** | Handles primary pane selection, secondary pane selection, panel history push, DOM cache, sidebar update, VTTY load, disconnected overlay. |
| 4 | `updatePanelCommandInfo()` | command-ui.js | **111** | Loops all panels, builds header HTML with command name, server badge, pause button, restart button, resource badge, exited banner. |
| 5 | `_buildSidebar()` | sidebar.js | **132** | Contains nested `renderCmdList()` (124 lines). Builds entire sidebar HTML with command items, inline event handlers, freeze buttons, keep badges. |
| 6 | `connectVttyWs()` | websocket.js | **157** | Legacy global WS. Entire lifecycle: URL construction, WS creation, ping/pong, onmessage → throttle → display, onclose → reconnect, onerror. |
| 7 | `connectPanelWs()` | websocket.js | **137** | Per-panel WS. Near-identical to `connectVttyWs()` — same ping/pong, message handling, reconnect logic. |
| 8 | `_connectSecondaryWs()` | websocket.js | **111** | Yet another WS setup. Same pattern again. |
| 9 | `showSpecialKeysHelp()` | panels.js | **97** | Builds a modal with an HTML table of special key codes. Pure string building. |
| 10 | `applyVttyDiffForPanel()` | vtty.js | **94** | Applies VTTY cell-level diffs to the DOM. Complex but inherently so. |

### Functions Over 50 Lines (Complete List — 42 total)

**panels.js (12):** renderPanels (302), updateSharedToolbar (87), showSpecialKeysHelp (97), screenshotPanel (77), showPanelContextMenu (75), toggleMaxFit (72), showCmdContextMenu (62), onPanelDrop (62), closePanelContent (54), startRenamePanel (51), _renderSplitContainer (68), confirmAddPanel (47→no)

**websocket.js (3):** connectVttyWs (157), connectPanelWs (137), _connectSecondaryWs (111)

**vtty.js (6):** applyVttyDiffForPanel (94), applyVttyDiff (88), loadVttyHttp (79), updateVttyMetadata (55), updateVttyDisplay (53), _rebuildStyle (53)

**sidebar.js (4):** _buildSidebar (132), renderCmdList (124), rearrangePinnedCommands (55), updateTerminalDisconnectedOverlay (50)

**command-ui.js (1):** updatePanelCommandInfo (111)

**command-selection.js (1):** selectCommand (101)

**keyboard.js (1):** anonymous keydown (198)

**dragdrop.js (2):** _cmdReorderMouseMove (78), _cmdReorderMouseUp (70)

**server-connections.js (2):** fetchServerConfig (54), confirmAddServer (51)

**commands-core.js (1):** loadCommands (76)

**spawn.js (1):** spawnCommand (varies)

**workspaces.js (1):** _renderWorkspaceList (varies)

**logs.js (1):** _buildLogContent (varies)

## 4. Cross-Module Coupling Matrix

How many other modules each module calls (outgoing dependencies):

| Module | Calls into | Called by |
|--------|-----------|-----------|
| panels.js | **15+** modules | 10+ modules |
| websocket.js | **12+** modules | 6 modules |
| command-selection.js | **10+** modules | 8 modules |
| sidebar.js | **8+** modules | 10+ modules |
| server-connections.js | **15+** modules | 5 modules |
| vtty.js | **8+** modules | 6 modules |
| command-ui.js | **5** modules | 8+ modules |
| commands-core.js | **8+** modules | 6 modules |

`panels.js` is the worst — it's called by nearly everything AND calls nearly everything. It's the god module.

## 5. Runtime Problems

### 5.1 renderPanels() — Full DOM Rebuild Destroys State

`renderPanels()` has a "fast path" that skips DOM rebuild when nothing changed. But the fast-path check depends on comparing:
- Panel count
- Panel IDs
- Split state
- Welcome state

If ANY of these change, it does a **full `innerHTML` rebuild** of the entire panel area. This:
1. Destroys all DOM elements including `<pre>` terminals
2. Invalidates all event listeners set on those elements
3. Kills any ongoing CSS animations
4. Causes visible flicker

The WS `onmessage` handler uses `document.getElementById()` to find the `<pre>` element, so it survives the rebuild — but only if the IDs match. The rebuild was causing the terminal freeze bug because:
- Full rebuild → old `<pre>` destroyed → WS closure still references old DOM via closure variables → new `<pre>` exists but `panelEl` in the closure is stale

### 5.2 _throttleRefresh() — Single Timer for All Panels

`_throttleRefresh()` in `misc.js` uses a single `state._refreshThrottleTimer`. When panel A's WS fires, it sets the timer. When panel B's WS fires 10ms later, the timer is already set, so panel B's update is silently dropped. When the timer fires, `_flushThrottledRefresh()` was only flushing the globally-selected command, not all panels.

This was the root cause of the terminal freeze bug. The fix (per-panel timers in `_flushThrottledRefresh`) was a band-aid — the real fix is to not have a global throttle at all.

### 5.3 Stale Closure References

WS `onmessage` closures capture `panelEl` at connection time:
```js
// In connectPanelWs:
ws.onmessage = function(event) {
    const panelEl = document.getElementById('panel-' + panelId);
    // ^ This re-queries DOM each time — OK
    // But some code paths use the panelObj.ws reference which may be stale
};
```

When `renderPanels()` does a full rebuild, the `panelObj.ws` is NOT disconnected (it was, but then the rebuild reconnection code reconnects). The race condition between "disconnect old WS" and "rebuild DOM" and "reconnect new WS" creates windows where updates are lost.

### 5.4 No Debouncing on User Input

`loadCommands()` is called on every keystroke in the filter input (`oninput="loadCommands()"`). Each call makes HTTP requests to ALL connected servers. With 3 servers, typing "htop" fires 4 HTTP request rounds.

### 5.5 Inline Event Handlers — Memory Leak Risk

`renderPanels()` and `_buildSidebar()` build HTML strings with inline `onclick` handlers:
```js
`<button onclick="selectCommand('${url}','${id}','${name}')">`
```

Each full rebuild creates new function closures for every button. The old functions from the previous rebuild are not cleaned up — they reference the old DOM nodes, preventing GC.

### 5.6 Excessive DOM Queries

`updateSharedToolbar()` (87 lines) calls `document.getElementById()` **20+ times** per invocation. It's called after every `renderPanels()`, `focusPanel()`, and command state change. Most of these queries could be cached.

## 6. Code Organization Problems

### 6.1 HTML String Building Everywhere

`panels.js` builds ~300 lines of HTML via string concatenation. `sidebar.js` builds ~250 lines. Neither uses a template engine, virtual DOM, or even `document.createElement()`. This makes:
- XSS possible if any data is untrusted (mitigated by `escHtml()` but not guaranteed)
- Debugging hard — no way to set breakpoints inside string templates
- Testing impossible — no way to unit test rendering without a full DOM

### 6.2 State Scattered Across Modules

State lives in three places:
1. `state` object (state.js) — 54+ properties
2. Module-level `let` variables in state.js — 9 variables (e.g., `_lastCommandState`, `_showingWelcome`)
3. Module-level variables in other modules — e.g., `_draggedCmd` in dragdrop.js, `_focusState` in focus.js, `_maxFitState` in panels.js, `_splitState` in panels.js

There's no single source of truth. The same logical concept (e.g., "is this command selected?") is tracked in both `state.selectedCmdId` AND by CSS classes on DOM elements.

### 6.3 Mixed Concerns in Single Modules

**panels.js (1901 lines)** handles:
- Panel CRUD (add, remove, split, unsplit, minimize)
- Layout management (grid, presets)
- Shared toolbar rendering
- Context menus
- Terminal operations (copy, export, screenshot)
- Keyboard input to panels (sendKeysToPanel)
- Drag and drop
- Panel rename
- Max-fit/max-font sizing
- VTTY container rendering

This should be at least 5 separate modules.

**server-connections.js (529 lines)** handles:
- Server connection management
- Certificate loading
- Freeze/thaw
- Command restart
- Server config fetching
- Add server modal
- Spawn from welcome

**sidebar.js (547 lines)** handles:
- Sidebar toggle
- Tab switching
- Command list rendering
- Pin/reorder commands
- Bottom bar
- Logs view
- Disconnected overlay/banner
- Resource toggling

### 6.4 Three Freeze/Thaw Implementations

1. `togglePauseRun()` — global version (server-connections.js)
2. `togglePauseRunPanel()` — per-panel version (server-connections.js)
3. `togglePauseRunPanelByIdx()` — sidebar inline version (called from onclick in HTML string)

All three do nearly the same thing: find the command, check if frozen, call freeze or thaw API. They differ only in how they identify which command.

### 6.5 Undocumented `_cacheTerminalForSwitch` / `_restoreCachedDom`

When switching between commands in a panel, the current terminal DOM is cached and restored when switching back. This is a significant feature (~40 lines across 2 functions) with no documentation explaining:
- When caching happens vs when it doesn't
- What gets cached (innerHTML? scroll position?)
- When the cache is invalidated
- Memory implications (cached DOMs are never evicted)

## 7. Quantitative Complexity

### Lines of Code by Module

| Module | Lines | Functions | Functions >50L | State Reads | State Writes | DOM Queries | Cross-Module Calls |
|--------|-------|-----------|----------------|-------------|--------------|--------------|-------------------|
| panels.js | 1,901 | 43 | 12 | 120+ | 20+ | 60+ | 40+ |
| websocket.js | 772 | 21 | 3 | 40+ | 15+ | 15+ | 30+ |
| vtty.js | 787 | 16 | 6 | 35+ | 10+ | 25+ | 20+ |
| sidebar.js | 547 | 16 | 4 | 50+ | 10+ | 20+ | 15+ |
| keyboard.js | 561 | 1 | 1 | 10+ | 5+ | 5+ | 20+ |
| dragdrop.js | 291 | 8 | 2 | 5+ | 5+ | 10+ | 10+ |
| server-connections.js | 529 | 17 | 2 | 50+ | 15+ | 15+ | 30+ |
| command-selection.js | 277 | 11 | 1 | 30+ | 15+ | 10+ | 20+ |
| command-ui.js | 200 | 5 | 1 | 20+ | 5+ | 10+ | 5+ |
| spawn.js | ~400 | ~12 | 1 | 15+ | 5+ | 10+ | 10+ |
| commands-core.js | 198 | 7 | 1 | 15+ | 5+ | 5+ | 15+ |
| workspaces.js | ~400 | ~10 | 1 | 10+ | 5+ | 10+ | 5+ |
| search.js | ~500 | ~8 | 2 | 10+ | 10+ | 10+ | 5+ |
| logs.js | ~300 | ~6 | 1 | 5+ | 2+ | 5+ | 3+ |
| state.js | 131 | 0 | 0 | 0 | 54+ | 0 | 1 |
| eventbus.js | 13 | 4 | 0 | 0 | 0 | 0 | 0 |
| focus.js | 93 | 3 | 0 | 0 | 0 | 0 | 0 |
| utils.js | 148 | 9 | 0 | 4+ | 0 | 1+ | 0 |
| misc.js | ~130 | 3 | 0 | 5+ | 3+ | 5+ | 3+ |
| notifications.js | ~80 | 3 | 0 | 2+ | 0 | 2+ | 0 |
| theme.js | ~100 | 4 | 0 | 3+ | 2+ | 5+ | 0 |
| snapshot.js | ~80 | 3 | 0 | 2+ | 0 | 3+ | 2+ |
| templates.js | ~60 | 2 | 0 | 0 | 0 | 2+ | 2+ |
| **TOTAL** | **~10,700** | **~239** | **42** | **~530** | **~190** | **~250** | **~235** |

## 8. Specific Bugs and Fragilities

### 8.1 Terminal Freeze Bug (fixed with band-aid)
**Root cause**: `_throttleRefresh()` used a single global timer. `_flushThrottledRefresh()` only flushed the globally-selected command. Multi-panel setups silently dropped updates for non-focused panels.
**Fix applied**: Per-panel timer keys in `scheduleVttyHttpForPanel`, multi-panel flush in `_flushThrottledRefresh`.
**Proper fix**: Eliminate global throttle entirely; each panel's WS/HTTP pipeline should be independent.

### 8.2 renderPanels Fast-Path Bypass
`shouldShowWelcome` was missing `!state.serverReachable`, causing the fast-path to return `true` (no rebuild needed) when the server was disconnected. This meant the welcome screen never showed on disconnect.
**Status**: Fixed, but reveals how fragile the fast-path heuristic is.

### 8.3 WS Reconnection After Full Rebuild
After a full DOM rebuild in `renderPanels()`, WS objects on `panelObj` survive (their `onmessage` uses `document.getElementById` which finds new elements). But if the WS silently died during rebuild, there was no reconnection. Fix was added to reconnect dead WS after rebuild — but this is a symptom of the architectural problem (full DOM rebuilds shouldn't be needed).

### 8.4 scheduleVttyHttpForPanel Timer Collision
Was using `state._vttyHttpTimer` (single key) for all panels. Panel A's HTTP fetch could cancel panel B's pending fetch. Fixed with per-panel timer keys (`state._vttyHttpTimer_<panelId>`).

### 8.5 Icon/Color Changes Breaking Functionality
Multiple instances where changing a CSS color or icon character broke the UI because:
- Icons were emoji (immune to CSS `color`)
- Button styles were inline `style=""` attributes mixed with CSS classes
- The `btn-primary` class was toggled via JS but also overridden by inline styles

This indicates **no separation of concerns** between visual styling and functional state.

## 9. Refactoring Recommendations (by priority)

### P0: Delete Dead Code
1. **Delete `eventbus.js`** — 13 lines, never used
2. **Delete all global (non-panel) WS/VTTY functions** — `connectVttyWs`, `disconnectVttyWs`, `startUpdateMode`, `stopUpdateMode`, `startPoll`, `stopPoll`, `pollOnce`, `scheduleVttyHttp`, `loadVttyHttp`, `updateVttyDisplay`, `applyVttyDiff`, `updateVttyMetadata`, `updateVttyMetadataFromHttp` — ~600 lines
3. **Delete "secondary pane" WS system** — `_connectSecondaryWs` and 6 related functions — ~270 lines
4. **Remove legacy state properties** — `vttyWs`, `vttyWsUrl`, `vttyWsCmdId`, `selectedInstUrl`, `selectedCmdId` from the global state (only per-panel `panelObj.selectedCmdId` should exist)
5. **Remove `_htmlEscapeChar`** from utils.js (unused)

**Estimated reduction: ~900 lines**

### P1: Eliminate Circular Dependencies
1. Create a simple event system (or use the deleted EventBus) for cross-module communication
2. Panels should emit events ("command-selected", "panel-focused") instead of calling sidebar/command-selection directly
3. Sidebar should emit events ("command-clicked") instead of calling selectCommand/renderPanels directly
4. This breaks the panels↔sidebar↔command-selection cycle

### P2: Split panels.js
Split the 1901-line god module into:
1. `panels-layout.js` — panel CRUD, layout, grid management (~300 lines)
2. `panels-toolbar.js` — shared toolbar, button state (~150 lines)
3. `panels-render.js` — renderPanels and helpers (~300 lines, still large but focused)
4. `panels-context-menu.js` — right-click menus (~200 lines)
5. `panels-terminal-ops.js` — copy, export, screenshot, send keys (~200 lines)
6. `panels-dragdrop.js` — already partially separate (~100 lines)

### P3: Decompose renderPanels()
Split the 302-line function into:
1. `_shouldFullRebuild()` — 10 lines
2. `_buildPanelContainerHtml(panels)` — 30 lines
3. `_buildPanelHeaderHtml(panel)` — 40 lines
4. `_buildPanelBodyHtml(panel)` — 20 lines
5. `_buildSharedToolbarHtml()` — 50 lines
6. `_buildMobileTabsHtml(panels)` — 20 lines
7. `_buildWelcomeHtml()` — 20 lines
8. `_applyPostRenderUpdates()` — 30 lines (WS reconnect, toolbar sync)

### P4: Unify State Management
1. Move all module-level `let` variables into `state.*`
2. Delete `_lastCommandState`, `_navCommands`, `_showingWelcome` from module scope — put them in `state`
3. Add change notification (or at minimum, document which module owns which state)
4. Separate transient UI state (`_userScrolling`) from persistent state (`connections`, `panels`)

### P5: Replace HTML String Building
1. Use `document.createElement()` for panels (or a simple template function)
2. Move inline `onclick` handlers to `addEventListener` after DOM insertion
3. This eliminates XSS risk and enables unit testing

### P6: Consolidate Duplicate Logic
1. Merge `togglePauseRun`, `togglePauseRunPanel`, `togglePauseRunPanelByIdx` into one function with a `(instUrl, cmdId)` signature
2. Merge `fetchServerConfig` and `healthCheckConnections` (both ping servers)
3. Extract common "freeze/thaw API call" helper

## 10. Estimated Impact

| Action | Lines Removed | Lines Added | Net |
|--------|--------------|-------------|-----|
| Delete dead code (P0) | ~900 | 0 | **-900** |
| Split panels.js (P2) | 0 | ~50 (module boilerplate) | +50 |
| Decompose renderPanels (P3) | 0 | ~20 (function headers) | +20 |
| Unify freeze/thaw (P6) | ~60 | ~20 | **-40** |
| Delete secondary pane system (P0) | ~270 | 0 | **-270** |
| **Total estimated** | **~1,230** | **~90** | **~-1,140** |

The codebase could go from **~10,700 lines to ~9,560 lines** just from deleting dead/duplicate code, while becoming significantly more maintainable. Further reductions are possible by simplifying the remaining functions (P3 decomposition often reveals more dead code paths).