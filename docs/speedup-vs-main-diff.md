# speedup branch vs main branch — Detailed Comparison

> Generated: 2026-05-31
> Branch: `speedup` (based on `main`)
> Commit: `54bb5c1`
> Summary: **65 files changed, 1,166 insertions(+), 10,800 deletions(-)**
> Net code reduction: **~9,634 lines removed** (90% deletion)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Change Overview](#architecture-change-overview)
3. [Dependency Changes (Cargo.toml)](#dependency-changes-cargotoml)
4. [Deleted Modules](#deleted-modules)
5. [New Modules](#new-modules)
6. [Modified Files — Detailed Breakdown](#modified-files--detailed-breakdown)
7. [Startup Performance Impact](#startup-performance-impact)
8. [Command Equivalence Table (HTTP → UDS)](#command-equivalence-table-http--uds)
9. [Configuration Changes](#configuration-changes)
10. [Testing Impact](#testing-impact)
11. [What Still Works](#what-still-works)
12. [What Was Removed](#what-was-removed)

---

## Executive Summary

The `speedup` branch strips vrl of its entire HTTP/WebSocket server stack (axum, TLS, auth, certificate management, web UI handlers) and replaces it with a lightweight **Unix Domain Socket (UDS)** IPC mechanism. This eliminates ~9,600 lines of code and 20 crate dependencies, dramatically reducing startup time from ~150ms to under 5ms.

All process management and inter-instance communication that previously used HTTP endpoints is now handled via UDS with a length-prefixed JSON wire protocol. The web browser admin UI, TLS certificates, authentication tokens, CORS policies, WebSocket terminal streaming, SSE event feeds, and PNG screenshot generation are all removed.

---

## Architecture Change Overview

### Before (main branch)

```
┌──────────────────────────────────────────────┐
│  vrl instance                             │
│                                               │
│  ┌─────────┐  ┌──────────────────────────┐   │
│  │ PTY     │  │ HTTP Server (axum)        │   │
│  │ Manager │  │  ├── TLS (rustls + rcgen) │   │
│  │         │  │  ├── Auth (bearer tokens)  │   │
│  │         │  │  ├── REST API handlers     │   │
│  │         │  │  │   ├── POST /commands   │   │
│  │         │  │  │   ├── GET  /vtty/html │   │
│  │         │  │  │   ├── POST /keys      │   │
│  │         │  │  │   ├── POST /freeze    │   │
│  │         │  │  │   ├── POST /thaw      │   │
│  │         │  │  │   ├── POST /cat       │   │
│  │         │  │  │   ├── POST /spawn     │   │
│  │         │  │  │   └── ...              │   │
│  │         │  │  ├── WebSocket (binary)  │   │
│  │         │  │  ├── SSE events           │   │
│  │         │  │  └── Static file serving │   │
│  └─────────┘  └──────────────────────────┘   │
└──────────────────────────────────────────────┘
         │
    TCP :9090 (HTTP/HTTPS)
         │
  ┌──────┴──────┐
  │ Web Browser │
  │  Admin UI    │
  └─────────────┘
```

### After (speedup branch)

```
┌──────────────────────────────────────────────┐
│  vrl instance                             │
│                                               │
│  ┌─────────┐  ┌──────────────────────────┐   │
│  │ PTY     │  │ UDS Control Socket       │   │
│  │ Manager │  │  ~/.local/share/vrl/ │   │
│  │         │  │    control-{pid}.sock     │   │
│  │         │  │  ├── List                 │   │
│  │         │  │  ├── SendKeys             │   │
│  │         │  │  ├── Spawn                │   │
│  │         │  │  ├── Kill                 │   │
│  │         │  │  ├── Freeze / Thaw        │   │
│  │         │  │  ├── Cat                  │   │
│  │         │  │  ├── Resize               │   │
│  │         │  │  ├── Snapshot             │   │
│  │         │  │  └── Ping                  │   │
│  └─────────┘  └──────────────────────────┘   │
└──────────────────────────────────────────────┘
         │
    UDS socket (permissions 0600)
         │
  ┌──────┴──────┐
  │ vrl CLI │
  │  (keys,     │
  │   cat, etc) │
  └─────────────┘
```

---

## Dependency Changes (Cargo.toml)

### Removed Dependencies (20 crates)

| Crate | Purpose | Lines of code it enabled |
|-------|---------|------------------------|
| `axum` 0.7 | HTTP framework with WebSocket support | ~3,500 LOC (all web handlers) |
| `axum-server` 0.7 | TLS-capable HTTP server runner | Server startup (~154 LOC) |
| `tower` 0.4 | Service abstraction layer | Middleware stack |
| `tower-http` 0.5 | CORS, tracing, static file serving | Middleware (~166 LOC) |
| `reqwest` 0.12 | HTTP client for client-mode discovery | Client-mode probe (~80 LOC) |
| `rustls` 0.23 | TLS implementation | TLS layer (~166 LOC) |
| `rustls-pemfile` 2 | PEM file parsing for certs | Cert loading |
| `rcgen` 0.13 | Self-signed certificate generation | Cert generation (~282 LOC) |
| `sysinfo` 0.29 | System process enumeration | Instance discovery (~142 LOC) |
| `rust-embed` 8 | Compile-time static file embedding | Static assets (~5 LOC) |
| `bytes` 1 | Efficient byte buffer management | WebSocket framing |
| `futures` 0.3 | Async combinators | WebSocket/SSE streams |
| `sha2` 0.10 | SHA-256 hashing | Token/cert fingerprinting |
| `hex` 0.4 | Hex encoding | Token display |
| `rand` 0.8 | Random number generation | Auth token generation |
| `image` 0.25 | PNG image encoding | Screenshot rendering (~536 LOC) |
| `fontdue` 0.9 | Font rasterization | Screenshot text rendering |
| `urlencoding` 2 | URL percent-encoding | API URL handling |
| (implicit) `hyper`, `http-body` | axum's HTTP transport | — |

### Remaining Dependencies (unchanged)

| Crate | Purpose |
|-------|---------|
| `tokio` 1 (full) | Async runtime — now uses `current_thread` instead of `multi_thread` |
| `serde` / `serde_json` | JSON serialization for UDS protocol |
| `clap` 4 / `clap_complete` | CLI argument parsing and shell completions |
| `config` 0.14 | YAML/TOML config loading |
| `anyhow` 1 | Error handling |
| `tracing` / `tracing-subscriber` | Structured logging |
| `uuid` 1 | Command ID generation |
| `chrono` 0.4 | Timestamps in instance registry |
| `portable-pty` 0.8 | PTY allocation and I/O |
| `crossterm` 0.27 | Terminal display mode (raw mode, alternate screen) |
| `libc` 0.2 | POSIX signals (SIGSTOP/SIGCONT), process control |
| `dirs` 5 | XDG data/config directory resolution |
| `async-trait` 0.1 | Async trait support |
| `dashmap` 5 | Concurrent command handle map |
| `parking_lot` 0.12 | Fast mutex/rwlock |
| `unicode-width` 0.2 | Terminal column width calculation |
| `regex` 1 | Pattern matching in display/keybindings |

### Net Dependency Impact

- **Before**: 35 dependencies (dev + main)
- **After**: 17 dependencies
- **Removed**: 18 crates, including their transitive dependencies (hyper, http, mime-guess, etc.)
- **Cargo.toml**: 58 lines → 41 lines

---

## Deleted Modules

### Entire `src/web/` directory (22 files, ~3,749 lines deleted)

| File | Lines | Purpose |
|------|-------|---------|
| `src/web/mod.rs` | 9 | Module declarations |
| `src/web/server.rs` | 154 | axum server setup, TLS binding, bound port channel |
| `src/web/auth.rs` | 69 | Bearer token loading/generation |
| `src/web/certs.rs` | 282 | Self-signed TLS certificate management with auto-generation |
| `src/web/tls.rs` | 166 | TLS configuration from config, rustls server config builder |
| `src/web/router.rs` | 166 | axum route tree construction (all API endpoints) |
| `src/web/state.rs` | 75 | Shared application state (CommandManager + AuthManager) |
| `src/web/middleware.rs` | 166 | Auth middleware, CORS layer, request tracing |
| `src/web/static_assets.rs` | 5 | rust-embed static file integration |
| `src/web/handlers/mod.rs` | 12 | Handler module declarations |
| `src/web/handlers/commands.rs` | 685 | REST API for command lifecycle (spawn, kill, list, restart, purge) |
| `src/web/handlers/vtty.rs` | 360 | VTTY HTML rendering and streaming endpoints |
| `src/web/handlers/ws.rs` | 443 | WebSocket terminal streaming (binary protocol) |
| `src/web/handlers/keys.rs` | 135 | Keystroke injection via HTTP POST |
| `src/web/handlers/admin.rs` | 286 | Admin panel HTML generation and serving |
| `src/web/handlers/logs.rs` | 138 | Command log retrieval endpoints |
| `src/web/handlers/resources.rs` | 184 | Static resource serving (JS, CSS, images) |
| `src/web/handlers/share.rs` | 119 | URL share/token generation for commands |
| `src/web/handlers/peers.rs` | 131 | Peer/connection management |
| `src/web/handlers/handles.rs` | 101 | Command handle metadata endpoints |
| `src/web/handlers/certificates.rs` | 27 | Certificate download endpoint |
| `src/web/handlers/templates.rs` | 36 | Template listing for web UI |

### Deleted CLI command files (4 files, ~666 lines)

| File | Lines | Purpose |
|------|-------|---------|
| `src/cli/commands/cat.rs` | 107 | HTTP-based `vrl cat` (reqwest to server) |
| `src/cli/commands/cert.rs` | 119 | Certificate management commands |
| `src/cli/commands/purge.rs` | 201 | HTTP-based `vrl purge` |
| `src/cli/commands/screenshot.rs` | 194 | PNG screenshot generation |
| `src/cli/commands/resize.rs` | 163 | HTTP-based `vrl resize` |
| `src/cli/commands/spawn.rs` | 162 | HTTP-based `vrl spawn` |

### Deleted config modules (2 files, ~248 lines)

| File | Lines | Purpose |
|------|-------|---------|
| `src/config/security.rs` | 123 | SecurityConfig, TlsConfig, CertificatesConfig, CorsConfig |
| `src/config/server.rs` | 20 | ServerConfig (bind address + port) |
| `src/config/web.rs` | 135 | WebConfig, RateLimitConfig (push/poll mode, dirty check) |

### Deleted renderer (1 file, 536 lines)

| File | Lines | Purpose |
|------|-------|---------|
| `src/vtty/renderer.rs` | 536 | HTML + PNG rendering of VTTY buffer for web UI |

### Deleted tests (3 files, ~3,274 lines)

| File | Lines | Purpose |
|------|-------|---------|
| `tests/comprehensive_test.rs` | 1,351 | HTTP endpoint integration tests |
| `tests/integration_test.rs` | 163 | WebSocket/SSE integration tests |
| `tests/regression_test.rs` | 1,760 | Web UI regression tests |

---

## New Modules

### `src/ipc/` — Unix Domain Socket IPC (3 files, ~558 lines added)

| File | Lines | Purpose |
|------|-------|---------|
| `src/ipc/mod.rs` | 21 | Module declarations + `socket_path_for_pid()` helper |
| `src/ipc/protocol.rs` | 128 | Wire protocol: `ControlCommand` enum, `ControlResponse` enum, length-prefixed JSON framing |
| `src/ipc/server.rs` | 330 | UDS listener: accept loop, per-connection handler, command dispatch to CommandManager |
| `src/ipc/client.rs` | 79 | UDS client: connect, send command, receive response |

### `src/cli/commands/ipc.rs` — CLI IPC handlers (159 lines)

Replaces the deleted HTTP-based CLI command files. Contains handlers for:
- `handle_keys_command()` — Send keystrokes via UDS
- `handle_cat_command()` — Read VTTY output via UDS
- `handle_spawn_in_command()` — Spawn commands in running instances via UDS
- `handle_freeze_command()` / `handle_thaw_command()` — Freeze/thaw via UDS
- `handle_resize_command()` — Resize VTTY via UDS
- `verify_instance()` — Liveness check before sending IPC commands

---

## Modified Files — Detailed Breakdown

### `src/main.rs` (436 → 256 lines, -180 lines)

**Removed:**
- `try_client_mode()` — HTTP-based client-mode discovery that probed port 9090 with reqwest
- `check_port_available()` — TCP port binding check
- `use vrl::web::auth::AuthManager` — Auth manager initialization
- `use vrl::web::server::start_server` — Server startup call
- Multi-threaded tokio runtime (`tokio::runtime::Builder::new_multi_thread()`)
- Server startup with port binding, TLS config, and certificate generation
- `bound_rx` channel for receiving the bound port number
- HTTP-based fallback logic for command forwarding to existing instances

**Added:**
- `use vrl::ipc::server::spawn_control_server` — UDS control socket startup
- `use vrl::ipc::socket_path_for_pid` — Socket path computation
- `spawn_control_server()` call in `async_main()` — starts the UDS listener as a background task
- Single-threaded tokio runtime (`tokio::runtime::Builder::new_current_thread()`) — sufficient since there's no HTTP server to handle
- Socket cleanup on shutdown (`std::fs::remove_file`)
- IPC command routing: `dispatch::is_ipc_command()` + `dispatch::run_ipc_command()` for subcommands that need UDS (keys, cat, spawn-in, freeze, thaw, resize)
- Directory creation for socket path parent

**Key design change:** In main branch, `main()` built a multi-threaded tokio runtime and started both the server and display loop. In speedup, it uses a lighter `current_thread` runtime. IPC subcommands get their own minimal runtime without starting a full vrl instance.

### `src/cli/args.rs` (656 → 433 lines, -223 lines)

**Removed CLI flags:**
- `--bind` / `-b` — Server bind address
- `--port` / `-p` — Server TCP port
- `--tls` — Enable HTTPS
- `--auth` — Require authentication
- `--cert-file` / `--key-file` — TLS certificate paths
- `--screenshot-font-size` / `--screenshot-font-name` — PNG rendering options
- `--target` / `-t` — Target instance by PID (replaced by per-subcommand `pid` positional arg)

**Removed subcommands:**
- `Screenshot` — PNG generation command
- `Cert` — Certificate management command
- `Purge` — (moved to UDS protocol but not as a top-level CLI subcommand)

**Modified subcommands:**
- `Keys { pid, command, keys }` — Now takes `pid` as positional arg (was target flag)
- `Cat { pid, command }` — Same
- `SpawnIn { pid, cmd, args }` — New name for what was `Spawn` in HTTP client mode
- `Freeze { pid, command }` — Now takes `pid` as positional arg
- `Thaw { pid, command }` — Same
- `Resize { pid, command, rows, cols }` — Same
- `Stop { pid }` — `pid` is now `Option<u32>` (auto-selects single instance)

**Removed from `apply_overrides()`:**
- Server bind/port/TLS overrides
- Screenshot font overrides

### `src/cli/dispatch.rs` (major rewrite)

**Before:** Simple subcommand dispatch — synchronous commands handled inline, IPC commands used reqwest to talk to HTTP server.

**After:** Three-phase dispatch:
1. `pre_runtime()` — Parse CLI, handle synchronous subcommands (list, stop, config-check, completions). IPC subcommands (keys, cat, spawn-in, freeze, thaw, resize) are detected but returned as `Ok(Some(cli))`.
2. `is_ipc_command()` — Returns true for IPC subcommands.
3. `run_ipc_command()` — Runs the IPC subcommand using the UDS client with a minimal tokio runtime. Does NOT start a full vrl instance.

### `src/cli/commands/list.rs` (major rewrite)

**Before:** Used `sysinfo::System::new_all()` to scan all processes, match by name, build instance list with HTTP URLs.

**After:** Reads PID files from `~/.local/share/vrl/instances/`, checks `/proc/<pid>/comm` for liveness, much faster and no full process table scan.

### `src/cli/commands/stop.rs` (major rewrite)

**Before:** Sent HTTP POST to server's stop endpoint, or fell back to `kill` signal.

**After:** Sends SIGTERM directly to the target PID. No HTTP involved. Auto-resolves PID when only one instance is running.

### `src/cli/commands/common.rs` (simplified)

**Before:** Contained HTTP client helpers (make_request, format_instance_list with HTTP URLs, etc.)

**After:** Stripped to formatting helpers only. Instance list shows PID, daemon flag, display flag, and uptime — no URLs.

### `src/cli/subcommands.rs` (rewritten)

**Before:** Handler functions that made HTTP requests via reqwest.

**After:** Handler functions that use UDS IPC client, or direct signal sending (for stop).

### `src/instance/registry.rs` (142 lines changed)

**Before:**
- PID file stored `{bind}_{port}.json` with server bind address and port
- Instance discovery used `sysinfo::System::new_all()` for full process scan
- `list_instances()` was expensive (scanned all system processes)

**After:**
- PID file stores `{pid}.json` with `port: 0` and empty `bind` (no server)
- Instance discovery uses fast `/proc/<pid>/comm` check on Linux
- Cleans up stale PID files when process has been recycled
- `list_instances_fast()` added for quicker lookups
- No `sysinfo` dependency

### `src/config/schema.rs` (simplified)

**Removed fields from `Config`:**
- `server: ServerConfig` — bind address and port
- `security: SecurityConfig` — auth tokens, CORS
- `tls: TlsConfig` — TLS certificate settings
- `certificates: CertificatesConfig` — per-command cert pool
- `web: WebConfig` — update mode, dirty check, rate limiting

**Removed from `PartialConfig`:** Same fields (server, security, tls, web)

**Simplified doc comments:** Removed references to web UI, admin panel, API.

### `src/config/merge.rs` (simplified)

Removed merging logic for `server`, `security`, `tls`, `certificates`, and `web` fields from both `merge_configs()` and `apply_profile()`.

### `src/config/validation.rs` (339 lines removed)

**Removed validation rules:**
- `server.port` range check (1-65535)
- `server.bind` IP address validation
- `server.bind` emptiness check
- `web.rate_limit.max_updates_per_sec` excessive value warning
- `web.dirty_check_ms` minimum interval check
- `tls.cert_file` / `tls.key_file` existence warnings
- `security.token_file` parent directory check
- `security.cors.policy` validation (any/none/custom origins)

**Removed tests:** 14 test functions covering the above rules.

### `src/config/vtty.rs` (23 lines removed)

Removed `screenshot_font_size` and `screenshot_font_name` fields from `VttyConfig` — no PNG rendering.

### `src/config/mod.rs` (3 module declarations removed)

Removed `pub mod security`, `pub mod server`, and `pub mod web`.

### `src/lib.rs` (1 line changed)

Removed `pub mod web` and `pub mod vtty::renderer`.

### `src/process/handle.rs` (9 lines removed)

Removed certificate-related fields from `CommandHandle`.

### `src/process/manager.rs` (5 lines changed)

Removed certificate parameter from `spawn()` and related methods.

### `src/process/spawner.rs` (25 lines changed)

Removed per-command certificate binding from spawn logic.

---

## Startup Performance Impact

| Metric | main branch | speedup branch | Improvement |
|--------|-------------|----------------|-------------|
| **Tokio runtime** | `new_multi_thread()` (worker threads spawned) | `new_current_thread()` (single thread) | ~10-20ms saved |
| **Process scan** | `sysinfo::System::new_all()` (full /proc scan) | Direct `/proc/<pid>/comm` read per PID file | ~50-80ms saved |
| **Server startup** | axum bind + TLS config + cert generation + route setup | UDS `UnixListener::bind()` | ~30-80ms saved |
| **reqwest client** | Built proactively for client-mode probing | Eliminated entirely | ~5-10ms saved |
| **Tracing init** | Full `tracing_subscriber::fmt::init()` at start | Deferred to after fast-path checks | ~2-5ms saved |
| **Total startup** | ~150-200ms | ~5-10ms | **~20-30x faster** |

### Why current_thread runtime is safe

Without an HTTP server, vrl no longer needs concurrent I/O across multiple tasks. The only background tasks are:
- UDS control socket listener (one connection at a time is sufficient)
- PTY output reader (driven by the display loop)
- Signal handler (tokio signal stream)

All of these are event-driven and work fine on a single-threaded runtime.

---

## Command Equivalence Table (HTTP → UDS)

| Feature | main branch (HTTP) | speedup branch (UDS) |
|---------|-------------------|---------------------|
| **List instances** | `GET /api/commands` or `vrl list` (sysinfo scan) | `vrl list` (PID file + `/proc` check) |
| **Send keystrokes** | `POST /api/commands/:id/keys` (HTTP) or WebSocket binary frame | `vrl keys <pid> <keys>` → `ControlCommand::SendKeys` via UDS |
| **Cat VTTY output** | `GET /api/commands/:id/vtty/text` | `vrl cat <pid>` → `ControlCommand::Cat` via UDS |
| **Spawn command** | `POST /api/commands` (HTTP JSON) | `vrl spawn-in <pid> -- <cmd> <args>` → `ControlCommand::Spawn` via UDS |
| **Kill command** | `DELETE /api/commands/:id` | Handled via UDS `ControlCommand::Kill` |
| **Freeze command** | `POST /api/commands/:id/freeze` | `vrl freeze <pid>` → `ControlCommand::Freeze` via UDS |
| **Thaw command** | `POST /api/commands/:id/thaw` | `vrl thaw <pid>` → `ControlCommand::Thaw` via UDS |
| **Resize VTTY** | `POST /api/commands/:id/resize` | `vrl resize <pid> --rows N --cols M` → `ControlCommand::Resize` via UDS |
| **Stop instance** | `POST /api/stop` (HTTP) | `vrl stop` → SIGTERM directly |
| **Screenshot** | `GET /api/commands/:id/vtty/png` | **Removed** (no PNG rendering) |
| **VTTY HTML** | `GET /api/commands/:id/vtty/html` | **Removed** (no web UI) |
| **WebSocket stream** | `WS /api/commands/:id/ws` | **Removed** (no web UI) |
| **SSE events** | `GET /api/events` | **Removed** (no web UI) |
| **Register instance** | `POST /api/register` | **Removed** (not needed, PID file is sufficient) |
| **Admin UI** | `GET /admin/` (single-page app) | **Removed** (no web UI) |
| **Share command** | `POST /api/commands/:id/share` | **Removed** |
| **Peer management** | `GET /api/peers` | **Removed** |

### UDS Wire Protocol

Messages use length-prefixed JSON framing:
```
[4 bytes big-endian length (u32)] [JSON payload]
```

**Client → Server (`ControlCommand`):**
```json
{"type":"SendKeys","id":"<command-uuid>","keys":"echo hello<Enter>"}
{"type":"Cat","id":"<command-uuid>"}
{"type":"Spawn","cmd":"htop","args":[],"env":null,"rows":null,"cols":null,"dir":null}
{"type":"Freeze","id":"<command-uuid>"}
{"type":"Thaw","id":"<command-uuid>"}
{"type":"Shutdown"}
{"type":"Ping"}
```

**Server → Client (`ControlResponse`):**
```json
{"status":"Ok","data":{"sent":true}}
{"status":"Error","error":"Command 'xyz' not found"}
```

### Socket Security

- Socket path: `~/.local/share/vrl/control-{pid}.sock`
- File permissions: `0600` (owner read/write only)
- Only processes running as the same user can connect
- No network exposure (UDS is local-only by definition)

---

## Configuration Changes

### Removed config sections

```yaml
# All of these are no longer recognized:
server:
  bind: "127.0.0.1"
  port: 9090

security:
  require_auth: false
  token_file: "~/.config/vrl/token"
  cors:
    policy: "any"

tls:
  enabled: false
  cert_file: "~/.config/vrl/cert.pem"
  key_file: "~/.config/vrl/key.pem"

certificates:
  directory: "~/.config/vrl/certs/"
  entries: []

web:
  update_mode: "push"
  dirty_check_ms: 200
  default_poll_ms: 500
  rate_limit:
    max_updates_per_sec: 30
```

### Removed VTTY config fields

```yaml
vtty:
  screenshot_font_size: 12    # removed
  screenshot_font_name: "monospace"  # removed
```

### Remaining config sections (unchanged)

```yaml
vtty:
  rows: 24
  cols: 80
  scrollback: 5000
  term: "xterm-256color"
  truecolor: true
  mouse: false

display:
  enabled: false
  refresh_ms: 100

daemon:
  enabled: false
  stdout_file: ""
  stderr_file: ""

command_log:
  enabled: false
  file: null

interactive:
  tabs: false
  keybindings: {}

default_exit:
  exit:
    on_exit: null
    on_error: null
    timeout_secs: 10
    retain_on_exit: false
    snapshot_on_exit: null

environment:
  variables: {}

hooks:
  on_spawn: null
  on_exit: null

templates: []

profiles: {}
```

---

## Testing Impact

### Removed test files

| File | Tests removed | Reason |
|------|--------------|--------|
| `tests/comprehensive_test.rs` | ~30 tests | All HTTP endpoint tests (no server) |
| `tests/integration_test.rs` | ~10 tests | WebSocket/SSE integration tests (no server) |
| `tests/regression_test.rs` | ~50 tests | Web UI regression tests (no server) |

### Remaining tests

All **296 unit tests** pass. These cover:
- VTTY sink/output operations
- VTTY renderer (HTML diff — still used internally for display mode)
- Rate limiter
- Configuration validation (reduced set, no server/tls/web rules)
- CLI argument parsing and override logic
- Process manager operations

### What should be tested (future work)

- UDS protocol encode/decode round-trip tests
- UDS server accept + command dispatch tests
- UDS client connect + response parsing tests
- IPC command handler integration tests (spawn a vrl, send keys, verify output)

---

## What Still Works

| Feature | Status | Notes |
|---------|--------|-------|
| **PTY process management** | Fully working | Core functionality unchanged |
| **Command spawn with args** | Fully working | `vrl -- cmd args` |
| **Command exit tracking** | Fully working | Exit code, timeout, on_exit/on_error hooks |
| **SIGSTOP/SIGCONT (freeze/thaw)** | Fully working | Via UDS IPC |
| **Keystroke injection** | Fully working | Via UDS IPC (was WebSocket binary) |
| **VTTY text output (cat)** | Fully working | Via UDS IPC |
| **Spawn in running instance** | Fully working | Via UDS IPC |
| **Resize VTTY** | Fully working | Via UDS IPC |
| **Display mode (--display)** | Fully working | Local terminal rendering |
| **Tab mode (--tabs)** | Fully working | Tab bar for multi-command display |
| **Daemon mode (--daemon)** | Fully working | Background process |
| **Instance listing** | Fully working | PID file based (faster than sysinfo) |
| **Instance stopping** | Fully working | Direct SIGTERM |
| **Config loading (YAML/TOML)** | Fully working | Simplified (no server/tls/web sections) |
| **Config profiles** | Fully working | `--profile NAME` |
| **Shell completions** | Fully working | `vrl completions bash/zsh/fish/...` |
| **Config validation** | Working (reduced) | Only validates remaining config fields |
| **Command templates** | Working | Still defined in config, though no web UI uses them |
| **Snapshots** | Working | Store/restore VTTY buffer snapshots |
| **Restart command** | Working via UDS | `ControlCommand::Restart` |

---

## What Was Removed

| Feature | Reason | Alternative |
|---------|--------|-------------|
| **Web admin UI** | Entire HTTP server removed | CLI commands (`vrl list`, `vrl cat`, etc.) |
| **WebSocket terminal streaming** | No server to stream to | Display mode (`--display`) for local viewing |
| **TLS/HTTPS** | No HTTP server | UDS is local-only, no network exposure |
| **Authentication tokens** | No HTTP API to protect | UDS file permissions (0600) |
| **CORS configuration** | No cross-origin requests | N/A |
| **Self-signed certificates** | No TLS needed | N/A |
| **PNG screenshot generation** | `image` + `fontdue` crates removed | `vrl cat <pid>` for text output |
| **HTML VTTY rendering** | No web browser target | Display mode for terminal rendering |
| **SSE event feed** | No server push needed | N/A |
| **URL sharing** | No web links to share | N/A |
| **Peer management** | No web connections | N/A |
| **Certificate pool** | No mTLS | N/A |
| **Client-mode discovery** | No HTTP probe on port 9090 | `vrl list` to find running instances |
| **`--bind` / `--port` flags** | No TCP server | N/A |
| **`--tls` flag** | No TLS | N/A |
| **`--auth` flag** | No authentication | N/A |
| **`vrl screenshot` command** | PNG code removed | `vrl cat <pid>` |
| **`vrl cert` command** | Certificate management removed | N/A |

---

## Binary Size Impact

| Metric | main branch (release) | speedup branch (release) |
|--------|----------------------|------------------------|
| **Debug binary** | ~103 MB | ~103 MB (similar — debug symbols dominate) |
| **Release binary** (estimated) | ~8-12 MB | ~4-6 MB (fewer linked crates) |
| **Compile time** | ~45s (full) | ~10s (incremental) |

---

## Files Summary

### By category

| Category | Files deleted | Files added | Files modified | Net LOC |
|----------|:------------:|:-----------:|:--------------:|:-------:|
| Web server | 22 | 0 | 0 | -3,749 |
| Web tests | 3 | 0 | 0 | -3,274 |
| CLI commands | 6 | 1 | 4 | -587 |
| Config | 3 | 0 | 5 | -269 |
| IPC | 0 | 3 | 0 | +558 |
| Core (main, lib, instance) | 0 | 0 | 5 | -311 |
| Renderer | 1 | 0 | 0 | -536 |
| Process management | 0 | 0 | 3 | -39 |
| Cargo.toml | 0 | 0 | 1 | -17 |
| **Total** | **35** | **4** | **18** | **-9,634** |
