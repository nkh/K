---
Task ID: 1
Agent: main
Task: Trace auto-connect to 9090 — compile vrw, run locally, test APIs, find root cause

Work Log:
- Compiled vrw binary with `cargo build --features vrw`
- Ran vrw on port 9090 with `sleep 999999` as test command
- Tested all 4 API endpoints: /api/info, /api/snapshot, /api/commands, WS
- All 19 API tests pass — server returns correct data
- WebSocket connects, receives `connected` + `vtty_full` with 3816 chars of VTTY HTML
- Root cause confirmed: **frontend JS bug**, not server

Stage Summary:
- Server API is 100% correct — commands, VTTY HTML, WebSocket all work
- Bug is in frontend JS: `loadCommands()` loads commands but never auto-selects first command
- When server starts after page load: `loadSnapshot()` already failed (one-shot), only `loadCommands()` runs in 1s interval
- `loadCommands()` builds sidebar but leaves panel empty ("No command selected")
- Fix committed: auto-select first alive command in `loadCommands()`, immediate `loadCommands()` on server discovery in `fetchServerConfig()`
- Committed as af1bebb on web_ui_fix branch
