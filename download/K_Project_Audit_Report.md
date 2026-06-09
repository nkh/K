# K Project Comprehensive Audit Report

**Date:** June 9, 2026  
**Branch:** web_ui_fix2 (commit 5970851)  
**Scope:** Documentation, Tests, Man Pages, Shell Completions, Feature Completeness

---

## 1. Executive Summary

This report presents a comprehensive audit of the K project (vrc/vrw), covering five critical areas: Rust test coverage, JavaScript web UI test coverage, man page completeness, shell completion scripts, and documentation completeness. The audit was performed on the `web_ui_fix2` branch which includes all Phase 2 and Phase 3 web UI fixes, the kill-all/stop-all feature, per-server header colors, and state persistence.

The K project is a Rust-based terminal multiplexer and process manager with two binaries: **vrc** (CLI-focused, Unix Domain Socket IPC) and **vrw** (web dashboard with WebSocket-based terminal emulation). The project features a virtual terminal emulator (vtty), interactive display with split panes, a web admin interface with modular JavaScript, daemon mode, TLS support, and per-instance process management.

Key findings include a significant test coverage gap (73.3% of Rust source files have no unit tests), critical man page defects (VRL typo in 10 pages, missing pages for keep/unkeep commands), zero shell completion documentation in user-facing docs, and several stale values in the requirements document. The kill-all and stop-all commands have been implemented with basic tests.

### 1.1 Key Metrics at a Glance

| Metric | Value | Assessment |
|--------|-------|------------|
| Rust source files | 101 | Large codebase |
| Files with unit tests | 27 (26.7%) | **FAIL** - below 50% |
| Total Rust unit tests | ~431 | Good quantity |
| Total Rust integration tests | 276 | Good quantity |
| Web UI JS modules | 26 | Moderate |
| JS modules with tests | 24 (92%) | PASS |
| JS trivial assertion ratio | ~36% | WARN - needs improvement |
| Man pages total | 34 | Comprehensive |
| Missing man pages | 2 (keep, unkeep) | FAIL |
| Man pages with typos | 10 (VRL instead of VRC) | FAIL |
| Shell completions | 5 shells (bash/zsh/fish/elvish/powershell) | PASS |
| Completion documentation | Only in man pages | FAIL - absent from README/MANUAL/FAQ |
| Doc pages (mdBook) | ~57 | Very comprehensive |
| Stale requirement values | 2 (port, config path) | WARN |

---

## 2. Rust Test Coverage Audit

The Rust backend consists of 101 source files across 13 modules. Unit tests are concentrated in the vtty and cli modules, which together account for the majority of the ~431 unit tests. Integration tests in the `tests/` directory add 276 more tests, bringing the grand total to approximately 707 tests. However, the distribution is highly uneven: entire modules such as `ipc`, `daemon`, `logging`, and `handles` have zero unit test coverage.

### 2.1 Coverage by Module

| Module | Files | Tested | Coverage % | Status |
|--------|-------|--------|------------|--------|
| vtty/ | 14 | 9 | 64% | PASS |
| process/ | 7 | 4 | 57% | WARN |
| cli/ | 13 | 5 | 38% | WARN |
| cli/commands/ | 14 | 5 | 36% | WARN |
| config/ | 16 | 5 | 31% | **FAIL** |
| interactive/ | 7 | 2 | 29% | **FAIL** |
| web/ | 17 | 2 | 12% | **FAIL** |
| web/handlers/ | 14 | 1 | 7% | **FAIL** |
| ipc/ | 4 | 0 | 0% | **FAIL** |
| daemon/ | 3 | 0 | 0% | **FAIL** |
| logging/ | 2 | 0 | 0% | **FAIL** |
| instance/ | 3 | 0 | 0% | **FAIL** |
| handles/ | 6 | 0 | 0% | **FAIL** |

### 2.2 Well-Tested Files (Top 10 by Test Count)

| File | Tests | Focus Areas |
|------|-------|-------------|
| src/vtty/emulator.rs | 83 | Text rendering, colors, cursor, scroll, OSC, sixel, CJK |
| src/cli/args.rs | 68 | CLI flag parsing, conflicts, implicit spawn, subcommands |
| src/vtty/parser.rs | 51 | CSI/OSC/DCS parsing, UTF-8, escape sequences |
| src/vtty/renderer.rs | 31 | Plain/ANSI/HTML output, RLE, wide chars, diff |
| src/vtty/buffer.rs | 21 | Scroll, resize, insert/delete, diff, generation tracking |
| src/vtty/sink.rs | 19 | Broadcast, in-memory, log sinks, lifecycle |
| src/vtty/rate_limiter.rs | 15 | Burst, throttle, refill, config, disabled state |
| src/vtty/cell.rs | 14 | Character width (ASCII, CJK, emoji, combining) |
| src/process/error.rs | 14 | Error display, IO error mapping, Send/Sync |
| src/cli/commands/common.rs | 10 | Target resolution, display string, auto-select |

### 2.3 Critical Untested Modules

#### 2.3.1 IPC Module (Zero Tests)

The inter-process communication module (`src/ipc/`) provides the Unix Domain Socket protocol used by vrc to communicate with running instances. Functions such as `encode_frame()`, `decode_frame()`, and `spawn_control_server()` are tested only through integration tests. Frame encoding/decoding bugs could cause silent data corruption or command injection vulnerabilities. The server's socket lifecycle and client reconnection logic also lack unit-level validation.

#### 2.3.2 Web Handler Modules (7% Coverage)

All 14 web handler files in `src/web/handlers/` have essentially zero unit tests. These handlers implement the REST API and WebSocket endpoints for the vrw web dashboard. Critical handlers such as `ws.rs` (WebSocket terminal), `commands.rs` (process lifecycle), `auth.rs` (authentication), and `certificates.rs` (TLS certificate management) are security-sensitive and handle untrusted network input. The only handler with any test is `commands.rs`, which has 2 tests for the kill-all response structure added on the `web_ui_fix2` branch.

#### 2.3.3 Instance Registry (Zero Tests)

The instance module (`src/instance/`) provides the `InstanceRegistry` which is the foundation for multi-instance management. Functions for registering, discovering, and tracking running instances across the system have no unit tests. This is particularly important for the recently added stop-all command, which iterates over all registered instances.

#### 2.3.4 Configuration Loader (Zero Tests)

The config loader (`src/config/loader.rs`) reads and parses YAML configuration files, applying environment variable overrides and profile selection. Despite being a critical code path that every invocation depends on, it has no unit tests. Configuration loading bugs could manifest as silent misconfigurations or panics at startup.

### 2.4 Integration Test Summary

The five integration test files provide good coverage of the core process lifecycle: spawn, list, kill, VTTY output, key encoding, snapshot/diff, freeze/thaw, and resize. The regression tests (64 tests) cover real-world scenarios including process exit detection, concurrent spawns, and display rendering. However, integration tests do not cover web API endpoints, WebSocket communication, or TLS certificate workflows.

### 2.5 Test Infrastructure Gaps

- Only one dev-dependency: `tokio-test = "0.4"`. Missing: tempfile, assert_cmd, mockall, tower-testutil, axum-test.
- No clippy configuration (`clippy.toml` or `.clippy.toml`). All lints use default settings.
- No code coverage tool configured (no cargo-tarpaulin, cargo-llvm-cov, or codecov).
- No `[profile.test]` configuration for optimized test builds.
- No test-specific features.
- No `[[test]]` configuration for integration test harness customization.

### 2.6 kill-all / stop-all Command Status

Both kill-all and stop-all commands exist and are implemented on the `web_ui_fix2` branch:

- **`handle_kill_all_commands()`** in `src/cli/commands/ipc.rs` — kills all commands in a single running vrc instance by PID.
- **`handle_stop_all_commands()`** in `src/cli/commands/stop.rs` — stops all commands across all running vrw instances (via scanning the instance registry).
- **Web API endpoint** `POST /api/commands/kill-all` in `src/web/handlers/commands.rs`.
- Tests: 2 unit tests for kill-all response structure in `commands.rs`, 3 tests in `stop.rs` including stop-all.
- Man page: `vrw-stop-command.1` already documents `--all` flag in synopsis and options. `vrw-kill.1` is an alias page pointing to `stop-command`.

---

## 3. Web UI Test Coverage Audit

The web frontend consists of 26 JavaScript modules in `static/admin/modules/`. Test coverage is generally good, with 22 modules having direct test files and 3 having indirect coverage through related test files. The test suite uses a custom zero-dependency test framework with mock DOM, providing approximately 500+ assertions across 22 test files.

### 3.1 Module-to-Test Mapping

| Module | Test File | Status |
|--------|-----------|--------|
| app.js | **NONE** | **FAIL** |
| eventbus.js | test_eventbus.js | PASS |
| state.js | test_state.js | PASS |
| utils.js | test_utils.js | PASS |
| focus.js | test_focus.js | PASS |
| theme.js | test_theme.js | PASS |
| sidebar.js | test_sidebar.js | PASS |
| panels.js | test_panels.js | PASS |
| commands.js | test_commands.js | PASS |
| websocket.js | test_websocket.js | PASS |
| vtty.js | test_vtty.js | PASS |
| spawn.js | test_spawn.js | PASS |
| logs.js | test_logs.js | PASS |
| keyboard.js | test_keyboard.js | PASS |
| search.js | test_search.js | PASS |
| notifications.js | test_notifications.js | PASS |
| onboarding.js | test_onboarding.js | PASS |
| templates.js | test_templates.js | PASS |
| dragdrop.js | test_dragdrop.js | PASS |
| workspaces.js | test_workspaces.js | PASS |
| misc.js | test_misc.js | PASS |
| snapshot.js | **NONE (indirect only)** | **FAIL** |
| command-selection.js | test_commands.js (indirect) | WARN |
| command-ui.js | test_commands.js (indirect) | WARN |
| commands-core.js | test_commands.js (indirect) | WARN |
| server-connections.js | test_commands.js (indirect) | WARN |

### 3.2 Critical Gaps

The two modules with the highest risk are `app.js` and `snapshot.js`. The `app.js` module is the main orchestrator controlling the entire application lifecycle, including initialization, auto-connect logic, refresh scheduling, and startup sequencing. No part of this initialization flow is tested. The `snapshot.js` module handles state persistence (`loadSnapshot`, `fetchServerConfig`), which was recently implemented on the `web_ui_fix2` branch and is completely untested in isolation.

### 3.3 Test Quality Assessment

Test quality varies significantly across files. High-quality tests (`test_utils.js`, `test_eventbus.js`, `test_regression.js`) use specific expected-value assertions and behavioral verification. However, approximately 36% of all assertions are trivial "does not throw" smoke tests that would pass even if the function body were empty. Six test files have over 70% trivial assertions:

| Test File | Trivial % |
|-----------|-----------|
| test_onboarding.js | ~80% |
| test_search.js | ~75% |
| test_notifications.js | ~75% |
| test_logs.js | ~75% |
| test_sidebar.js | ~70% |
| test_spawn.js | ~65% |

The test infrastructure uses a custom mock DOM that only supports basic operations (`#id`, `.class`, tag selectors). No innerHTML parsing means DOM rendering tests are severely limited. Module loading via `eval()` shares global scope, so stub functions from `setup.js` (140+ pre-declared no-ops) can silently replace real implementations, causing tests to exercise no-ops rather than actual code.

### 3.4 Regression Test Suite

The regression suite is excellent: 51 named regression cases in `test_regression.js` plus 14 bug-fix cases in `test_regression_bugs.js` cover cross-module scenarios such as welcome panel dismissal, generation skip optimization, theme persistence, XSS prevention, drag-drop data transfer, and multiple UI state transitions. These tests provide the strongest behavioral guarantees in the suite.

---

## 4. Man Page Completeness Audit

The project includes 34 man pages covering both vrc and vrw binaries and their subcommands. Coverage is excellent overall, with only two subcommands missing man pages entirely. However, several formatting bugs and content gaps were identified that affect usability and discoverability.

### 4.1 Missing Man Pages

| Command | Expected File | Priority | Status |
|---------|-------------|----------|--------|
| vrw keep | vrw-keep.1 | High | **FAIL** |
| vrw unkeep | vrw-unkeep.1 | High | **FAIL** |

Both `keep` and `unkeep` commands are defined in `src/cli/args.rs` (lines 467-484) and have handler implementations in `src/cli/commands/keep.rs`. However, no man pages exist for either command. The `vrw.1` SEE ALSO section also does not reference these commands.

### 4.2 Formatting Bug: VRL Typo in 10 Man Pages

**10 of 12 vrc subcommand man pages** have `VRL` instead of `VRC` in their `.TH` macro title. This causes `apropos` and `man -k` to index these pages under the wrong name. Users searching for vrc-list, vrc-stop, etc. via apropos will not find them.

| File | Current `.TH` | Should Be |
|------|---------------|-----------|
| vrc-list.1 | VRL\-LIST | VRC\-LIST |
| vrc-stop.1 | VRL\-STOP | VRC\-STOP |
| vrc-config-check.1 | VRL\-CONFIG\-CHECK | VRC\-CONFIG\-CHECK |
| vrc-completions.1 | VRL\-COMPLETIONS | VRC\-COMPLETIONS |
| vrc-keys.1 | VRL\-KEYS | VRC\-KEYS |
| vrc-cat.1 | VRL\-CAT | VRC\-CAT |
| vrc-spawn-in.1 | VRL\-SPAWN\-IN | VRC\-SPAWN\-IN |
| vrc-freeze.1 | VRL\-FREEZE | VRC\-FREEZE |
| vrc-thaw.1 | VRL\-THAW | VRC\-THAW |
| vrc-resize.1 | VRL\-RESIZE | VRC\-RESIZE |

Correctly named: `vrc.1`, `vrc-kill.1`, `vrc-stop-command.1`

### 4.3 Other Man Page Issues

- **vrw-screenshot.1** has unescaped hyphens in `.TH` (`VRW-SCREENSHOT` instead of `VRW\-SCREENSHOT`), inconsistent with other pages.
- **vrw-purge.1** is missing the `-i/--interactive` option in its synopsis and options section, despite the CLI defining this flag.
- **vrc.1** has no EXAMPLES section, despite being the main entry point for the vrc binary.
- **vrc.1** SEE ALSO is missing `vrc-config-check(1)` and `vrc-completions(1)` references.
- **vrw.1** SEE ALSO is missing `vrw-keep(1)` and `vrw-unkeep(1)` references (secondary, as pages do not yet exist).

### 4.4 Man Page Quality Summary

| Aspect | Rating | Notes |
|--------|--------|-------|
| Coverage (32/34 subcommands) | 4/5 | Only keep/unkeep missing |
| Descriptions | 5/5 | Thorough, include UDS/HTTP mechanics and curl examples |
| Examples | 4/5 | Most pages have 3-5 examples; vrc.1 has none |
| Options documentation | 4/5 | vrw-purge missing -i flag |
| Cross-references | 4/5 | vrc.1 missing 2 refs |
| Formatting consistency | 3/5 | VRL typo in 10 pages is significant |

---

## 5. Shell Completion Scripts Audit

Shell completions are implemented via the `clap_complete` v4 crate, supporting five shells: Bash, Zsh, Fish, Elvish, and PowerShell. Completions are generated on demand at runtime through the `vrw completions <SHELL>` and `vrc completions <SHELL>` subcommands. No pre-generated completion files exist in the repository; the `build.rs` file does not generate completions at build time.

### 5.1 Completion Generation Architecture

When the vrw binary is compiled with both features (vrc and vrw), the completion tree for vrc is manually constructed by starting from the full vrw command tree, hiding vrw-only subcommands and flags, and adding vrc-only subcommands (keys, spawn-in). The code comment in `src/cli/args.rs` acknowledges this is "close enough for shell completion purposes." The binary name is resolved from `argv[0]` via `runtime_binary_name()`, satisfying requirement FR-91 for renamed binary support.

This manual construction approach introduces a maintenance risk: as new subcommands are added, the vrc completion command builder must be manually updated to hide/show the appropriate commands.

### 5.2 Documentation Gap

Shell completions are essentially undocumented in user-facing documentation. The README.md, MANUAL.md, and FAQ.md contain no mention of the completions subcommand, no installation instructions, and no examples of how to enable tab completion. The only documentation exists in two man pages (`vrw-completions.1` and `vrc-completions.1`) and a brief listing in `docs/reference/cli.md`. This means most users will never discover this feature.

The `vrc-completions.1` man page also has the VRL typo (same as other vrc subcommand pages) and is less detailed than the `vrw-completions.1` counterpart.

### 5.3 Completion Assessment

| Aspect | Status | Notes |
|--------|--------|-------|
| Shells supported | PASS | 5 shells via clap_complete v4 |
| Pre-generated files | WARN | None shipped; on-demand only |
| build.rs generation | WARN | Not configured; needed for packaging |
| vrc accuracy (dual-compile) | WARN | Manually constructed; may drift |
| Man pages | WARN | vrw-completions good; vrc-completions has typo |
| User-facing docs | **FAIL** | Absent from README, MANUAL, FAQ |
| Renamed binary support | PASS | FR-91 satisfied via runtime_binary_name() |

---

## 6. Documentation Completeness Audit

The project has extensive documentation organized using the Diataxis framework, with approximately 57 pages in the mdBook documentation plus a comprehensive MANUAL.md (2600+ lines). Documentation quality is generally high, but several significant gaps exist.

### 6.1 README.md Assessment

The README (228 lines) is well-structured with a binary comparison table, feature lists, quick start instructions, usage examples, and architecture overview. Missing elements include: no mention of shell completions, no minimum Rust version (FAQ says 1.75+), no `cargo install` one-liner for quick installation, no build status badges, and no contributor guidelines link.

### 6.2 MANUAL.md Assessment

The MANUAL (2600+ lines) is comprehensive, organized into six parts plus appendices. It covers getting started, everyday use, advanced topics (split-pane, retain/purge, TLS, daemon mode), API reference, security model, and contributor guidelines. Notable gaps: no shell completions section, no performance/benchmarks section, and the `--display-all` flag deprecation messaging is inconsistent between sections.

### 6.3 Configuration Documentation Gap

The `docs/configuration.md` file (560 lines) documents all shared configuration sections thoroughly with CLI flag mapping tables. However, vrw-only configuration sections (server, security, tls, web) are not documented in this reference file despite being referenced in MANUAL.md. The recently added `web.panel_colors` configuration for per-server header colors (on `web_ui_fix2` branch) is documented through code but not in the configuration reference documentation.

### 6.4 CLI Reference Documentation Gap — 7 Missing Flags

The `docs/reference/cli.md` file (666 lines) provides detailed CLI documentation but is missing seven flags that exist in `src/cli/args.rs`:

| Flag | Defined in args.rs | Description |
|------|---------------------|-------------|
| `--profile <name>` | line 194 | Apply a named configuration profile from the config file |
| `--working-directory <path>` | line 202 | Set the working directory for spawned commands |
| `--server-name <name>` | line 44 | Name the server instance (vrw only) |
| `--pid <PID>` / `-t` | line 198 | Target a specific instance by PID |
| `--register-with <port>` | line 59 | Register this instance with another vrw instance (vrw only) |
| `--certificate <name>` | line 84 | Define named certificates for per-command access control (vrw only) |
| `--token-file <path>` | line 64 | Path to bearer token file (vrw only) |

### 6.5 FAQ Assessment

The FAQ (322 lines) is comprehensive for vrc usage but entirely vrc-focused. No questions address vrw-specific topics such as the web dashboard, API usage, TLS configuration, remote access, WebSocket connections, or environment presets. There is also no FAQ entry about configuration file locations or precedence.

### 6.6 Stale Requirement Values

The requirements document (`docs/requirements.md`) contains two stale values that no longer match the codebase:

- **FR-38**: References config path as `~/.config/vrw/config.yaml`, but the actual code uses `~/.config/vrc/config.yaml`.
- **FR-41**: Lists default port as `8080`, but the actual default is `9090`.

### 6.7 Testing Documentation Gap

The `docs/testing.md` file (271 lines) documents Rust unit tests, comprehensive tests, integration tests, regression tests, and benchmarks. However, it completely omits three test categories that exist in the repository:

- **VTTY integration tests** in `tests/vtty/` (6+ shell scripts testing terminal rendering at various dimensions with raw output comparison)
- **Web UI JavaScript tests** in `static/admin/test/` (22 test files with 500+ assertions, custom mock DOM framework)
- **Cookbook test scripts** in `docs/cookbook/scripts/` (5 test scripts for multi-service, dev-server, CI pipeline, remote TLS, and pair-programming scenarios)

### 6.8 Documentation Summary

| Document | Lines | Completeness | Key Gaps |
|----------|-------|-------------|----------|
| README.md | 228 | Good | No completions, no minimum Rust version |
| MANUAL.md | 2600+ | Excellent | No completions section, --display-all inconsistency |
| docs/configuration.md | 560 | Good | Missing vrw-only sections (server, security, tls, web) |
| docs/reference/cli.md | 666 | Good | Missing 7 CLI flags |
| docs/faq.md | 322 | Moderate | vrc-only; no vrw-specific questions |
| docs/requirements.md | 235 | Moderate | 2 stale values (port, config path) |
| docs/testing.md | 271 | Moderate | Missing 3 test categories |
| Man pages (34 total) | — | Good | VRL typo (10 pages), missing keep/unkeep |

---

## 7. Prioritized Action Items

### 7.1 Batch 1 — Quick Fixes (~5 min each)

| # | Fix | Files |
|---|-----|-------|
| 1 | Fix VRL→VRC typo in `.TH` of 10 vrc man pages | `man/vrc-list.1`, `vrc-stop.1`, `vrc-config-check.1`, `vrc-completions.1`, `vrc-keys.1`, `vrc-cat.1`, `vrc-spawn-in.1`, `vrc-freeze.1`, `vrc-thaw.1`, `vrc-resize.1` |
| 2 | Fix unescaped hyphens in `vrw-screenshot.1` `.TH` | `man/vrw-screenshot.1` |
| 3 | Fix stale requirements: port 8080→9090, config path vrw→vrc | `docs/requirements.md` |
| 4 | Add `-i/--interactive` to `vrw-purge.1` synopsis | `man/vrw-purge.1` |

### 7.2 Batch 2 — Missing Content (~30 min each)

| # | Fix | Files |
|---|-----|-------|
| 5 | Create `vrw-keep.1` man page | `man/vrw-keep.1` (new) |
| 6 | Create `vrw-unkeep.1` man page | `man/vrw-unkeep.1` (new) |
| 7 | Add SEE ALSO refs for keep/unkeep/config-check/completions | `man/vrw.1`, `man/vrc.1` |
| 8 | Add EXAMPLES section to `vrc.1` | `man/vrc.1` |
| 9 | Add shell completions section to README.md | `README.md` |
| 10 | Add shell completions section to MANUAL.md | `MANUAL.md` |
| 11 | Add shell completions FAQ entry | `docs/faq.md` |
| 12 | Add 7 missing CLI flags to reference doc | `docs/reference/cli.md` |
| 13 | Add vrw-specific FAQ entries (~5-10 questions) | `docs/faq.md` |
| 14 | Update testing.md with 3 missing test categories | `docs/testing.md` |
| 15 | Document vrw-only config sections in config reference | `docs/configuration.md` |

### 7.3 Batch 3 — Code Quality (longer tasks)

| # | Fix | Files |
|---|-----|-------|
| 16 | Add `test_app.js` — test init(), auto-connect, startup | `static/admin/test/test_app.js` (new) |
| 17 | Add `test_snapshot.js` — test loadSnapshot, fetchServerConfig | `static/admin/test/test_snapshot.js` (new) |
| 18 | Add IPC unit tests (encode_frame, decode_frame) | `src/ipc/protocol.rs` |
| 19 | Add web handler tests for kill-all endpoint | `src/web/handlers/commands.rs` |
| 20 | Add config loader tests | `src/config/loader.rs` |
| 21 | Upgrade trivial JS tests → behavioral in 6 files | `static/admin/test/test_sidebar.js`, `test_search.js`, `test_notifications.js`, `test_logs.js`, `test_onboarding.js`, `test_spawn.js` |
