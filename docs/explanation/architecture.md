# Architecture

This document provides a comprehensive technical architecture overview of **vrl** and
its companion binary **vrunner**. Both are built from the shared `vrl_core` library and
provide the same core process-management capabilities, but differ in their transport
layer and operating model. It covers the dual-binary design, the design principles
that guide every module, the system context and module relationships, data-flow
diagrams for common operations, the concurrency model, the lifecycle policy, and
extension points for contributors. You should read this document if you want to
understand how vrl and vrunner are structured, how data moves through the system, or
where to add new functionality.

---

## Table of Contents

1. [Dual-Binary Architecture](#dual-binary-architecture)
2. [Design Principles](#design-principles)
3. [System Context](#system-context)
4. [Module Breakdown](#module-breakdown)
5. [Data Flow](#data-flow)
6. [Concurrency Model](#concurrency-model)
7. [Lifecycle Policy](#lifecycle-policy)
8. [Extension Points](#extension-points)
9. [Key Crate Dependencies](#key-crate-dependencies)

---

## Dual-Binary Architecture

The project produces two binaries from a single `vrl_core` library:

| Aspect | `vrl` | `vrunner` |
|---|---|---|
| **Cargo feature** | `vrl` (default) | `vrunner` |
| **Entry point** | `src/bin/vrl.rs` | `src/bin/vrunner.rs` |
| **Transport** | Unix Domain Socket (UDS) | HTTP / WebSocket (Axum) |
| **Address** | `~/.local/share/vrl/control-{pid}.sock` | `127.0.0.1:9090` (configurable) |
| **Security model** | Filesystem permissions (`0600`) | Optional bearer-token auth + TLS |
| **CLI client** | Built-in (`vrl list`, `vrl keys`, …) | Any HTTP client / embedded admin UI |
| **Tokio runtime** | Current-thread (single-threaded) | Multi-threaded |
| **Wire protocol** | Length-prefixed JSON over UDS | REST JSON + WebSocket over TCP |
| **Exclusive module** | `ipc/` (client, server, protocol) | `web/` (server, router, handlers, middleware, auth, TLS, certs, static assets) |

Both binaries share every module exposed by `src/lib.rs`:

```rust
pub mod cli;        // argument parsing and dispatch
pub mod config;     // layered configuration
pub mod daemon;     // daemonization and signal handling
pub mod handles;    // output fan-out sinks
pub mod hooks;      // lifecycle hooks
pub mod instance;   // instance registry (PID files, liveness)
pub mod interactive; // terminal display loop
pub mod ipc;        // UDS IPC (used by vrl)
pub mod logging;    // structured logging
pub mod process;    // process management (spawn, monitor, I/O)
pub mod vtty;       // terminal emulator

#[cfg(feature = "vrunner")]
pub mod web;        // HTTP server (used by vrunner)
```

The `ipc` module is always compiled (vrl needs it), but `web` is gated behind the
`vrunner` feature flag and pulls in Axum, tower, rustls, and other HTTP-specific
dependencies.

---

## Design Principles

Both binaries share a common foundation, but each follows a distinct design
philosophy for its transport layer.

### Silent by Default (vrl)

vrl produces no output unless something requires the user's attention. When a
command exits cleanly there is no fanfare, no summary table, no timestamp. This makes
vrl ideal as a wrapper in scripts and pipes—stdout and stderr belong to the
child process. Diagnostic messages are routed to logging subsystems and only surface
when the user explicitly raises verbosity.

### Web-First by Design (vrunner)

vrunner is designed to be accessed primarily through its HTTP API and embedded admin
UI rather than a CLI. It starts an HTTP server on port 9090 (configurable via
`--port` or `VRUNNER_PORT`) and exposes a full REST API, WebSocket streams for
real-time VTTY output, and an embedded single-page admin UI. When no `--port` is
specified and a command is given, vrunner transparently forwards the command to an
already-running instance on the default port (client-mode auto-detection).

### Local IPC Only (vrl)

All inter-process communication uses Unix Domain Sockets. There is no HTTP server,
no network binding, no TLS, and no authentication mechanism. The UDS socket is
created with `0600` permissions, ensuring only the owning user can connect.
This provides security through filesystem permissions without the complexity of TLS,
certificates, or bearer tokens.

### Optional Network Security (vrunner)

vrunner binds to a TCP socket and therefore requires explicit security controls:
- **Bearer-token auth** — when `security.require_auth` is true, a token is
  generated (or loaded from `security.token_file`) and required on all `/api/`
  requests.
- **TLS** — when `tls.enabled` is true, the server serves HTTPS using rustls.
  Certificates are loaded from `tls.cert_file` / `tls.key_file`, or auto-generated
  via `rcgen` on first run.
- **CORS** — configurable per-origin allow-list for browser-based clients.

### Separation of Concerns

Each module owns exactly one responsibility. The daemon starts processes; the IPC
server (vrl) or HTTP server (vrunner) handles control commands; the instance registry
tracks state; the VTTY emulator renders terminal output. Modules communicate through
well-defined interfaces—never by reaching into each other's internals.

### Extensibility

vrl is designed so that new features can be added without modifying core logic.
IPC commands register themselves with a central `CommandManager` (vrl) or are added
as Axum handlers (vrunner). Handle sinks receive process output without being coupled
to the transport layer. New handle sinks (database writers, file loggers, alerting
systems) can be plugged in behind the same interface.

### Async-First

The entire application is built on Tokio. IPC I/O, HTTP request handling, process
spawning, periodic timers, and shutdown signalling all use asynchronous primitives.
Synchronous operations (PTY I/O via `portable-pty`) are isolated behind bounded
channels so they never block the async runtime.

### Multi-Instance Awareness

A single vrl or vrunner invocation can manage dozens of commands simultaneously. The
instance registry ensures that resources are cleaned up when instances exit, and that
clients can address instances by PID. vrunner additionally supports **peer
registration** for multi-instance discovery and failover via the `/api/peers`
endpoint.

---

## System Context

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                              User / Operator                                     │
└──────┬───────────────┬───────────────────────────┬─────────────────────────────┘
       │               │                           │
       ▼               ▼                           ▼
┌──────────────┐ ┌───────────────┐    ┌──────────────────────────────────────────┐
│    CLI       │ │   Config      │    │         Transport Layer                    │
│  (bin/vrl.rs │ │  (config/)    │    │                                           │
│   or         │ │               │    │  ┌──────────────────┐ ┌────────────────┐  │
│  bin/        │ │ • vtty        │    │  │ UDS Clients      │ │ HTTP Clients   │  │
│  vrunner.rs) │ │ • display     │    │  │  (vrl binary)    │ │ (browser, curl,│  │
│              │ │ • daemon      │    │  │                  │ │  admin UI, ws) │  │
│ • args       │ │ • per-cmd opts│    │  │ • vrl list      │ │ • GET /api/... │  │
│ • subcommands│ │ • hooks       │    │  │ • vrl keys      │ │ • POST /api/.. │  │
│ • daemonize │ │ • env vars    │    │  │ • vrl cat       │ │ • WS /api/...  │  │
│              │ │ • profiles     │    │  └────────┬─────────┘ └──────┬─────────┘  │
└──────┬───────┘ │ • server/tls  │    │           │                  │            │
       │         └──────┬────────┘    │           │                  │            │
       │                │             │           ▼                  ▼            │
       ▼                ▼             │  ┌──────────────────┐ ┌────────────────┐  │
┌─────────────────────────────────┐   │  │ UDS Control Socket│ │  Axum HTTP     │  │
│       Instance Registry        │   │  │  ~/.local/share/  │ │  Server        │  │
│       (instance/)               │   │  │   vrl/control-    │ │  :9090         │  │
│                                 │   │  │   {pid}.sock      │ │                │  │
│  ┌─────────┐ ┌───────────────┐  │   │  │                    │ │  • REST API    │  │
│  │ Instance │ │ Command       │  │   │  │  • Ping           │ │  • WebSocket  │  │
│  │ (pid,    │ │ Manager       │  │   │  │  • List           │ │  • Admin UI   │  │
│  │  config) │ │ (DashMap<id,  │  │   │  │  • SendKeys       │ │  • TLS        │  │
│  └────┬─────┘ │   Command>)    │  │   │  │  • Cat VTTY       │ │  • Auth       │  │
│       │               │          │   │  │  • Spawn           │ │  • CORS       │  │
│       │               │          │   │  │  • Kill / Freeze   │ │  • Peers      │  │
│       │               │          │   │  │  • Shutdown        │ │  • Share      │  │
└───────┼───────────────┼──────────┘   │  └────────┬─────────┘ └──────┬─────────┘  │
        │               │              │           │                  │            │
        ▼               ▼              │           └────────┬─────────┘            │
┌─────────────────────────────────┐     │                    │                     │
│         Shared State            │◄────┼────────────────────┘                     │
│      (ipc/state.rs or           │     │                                          │
│       web/state.rs)             │     │                                          │
│                                 │     │                                          │
│  • Arc<InstanceRegistry>        │     │                                          │
│  • Arc<CommandManager>          │     │                                          │
│  • shutdown_tx (broadcast)     │     │                                          │
└───────────────┬─────────────────┘     │                                          │
                │                       │                                          │
                ▼                       │                                          │
┌─────────────────────────────────┐     │                                          │
│         Daemon                  │     │                                          │
│        (daemon/)                │     │                                          │
│                                 │     │                                          │
│  • daemonize / re-parent        │     │                                          │
│  • PID file management          │     │                                          │
│  • signal handling              │     │                                          │
│  • lifecycle orchestration      │     │                                          │
└───────────────┬─────────────────┘     │                                          │
                │                       │                                          │
                ▼                       │                                          │
┌─────────────────────────────────┐     │                                          │
│    Process Management           │     │                                          │
│     (process/)                  │     │                                          │
│                                 │     │                                          │
│  • manager.rs  – lifecycle      │     │                                          │
│  • spawner.rs  – PTY creation   │     │                                          │
│  • handle.rs   – I/O routing    │     │                                          │
└───────────────┬─────────────────┘     │                                          │
                │                       │                                          │
                ▼                       │                                          │
┌─────────────────────────────────┐     │                                          │
│     VTTY Emulator               │     │                                          │
│       (vtty/)                   │     │                                          │
│                                 │     │                                          │
│  • emulator.rs – state machine  │     │                                          │
│  • parser.rs   – xterm/VT100    │     │                                          │
│  • buffer.rs   – cell grid      │     │                                          │
│  • display.rs  – mode control   │     │                                          │
└───────────────┬─────────────────┘     │                                          │
                │                       │                                          │
                ▼                       │                                          │
┌─────────────────────────────────┐     │                                          │
│      Handle System             │     │                                          │
│       (handles/)               │     │                                          │
│                                 │     │                                          │
│  • File sink                   │     │                                          │
│  • VTTY sink                   │     │                                          │
│  • Null sink                    │     │                                          │
└─────────────────────────────────┘     └──────────────────────────────────────────┘
```

---

## Module Breakdown

### CLI — `cli/`

The CLI layer parses command-line arguments, selects the appropriate subcommand, and
either executes directly or daemonizes. Both binaries share the same `cli/` module
and the same argument parser (`clap`). The subcommand set includes:

| Subcommand | Purpose | Available in |
|---|---|---|
| `list` | List all running instances with their commands | vrl, vrunner |
| `stop` | Stop a running instance | vrl, vrunner |
| `spawn-in` | Spawn a new command in a running instance | vrl |
| `keys` | Send keystrokes to a command | vrl |
| `cat` | Print VTTY buffer of a command | vrl |
| `freeze` | Pause a command (SIGSTOP) | vrl |
| `thaw` | Resume a command (SIGCONT) | vrl |
| `resize` | Resize a command's VTTY | vrl |
| `config-check` | Validate configuration files | vrl, vrunner |

vrunner additionally supports `--port`, `--bind`, `--tls`, and `--register-with`
flags for controlling the HTTP server and peer registration.

### Configuration — `config/`

Configuration is a layered system with clear precedence:

```
Priority (highest → lowest):
  1. CLI flags               (--vtty-rows, --display, --port, --tls, etc.)
  2. Environment variables   (VRUNNER_PORT, etc.)
  3. Configuration file       (~/.config/vrl/config.yaml)
  4. Built-in defaults
```

vrunner extends the configuration schema with HTTP/TLS-specific sections:
`server.bind`, `server.port`, `tls.enabled`, `tls.cert_file`, `tls.key_file`,
`security.require_auth`, `security.token_file`, `security.cors`, and
`certificates.entries`.

### IPC Server — `ipc/` (vrl-only)

The UDS IPC server replaces the entire HTTP server stack for the vrl binary. It
listens on a Unix Domain Socket and dispatches commands to the `CommandManager`.

#### Wire Protocol

Messages use length-prefixed JSON framing:

```
[4 bytes big-endian length (u32)] [JSON payload]
```

The socket is created with `0600` permissions at
`~/.local/share/vrl/control-{pid}.sock`.

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

#### Module Files

| File | Purpose |
|---|---|
| `ipc/mod.rs` | Module root, re-exports |
| `ipc/protocol.rs` | `ControlCommand` / `ControlResponse` enums, JSON serialization |
| `ipc/server.rs` | Accept loop, frame parsing, command dispatch |
| `ipc/client.rs` | Client-side connect, send, receive for CLI subcommands |

### Web Server — `web/` (vrunner-only)

The web module replaces the UDS IPC stack with a full-featured HTTP server built on
Axum. It is gated behind the `#[cfg(feature = "vrunner")]` feature flag and consists
of 22 source files organized into a handler-based architecture.

#### Module Files

| File | Purpose |
|---|---|
| `web/mod.rs` | Module root, re-exports |
| `web/server.rs` | `start_server()` — bind, TLS setup, graceful shutdown |
| `web/router.rs` | `create_router()` — all route definitions (40+ routes) |
| `web/state.rs` | `AppState` — shared state (CommandManager, auth token, cert store, event channels) |
| `web/middleware.rs` | Auth, CORS, request logging, error handling middleware |
| `web/auth.rs` | `AuthManager` — token generation, loading, validation |
| `web/tls.rs` | `TlsManager` — rustls configuration, auto-generation |
| `web/certs.rs` | `CertificateStore` — named certificate management |
| `web/static_assets.rs` | Embedded admin UI via `rust-embed` |
| `web/handlers/mod.rs` | Handler sub-module root |
| `web/handlers/commands.rs` | CRUD + lifecycle (start, kill, freeze, thaw, restart, snapshot, diff) |
| `web/handlers/keys.rs` | SendKeys, SendMouse |
| `web/handlers/vtty.rs` | VTTY queries (full, html, buffer, text, png, partial, diff, changed) |
| `web/handlers/ws.rs` | WebSocket streams (VTTY output, log events) |
| `web/handlers/handles.rs` | Handle sink management |
| `web/handlers/logs.rs` | Log retrieval |
| `web/handlers/peers.rs` | Peer registration / discovery for multi-instance |
| `web/handlers/share.rs` | Share tokens (temporary VTTY snapshots) |
| `web/handlers/resources.rs` | System resource usage |
| `web/handlers/templates.rs` | Command templates |
| `web/handlers/certificates.rs` | Certificate listing |
| `web/handlers/admin.rs` | Admin UI page, static asset serving, share page, smart fallback |

#### REST API Surface

```
/api/snapshot              GET     Full system snapshot
/api/info                  GET     Server info
/api/commands              GET     List commands
/api/commands              POST    Start a command
/api/commands/lookup/:name GET     Lookup command by name
/api/commands/kill-pid/:pid POST  Kill by OS PID
/api/commands/:id/keys     POST    Send keystrokes
/api/commands/:id/mouse    POST    Send mouse events
/api/commands/:id/kill     POST    Kill command
/api/commands/:id/restart  POST    Restart command
/api/commands/:id          DELETE  Purge command
/api/commands/:id/freeze   POST    Freeze (SIGSTOP)
/api/commands/:id/thaw     POST    Thaw (SIGCONT)
/api/commands/:id/vtty     GET     VTTY (JSON)
/api/commands/:id/vtty/html     GET  VTTY (rendered HTML)
/api/commands/:id/vtty/buffer   GET  VTTY raw buffer
/api/commands/:id/vtty/text     GET  VTTY plain text
/api/commands/:id/vtty/png      GET  VTTY screenshot (PNG)
/api/commands/:id/vtty/partial  GET  VTTY incremental update
/api/commands/:id/vtty/diff     GET  VTTY diff since last request
/api/commands/:id/vtty/changed   GET  Poll for VTTY changes
/api/commands/:id/resize   POST    Resize VTTY
/api/commands/:id/snapshot  POST    Create named snapshot
/api/commands/:id/snapshots GET    List snapshots
/api/commands/:id/diff      POST    Diff against snapshot
/api/commands/:id/snapshots/:name DELETE  Delete snapshot
/api/commands/:id/handles   GET/POST Handle sink management
/api/commands/:id/resources GET     System resources
/api/commands/:id/ws        GET     WebSocket VTTY stream
/api/commands/:id/share     POST    Create share token
/api/ws/logs                GET     WebSocket log stream
/api/share/:token           GET     Access shared VTTY
/api/certificates           GET     List certificates
/api/templates             GET     List command templates
/api/log                    GET     Log retrieval
/api/peers                  GET/POST Peer registration
/api/peers/:url             DELETE  Unregister peer
/api/shutdown               POST    Graceful shutdown

/                          GET     Admin UI
/admin                     GET     Admin UI
/admin/*path               GET     Admin UI static assets
/share/:token              GET     Share page
```

### Instance Registry — `instance/`

The instance registry is the single source of truth for all running commands.
It reads PID files from `~/.local/share/vrl/instances/` and validates liveness
via `/proc/<pid>/comm` on Linux. Both binaries use the same registry.

### Daemon — `daemon/`

The daemon module handles three responsibilities:

1. **Re-parenting** — When `--daemon` is passed, the process forks, writes a PID file, and redirects stdio.
2. **Signal handling** — Listens for `SIGTERM`, `SIGINT`, and `SIGUSR1`.
3. **Lifecycle loop** — When the registry is empty and the daemon is in default mode, it initiates shutdown.

Both binaries support `--daemon` mode.

### Process Management — `process/`

#### `manager.rs`

The process manager owns the high-level lifecycle of commands. It:
- Creates the PTY via the spawner.
- Wires up I/O channels between the PTY master and the handle system.
- Monitors the child process for exit.
- Enforces per-command options (retain-on-exit, snapshot-on-exit, send-keys).

#### `spawner.rs`

The spawner is a thin abstraction over `portable-pty`. It configures the PTY size,
working directory, environment variables, and the executable + arguments.

#### `handle.rs`

The handle represents a running process. It provides:
- `write_input(data: &[u8])` — Send bytes to the child's stdin.
- `resize(cols, rows)` — Send a terminal resize event.
- `kill()` — Terminate the child.
- `pid()` — Retrieve the OS process ID.
- `wait()` — An async future that resolves when the child exits.

### VTTY Emulator — `vtty/`

The VTTY emulator is vrl's terminal rendering engine. It interprets VT100 /
xterm escape sequences and maintains an in-memory grid of cells.

#### `emulator.rs`

The emulator holds a reference to the parser and buffer. Each call to
`emulator.feed(bytes)` runs the parser and updates the buffer.

#### `parser.rs`

The parser is a byte-level state machine that recognizes printable characters,
control characters, CSI sequences, OSC sequences, and DCS sequences.

#### `buffer.rs`

The buffer maintains a 2D grid of `Cell` structs with scrollback, cursor
save/restore, and line insertion/deletion.

#### `display.rs`

`display.rs` implements the **display mode** state machine that governs how
vrl exposes terminal output to the user (headless, active, monitor modes).

### Handle System — `handles/`

The handle system is the output fan-out layer. Every byte written by a child
process to its PTY is routed through the handle system to one or more **sinks**:

```
  PTY stdout
      │
      ▼
  ┌──────────┐
  │  Handle   │──────► File Sink
  │  System   │──────► VTTY Sink
  │           │──────► Null Sink
  └──────────┘
```

---

## Data Flow

### Starting an Instance (vrl)

```
 CLI                        Instance Registry         Process Mgmt          VTTY
   │                              │                         │                │
   │  vrl -- htop                  │                         │                │
   │─────────────────────────────►│                         │                │
   │                              │  register(pid)            │                │
   │                              │─────────────────────────►│                │
   │                              │                         │                │
   │                              │           spawn PTY      │                │
   │                              │─────────────────────────────────────────►│                │
   │                              │                         │                │
   │                              │              create VTTY emulator                 │
   │                              │─────────────────────────────────────────────────────────►│
   │                              │                         │                │
   │                              │  wire handle (PTY → VTTY)                   │
   │                              │◄─────────────────────────────────────────┤                │
   │                              │                         │                │
   │                              │                         │                │
   │                              │    start UDS IPC server                       │
   │                              │─────────────────────────────────────────┤                │
   │                              │                         │                │
   │  Instance ready              │                         │                │
   │◄─────────────────────────────│                         │                │
```

### Starting an Instance (vrunner)

```
 CLI                        Instance Registry         Process Mgmt          VTTY
   │                              │                         │                │
   │  vrunner -- htop              │                         │                │
   │─────────────────────────────►│                         │                │
   │                              │  register(pid)            │                │
   │                              │─────────────────────────►│                │
   │                              │                         │                │
   │                              │           spawn PTY      │                │
   │                              │─────────────────────────────────────────►│                │
   │                              │                         │                │
   │                              │              create VTTY emulator                 │
   │                              │─────────────────────────────────────────────────────────►│
   │                              │                         │                │
   │                              │  wire handle (PTY → VTTY)                   │
   │                              │◄─────────────────────────────────────────┤                │
   │                              │                         │                │
   │                              │                         │                │
   │                              │  start Axum HTTP server (:9090)            │
   │                              │─────────────────────────────────────────┤                │
   │                              │                         │                │
   │  Server listening            │                         │                │
   │◄─────────────────────────────│                         │                │
```

### Listing Instances (vrl)

```
 CLI                        Instance Registry
   │                              │
   │  vrl list                    │
   │─────────────────────────────►│
   │                              │  read pidfiles + /proc check
   │                              │─────────────────────────►│
   │                              │                         │
   │                              │  Vec<(pid, status, uptime)>
   │                              │◄─────────────────────────│
   │                              │                         │
   │  Print table                 │                         │
   │◄─────────────────────────────│                         │
```

### Listing Instances (vrunner)

```
 HTTP Client                   Instance Registry
   │                              │
   │  GET /api/commands           │
   │─────────────────────────────►│
   │                              │  read CommandManager state
   │                              │─────────────────────────►│
   │                              │                         │
   │                              │  JSON array of commands
   │                              │◄─────────────────────────│
   │                              │                         │
   │  200 OK + JSON               │                         │
   │◄─────────────────────────────│                         │
```

### Stopping an Instance (vrl)

```
 CLI
   │
   │  vrl stop 12345
   │─────────►
   │  kill(SIGTERM, 12345)
   │
```

### Stopping an Instance (vrunner)

```
 HTTP Client
   │
   │  POST /api/commands/<id>/kill
   │─────────►
   │  kill(SIGTERM, pid)
   │
   │  200 OK + JSON
   │◄─────────
```

---

## Concurrency Model

Both binaries are built on the **Tokio** asynchronous runtime, but with different
runtime configurations tuned to their transport layer.

### vrl: Current-Thread Runtime

```
┌────────────────────────────────────────────┐
│           Tokio Current-Thread Runtime     │
│                                            │
│  No worker threads — sufficient for UDS +  │
│  PTY I/O without an HTTP server           │
└────────────────────────────────────────────┘
```

vrl uses `tokio::runtime::Builder::new_current_thread()` for all code paths
(IPC client mode and daemon/server mode). A single-threaded runtime is sufficient
because UDS I/O is inherently single-connection-at-a-time from the CLI, and PTY
read loops are light enough to share one thread.

### vrunner: Multi-Thread Runtime

```
┌──────────────────────────────────────────────────────────────┐
│              Tokio Multi-Thread Runtime                       │
│                                                              │
│  Worker threads (default: num_cpus) — required for serving   │
│  concurrent HTTP requests, WebSocket connections, and SSE     │
│  streams alongside PTY I/O                                   │
└──────────────────────────────────────────────────────────────┘
```

vrunner uses `tokio::runtime::Builder::new_multi_thread()` because it must serve
concurrent HTTP requests, maintain persistent WebSocket connections for real-time
VTTY streaming, handle SSE log streams, and process REST API calls simultaneously.
The multi-threaded runtime ensures that a slow HTTP client or long-polling
connection does not block PTY I/O or other requests.

### DashMap for Concurrent Commands

The `CommandManager` uses `DashMap` rather than a `Mutex<HashMap>` to provide
lock-free concurrent access. This is critical for vrunner where multiple HTTP
handlers may read/write command state simultaneously.

### Per-Command Tasks

Each running command gets its own set of tasks:

**vrl:**
```
Command "web-server"
├── Task 1: PTY read loop  (PTY stdout → VTTY → Handle sinks)
├── Task 2: Process monitor (await child exit)
└── Task 3: UDS IPC handler (control commands from CLI)
```

**vrunner:**
```
Command "web-server"
├── Task 1: PTY read loop  (PTY stdout → VTTY → Handle sinks)
├── Task 2: Process monitor (await child exit)
├── Task N: WebSocket connections (one per connected client)
└── Task N+M: REST API requests (short-lived, spawned per request)
```

### Sync/Async Bridge

The PTY library (`portable-pty`) uses synchronous I/O. Both binaries bridge this
with Tokio's `spawn_blocking`:

```rust
let master = pty.master();
let (tx, rx) = tokio::sync::mpsc::channel(128);

tokio::task::spawn_blocking(move || {
    let mut buf = [0u8; 8192];
    loop {
        let n = master.read(&mut buf).unwrap();
        if tx.blocking_send(buf[..n].to_vec()).is_err() {
            break;  // receiver dropped
        }
    }
});

// On the async side:
while let Some(data) = rx.recv().await {
    emulator.feed(&data);
}
```

### Shutdown via Broadcast

Graceful shutdown uses Tokio's `broadcast` channel:

1. The CLI, signal handler, or HTTP shutdown endpoint sends `()` on `shutdown_tx`.
2. All tasks that hold a `shutdown_rx` receive the signal.
3. Each task performs its cleanup and exits.
4. vrunner additionally calls `axum_server::Handle::graceful_shutdown()` with a
   2-second timeout to drain persistent connections.

---

## Lifecycle Policy

vrl's lifecycle is governed by the **"Last-Command-Standing"** principle.

| Mode | Description | Behavior on Last Command Exit |
|---|---|---|
| **Headless** | No display, daemon mode only | Exits immediately |
| **Display** | Active client connected | Transitions to Monitor |
| **Monitor** | No active client, buffering | Exits when no commands remain |
| **Retain-on-Exit** | Per-command override | Command entry persists in registry |

vrunner follows the same lifecycle policy. In addition, when started without an
explicit `--port` and a command is specified, vrunner attempts **client-mode
auto-detection**: it probes `http://127.0.0.1:9090/api/commands` and, if a running
instance responds, forwards the spawn request over HTTP and exits without starting
a new server. If the probe fails, vrunner starts a new server on the default port.

---

## Extension Points

### Adding a New IPC Command (vrl)

1. **Define the command** — Add a variant to `ControlCommand` in `ipc/protocol.rs`.
2. **Handle the command** — Add dispatch logic in `ipc/server.rs`.
3. **Add CLI handler** — Add a handler function in `cli/commands/ipc.rs`.
4. **Add CLI subcommand** — Register the subcommand variant in `cli/args.rs`.
5. **Update tests** — Add unit and integration tests.

### Adding a New HTTP Endpoint (vrunner)

1. **Define the handler** — Add a handler function in the appropriate `web/handlers/*.rs` file.
2. **Register the route** — Add the route in `web/router.rs` under `api_routes`.
3. **Add request/response types** — Define serializable structs alongside the handler.
4. **Update admin UI** (optional) — Add frontend components in the embedded static assets.
5. **Update tests** — Add unit and integration tests.

### Adding a New Handle Sink

1. **Implement the trait** — Create a struct that implements `HandleSink`.
2. **Register at spawn time** — When creating a command, instantiate the sink and add it to the handle system.
3. **Configure** — Add configuration options to `CommandConfig`.

### Adding a New WebSocket Event Stream

1. **Define the event type** — Add a variant to the WebSocket message enum in `web/handlers/ws.rs`.
2. **Create an event sender** — Add a broadcast channel in `CommandManager` or `AppState`.
3. **Bridge to the handler** — Subscribe to the channel in the WebSocket upgrade handler.
4. **Emit events** — Send events from the appropriate module (e.g., VTTY changes, log lines).

---

## Key Crate Dependencies

| Crate | Role | Binary |
|---|---|---|
| `tokio` | Async runtime, channels, timer, signal handling | Both |
| `portable-pty` | Cross-platform PTY creation and I/O | Both |
| `dashmap` | Concurrent hash map for command registry | Both |
| `serde` / `serde_json` | JSON serialization for IPC and REST | Both |
| `clap` | Command-line argument parsing | Both |
| `config` | Configuration file loading | Both |
| `tracing` / `tracing-subscriber` | Structured logging | Both |
| `parking_lot` | Fast mutex/rwlock | Both |
| `crossterm` | Local terminal display | Both |
| `libc` | POSIX signals, daemonization | Both |
| `axum` | HTTP framework (routes, extractors, WebSocket) | vrunner |
| `axum-server` | HTTP/TLS server with graceful shutdown | vrunner |
| `tower` / `tower-http` | Middleware layers (CORS, tracing, fs) | vrunner |
| `reqwest` | HTTP client (peer registration, client-mode probe) | vrunner |
| `rustls` / `rustls-pemfile` | TLS support | vrunner |
| `rcgen` | Self-signed certificate generation | vrunner |
| `rust-embed` | Embedded static assets (admin UI) | vrunner |
| `sha2` / `hex` / `rand` | Auth token generation and hashing | vrunner |
| `sysinfo` | System resource queries | vrunner |
| `image` / `fontdue` | VTTY PNG screenshot rendering | vrunner |

---

*This document is part of the [Diátaxis](https://diataxis.fr/) documentation framework
for vrl. See the [explanation index](./) for related topics.*
