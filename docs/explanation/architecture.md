# Architecture

This document provides a comprehensive technical architecture overview of **vrl**. It
covers the design principles that guide every module, the system context and module
relationships, data-flow diagrams for common operations, the concurrency model, the
lifecycle policy, and extension points for contributors. You should read this document
if you want to understand how vrl is structured, how data moves through the
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

vrl is built on six core principles that influence every design decision:

### Silent by Default

vrl produces no output unless something requires the user's attention. When a
command exits cleanly there is no fanfare, no summary table, no timestamp. This makes
vrl ideal as a wrapper in scripts and pipes—stdout and stderr belong to the
child process. Diagnostic messages are routed to logging subsystems and only surface
when the user explicitly raises verbosity.

### Local IPC Only

All inter-process communication uses Unix Domain Sockets. There is no HTTP server,
no network binding, no TLS, and no authentication mechanism. The UDS socket is
created with `0600` permissions, ensuring only the owning user can connect.
This provides security through filesystem permissions without the complexity of TLS,
certificates, or bearer tokens.

### Separation of Concerns

Each module owns exactly one responsibility. The daemon starts processes; the IPC
server handles control commands; the instance registry tracks state; the VTTY emulator
renders terminal output. Modules communicate through well-defined interfaces—never by
reaching into each other's internals.

### Extensibility

vrl is designed so that new features can be added without modifying core logic.
IPC commands register themselves with a central `CommandManager`. Handle sinks receive
process output without being coupled to the IPC layer. New handle sinks (database
writers, file loggers, alerting systems) can be plugged in behind the same interface.

### Async-First

The entire application is built on Tokio. IPC I/O, process spawning, periodic
timers, and shutdown signalling all use asynchronous primitives. Synchronous
operations (PTY I/O via `portable-pty`) are isolated behind bounded channels so they
never block the async runtime.

### Multi-Instance Awareness

A single vrl invocation can manage dozens of commands simultaneously. The
instance registry ensures that resources are cleaned up when instances exit, and that
clients can address instances by PID.

---

## System Context

```
┌──────────────────────────────────────────────────────────────────────────┐
│                              User / Operator                             │
└──────┬───────────────┬───────────────────────────┬─────────────────────┘
       │               │                           │
       ▼               ▼                           ▼
┌──────────────┐ ┌───────────────┐    ┌──────────────────────────────────┐
│    CLI       │ │   Config      │    │         UDS Clients               │
│  (main.rs,   │ │  (config/)    │    │  ┌──────────┐  ┌────────────────┐  │
│   cli/)      │ │               │    │  │ vrl list │  │  vrl keys      │  │
│              │ │ • vtty        │    │  └──────────┘  └───────┬────────┘  │
│ • args       │ │ • display     │    │       │               │            │
│ • subcommands│ │ • daemon      │    │       │               │            │
│ • daemonize │ │ • per-cmd opts│    │       ▼               ▼            │
└──────┬───────┘ │ • hooks       │    │  ┌────────────────────────────┐   │
       │         │ • env vars    │    │  │  UDS Control Socket       │   │
       │         │ • profiles     │    │  │  ~/.local/share/vrl/     │   │
       │         └──────┬────────┘    │  │    control-{pid}.sock    │   │
       │                │             │  │                            │   │
       ▼                ▼             │  │  • Ping                   │   │
┌─────────────────────────────────┐    │  │  • List commands          │   │
│       Instance Registry        │    │  │  • SendKeys              │   │
│       (instance/)               │◄───┼─►│  • Cat VTTY output        │   │
│                                 │    │  │  • Spawn                 │   │
│  ┌─────────┐ ┌───────────────┐  │    │  │  • Kill                  │   │
│  │ Instance │ │ Command       │  │    │  │  • Freeze / Thaw         │   │
│  │ (pid,    │ │ Manager       │  │    │  │  • Resize               │   │
│  │  config) │ │ (DashMap<id,  │  │    │  │  • Shutdown              │   │
│  └────┬─────┘ │   Command>)    │  │    │  └────────────┬───────────┘   │
│       │               │          │    │               │                   │
│       │               │          │    │               ▼                   │
└───────┼───────────────┼──────────┘    │  ┌────────────────────────────┐  │
        │               │               │  │    Command Manager          │  │
        ▼               ▼               │  │    (process/manager.rs)     │  │
┌─────────────────────────────────┐     │  └────────────┬───────────────┘   │
│         Shared State            │     │               │                   │
│      (ipc/state.rs)             │◄────┼─►             │                   │
│                                 │     │               ▼                   │
│  • Arc<InstanceRegistry>        │     │  ┌────────────────────────────┐  │
│  • Arc<CommandManager>          │     │  │    Process Management       │  │
│  • shutdown_tx (broadcast)     │     │  │    (process/)               │  │
└───────────────┬─────────────────┘     │  └────────────┬───────────────┘   │
                │                       │               │                   │
                ▼                       │               ▼                   │
┌─────────────────────────────────┐     │  ┌────────────────────────────┐  │
│         Daemon                  │     │  │     VTTY Emulator         │  │
│        (daemon/)                │     │  │       (vtty/)              │  │
│                                 │     │  └────────────────────────────┘  │
│  • daemonize / re-parent        │     │                                   │
│  • PID file management          │     │  ┌────────────────────────────┐  │
│  • signal handling              │     │  │      Handle System         │  │
│  • lifecycle orchestration      │     │  │       (handles/)           │  │
└───────────────┬─────────────────┘     │  └────────────────────────────┘  │
                │                       └───────────────────────────────────┘
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
│  • display.rs  – mode control   │
└───────────────┬─────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│      Handle System             │
│       (handles/)               │
│                                 │
│  • File sink                   │
│  • VTTY sink                   │
│  • Null sink                    │
└─────────────────────────────────┘
```

---

## Module Breakdown

### CLI — `main.rs`, `cli/`

The CLI layer is the single entry-point for every vrl invocation. It parses
command-line arguments, selects the appropriate subcommand, and either executes
directly or daemonizes. The subcommand set includes:

| Subcommand | Purpose |
|---|---|
| `list` | List all running vrl instances with their commands |
| `stop` | Stop a running vrl instance |
| `spawn-in` | Spawn a new command in a running instance |
| `keys` | Send keystrokes to a command |
| `cat` | Print VTTY buffer of a command |
| `freeze` | Pause a command (SIGSTOP) |
| `thaw` | Resume a command (SIGCONT) |
| `resize` | Resize a command's VTTY |
| `config-check` | Validate configuration files |

### Configuration — `config/`

Configuration is a layered system with clear precedence:

```
Priority (highest → lowest):
  1. CLI flags               (--vtty-rows, --display, etc.)
  2. Environment variables   (VRUNNER_PORT, etc.)
  3. Configuration file       (~/.config/vrl/config.yaml)
  4. Built-in defaults
```

### IPC Server — `ipc/`

The UDS IPC server replaces the entire HTTP server stack. It listens on a Unix
Domain Socket and dispatches commands to the `CommandManager`.

#### Wire Protocol

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

### Instance Registry — `instance/`

The instance registry is the single source of truth for all running commands.
It reads PID files from `~/.local/share/vrl/instances/` and validates liveness
via `/proc/<pid>/comm` on Linux.

### Daemon — `daemon/`

The daemon module handles three responsibilities:

1. **Re-parenting** — When `--daemon` is passed, the process forks, writes a PID file, and redirects stdio.
2. **Signal handling** — Listens for `SIGTERM`, `SIGINT`, and `SIGUSR1`.
3. **Lifecycle loop** — When the registry is empty and the daemon is in default mode, it initiates shutdown.

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

### Starting an Instance

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

### Listing Instances

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

### Stopping an Instance

```
 CLI
   │
   │  vrl stop 12345
   │─────────►
   │  kill(SIGTERM, 12345)
   │
```

---

## Concurrency Model

vrl is built entirely on the **Tokio** asynchronous runtime (single-threaded flavor).

### Runtime Configuration

```
┌────────────────────────────────────────────┐
│           Tokio Current-Thread Runtime     │
│                                            │
│  No worker threads — sufficient for UDS +  │
│  PTY I/O without an HTTP server           │
└────────────────────────────────────────────┘
```

### DashMap for Concurrent Commands

The `CommandManager` uses `DashMap` rather than a `Mutex<HashMap>` to provide
lock-free concurrent access.

### Per-Command Tasks

Each running command gets its own set of tasks:

```
Command "web-server"
├── Task 1: PTY read loop  (PTY stdout → VTTY → Handle sinks)
├── Task 2: Process monitor (await child exit)
└── Task 3: UDS IPC handler (control commands from CLI)
```

### Sync/Async Bridge

The PTY library (`portable-pty`) uses synchronous I/O. vrl bridges this
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
3. Each task performs its cleanup and exits.

---

## Lifecycle Policy

vrl's lifecycle is governed by the **"Last-Command-Standing"** principle.

| Mode | Description | Behavior on Last Command Exit |
|---|---|---|
| **Headless** | No display, daemon mode only | Exits immediately |
| **Display** | Active client connected | Transitions to Monitor |
| **Monitor** | No active client, buffering | Exits when no commands remain |
| **Retain-on-Exit** | Per-command override | Command entry persists in registry |

---

## Extension Points

### Adding a New IPC Command

1. **Define the command** — Add a variant to `ControlCommand` in `ipc/protocol.rs`.
2. **Handle the command** — Add dispatch logic in `ipc/server.rs`.
3. **Add CLI handler** — Add a handler function in `cli/commands/ipc.rs`.
4. **Add CLI subcommand** — Register the subcommand variant in `cli/args.rs`.
5. **Update tests** — Add unit and integration tests.

### Adding a New Handle Sink

1. **Implement the trait** — Create a struct that implements `HandleSink`.
2. **Register at spawn time** — When creating a command, instantiate the sink and add it to the handle system.
3. **Configure** — Add configuration options to `CommandConfig`.

---

## Key Crate Dependencies

| Crate | Role |
|---|---|
| `tokio` | Async runtime, channels, timer, signal handling |
| `portable-pty` | Cross-platform PTY creation and I/O |
| `dashmap` | Concurrent hash map for command registry |
| `serde` / `serde_json` | JSON serialization for IPC protocol |
| `clap` | Command-line argument parsing |
| `config` | Configuration file loading |
| `tracing` / `tracing-subscriber` | Structured logging |
| `parking_lot` | Fast mutex/rwlock |
| `crossterm` | Local terminal display |
| `libc` | POSIX signals, daemonization |

---

*This document is part of the [Diátaxis](https://diataxis.fr/) documentation framework
for vrl. See the [explanation index](./) for related topics.*
