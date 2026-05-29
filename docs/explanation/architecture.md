# Architecture

This document provides a comprehensive technical architecture overview of **vrunner**. It
covers the design principles that guide every module, the system context and module
relationships, data-flow diagrams for common operations, the concurrency model, the
lifecycle policy, and extension points for contributors. You should read this document
if you want to understand how vrunner is structured, how data moves through the
system, or where to add new functionality.

---

## Table of Contents

1. [Design Principles](#design-principles)
2. [System Context](#system-context)
3. [Module Breakdown](#module-breakdown)
4. [Data Flow](#data-flow)
5. [Concurrency Model](#concurrency-model)
6. [Lifecycle Policy](#lifecycle-policy)
7. [Extension Points](#extension-points)
8. [Key Crate Dependencies](#key-crate-dependencies)

---

## Design Principles

vrunner is built on six core principles that influence every design decision:

### Silent by Default

vrunner produces no output unless something requires the user's attention. When a
command exits cleanly there is no fanfare, no summary table, no timestamp. This makes
vrunner ideal as a wrapper in scripts and pipes—stdout and stderr belong to the
child process. Diagnostic messages are routed to logging subsystems and only surface
when the user explicitly raises verbosity.

### Secure by Default

Out of the box vrunner binds to `127.0.0.1` only, requires no authentication for local
access, and refuses connections from non-loopback interfaces. Remote access is an
opt-in action: the user must explicitly enable authentication tokens and optionally
TLS. Every network-facing component treats the local-only case as the happy path.

### Separation of Concerns

Each module owns exactly one responsibility. The daemon starts processes; the web
server serves HTTP; the instance registry tracks state; the VTTY emulator renders
terminal output. Modules communicate through well-defined interfaces—never by
reaching into each other's internals. This separation makes it possible to test,
replace, or extend any piece independently.

### Extensibility

vrunner is designed so that new features can be added without modifying core logic.
Web commands register themselves with a central `CommandManager`. Handle sinks receive
process output without being coupled to the web layer. New handle sinks (database
writers, file loggers, alerting systems) can be plugged in behind the same interface.

### Async-First

The entire application is built on Tokio. Network I/O, process spawning, periodic
timers, and shutdown signalling all use asynchronous primitives. Synchronous
operations (PTY I/O via `portable-pty`) are isolated behind bounded channels so they
never block the async runtime.

### Multi-Instance Awareness

A single vrunner invocation can manage dozens of named instances simultaneously. The
instance registry ensures that names are unique, that resources are cleaned up when
instances exit, and that clients can address instances by name rather than by
internal process IDs.

---

## System Context

The following ASCII diagram shows the major components of vrunner and the
relationships between them. Solid arrows represent primary data flow; dashed arrows
represent configuration or control signals.

```
┌──────────────────────────────────────────────────────────────────────────┐
│                              User / Operator                             │
└──────┬───────────────┬───────────────────────────┬─────────────────────┘
       │               │                           │
       ▼               ▼                           ▼
┌──────────────┐ ┌───────────────┐    ┌──────────────────────────────────┐
│    CLI       │ │   Config      │    │         Clients                    │
│  (main.rs,   │ │  (config/)    │    │  ┌──────────┐  ┌────────────────┐  │
│   cli/)      │ │               │    │  │ Browser  │  │  curl / API    │  │
│              │ │ • port        │    │  │ (xterm)  │  │  client        │  │
│ • args       │ │ • host        │    │  └────┬─────┘  └───────┬────────┘  │
│ • subcommands│ │ • auth token  │    │       │               │            │
│ • daemonize │ │ • TLS certs   │    │       │               │            │
└──────┬───────┘ │ • per-cmd opts│    │       │               │            │
       │         └──────┬────────┘    │       │               │            │
       │                │             │       ▼               ▼            │
       ▼                ▼             │  ┌────────────────────────────┐   │
┌─────────────────────────────────┐    │  │       Web Server           │   │
│       Instance Registry        │    │  │  (web/server.rs,           │   │
│       (instance/)               │◄───┼─►│   router.rs, handlers/)   │   │
│                                 │    │  │                            │   │
│  ┌─────────┐ ┌───────────────┐  │    │  │  • /api/commands/*        │   │
│  │ Instance │ │ Command       │  │    │  │  • /api/instances/*      │   │
│  │ (name,   │ │ Manager       │  │    │  │  • /ws/:id               │   │
│  │  handle, │ │ (DashMap<id,  │  │    │  │  • /                     │   │
│  │  config) │ │   Command>)    │  │    │  │                            │   │
│  └────┬─────┘ └───────┬───────┘  │    │  └────────────┬───────────────┘   │
│       │               │          │    │               │                   │
│       │               │          │    │               ▼                   │
└───────┼───────────────┼──────────┘    │  ┌────────────────────────────┐  │
        │               │               │  │     Auth / TLS / Middleware │  │
        ▼               ▼               │  │  (auth.rs, middleware.rs,   │  │
┌─────────────────────────────────┐     │  │   tls.rs)                   │  │
│         AppState                │     │  │                              │  │
│      (web/state.rs)             │◄────┼─►│  • token validation         │  │
│                                 │     │  │  • CORS headers             │  │
│  • Arc<InstanceRegistry>        │     │  │  • TLS termination          │  │
│  • Arc<CommandManager>          │     │  │  • request logging          │  │
│  • auth_token                   │     │  └────────────────────────────┘  │
│  • shutdown_tx (broadcast)     │     │                                   │
└───────────────┬─────────────────┘     │  ┌────────────────────────────┐  │
                │                       │  │    Admin Interface          │  │
                ▼                       │  │    (static/admin/)          │  │
┌─────────────────────────────────┐     │  │                              │  │
│         Daemon                  │     │  │  • index.html                │  │
│        (daemon/)                │     │  │  • JS/WS client             │  │
│                                 │     │  │  • CSS                       │  │
│  • daemonize / re-parent        │     │  └────────────────────────────┘  │
│  • PID file management          │     └───────────────────────────────────┘
│  • signal handling              │
│  • lifecycle orchestration      │
└───────────────┬─────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│    Process Management           │
│     (process/)                  │
│                                 │
│  • manager.rs  – lifecycle      │
│  • spawner.rs  – PTY creation   │
│  • handle.rs   – I/O routing    │
└───────────────┬─────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│     VTTY Emulator               │
│       (vtty/)                   │
│                                 │
│  • emulator.rs – state machine  │
│  • parser.rs   – xterm/VT100    │
│  • buffer.rs   – cell grid      │
│  • renderer.rs – HTML / diff    │
│  • display.rs  – mode control   │
└───────────────┬─────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│      Handle System             │
│       (handles/)               │
│                                 │
│  • WebSocket sink               │
│  • Future sinks (DB, log, ...) │
└─────────────────────────────────┘
```

---

## Module Breakdown

### CLI — `main.rs`, `cli/`

The CLI layer is the single entry-point for every vrunner invocation. It parses
command-line arguments, selects the appropriate subcommand, and either executes
directly or daemonizes. The subcommand set includes:

| Subcommand | Purpose |
|---|---|
| `run` | Start one or more named commands with optional web UI |
| `daemon` | Run as a background daemon, accepting commands over the API |
| `stop` | Signal a running daemon to shut down |
| `status` | Query daemon health and list running instances |

The CLI module performs **zero** I/O beyond argument parsing and configuration
loading. All real work is delegated to the daemon, instance registry, and web server.

### Configuration — `config/`

Configuration is a layered system with clear precedence:

```
Priority (highest → lowest):
  1. CLI flags               (--port, --host, --auth-token, etc.)
  2. Environment variables   (VRUNNER_PORT, VRUNNER_AUTH_TOKEN, etc.)
  3. Configuration file       (~/.config/vrunner/config.toml)
  4. Built-in defaults        (localhost:8080, no auth, no TLS)
```

The configuration module parses TOML, merges layers, and produces a single
`Config` struct that is shared via `Arc` throughout the application. Per-command
options (e.g. `--retain-on-exit`) are captured in a `CommandConfig` that lives
inside each `Command` entry.

### AppState — `web/state.rs`

`AppState` is the shared application state that is injected into every Axum handler
via the standard `State` extractor. It holds:

```rust
pub struct AppState {
    pub registry:        Arc<InstanceRegistry>,
    pub command_manager:  Arc<CommandManager>,
    pub auth_token:       Option<String>,
    pub shutdown_tx:      broadcast::Sender<()>,
    pub config:           Arc<Config>,
}
```

Wrapping the fields in `Arc` means every handler gets a cheap clone while the
underlying data stays synchronized across all concurrent tasks.

### Security — `auth.rs`, `middleware.rs`

Security is split into two concerns:

- **`auth.rs`** — Token generation (256-bit `OsRng`), token file read/write,
  and the `validate_token` function used by handlers.
- **`middleware.rs`** — Axum middleware layer that extracts the `Bearer` token
  from the `Authorization` header and compares it against the configured token.
  If authentication is disabled (no token configured), the middleware is a
  no-op pass-through.

Both modules are covered in detail in [security-model.md](security-model.md).

### TLS — `tls.rs`

The TLS module builds a `rustls::ServerConfig` from either:

- **Auto-generated self-signed certificates** via `rcgen` (the default when the
  user passes `--tls` with no additional arguments), or
- **User-provided certificate and key paths** loaded with `rustls-pemfile`.

No dependency on OpenSSL or any system TLS library exists—vrunner uses pure-Rust
TLS exclusively. Certificates are generated with appropriate SAN entries and a
configurable validity period.

### Instance Registry — `instance/`

The instance registry is the single source of truth for all running commands:

```rust
pub struct InstanceRegistry {
    instances: DashMap<String, Instance>,
}
```

Key operations:

| Method | Description |
|---|---|
| `register(name, handle, config)` | Insert a new instance; returns error on duplicate name |
| `unregister(name)` | Remove by name; triggers cleanup |
| `get(name)` | Borrow a reference to an instance |
| `list()` | Iterator over all registered names and metadata |
| `is_empty()` | Check whether any instances are still alive |

The registry is thread-safe (DashMap), lock-free for reads, and designed to be
scanned frequently by the daemon's lifecycle loop.

### Daemon — `daemon/`

The daemon module handles three responsibilities:

1. **Re-parenting** — When `--daemon` is passed, the process forks (Unix) or
   detaches (conceptually on other platforms), writes a PID file, and redirects
   stdio to `/dev/null`.
2. **Signal handling** — Listens for `SIGTERM`, `SIGINT`, and `SIGUSR1` to
   trigger graceful shutdown or reload.
3. **Lifecycle loop** — A Tokio task that periodically checks the instance
   registry. When the registry is empty (no commands running) and the daemon is
   in the default mode, it initiates shutdown. This is the **last-command-standing**
   policy described in [lifecycle-policy.md](lifecycle-policy.md).

### Process Management — `process/`

This module contains the three files that manage the lifecycle of child processes:

#### `manager.rs`

The process manager owns the high-level lifecycle of a single command. It:

- Creates the PTY via the spawner.
- Wires up I/O channels between the PTY master and the handle system.
- Monitors the child process for exit.
- Enforces per-command options (retain-on-exit, snapshot-on-exit, send-keys).
- Notifies the instance registry when the process exits.

#### `spawner.rs`

The spawner is a thin abstraction over `portable-pty`:

```rust
pub fn spawn(config: &CommandConfig) -> Result<(ChildProcess, PtyMaster), SpawnError>
```

It configures the PTY size, working directory, environment variables, and the
executable + arguments. Platform-specific quirks (e.g., Unix domain sockets for
PTY handles) are isolated here.

#### `handle.rs`

The handle represents a running process from the perspective of the rest of the
system. It provides:

- `write_input(data: &[u8])` — Send bytes to the child's stdin.
- `resize(cols, rows)` — Send a terminal resize event.
- `kill()` — Terminate the child.
- `pid()` — Retrieve the OS process ID.
- `wait()` — An async future that resolves when the child exits.

### VTTY Emulator — `vtty/`

The VTTY emulator is vrunner's terminal rendering engine. It interprets VT100 /
xterm escape sequences and maintains an in-memory grid of cells that can be
rendered to HTML or incremental diffs.

#### `emulator.rs`

The emulator is the top-level state machine. It holds:

- A reference to the **parser** for incoming bytes.
- A reference to the **buffer** for the cell grid.
- A reference to the **renderer** for output generation.

Each call to `emulator.feed(bytes)` runs the parser, updates the buffer, and
(optionally) produces a renderable snapshot.

#### `parser.rs`

The parser is a byte-level state machine that recognizes:

- Printable characters (ASCII and UTF-8).
- Control characters (`\r`, `\n`, `\t`, `\b`, `\x1b`).
- CSI sequences (e.g., `\x1b[34m` for SGR, `\x1b[H` for cursor positioning).
- OSC sequences (e.g., window title setting).
- DCS and other less common sequences.

Unrecognized sequences are consumed silently to avoid parser deadlocks.

#### `buffer.rs`

The buffer maintains a 2D grid of `Cell` structs:

```rust
pub struct Cell {
    pub ch:    char,
    pub fg:    Color,
    pub bg:    Color,
    pub flags: CellFlags,  // bold, italic, underline, etc.
}

pub struct Buffer {
    pub cols:  usize,
    pub rows:  usize,
    pub cells: Vec<Cell>,
    pub cursor: Cursor,
    pub scroll_top:    usize,
    pub scroll_bottom: usize,
}
```

Scroll regions, cursor save/restore, and line insertion/deletion are all handled
inside the buffer.

#### `renderer.rs`

The renderer converts the buffer state into a format suitable for transmission
to the client. Two output modes exist:

1. **Full snapshot** — Generates a complete HTML representation of every visible
   cell (used for initial load and resynchronization).
2. **Incremental diff** — Compares the current buffer against the previously
   transmitted snapshot, emits only the changed cells. See
   [incremental-diff.md](incremental-diff.md) for the full protocol.

#### `display.rs`

`display.rs` implements the **display mode** state machine that governs how
vrunner exposes terminal output to the user:

```
                    ┌─────────────┐
                    │   Headless  │  (no display)
                    └──────┬──────┘
                           │ command starts
                           ▼
                    ┌─────────────┐
              ┌────►│   Active    │  (client connected, live updates)
              │     └──────┬──────┘
              │            │ client disconnects
              │            ▼
              │     ┌─────────────┐
              │     │   Monitor   │  (no client, buffering diffs)
              │     └──────┬──────┘
              │            │ client reconnects
              └────────────┘
```

The mode transitions are documented in [lifecycle-policy.md](lifecycle-policy.md).

### Handle System — `handles/`

The handle system is the output fan-out layer. Every byte written by a child
process to its PTY is routed through the handle system to one or more **sinks**:

```
  PTY stdout
      │
      ▼
  ┌──────────┐
  │  Handle   │──────► WebSocket Sink  (live terminal to browser)
  │  System   │──────► (future: File Sink)
  │           │──────► (future: DB Sink)
  └──────────┘
```

The handle system is designed as a trait:

```rust
pub trait HandleSink: Send + Sync {
    fn on_data(&self, bytes: &[u8]);
    fn on_exit(&self, exit_code: Option<i32>);
}
```

Adding a new sink means implementing this trait and registering it with the
handle system at command start time.

### Web Server — `web/`

The web server is built on Axum and serves both the JSON API and the WebSocket
terminal streams.

#### `server.rs`

Creates the Axum `Router`, attaches middleware (CORS, auth, logging), binds to
the configured address, and serves HTTPS when TLS is enabled.

#### `router.rs`

Defines all routes and wires them to handlers:

| Route | Method | Handler | Description |
|---|---|---|---|
| `/api/commands` | POST | `start_command` | Start a new named command |
| `/api/commands` | GET | `list_commands` | List all running commands |
| `/api/commands/{id}` | DELETE | `stop_command` | Stop a specific command |
| `/api/commands/{id}` | GET | `get_command` | Get command metadata |
| `/api/commands/{id}/resize` | POST | `resize_command` | Resize a command's PTY |
| `/api/commands/{id}/keys` | POST | `send_keys` | Send keystrokes to a command |
| `/api/instances` | GET | `list_instances` | List all instance metadata |
| `/ws/:id` | GET (upgrade) | `ws_handler` | WebSocket for terminal I/O |
| `/` | GET | `static_handler` | Serve the admin UI |
| `/assets/*` | GET | `static_handler` | Serve static assets |

#### `handlers/`

Each file in `handlers/` corresponds to one API endpoint group. Handlers extract
`State<AppState>`, perform validation, call the appropriate registry or command
manager methods, and return JSON responses or WebSocket upgrades.

### Admin Interface — `static/admin/`

The admin interface is a single-page application served at the root URL:

- **`index.html`** — The main page shell; loads JS and CSS.
- **JavaScript** — Opens a WebSocket to `/ws/:id`, renders terminal output using
  `xterm.js`, and provides buttons for start/stop/resize/send-keys.
- **CSS** — Minimal styling for layout and responsiveness.

The admin interface is a **read-only static bundle**. It is not compiled or
bundled at build time; it is simply included as static files. This keeps the build
simple and allows the interface to be replaced without recompiling vrunner.

---

## Data Flow

### Starting an Instance

```
 Client                    CLI / API              Instance Registry         Process Mgmt          VTTY
   │                          │                          │                         │                │
   │  POST /api/commands      │                          │                         │                │
   │  {name, cmd, args}      │                          │                         │                │
   │─────────────────────────►│                          │                         │                │
   │                          │  register(name, ...)     │                         │                │
   │                          │─────────────────────────►│                         │                │
   │                          │                          │                         │                │
   │                          │           spawn PTY      │                         │                │
   │                          │─────────────────────────────────────────────────►│                │
   │                          │                          │                         │                │
   │                          │              create VTTY emulator                 │                │
   │                          │─────────────────────────────────────────────────────────────────►│
   │                          │                          │                         │                │
   │                          │    wire handle (PTY → VTTY → WebSocket)           │                │
   │                          │◄─────────────────────────────────────────────────┤                │
   │                          │                          │                         │                │
   │  201 Created             │                          │                         │                │
   │  {id, name, status}      │                          │                         │                │
   │◄─────────────────────────│                          │                         │                │
   │                          │                          │                         │                │
   │  GET /ws/:id (upgrade)   │                          │                         │                │
   │─────────────────────────►│                          │                         │                │
   │                          │  stream terminal data    │                         │                │
   │◄═════════════════════════│◄═════════════════════════════════════════════════│◄══════════════│
```

### Listing Instances

```
 Client                    CLI / API              Instance Registry
   │                          │                          │
   │  GET /api/instances      │                          │
   │─────────────────────────►│                          │
   │                          │  registry.list()         │
   │                          │─────────────────────────►│
   │                          │                          │
   │                          │  Vec<(name, status, pid) │
   │                          │◄─────────────────────────│
   │                          │                          │
   │  200 OK                  │                          │
   │  [{name, status, pid}]   │                          │
   │◄─────────────────────────│                          │
```

### Stopping an Instance

```
 Client                    CLI / API              Instance Registry       Process Mgmt        VTTY
   │                          │                          │                      │                 │
   │  DELETE /api/commands/{id} │                          │                      │                 │
   │─────────────────────────►│                          │                      │                 │
   │                          │  registry.get(id)       │                      │                 │
   │                          │─────────────────────────►│                      │                 │
   │                          │                          │                      │                 │
   │                          │           handle.kill() │                      │                 │
   │                          │──────────────────────────────────────────────►│                 │
   │                          │                          │  emulator shutdown   │                 │
   │                          │─────────────────────────────────────────────────────────────────►│
   │                          │                          │                      │                 │
   │                          │  registry.unregister(id)│                      │                 │
   │                          │─────────────────────────►│                      │                 │
   │                          │                          │                      │                 │
   │                          │  lifecycle check        │                      │                 │
   │                          │─────────────────────────►│                      │                 │
   │                          │                          │                      │                 │
   │  200 OK / 404            │                          │                      │                 │
   │◄─────────────────────────│                          │                      │                 │
```

---

## Concurrency Model

vrunner is built entirely on the **Tokio** asynchronous runtime. The concurrency
architecture has several key elements:

### Runtime Configuration

```
┌────────────────────────────────────────────┐
│           Tokio Multi-Thread Runtime       │
│                                            │
│  Worker threads = num_cpus                 │
│  Blocking pool = 512 threads (for PTY I/O) │
│  Timer precision = 1ms                      │
└────────────────────────────────────────────┘
```

The runtime is configured in `main.rs` via `tokio::main` with the `multi_thread`
flavor, providing a thread pool whose size defaults to the number of logical CPUs.

### Shared State via `Arc`

All shared state is wrapped in `Arc` (atomic reference counted) to allow cheap
cloning across async tasks:

```
AppState (Arc)
├── InstanceRegistry (Arc<DashMap<String, Instance>>)
├── CommandManager   (Arc<DashMap<String, Command>>)
├── auth_token        (Option<String>)
├── shutdown_tx       (broadcast::Sender<()>)
└── config           (Arc<Config>)
```

### DashMap for Concurrent Commands

The `CommandManager` uses `DashMap` rather than a `Mutex<HashMap>` to provide
lock-free concurrent access:

| Operation | Locking Behavior |
|---|---|
| Insert a new command | Fine-grained shard lock (O(1)) |
| Read a command | Lock-free snapshot |
| Remove a command | Fine-grained shard lock (O(1)) |
| Iterate all commands | Reader-writer lock on all shards |

This means that starting, stopping, and querying commands can happen
simultaneously without blocking each other.

### Per-Command Tasks

Each running command gets its own set of Tokio tasks:

```
Command "web-server"
├── Task 1: PTY read loop  (PTY stdout → VTTY → Handle sinks)
├── Task 2: WebSocket handler (client input → PTY stdin)
├── Task 3: Periodic diff timer (every 200ms, VTTY → renderer)
└── Task 4: Process monitor (await child exit)
```

These tasks communicate through bounded channels:

```
┌──────────┐   bytes   ┌──────────┐   cells   ┌──────────┐
│ PTY Read │──────────►│   VTTY   │──────────►│ Renderer │
│   Loop   │           │ Emulator │           │          │
└──────────┘           └──────────┘           └────┬─────┘
                                                    │ diffs
                                              ┌─────▼─────┐
                                              │  Handle   │
                                              │  System   │
                                              └─────┬─────┘
                                                    │
                                              ┌─────▼─────┐
                                              │ WebSocket │
                                              │   Sink    │
                                              └───────────┘
```

Channel bounds are small (typically 64 or 128 frames) to prevent unbounded memory
growth if a sink is slow. When a channel is full, the sender applies backpressure
by awaiting capacity.

### Sync/Async Bridge

The PTY library (`portable-pty`) uses synchronous I/O. vrunner bridges this
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

1. The CLI or signal handler sends `()` on `shutdown_tx`.
2. All tasks that hold a `shutdown_rx` receive the signal.
3. Each task performs its cleanup (flush buffers, close PTY, send close frames).
4. Tasks complete; the Tokio runtime shuts down.

```
┌───────────┐    ()     ┌────────────┐    ()     ┌────────────┐
│  CLI /    │──────────►│  Web       │──────────►│  Command   │
│  Signal   │           │  Server    │           │  Task 1    │
│  Handler  │           │  shutdown  │           │  shutdown  │
└───────────┘           └────────────┘           └────────────┘
       │                                                  │
       │             ┌────────────┐    ()                 │
       └────────────►│  Command   │──────────────────────┘
                     │  Task 2    │
                     │  shutdown  │
                     └────────────┘
                            │
                     ┌──────▼──────┐
                     │  Command    │
                     │  Task 3     │
                     │  shutdown   │
                     └─────────────┘
```

---

## Lifecycle Policy

vrunner's lifecycle is governed by the **"Last-Command-Standing"** principle. This
policy and the display mode state machine are covered in full in
[lifecycle-policy.md](lifecycle-policy.md). Here is a summary:

| Mode | Description | Behavior on Last Command Exit |
|---|---|---|
| **Headless** | No display, daemon mode only | Daemon exits immediately |
| **Display** | Active client connected | Transitions to Monitor |
| **Monitor** | No active client, buffering | Retains output; exits after timeout |
| **Retain-on-Exit** | Per-command override | Command entry persists in registry |

The daemon's lifecycle loop runs every 500ms:

```rust
loop {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_millis(500)) => {
            if registry.is_empty() && !any_retained {
                info!("No instances remaining; shutting down");
                shutdown_tx.send(()).ok();
                break;
            }
        }
        _ = shutdown_rx.recv() => {
            info!("Shutdown signal received");
            break;
        }
    }
}
```

---

## Extension Points

### Adding a New Web Command

To add a new command that is accessible via the API:

1. **Define the handler** — Create a new function in `web/handlers/` that
   accepts `State<AppState>` and request parameters.
2. **Add the route** — Register the handler in `web/router.rs` with an
   appropriate HTTP method and path.
3. **Add CLI support** — If the command should also be triggerable from the
   CLI, add a new subcommand variant in `cli/`.
4. **Update tests** — Add unit tests for the handler logic and integration
   tests for the full request cycle.

### Adding a New Handle Sink

To route process output to a new destination (e.g., a database, a file, an
alerting system):

1. **Implement the trait** — Create a struct that implements `HandleSink`:

```rust
struct DatabaseSink {
    pool: sqlx::PgPool,
}

impl HandleSink for DatabaseSink {
    fn on_data(&self, bytes: &[u8]) {
        // Write to database
    }
    fn on_exit(&self, exit_code: Option<i32>) {
        // Record exit event
    }
}
```

2. **Register at spawn time** — When creating a command in the process manager,
   instantiate the sink and add it to the handle system's sink list.
3. **Configure** — Add any necessary configuration options (e.g., database URL)
   to `CommandConfig`.

---

## Key Crate Dependencies

The following table lists the major Rust crates that vrunner depends on, along
with their role and version:

| Crate | Version | Role |
|---|---|---|
| `tokio` | 1.x | Async runtime, channels, timer, signal handling |
| `axum` | 0.7+ | HTTP framework (routing, extractors, middleware) |
| `tower-http` | 0.5+ | CORS, compression, request tracing middleware |
| `portable-pty` | 0.8+ | Cross-platform PTY creation and I/O |
| `rustls` | 0.23+ | Pure-Rust TLS implementation (server side) |
| `rcgen` | 0.12+ | Self-signed certificate generation |
| `rustls-pemfile` | 2.x | PEM file parsing for custom certificates |
| `dashmap` | 6.x | Concurrent hash map for command registry |
| `serde` / `serde_json` | 1.x | JSON serialization for API responses |
| `clap` | 4.x | Command-line argument parsing |
| `toml` | 0.8+ | Configuration file parsing |
| `rand` | 0.8+ | Cryptographically secure token generation |
| `tracing` / `tracing-subscriber` | 0.1+ | Structured logging |
| `xterm-js` (bundled) | 5.x | Browser-side terminal emulator (admin UI) |

---

*This document is part of the [Diátaxis](https://diataxis.fr/) documentation framework
for vrunner. See the [explanation index](./) for related topics.*
