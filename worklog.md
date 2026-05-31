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
---
Task ID: 2
Agent: main
Task: Remove all server-related and PNG generation code from speedup branch

Work Log:
- Deleted entire src/web/ directory (20+ files: server, router, handlers, auth, TLS, middleware, state, certs, static assets)
- Deleted server config modules: config/server.rs, config/security.rs, config/web.rs
- Deleted HTTP-dependent CLI subcommands: cat, cert, screenshot, spawn, freeze, thaw, purge, resize
- Deleted 3 test files that tested removed server features
- Removed all server CLI args from args.rs (--bind, --port, --remote, --auth, --tls, etc.)
- Rewrote main.rs to remove all server init code (try_client_mode, check_port_available, start_server, bound_tx, register-with)
- Converted vrunner to pure PTY + terminal display tool with current_thread runtime
- Rewrote dispatch.rs to handle subcommands synchronously (no tokio runtime needed)
- Converted list/stop subcommands to work via PID files and SIGTERM (no HTTP)
- Removed RateLimitConfig from spawner, replaced with u32 constant
- Removed screenshot font fields from VttyConfig
- Stripped all #[cfg(feature = "png")] code from renderer.rs and handle.rs
- Rewrote config/schema.rs, config/merge.rs, config/validation.rs to remove server fields
- Updated Cargo.toml: removed axum, reqwest, rustls, rcgen, image, fontdue, rust-embed, sha2, hex, etc.

Stage Summary:
- 54 files changed, 220 insertions(+), 11058 deletions(-)
- All 296 tests pass
- vrunner is now a pure PTY + terminal display tool
---
Task ID: 3
Agent: main
Task: Rename binary to vrl, fix list UDS diagnostics, update docs and manpages

Work Log:
- Changed Cargo.toml package name from "vrunner" to "vrl"
- Updated CLI name in args.rs: `#[command(name = "vrl")]` and `parse_with_version()`
- Updated completion generation in dispatch.rs: "vrl" instead of "vrunner"
- Renamed all source references: use vrl::, data paths, config paths, process name checks
- is_pid_vrunner() → is_pid_vrl(), /proc check now looks for "vrl" in comm
- Data dir: ~/.local/share/vrl/, config dir: ~/.config/vrl/, socket: ~/.local/share/vrl/control-{pid}.sock
- Config filenames: vrl.yaml, vrl.toml (local config files)
- Simplified InstanceInfo: removed port, bind, command fields (commands now queried via UDS)
- Removed initial_command from register_current() and PID file
- Removed fallback PID-file command display in list command - now shows warnings on UDS errors
- Removed dead format_instance_list() from common.rs
- Cleaned up all user-facing messages to use "vrl"
- Replaced old manpages (vrunner-*.1) with new vrl manpages: vrl.1, vrl-list.1, vrl-stop.1
- Batch-updated 18 documentation files: vrunner → vrl in architecture, cli reference, usage, getting-started, README, etc.
- All 296 tests pass, clippy clean

Stage Summary:
- Binary renamed to vrl across all source, config, and documentation
- List command now surfaces UDS errors instead of silently falling back to PID file
- PID files simplified: no longer store initial command (always query via UDS)
- 23 source files updated, 18 docs updated, 3 manpages created
