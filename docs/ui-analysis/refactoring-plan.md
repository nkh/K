# rvw Admin UI — Refactoring Plan

**Goal**: 80% less complexity, 50% less code. From ~10,700 lines to ~5,000 lines.

---

## The Problem in One Sentence

The UI has no architecture. It is 23 files of functions calling each other through `window.*` globals, mutating a 54-property state object, rebuilding DOM with innerHTML strings, and duplicating every code path for global/per-panel/secondary-pane variants.

## What the UI Actually Is

A terminal multiplexer viewer. That's it. The user:
1. Connects to one or more servers
2. Sees a list of running commands
3. Clicks a command → terminal output appears in a panel
4. The terminal updates live via WebSocket
5. A toolbar has buttons for freeze, copy, screenshot, layout

There is no routing, no forms to validate, no complex state machines, no offline sync, no real-time collaboration. It is a **read-only viewer** with a handful of write actions (freeze, kill, restart, spawn).

The entire thing should fit in ~5,000 lines: a server communication layer, a panel renderer, and event handlers.

---

## Architecture: Three Layers

```
┌─────────────────────────────────────────────┐
│  Layer 3: UI Handlers                        │
│  One function per UI element.                │
│  Calls Layer 2. Never touches DOM directly.  │
│  Pure logic → returns data/state changes.    │
├─────────────────────────────────────────────┤
│  Layer 2: Render                              │
│  One render function per UI area.             │
│  Reads state, writes DOM. Stateless.          │
│  Called after any state change.               │
├─────────────────────────────────────────────┤
│  Layer 1: Server Communication                │
│  HTTP and WebSocket client.                   │
│  Returns promises. No DOM, no state mutation. │
│  Single point of contact with the backend.    │
└─────────────────────────────────────────────┘
```

### Rule: Layers only call down, never up.

- Layer 3 calls Layer 2 and Layer 1.
- Layer 2 calls Layer 1 (to fetch data it needs to render).
- Layer 1 never calls Layer 2 or 3.
- No circular dependencies. Ever.

---

## Layer 1: Server Communication (~400 lines, replaces ~2,000)

### Current state: 15+ modules make HTTP calls directly

`server-connections.js`, `commands-core.js`, `websocket.js`, `vtty.js`, `sidebar.js`, `panels.js`, `command-selection.js`, `spawn.js`, `workspaces.js`, `search.js`, `logs.js`, `snapshot.js` — all contain `fetch()` calls with URL construction, auth headers, and error handling duplicated in each.

### Proposed: single `api.js` module

```js
// api.js — every server interaction in one place
const api = {

    // ── Connections ──
    ping(instUrl)              { /* GET /api/ping */ },
    getInstances(instUrl)      { /* GET /api/instances */ },
    getCommands(instUrl)       { /* GET /api/commands */ },
    getServerConfig(instUrl)   { /* GET /api/config */ },
    getCertificates(instUrl)   { /* GET /api/certificates */ },

    // ── Commands ──
    getVtty(instUrl, cmdId)    { /* GET /api/commands/:id/vtty */ },
    freeze(instUrl, cmdId)     { /* POST /api/commands/:id/freeze */ },
    thaw(instUrl, cmdId)       { /* POST /api/commands/:id/thaw */ },
    kill(instUrl, cmdId)       { /* DELETE /api/commands/:id */ },
    restart(instUrl, cmdId)    { /* POST /api/commands/:id/restart */ },
    keep(instUrl, cmdId, v)    { /* PUT /api/commands/:id/keep */ },
    spawn(instUrl, args)       { /* POST /api/commands/spawn */ },

    // ── WebSocket ──
    connectVtty(instUrl, cmdId, onMessage, onClose) {
        // Returns { ws, close() }
        // Handles: URL construction, auth, ping/pong, reconnect
        // onMessage callback receives parsed VTTY data
        // onClose callback receives reason code
    },

    // ── Other ──
    getLogs(instUrl, opts)     { /* GET /api/logs */ },
    getWorkspaces()            { /* GET /api/workspaces */ },
    search(instUrl, query)     { /* GET /api/search?q=... */ },
};
```

**What gets deleted:**
- All `fetch()` calls scattered across 15 modules
- All `authHeaders()` / `authHeadersForInstance()` / `apiUrl()` call sites
- All URL string construction logic duplicated everywhere
- `connectVttyWs()` (157 lines), `connectPanelWs()` (137 lines), `_connectSecondaryWs()` (111 lines) → one `connectVtty()` in api.js (~60 lines)
- All HTTP error handling duplicated in every module → centralized in api.js

**Lines saved: ~1,600**

### WebSocket design

Current problem: 405 lines across 3 near-identical WS setup functions, plus 270 lines for the "secondary pane" clone.

Proposed `api.connectVtty()`:

```js
connectVtty(instUrl, cmdId, { onMessage, onClose, onMetadata }) {
    const ws = new WebSocket(this._wsUrl(instUrl, cmdId));
    let pingTimer;

    ws.onopen = () => {
        pingTimer = setInterval(() => {
            this._pingSendTime = Date.now();
            ws.send(JSON.stringify({ type: 'ping' }));
        }, 15000);
    };

    ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        if (data.type === 'pong') {
            this._wsLatency = Date.now() - this._pingSendTime;
            return;
        }
        onMessage(data);
    };

    ws.onclose = (event) => {
        clearInterval(pingTimer);
        onClose(event);
    };

    return { ws, close: () => { clearInterval(pingTimer); ws.close(); } };
}
```

~40 lines. Replaces 675 lines. Each panel calls `api.connectVtty()` and gets back `{ ws, close }`. The panel manages its own connection lifecycle. No globals, no state mutations in the WS module.

---

## Layer 2: Render (~1,500 lines, replaces ~4,000)

### Current state: innerHTML string building, inline onclick, no separation

`renderPanels()` is 302 lines that builds the entire panel area as one string. `_buildSidebar()` is 132 lines + a 124-line nested function. `updatePanelCommandInfo()` is 111 lines. `updateSharedToolbar()` is 87 lines. All mix data lookup, HTML construction, and event handler attachment.

### Proposed: one render function per UI area, pure data→DOM

```js
// render.js — every render function takes state, returns/sets DOM
const render = {

    // ── Full areas ──
    sidebar(state)             { /* rebuilds sidebar HTML */ },
    panels(state)              { /* rebuilds panel grid */ },
    toolbar(panelState)        { /* updates shared toolbar buttons */ },
    welcome(state)             { /* shows/hides welcome screen */ },

    // ── Incremental updates (no full rebuild) ──
    commandList(commands)      { /* re-renders just the command items */ },
    panelHeader(panel)         { /* updates one panel's header */ },
    panelTerminal(panel, data) { /* updates one panel's <pre> content */ },
    panelMetadata(panel, meta) { /* updates cursor pos, term size */ },

    // ── Modals/overlays ──
    contextMenu(items, x, y)   { /* renders context menu */ },
    searchOverlay(state)       { /* renders search UI */ },
    addServerModal()           { /* renders add-server form */ },
};
```

### Key principles:

1. **Render functions never call handlers.** They only read state and write DOM.
2. **Incremental updates by default.** `panelTerminal(panel, data)` updates one `<pre>`. No full DOM rebuild.
3. **No inline onclick.** Event listeners are attached in Layer 3 (UI Handlers) after render, using `addEventListener` on stable container elements.
4. **Full rebuilds only on structural change** (panel added/removed, layout changed). And even then, use `document.createElement()` instead of innerHTML strings.

### What gets deleted:
- `renderPanels()` 302-line monolith → split into `render.panels()` (~80 lines) + helpers
- `_buildSidebar()` 132+124 lines → `render.sidebar()` (~60 lines) + `render.commandList()` (~50 lines)
- All "update" functions that rebuild HTML to change one attribute → `render.panelHeader()` which patches specific elements
- `updatePanelCommandInfo()` 111 lines → split into per-panel `render.panelHeader()` (~20 lines each, called only for the changed panel)

**Lines saved: ~2,500**

---

## Layer 3: UI Handlers (~800 lines, replaces ~3,500)

### Current state: handler logic scattered everywhere, mixed with rendering

`selectCommand()` is 101 lines doing selection + history + cache + sidebar update + VTTY load.
`togglePauseRun()` exists in 3 versions across 2 files.
`renderPanels()` contains reconnection logic.
`keyboard.js` has a 198-line anonymous handler with no lookup table.

### Proposed: one handler per UI element, event delegation

```js
// handlers.js — every interactive element has one handler
const handlers = {

    // ── Sidebar ──
    onCommandClick(instUrl, cmdId)    { /* select command into active panel */ },
    onCommandFreeze(instUrl, cmdId)   { /* toggle freeze */ },
    onCommandKill(instUrl, cmdId)     { /* kill command */ },
    onCommandKeep(instUrl, cmdId)     { /* toggle keep */ },
    onCommandDrag(instUrl, cmdId)     { /* start drag */ },
    onFilterInput(text)               { /* filter command list */ },

    // ── Toolbar ──
    onFreeze()                        { /* freeze active panel's command */ },
    onCopy()                          { /* copy terminal selection */ },
    onExport()                        { /* export terminal as text */ },
    onScreenshot()                    { /* screenshot panel */ },
    onMaxFit()                        { /* auto-fit terminal size */ },
    onMaxFont()                       { /* toggle max font */ },
    onBufferToggle()                  { /* current / scrollback */ },
    onThemeToggle()                   { /* cycle panel theme */ },
    onSelectMode()                    { /* toggle text selection */ },
    onLayoutPreset(preset)            { /* apply grid layout */ },

    // ── Topbar ──
    onToggleSidebar()                 { /* show/hide sidebar */ },
    onPrevCommand()                   { /* navigate to prev command */ },
    onNextCommand()                   { /* navigate to next command */ },
    onAddPanel()                      { /* create new empty panel */ },
    onSearch()                        { /* open search overlay */ },
    onThemeCycle()                    { /* cycle global theme */ },

    // ── Panel ──
    onPanelFocus(panelId)             { /* focus panel, update toolbar */ },
    onPanelClose(panelId)             { /* close panel, cleanup WS */ },
    onPanelSplit(panelId, dir)        { /* split panel */ },
    onPanelContextMenu(e, panelId)    { /* right-click menu */ },

    // ── Modals ──
    onAddServer(url, label, token)    { /* add server connection */ },
    onSpawn(instUrl, cmd, env)        { /* spawn new command */ },
};
```

### Event delegation

Instead of inline `onclick` attributes that get rebuilt with every render, use event delegation on stable container elements:

```js
// Set up ONCE at init, never rebuilt
document.getElementById('sidebar').addEventListener('click', (e) => {
    const btn = e.target.closest('[data-action]');
    if (!btn) return;
    const { action, instUrl, cmdId } = btn.dataset;
    handlers[`on${action}`]?.(instUrl, cmdId);
});
```

HTML becomes:
```html
<button data-action="CommandFreeze" data-inst-url="..." data-cmd-id="...">⏸</button>
```

No inline JS. No function stringification. No memory leaks from orphaned closures. The event listener on `#sidebar` is set once and never replaced, even when the sidebar content is re-rendered.

### Keyboard handler

Replace the 198-line if/else chain with a lookup table:

```js
const keyBindings = [
    { key: 'Escape',     action: 'onSearchClose' },
    { key: 'f',          ctrl: true, action: 'onSearch' },
    { key: 'c',          ctrl: true, action: 'onCopy' },
    { key: 'w',          ctrl: true, action: 'onPanelClose' },
    { key: 'Tab',        action: 'onNextPanel' },
    { key: 'ArrowLeft',  action: 'onPrevCommand' },
    { key: 'ArrowRight', action: 'onNextCommand' },
    // ... etc
];

document.addEventListener('keydown', (e) => {
    if (e.target.tagName === 'INPUT') return; // don't capture typing in inputs
    const binding = keyBindings.find(b =>
        b.key === e.key && (!b.ctrl || e.ctrlKey) && (!b.shift || e.shiftKey)
    );
    if (binding) {
        e.preventDefault();
        handlers[binding.action]();
    }
});
```

~30 lines. Replaces 561 lines (the entire keyboard.js module).

### What gets deleted:
- 3 versions of freeze/thaw → 1 handler
- 198-line keyboard handler → 30-line lookup table
- 101-line `selectCommand()` → decomposed into `onCommandClick()` (~15 lines) + state update
- 62+75 line context menu builders → `onPanelContextMenu()` calls `render.contextMenu()` (~20 lines)
- All inline `onclick="..."` strings in HTML templates

**Lines saved: ~2,700**

---

## State Management (~200 lines, replaces ~700)

### Current state: god object + scattered module-level vars

54 properties on one object, 9 module-level `let` variables, plus module-local state in focus.js, dragdrop.js, panels.js (`_maxFitState`, `_splitState`).

### Proposed: flat state object, mutation through setters

```js
// state.js
const state = {
    // ── Server connections ──
    connections: [],          // [{ url, label, token, reachable, name }]

    // ── Commands ──
    commands: new Map(),      // "instUrl:cmdId" → command object

    // ── Panels ──
    panels: [],               // [{ id, instUrl, cmdId, ws, theme, split, ... }]
    activePanelId: null,

    // ── UI ──
    sidebarOpen: true,
    activeTab: 'servers',
    globalTheme: 'auto',      // auto | dark | grey | light

    // ── Server config (fetched) ──
    serverConfig: new Map(),  // instUrl → { refreshMs, updateMode, ... }
};

// No more module-level variables. No more legacy globals.
// No more _lastCommandState, _navCommands, _showingWelcome, etc.
// Those are derived: computed from state.panels and state.commands.
```

### Derived state (computed, never stored)

Current code stores "last rendered" snapshots to detect changes (`_lastRenderedPanelCount`, `_lastRenderedPanelIds`, `_lastSplitState`, `_lastShowingWelcome`). This is a change-detection system hand-rolled instead of using the render cycle.

**Proposed**: Render functions always run. They're cheap because they use incremental DOM updates (patching specific elements, not rebuilding). If nothing changed, the DOM patches are no-ops. No need for "last rendered" tracking.

| Current (stored) | Proposed (computed) |
|------------------|---------------------|
| `_showingWelcome` | `state.panels.length === 0 \|\| !state.panels.some(p => p.cmdId)` |
| `_lastRenderedPanelCount` | Not needed — render is always cheap |
| `_lastRenderedPanelIds` | Not needed |
| `_lastSplitState` | Not needed |
| `_lastCommandState` | Not needed — derived from `state.commands` |
| `_navCommands` | Not needed — computed from `state.commands.values()` |
| `_sidebarSort` | Not needed — sort in render, or store as `state.sidebarSort` |

**What gets deleted:**
- 9 module-level `let` variables in state.js
- 6 "last rendered" tracking variables
- All `_last*` and `_cached*` state properties
- Per-panel `_cachedDomPre`, `_cachedScrollPos`, `_pendingVttyData`, `_pendingVttyDirty`

**Lines saved: ~500** (from state.js and all the comparison logic in renderPanels)

---

## Panel Update Pipeline (the core loop, ~100 lines)

### Current: 3 parallel pipelines, ~800 lines total

1. Global: `connectVttyWs` → `updateVttyDisplay` → `applyVttyDiff` → `pre.innerHTML`
2. Per-panel: `connectPanelWs` → `updateVttyDisplayForPanel` → `applyVttyDiffForPanel` → `pre.innerHTML`
3. Secondary: `_connectSecondaryWs` → `_updateSecondaryVttyDisplay` → `_applySecondaryVttyDiff` → `pre.innerHTML`

Plus: `scheduleVttyHttp`, `scheduleVttyHttpForPanel`, `scheduleSecondaryVttyHttp` — three HTTP polling fallbacks.
Plus: `startUpdateMode/stopUpdateMode`, `startPanelUpdateMode/stopPanelUpdateMode` — two update mode systems.
Plus: `_throttleRefresh`, `_flushThrottledRefresh` — global throttle that caused the freeze bug.

### Proposed: one pipeline, per-panel

```js
// In the panel initialization (handlers.onCommandClick):

function startPanelUpdates(panel) {
    // Cancel any existing
    panel.cleanup?.();

    // Connect WebSocket
    const { ws, close } = api.connectVtty(panel.instUrl, panel.cmdId, {
        onMessage(data) {
            // Direct render — no global throttle, no shared timer
            if (data.type === 'vtty_diff') {
                render.panelTerminal(panel, data);
            }
            if (data.type === 'metadata') {
                render.panelMetadata(panel, data);
            }
        },
        onClose() {
            // Reconnect with backoff
            panel._reconnectTimer = setTimeout(() => startPanelUpdates(panel), 2000);
        }
    });

    panel.ws = ws;
    panel.cleanup = () => {
        clearTimeout(panel._reconnectTimer);
        close();
    };
}
```

**~25 lines. Replaces ~800 lines.**

No throttle needed at the WebSocket level — WS messages already arrive at the server's rate. The throttle was only needed because the old code was doing HTTP polling as a fallback AND as the primary update mechanism. With WS working correctly, messages arrive ~1/second for htop, which is perfectly fine to render immediately.

If throttle is ever needed (burst of WS messages), it's per-panel:
```js
onMessage(data) {
    if (panel._renderTimer) return; // drop if one pending
    panel._renderTimer = setTimeout(() => {
        panel._renderTimer = null;
        render.panelTerminal(panel, data);
    }, 50);
}
```

~5 lines. No global state. No shared timer. No cross-panel interference.

---

## Module Map (After Refactoring)

| Module | Lines | Purpose |
|--------|-------|---------|
| `api.js` | ~400 | All server communication (HTTP + WS) |
| `state.js` | ~80 | State definition + simple setters |
| `render.js` | ~1,500 | All DOM rendering (sidebar, panels, toolbar, modals) |
| `handlers.js` | ~800 | All UI event handlers |
| `keyboard.js` | ~40 | Key binding lookup table |
| `theme.js` | ~60 | Theme switching logic |
| `utils.js` | ~50 | escHtml, formatRuntime, URL parsing |
| `vtty-render.js` | ~400 | VTTY diff application (cell-level rendering) |
| `dragdrop.js` | ~150 | Drag and drop (sidebar reorder, panel drop) |
| `workspaces.js` | ~150 | Workspace save/load |
| `spawn.js` | ~100 | Spawn form logic |
| `search.js` | ~150 | Search overlay |
| `logs.js` | ~100 | Log viewer |
| `style.css` | ~500 | All styles (reduced by removing inline style overrides) |
| `index.html` | ~150 | Minimal HTML shell (no inline handlers) |
| **Total** | **~5,230** | |

**Current: ~10,700 lines → Target: ~5,230 lines = 51% reduction**

---

## Testing Framework (~200 lines)

### Current: zero tests, requires running browser and live server

### Proposed: dependency injection + DOM mocking

```js
// test/helpers.js
function createMockApi() {
    return {
        getCommands: sinon.stub().resolves([]),
        connectVtty: sinon.stub().returns({ ws: { close: sinon.stub() }, close: sinon.stub() }),
        freeze: sinon.stub().resolves({}),
        thaw: sinon.stub().resolves({}),
        kill: sinon.stub().resolves({}),
        // ... one stub per api method
    };
}

function createMockDom() {
    const container = document.createElement('div');
    // JSDOM provides full DOM in Node.js
    return { container, querySelector: (s) => container.querySelector(s) };
}

// test/handlers.test.js
describe('handlers.onCommandClick', () => {
    it('selects command into active panel and starts WS', async () => {
        const api = createMockApi();
        const state = { panels: [{ id: 'p1', cmdId: null }], activePanelId: 'p1' };
        const render = { panelTerminal: sinon.stub(), panelMetadata: sinon.stub() };

        await handlers.onCommandClick('http://srv1', 'cmd1', { api, state, render });

        assert.equal(state.panels[0].cmdId, 'cmd1');
        assert.equal(api.connectVtty.calledOnce, true);
    });

    it('does nothing if no active panel', async () => {
        const api = createMockApi();
        const state = { panels: [], activePanelId: null };

        await handlers.onCommandClick('http://srv1', 'cmd1', { api, state, render: {} });

        assert.equal(api.connectVtty.called, false);
    });
});
```

### How this works without a browser:

1. **api.js** takes no constructor args, so we replace it with a mock that has stubs. No real HTTP calls.
2. **render.js** functions receive DOM elements as parameters (not `document.getElementById` internally). In tests, we pass JSDOM elements. In production, we pass real DOM elements.
3. **handlers.js** receives `{ api, state, render }` as dependencies. In production, these are the real modules. In tests, they're mocks.
4. **state.js** is a plain object. No DOM, no side effects. Trivially testable.

### Key change to enable testing:

Render functions must accept DOM elements as parameters instead of querying `document.getElementById()` internally:

```js
// BEFORE (untestable — hardcoded DOM dependency):
function updateSharedToolbar() {
    const freezeBtn = document.getElementById('stFreezeBtn');
    // ...
}

// AFTER (testable — DOM element passed in):
function updateToolbar(toolbarEl, panel) {
    const freezeBtn = toolbarEl.querySelector('[data-action="Freeze"]');
    // ...
}
```

This single change makes every render function testable in Node.js with JSDOM, no browser needed.

### Test structure:

```
test/
  handlers/
    onCommandClick.test.js
    onFreeze.test.js
    onPanelClose.test.js
    ...
  render/
    sidebar.test.js
    panels.test.js
    toolbar.test.js
    ...
  api/
    connectVtty.test.js
    getCommands.test.js
    ...
  state/
    derived.test.js        (tests that derived state is correct)
  helpers.js               (mock factories)
```

---

## Execution Order

### Phase 1: Extract api.js (no behavior change)
1. Create `api.js` with all HTTP/WS functions
2. Replace all `fetch()` calls in existing modules with `api.*` calls
3. Delete dead code (global WS functions, secondary pane WS)
4. Verify: everything works identically
**Risk: low — pure extraction, no logic changes**

### Phase 2: Extract handlers.js (no behavior change)
1. Create `handlers.js` with one function per UI element
2. Move handler logic out of inline onclick, sidebar.js, panels.js, etc.
3. Switch to event delegation with `data-action` attributes
4. Verify: everything works identically
**Risk: low — moving code, not changing it**

### Phase 3: Extract render.js (no behavior change)
1. Create `render.js` with one function per UI area
2. Move rendering logic out of `renderPanels()`, `_buildSidebar()`, etc.
3. Change render functions to accept DOM elements as parameters
4. Verify: everything works identically
**Risk: medium — most code movement**

### Phase 4: Delete dead code
1. Delete eventbus.js
2. Delete all global (non-panel) function variants
3. Delete all "secondary pane" functions
4. Delete legacy state properties
5. Delete module-level state variables
6. Verify: everything still works
**Risk: medium — must ensure nothing still references deleted code**

### Phase 5: Simplify state
1. Remove derived state variables
2. Remove change-detection variables
3. Remove inline style overrides (move to CSS classes)
4. Verify: everything works
**Risk: low**

### Phase 6: Add tests
1. Set up JSDOM + test runner
2. Write tests for api.js (mock WebSocket)
3. Write tests for handlers (mock api + state + render)
4. Write tests for render (JSDOM elements)
5. Write tests for state derived values
**Risk: none — additive only**

### Phase 7: Delete old modules
1. Once all code is moved to new modules, delete the old 23-module structure
2. Final line count check
**Risk: low — old modules are empty at this point**

---

## Summary

| | Current | Target |
|---|---------|--------|
| JS Lines | ~10,700 | ~5,230 |
| Modules | 23 | 14 |
| Functions >50 lines | 42 | ~5 |
| Circular dependencies | 5+ cycles | 0 |
| Duplicate code | ~2,000 lines | 0 |
| Dead code | ~900 lines | 0 |
| Inline onclick handlers | ~100 | 0 |
| Testable without browser | No | Yes |
| State properties | 54 + 9 vars | ~20 |
| Largest function | 302 lines (renderPanels) | ~80 lines |