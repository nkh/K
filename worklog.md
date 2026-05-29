---
Task ID: 1
Agent: main
Task: Fix broken font-size A-/A+ buttons and resize button in web UI

Work Log:
- Read static/admin/index.html and static/admin/app.js to understand the UI control structure
- Searched for font-size and resize related code in app.js and style.css
- Identified Bug 1: CSS `.vtty-container pre` had `font-size: var(--font-size)` which explicitly references the global CSS variable, ignoring the per-panel inline `font-size` set by `changePanelFontSize()` on `.vtty-container`. The `<pre>` element never picked up the per-panel font change.
- Identified Bug 2: `resizeTerminalPanel()` checked `panelObj.cmdId` but panel objects never had a `cmdId` property — only `state.selectedCmdId` exists. The function always returned early without making the API call.
- Identified Bug 3 (related): `switchBufferPanel()` had the same broken `panelObj.cmdId` reference.
- Fixed style.css: Changed `font-size: var(--font-size)` to `font-size: inherit` on `.vtty-container pre`
- Fixed app.js: Changed `resizeTerminalPanel()` to derive cmdId from `state.selectedCmdId` when panel matches selected instance
- Fixed app.js: Changed `switchBufferPanel()` to use `state.selectedCmdId` instead of `panelObj.cmdId`
- Compiled: `cargo build` — success
- All 502 tests pass (308 unit + 121 comprehensive + 4 integration + 67 regression + 2 doc-tests)
- Clippy: clean, no warnings
- Committed as d26c733 and pushed to origin/main

Stage Summary:
- Two root causes found and fixed: CSS inheritance issue and undefined `panelObj.cmdId`
- Commit d26c733 pushed to main
