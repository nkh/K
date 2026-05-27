# Worklog: vrunner Web UI Improvements

## Commit 11: Incremental DOM updates for command list

**Files modified:** `static/admin/app.js`

**Problem:** `loadCommands()` rebuilds the entire `#commandList` via `innerHTML` every 1 second, causing unnecessary DOM thrashing even when nothing changed.

**Solution:**
- Added `_lastCommandState` variable to store a fingerprint of command list state
- Built a lightweight fingerprint string from command data: `inst.url:cmd.id:cmd.alive:cmd.runtime_secs` joined with `|`
- Compare fingerprint against previous state before updating innerHTML
- Skip DOM rebuild entirely when fingerprint matches (no-op polls)
- Force full rebuild on state transitions (kill, purge, killAll) by clearing `_lastCommandState`
- When skipping DOM update, still process pending command selection and panel info updates

**Commit:** `c14b8ab` — `perf(web): skip redundant command list DOM updates`

---

## Commit 12: Per-instance font size for multi-panel layouts

**Files modified:** `static/admin/app.js`, `static/admin/style.css`

**Problem:** All panels share a single global font size. Multi-panel layouts with different instances may need different font sizes per panel.

**Solution:**
- Added `fontSize` property to each panel state object, initialized from global `state.fontSize`
- `addPanelDirect()` checks localStorage for per-panel saved font size (`vrunner_panel_font_{id}`)
- `renderPanels()` applies per-panel font size via inline `style="font-size: {px}px"` on the VTTY container
- Added font size controls (A-/A+ buttons) in panel header next to instance URL
- `changePanelFontSize(panelId, delta)` function: clamps 8-28px, updates panel state, applies inline style, persists to localStorage
- Added CSS for `.panel-font-size-ctrl` and `.panel-font-size` styling
- Global font size buttons (topbar) change the default for new panels only

**Commit:** `faac222` — `feat(web): per-instance font size for multi-panel layouts`

---

## Commit 13: Selection mode toggle for terminal text selection

**Files modified:** `static/admin/app.js`, `static/admin/style.css`

**Problem:** When mouse tracking is enabled by the child process, text selection on the terminal is impossible because all mouse events are forwarded to the PTY.

**Solution:**
- Added `selectionMode` property to panel state, persisted to localStorage (`vrunner_panel_sel_{id}`)
- Added `toggleSelectionMode(panelId)` function that toggles mode, updates CSS class, button state, and persists
- "Select" button in panel header (next to Copy and Export) toggles selection mode
- Keyboard shortcuts: `Ctrl+Shift+S` and `Alt+S` toggle selection mode
- In mouse event handlers (mousedown, mouseup, mousemove, wheel): if `selectionMode` is true, skip PTY forwarding and let browser handle natively
- CSS for `.vtty-container.selection-mode`: text cursor, accent outline border, forced `user-select: text` on pre element
- Button shows "✓ Select" when active (with btn-primary class for visual feedback)
- Updated shortcuts help overlay with new keyboard shortcuts

**Commit:** `cfb9e2e` — `feat(web): add selection mode toggle for terminal text selection`
