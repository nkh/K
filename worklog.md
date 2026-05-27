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
---
Task ID: 1
Agent: main
Task: Fix three web UI issues - missing theme button, oversized buttons/unorganized menu, direct keyboard input

Work Log:
- Analyzed current state of web UI files from git objects (repository uses nested git/submodule setup)
- Found that HEAD commit (1d7e662) already implemented direct keyboard input and theme toggle, but had crash bugs
- Discovered JS references to cursorPos, resizeRows, resizeCols DOM elements that were removed from HTML (would crash on every VTTY update)
- Restored cursorPos, scrollbackIndicator to bottom bar
- Added compact resize controls (rows x cols inputs + Resize button) to bottom bar
- Restored send-keys input bar below panel header (user explicitly asked to keep it)
- Added null checks for all DOM element references in updateVttyDisplay() and loadVttyHttp()
- Reorganized panel header: action buttons grouped in .panel-actions, send-keys in own .panel-keys-bar row
- Introduced btn-xxs ultra-compact button size variant
- Reduced topbar padding (0.35rem → 0.2rem), panel header padding (0.35rem → 0.2rem)
- Changed panel header buttons to use Unicode symbols (⌨ clipboard, ✎ select, ✕ remove) instead of text labels
- Applied btn-xxs to all topbar and panel header buttons for compactness
- Compiled (0 errors), linted (0 clippy warnings), tested (67 tests pass)
- Updated MANUAL.md §2.3 and §2.4, docs/usage.md with all UI changes
- Fixed HEAD commit message from UUID to proper conventional commit
- Pushed 3 commits: 1d7e662 (feat), d85f186 (fix), 4fed331 (docs)

Stage Summary:
- 3 commits pushed to origin/main
- Theme toggle: was already present, no changes needed
- Direct keyboard input: was already implemented, verified working
- Send-keys field: restored to panel headers as separate row
- Crash bug fixed: null checks added for missing DOM elements
- UI compactness: btn-xxs variant, reduced padding, grouped layout
