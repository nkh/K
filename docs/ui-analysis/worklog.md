# Refactoring Worklog

## Phase 0a: Remove diagnostic logging
- Removed all `[DBG]` console.log statements from `websocket.js` (8 lines) and `panels.js` (3 lines)
- Tests: 698→698 (no change, as expected)

## Phase 1a: Create api.js centralized API layer
- Created `modules/api.js` — single module with 29 methods covering all 24 unique HTTP endpoints + 2 WS endpoints
- All methods are async, use `apiUrl()` and `authHeadersForInstance()` from utils.js internally
- `connectVtty()` returns `{ ws, close(), readyState }` handle — replaces 3 duplicate WS setup functions (675 lines) with one 50-line implementation
- `connectLogWs()` returns same handle pattern
- `killAll()` handles both filtered (array of IDs) and unfiltered (kill-all endpoint) cases
- Added `test/test_api.js` — 37 tests verifying all methods exist, URL construction, handle objects
- Added `api.js` to `index.html` script load order (after utils.js)
- Added `api.js` to `test/setup.js` module load order
- Tests: 698→735 (37 new, all pass). Pre-existing 2 CSS failures unchanged.
- No existing modules migrated yet — api.js is available but not yet used by callers

## Phase 1b: Migrate all fetch() calls to api.js
- Migrated **37 fetch() calls** across **16 files** to use `api.*` methods
- Modules migrated: server-connections (7), spawn (11), panels (4), keyboard (2), commands-core (2), templates (3), notifications (1), websocket (2), workspaces (3), logs (1), search (6), snapshot (2), misc (1), vtty (5), sidebar (1)
- Added `api.getSnapshot()`, `api.getPeers()`, `api.getVttyText()`, `api.getJson()` to cover all endpoints
- **Zero fetch() calls remain outside api.js**
- Net result: **-156 lines** (259 removed, 103 added)
- Tests: 735→735, 0 regressions
---
Task ID: 0b
Agent: main
Task: Upgrade test framework with controllable fetch mock, async support, and comprehensive api.js tests

Work Log:
- Discovered that api.js captures fetch reference at eval time — replacing globalThis.fetch after module load has no effect
- Upgraded setup.js fetch mock to be controllable via _setFetchJson/_setFetchError/_setFetchText/_setFetchBlob/_resetFetch
- Added _fetchCalls array for tracking fetch calls in tests
- Made resetTestState() also call _resetFetch() to clean up between tests
- Upgraded run_all.js to async: supports _asyncTest promise for async test files, adds 10ms drain delay
- Created helpers.js with mock factories (createMockApi, createMockState, createMockRender, createFetchMock, createMockEvent)
- Added 12 new assertion helpers: assertDeepEq, assertNotEq, assertIncludes, assertGt, assertLt, assertType, assertInstanceOf, assertNull, assertNonNull, assertLength, assertProperty, assertThrowsAsync, assertResolves
- Created test_api_proper.js with 166 comprehensive tests for api.js
- Fixed test_commands-core.js: replaced globalThis.fetch replacement with _setFetchJson/_setFetchError (which actually work with captured-fetch pattern)
- Fixed test_commands.js: converted async loadCommands test to use _asyncTest pattern, restored real loadCommands from _realFunctions

Stage Summary:
- 166 new tests for api.js (all HTTP endpoints, auth, URL construction, WebSocket behavior, error handling)
- Test framework now supports: controllable fetch, async tests, 12+ assertion types, mock factories
- Fixed 7 pre-existing test breakages caused by Phase 1a api.js migration (fetch capture issue)
- Net: 905 passed (+172), 2 failed (same pre-existing CSS check), 0 regressions
- Commit: 0de7558 on fix_overcomplicated_ui, pushed to origin

Key discovery: api.js (loaded via (0, eval) in setup.js) captures the fetch function reference at eval time. This means tests CANNOT replace fetch by setting globalThis.fetch — they must use the controllable mock provided by setup.js. This is a fundamental constraint of the current module loading approach and affects all future test writing.

## Phase 2a-b: Create event delegation system
- Created `modules/delegate.js` — metadata-driven event dispatcher replacing inline onclick
- 59 actions registered with signature-based argument builders
- 9 signature types: none, event, tab-el, panelId, panelId-delta, preset, delta, panelId-value, value
- `_dispatchAction()` resolves handler from `_actions` registry lazily via `window.*`
- `initDelegation()` sets up document-level click + change listeners once
- `_dispatchModalBackdrop()` handles click-outside-to-close for 4 modal overlays
- Added `data-action-placeholder` support for elements not yet migrated (ignored safely)
- Click delegation skips `<select>` and `<input>` (those use change delegation)
- Test infrastructure: implemented MockElement.closest(), .matches(), dataset Proxy (camelCase↔kebab-case), upgraded emitEvent() to accept pre-built events
- Added `test/test_delegate.js` — 30 tests covering all signatures, closest traversal, modal backdrop, initDelegation, unknown/missing handlers, panelId resolution
- Tests: 905→1015 (+110 new), 0 regressions
- Commits: 205b608, efc8595, ea86ab1

## Phase 2c-e: Migrate index.html onclick to data-action
- **65 `onclick` attributes → 0** in index.html
- **65 `data-action` attributes added**
- **4 `data-close-action` attributes** on modal overlays
- Topbar: 12 buttons migrated
- Sidebar: 8 elements migrated (5 tabs + 3 action buttons)
- Shared toolbar: 16 buttons migrated (including 5 layout presets, font size ±, buffer toggle)
- Log viewer: 2 buttons
- Global search modal: 2 buttons + backdrop
- Add panel modal: 2 buttons + backdrop
- Add server modal: 2 buttons + backdrop
- Command manager: 2 buttons + backdrop
- Bottombar: 3 selects/inputs
- `initDelegation()` called as first line in app.js init
- 17 form/input handlers remain (oninput, onkeydown, onfocus, onblur, ondragover, ondrop) with complex inline logic — deferred to later phases
- ~40 more onclick handlers exist in dynamically-generated HTML (sidebar.js, panels.js, workspaces.js, templates.js, search.js) — will be migrated during Phase 3 render layer extraction
- Tests: 1015 passed, same baseline failures. Zero regressions.

## Phase 2f: Dynamic action signatures
- Added 5 new signatures for Phase 3 readiness:
  - `cmd-select`: (instUrl, cmdId, cmdName) from data-* attrs
  - `cmd-id`: (instUrl, cmdId) from data-* attrs
  - `data-value`: single data-value attribute
  - `el-panelId`: data-panel-id for panel-specific buttons
  - `element`: passes the DOM element itself

## Phase 3a: Migrate all dynamic onclick= to data-action
- **48 `onclick=` strings → 0** across all JS modules (sidebar, panels, search, templates, workspaces, commands-core)
- **0 `onclick=` remain** in the entire codebase (HTML + JS)
- delegate.js additions:
  - 8 new signatures: `inst-url`, `cmd-name`, `index`, `value-str`, `name`, `name-index`, `cmd-context`, `data-panel`
  - 37 new actions covering sidebar commands, panel controls, search results, templates, workspaces, groups
  - `stop: true` flag on action definitions — calls `event.stopPropagation()` before dispatch to prevent parent `data-action` from firing
  - `contextmenu` delegation listener for `ShowCmdContextMenu` and `ShowPanelContextMenu` actions
- Helper functions added: `_sortSidebarBy`, `_selectAndViewCmd`, `_toggleCmdInGroupAndRender`, `closeCmdPicker`, `closeSpecialKeysModal`, `closeWorkspaceManage`
- `ondragstart`/`ondragover`/`ondrop`/`ondragleave`/`ondragend` kept as inline attributes (not `onclick` — different event type, requires `dataTransfer` setup)
- `oncontextmenu` kept as inline attribute on cmd-item and panel-header (contextmenu delegation dispatches from `data-action` but the same element also uses `data-action` for click — need separate mechanism)
- Tests: 2416 passed (+62 new delegate tests), 0 regressions, 3 pre-existing CSS failures unchanged
- Net: +198 lines (delegate.js growth + new helpers) / -50 lines (removed inline onclick strings)
- Commit: 8341374 on fix_overcomplicated_ui, pushed to origin

## Phase 4a-b: Delete dead code
- Comprehensive dead-code audit: cataloged all `window.*` exports, searched cross-references across 24 modules + index.html + extract_modules.py
- **Deleted `eventbus.js`** — entire 13-line module, zero production callers (only test_eventbus.js referenced it)
- **Deleted `test/test_eventbus.js`** — 59 lines of now-irrelevant tests
- **Deleted 6 orphaned functions** (defined + exported but never called from any production code):
  - `vtty.js`: `_prefetchVttyHtml` (33 lines, superseded by per-panel version)
  - `vtty.js`: `switchBuffer` (18 lines, global version superseded by `switchBufferPanel`)
  - `websocket.js`: `disconnectAllPanelWs` (6 lines, no caller exists)
  - `websocket.js`: `pollOnce` (5 lines, legacy wrapper, only `pollOncePanel` is used)
  - `search.js`: `openCmdManager` (5 lines, no UI entry point to trigger it)
  - `workspaces.js`: `renderEnvironments` (35 lines, defined but never invoked)
- Updated `index.html` (removed script tag), `setup.js` (removed stubs), `extract_modules.py` (removed exports), `test_regression.js` (EventBus assert → VRW.state assert)
- Not deleted (deferred to later phases):
  - `connectVttyWs`/`disconnectVttyWs` (160+20 lines) — dead global WS, but complex, will be removed when WS pipeline is unified
  - `api.connectVtty()`/`api.connectLogWs()` — unused API wrappers, kept for future use
  - `state.vttyWs`/`vttyWsUrl`/`vttyWsCmdId` — only used by dead `connectVttyWs`, will be cleaned with it
  - Module-level state vars (`_lastCommandState`, `_navCommands`, etc.) — used for change detection, Phase 5 target
- Net: **-130 lines** from modules (9975→9845), **-227 lines** total including test file
- Tests: 1072→1072 (removed 8 eventbus tests), 1069 passed, 3 failed (same pre-existing CSS), 0 regressions
- Commit: 237af66 on fix_overcomplicated_ui, pushed to origin

## Phase 5a: Remove deprecated state properties
- Removed `state.vttyWs`, `state.vttyWsUrl`, `state.vttyWsCmdId` (only used by dead `connectVttyWs`)
- Removed `state._vttyHttpTimer` (unused; actual timers use `state['_vttyHttpTimer_'+panelId]`)
- Removed `state._pollTimer` (never set; per-panel polling replaced global)
- Fixed `server-connections.js` poll-interval change handler (removed `_pollTimer` guard)
- Updated test_state.js, test_websocket.js, helpers.js, setup.js
- Tests: 1069 passed, 3 failed (same), 0 regressions
- Commit: ef8eb61 on fix_overcomplicated_ui, pushed to origin

## Phase 4c: Delete dead global WebSocket functions
- Removed `connectVttyWs` (156 lines) and `disconnectVttyWs` (23 lines) from websocket.js
- These were legacy single-command WS functions, completely superseded by per-panel `connectPanelWs`
- No external caller existed — only self-referential reconnection logic within the dead code
- Removed `window.connectVttyWs`, `window.disconnectVttyWs` exports
- Updated extract_modules.py
- Tests: 1069 passed, 3 failed (same), 0 regressions (test_websocket.js has typeof guards)
- websocket.js: 743→564 lines. Total modules: 9845→9657 lines
- Commit: d3e4dcc on fix_overcomplicated_ui, pushed to origin

## Session 2: Keyboard refactor + continued dead code cleanup

### Keyboard lookup table refactor
- Replaced 12-branch if/else shortcut chain (117 lines) with `_shortcuts` array (42 lines) + 11-line dispatch loop
- Added `_inInput()` helper to DRY up input-field checks
- All 12 shortcuts preserved with identical semantics
- keyboard.js: 552→490 lines (−62)
- Commit: 790235c, pushed

### Workspaces.js cleanup (−102 lines)
- 3 dead functions deleted: `toggleWorkspaceDropdown`, `saveCurrentWorkspace`, `openWorkspaceManage` (no callers, no delegate actions)
- 7 internal-only functions unexported: `renderMarkdown`, `saveCmdGroups`, `getGroupCollapsedState`, `saveGroupCollapsedState`, `getWorkspaces`, `saveWorkspaces`, `renderWorkspaceList`
- workspaces.js: 744→642 lines
- Commit: ab4d12a, pushed

### Spawn.js cleanup (−26 lines)
- 2 dead functions deleted: `sendKeys()` (superseded by `sendKeysToPanel`), `resizeTerminal()` (superseded by `resizeTerminalPanel`)
- 5 internal-only functions unexported: `_loadSpawnHistory`, `_saveSpawnHistory`, `_addSpawnHistoryEntry`, `_renderSpawnHistoryDropdown`, `_applySpawnHistoryEntry`
- spawn.js: 561→535 lines
- Commit: b615900, pushed

### Running totals
- **Start: ~10,700 lines, 24 modules**
- **Current: 9,151 lines, 19 modules** (−1,549 lines, −5 modules)
- **Target: ~5,200 lines, ~14 modules**
- **Remaining: ~3,951 lines to cut**
- Biggest targets remaining: panels.js (1856), vtty.js (728), workspaces.js (640), websocket.js (564), search.js (504), server-connections.js (506), sidebar.js (548)

## Session 3: Phase 5b/c, Phase 7a-d module consolidation

### Phase 5b: Remove change-detection vars (−68 lines)
- Removed 5 pure change-detection module-level vars from state.js:
  `_lastCommandState`, `_lastRenderedPanelCount`, `_lastRenderedPanelIds`, `_lastSplitState`, `_lastShowingWelcome`
- sidebar.js: removed fingerprint computation + early-return, kept _pendingSelectId logic
- panels.js: removed structural-unchanged guard (always rebuild now)
- Removed 12 writes across spawn.js (8), server-connections.js (2), dragdrop.js (1), workspaces.js (2), app.js (1)
- Fixed test_regression.js REG-09 (tested unexported internals), fixed test_app.js _showingWelcome reference
- Tests: 1815 passed, 0 regressions
- Commit: 56eadaa

### Phase 5c: Migrate display:none to .hidden CSS class
- Added `.hidden { display: none !important; }` to style.css
- Replaced ~70 `.style.display = 'none'/''` across 15 modules + 25 `style="display:none"` in index.html
- Updated 6 test files to use classList.contains('hidden')
- Net zero line change (same-length replacement) but eliminates inline style mutations
- Tests: 1815 passed, 0 regressions
- Commit: 024b0d2

### Phase 6: Verified — 32 test files, 6,554 lines, 1815 tests

### Phase 7a: Merge focus.js + theme.js → utils.js (−638 lines incl extract_modules.py)
- focus.js (93 lines) and theme.js (86 lines) merged into utils.js
- Deleted obsolete extract_modules.py (456 lines, referenced non-existent monolith)
- 22 modules → removed 2 script tags
- Commit: 6373365

### Phase 7b: Merge notifications.js → misc.js
- notifications.js (202 lines) merged into misc.js
- 21 modules
- Commit: 7649492

### Phase 7c: Merge command-ui.js → commands-core.js
- command-ui.js (200 lines) merged into commands-core.js
- 20 modules
- Commit: 66644d8

### Phase 7d: Merge snapshot.js → commands-core.js
- snapshot.js (138 lines) merged into commands-core.js
- loadSnapshot references vtty.js functions but only called at runtime — load order safe
- 19 modules
- Commit: 16dff21

### Session 3 totals
- Net: 9,467 → 9,151 lines (−316), 23 → 19 modules (−4)
- All changes tested: 1815 passed, 3 failed (pre-existing), 0 regressions

## Session 4: Phase 7e-h continued module consolidation

### Phase 7e: Merge command-selection.js → panels.js
- command-selection.js (277 lines) merged into panels.js
- 18 modules, 9,151 lines (net -0 lines, -1 module — content moved, wrapper removed)
- Tests: 1815 passed, 3 failed (same), 0 regressions
- Commit: 12df994

### Phase 7f: Merge templates.js → misc.js
- templates.js (194 lines) merged into misc.js
- misc.js loads after all callers (panels.js, server-connections.js, sidebar.js) — safe
- 17 modules

### Phase 7g: Merge logs.js → misc.js
- logs.js (233 lines) merged into misc.js
- 16 modules

### Phase 7h: Merge dragdrop.js → panels.js
- dragdrop.js (290 lines) merged into panels.js
- All dragdrop callers are panel-related functions — natural fit
- 15 modules
- Tests: 1816 passed, 3 failed (same), 0 regressions
- Commit: 5343b0f

### Phase 7i: Merge workspaces.js → sidebar.js
- workspaces.js (640 lines) merged into sidebar.js
- Includes: docs viewer, environments, command groups, workspaces
- 14 modules — **target module count reached**
- Tests: 1818 passed, 3 failed (pre-existing), 0 regressions
- Commit: ad33559

### Phase 8a: Delete legacy (non-panel) vtty functions + dead scroll-pause mechanism
- Migrated all callers of legacy functions to per-panel versions across 8 modules:
  - keyboard.js: `scheduleVttyHttp`→`scheduleVttyHttpForPanel`, `loadVttyHttp`→`loadVttyHttpForPanel` (3 sites)
  - search.js: `loadVttyHttp`→`loadVttyHttpForPanel` (1 site)
  - spawn.js: `startUpdateMode`→`startPanelUpdateMode`, `stopUpdateMode`→`stopPanelUpdateMode`, `loadVttyHttp`→`loadVttyHttpForPanel` (3 sites)
  - sidebar.js: `scheduleVttyHttp`→`scheduleVttyHttpForPanel`, removed `_flushPendingVttyUpdate` call (2 sites)
  - server-connections.js: `stopUpdateMode`→`stopPanelUpdateMode`, `startUpdateMode`→`startPanelUpdateMode`, `stopPoll`/`startPoll`→direct per-panel calls (4 sites)
  - commands-core.js: `startUpdateMode`→`startPanelUpdateMode` (1 site)
  - panels.js: `loadVttyHttp`→`loadVttyHttpForPanel`, deleted `_flushPendingVttyUpdate`, removed 5 `_pendingVttyData`/`_pendingVttyDirty` assignments
- Deleted 5 legacy functions from vtty.js (~270 lines):
  - `updateVttyDisplay`, `updateVttyMetadata`, `applyVttyDiff`, `scheduleVttyHttp`, `loadVttyHttp`
- Deleted 4 legacy wrappers from websocket.js (~26 lines):
  - `startPoll`, `stopPoll`, `startUpdateMode`, `stopUpdateMode`
- Removed dead `_userScrolling`/`_pendingVttyData`/`_pendingVttyDirty` mechanism:
  - The per-panel WS path never set `_pendingVttyData`, making the scroll-pause mechanism non-functional
  - Removed: scroll event listener in app.js, `_flushPendingVttyUpdate` in panels.js, 3 state properties
- Updated 15 test files to use per-panel versions
- vtty.js: 728→457 (−271), websocket.js: 564→538 (−26), panels.js: 2347→2317 (−30), state.js: 109→100 (−9)
- Tests: 1818 passed (same), 3 failed (pre-existing), 0 regressions

### Running totals
- **Start: ~10,700 lines, 24 modules**
- **Current: 8,677 lines, 14 modules** (−2,023 lines, −10 modules)
- **Target: ~5,200 lines, ~14 modules**
- **Remaining: ~3,477 lines to cut**
- Modules: state(100), utils(272), api(282), delegate(376), commands-core(390), vtty(457), keyboard(490), server-connections(502), search(504), spawn(523), websocket(538), misc(769), sidebar(1157), panels(2317)
- Biggest targets: panels.js (2317), sidebar.js (1157), misc.js (769), websocket.js (538), spawn.js (523)
