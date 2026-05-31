# vrl Architecture Overview

This document describes the high-level architecture of **vrl**, a Rust-based virtual terminal runner with UDS (Unix Domain Socket) IPC, daemon mode, and instance registry.

---

## 1. Design Principles

1. **Silent by Default** — The local terminal is not a log sink unless explicitly requested.
2. **Local IPC Only** — All inter-process communication uses Unix Domain Sockets; no network exposure.
3. **Separation of Concerns** — CLI parsing, config loading, process management, terminal emulation, and instance tracking are distinct modules.
4. **Speed** — No HTTP server, no TLS, no heavy dependencies. Startup in under 5ms.
5. **Async-First** — Built on `tokio` (single-threaded) to handle concurrent connections, processes, and I/O loops.
6. **Multi-Instance Awareness** — The tool manages not only commands but also peer `vrl` processes on the same machine.

---

## 2. System Context

```
┌─────────────────────────────────────────────────────────────────────┐
│                         vrl binary                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐  │
│  │   CLI       │  │   Config    │  │   Instance  │  │  Daemon   │  │
│  │   Parser    │  │   Loader    │  │   Registry  │  │  Mode     │  │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └─────┬─────┘  │
│         │                │                │               │        │
│         └────────────────┴────────────────┴───────────────┘        │
│                          │                                         │
│              ┌───────────▼────────────┐                            │
│              │     Command Manager    │                            │
│              │   (Process Registry)   │                            │
│              └───────────┬────────────┘                            │
│                          │                                         │
│         ┌────────────────┼────────────────┐                       │
│         │                │                │                       │
│  ┌──────▼──────┐  ┌─────▼─────┐  ┌──────▼──────┐                │
│  │  Command 1  │  │ Command 2 │  │  Command N  │                │
│  │  + VTTY     │  │  + VTTY   │  │   + VTTY    │                │
│  │  + Handles  │  │ + Handles │  │  + Handles  │                │
│  └─────────────┘  └───────────┘  └─────────────┘                │
│                          │                                         │
│              ┌───────────▼────────────┐                            │
│              │    IPC Server            │                            │
│              │  (UDS control socket)     │                            │
│              └───────────┬────────────┘                            │
│                          │                                         │
│              ┌───────────▼────────────┐                            │
│              │   UDS Clients          │                            │
│              │  (vrl list, vrl keys,  │                            │
│              │   vrl cat, vrl spawn-in)│                            │
│              └────────────────────────┘                            │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Module Breakdown

### 3.1 CLI Entry Point (`src/main.rs` + `src/cli/`)

`main.rs` is a thin binary wrapper that parses CLI arguments, loads configuration, optionally daemonizes, and delegates to the library crate. `src/lib.rs` is the single source of truth for all `mod` declarations.

Uses `clap` with derive macros. Supports:
- Options before `--` (vrl flags)
- Command + args after `--` (child process)
- Subcommands: `list`, `stop`, `spawn-in`, `keys`, `cat`, `freeze`, `thaw`, `resize`, `config-check`, `completions`

### 3.2 Configuration Layer (`src/config/`)

| Component | Responsibility |
|-----------|-------------|
| `loader.rs` | Discovers global (`~/.config/vrl/config.yaml`) and local (`./vrl.yaml`) YAML configs, plus any CLI-specified path. |
| `schema.rs` | Typed structs with serde: `Config`, `VttyConfig`, `DisplayConfig`, `CommandLogConfig`, `DaemonConfig`, `HandleConfig`. |
| `merge.rs` | Override logic: local config overrides global config. CLI flags applied on top. |

### 3.3 IPC Server (`src/ipc/`)

The UDS IPC server is the heart of the speedup branch, replacing the entire HTTP server stack.

| Component | Responsibility |
|-----------|-------------|
| `server.rs` | Binds UDS socket at `~/.local/share/vrl/control-{pid}.sock` (permissions `0600`). Accept loop dispatches incoming commands to the `CommandManager`. |
| `protocol.rs` | Wire protocol: `ControlCommand` enum, `ControlResponse` enum, length-prefixed JSON framing (`[4-byte big-endian u32][JSON payload]`). |
| `client.rs` | UDS client for CLI subcommands: connect, send `ControlCommand`, receive `ControlResponse`. |
| `mod.rs` | Module declarations + `socket_path_for_pid()` helper. |

#### Supported IPC Commands

| Command | Description |
|---------|-------------|
| `Ping` | Health check |
| `List` | List running commands |
| `SendKeys` | Inject keystrokes into a command's PTY |
| `Cat` | Retrieve VTTY buffer as plain text |
| `Spawn` | Create a new command in a running instance |
| `Kill` | Terminate a running command |
| `Freeze` | Pause a command (SIGSTOP) |
| `Thaw` | Resume a paused command (SIGCONT) |
| `Resize` | Change VTTY dimensions (SIGWINCH) |
| `Shutdown` | Gracefully shut down the instance |

#### Socket Security

- Socket path: `~/.local/share/vrl/control-{pid}.sock`
- File permissions: `0600` (owner read/write only)
- Only processes running as the same user can connect
- No network exposure (UDS is local-only by definition)

### 3.4 Instance Registry (`src/instance/`)

Manages a directory of JSON pidfiles (`~/.local/share/vrl/instances/<PID>.json`).

| Component | Responsibility |
|-----------|-------------|
| `registry.rs` | `InstanceRegistry`: register, unregister, list, stop. Validates liveness via `/proc/<pid>/comm` on Linux. Auto-cleans stale pidfiles. |
| `info.rs` | `InstanceInfo`: serializable metadata (PID, start time, daemon/display flags, command). |

### 3.5 Daemon Mode (`src/daemon/`)

| Component | Responsibility |
|-----------|-------------|
| `mod.rs` | Platform dispatch. |
| `unix.rs` | Custom double-fork daemonization using raw `libc` calls (`fork`, `setsid`, fd redirection). Called before tokio runtime to avoid conflicts with async signal handling. |

### 3.6 Command Logger (`src/logging/`)

| Component | Responsibility |
|-----------|-------------|
| `command_log.rs` | `CommandLogger`: thread-safe logger that writes to screen, file, or both. |

### 3.7 Process Management (`src/process/`)

| Component | Responsibility |
|-----------|-------------|
| `manager.rs` | `CommandManager`: owns `DashMap<CommandId, CommandHandle>`. Spawns, lists, kills, freezes, thaws. Manages named VTTY buffer snapshots. |
| `spawner.rs` | Platform-specific PTY creation via `portable-pty`. Uses `mpsc::channel` bridge between synchronous PTY reads and async VTTY writes. |
| `handle.rs` | `CommandHandle`: per-command state (ID, PID, name, args, VTTY reference, spawn time). |

### 3.8 VTTY Emulator (`src/vtty/`)

| Component | Responsibility |
|-----------|-------------|
| `emulator.rs` | Terminal state machine supporting cursor movement, erase operations, scroll, SGR attributes, DEC private modes, alternate screen. |
| `parser.rs` | Streaming ANSI parser with state machine (CSI, OSC, DCS, simple escapes). |
| `buffer.rs` | 2D cell grid with scrollback, insert/delete lines/cells, clear operations. |
| `display.rs` | `TerminalDisplay`: renders the buffer to the local terminal using `crossterm` (only when `--display` is active). |

### 3.9 Handle System (`src/handles/`)

Extensible file descriptor routing.

| Component | Responsibility |
|-----------|-------------|
| `registry.rs` | `HandleRegistry`: map of name to sink. |
| `sink.rs` | `Sink` trait (async). |
| `file_sink.rs`, `vtty_sink.rs`, `null_sink.rs` | Implementations. |

---

## 4. Data Flow

### 4.1 Starting an Instance

```
CLI: vrl -- htop
       │
       ▼
┌──────────────┐
│ CLI Parser   │──► Extract vrl flags + child command
└──────────────┘
       │
       ▼
┌──────────────┐
│ Config       │──► Merge global + local + CLI overrides
│ Loader       │
└──────────────┘
       │
       ▼
┌──────────────┐
│ Daemonize?   │──► If --daemon, double-fork via libc (before tokio)
└──────────────┘
       │
       ▼
┌──────────────┐
│ Instance     │──► Write pidfile to registry
│ Registry     │
└──────────────┘
       │
       ▼
┌──────────────┐
│ Command      │──► Spawn htop in PTY, create VTTY
│ Manager      │──► Wire mpsc channel bridge (sync PTY → async VTTY)
└──────────────┘
       │
       ▼
┌──────────────┐
│ IPC Server   │──► Start UDS listener on ~/.local/share/vrl/control-{pid}.sock
└──────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────┐
│ Lifecycle:                                            │
│                                                      │
│ Display mode: render VTTY → monitor child → on exit   │
│   check manager.list().is_empty()                    │
│   → empty: restore terminal + shutdown                │
│   → not empty: switch to monitor mode                 │
│                                                      │
│ Headless mode: wait_for_child → check manager        │
│   → empty: shutdown                                   │
│   → not empty: idle wait on shutdown channel         │
│                                                      │
│ Idle mode: wait on shutdown channel (no child)       │
└──────────────────────────────────────────────────────┘
```

### 4.2 Listing Instances

```
CLI: vrl list
       │
       ▼
┌──────────────┐
│ Instance     │──► Read all pidfiles from ~/.local/share/vrl/instances/
│ Registry     │──► Filter out stale entries (check /proc/<pid>/comm)
└──────────────┘
       │
       ▼
   Print table
```

### 4.3 Stopping an Instance

```
CLI: vrl stop 12345
       │
       ▼
┌──────────────┐
│ Process      │──► Send SIGTERM to PID 12345
│ Signal       │
└──────────────┘
```

---

## 5. Concurrency Model

- **Tokio Runtime**: Single-threaded scheduler (`current_thread`), started after daemonization (if applicable). Sufficient since there is no HTTP server.
- **Command Manager**: `Arc<DashMap<CommandId, CommandHandle>>` for lock-free reads.
- **Per-Command Tasks**: `pty_reader` (blocking thread → mpsc channel), `stdin_writer`, `handle_writers`, `process_waiter`.
- **Sync/Async Bridge**: PTY reads happen on a blocking thread (`portable-pty`). Data is sent through a bounded `tokio::sync::mpsc::channel(64)` to a single async receiver task that feeds the VTTY emulator.
- **Command Logger**: `Arc<CommandLogger>` shared between process manager.
- **Shutdown**: `broadcast::Sender<()>` distributed. Signal handler sends on the channel; listener triggers graceful shutdown.
- **Lifecycle Policy**: "Last-command-standing" — vrl remains alive as long as at least one command exists. Shutdown occurs only when the command count reaches zero.

---

## 6. Crate Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime (current_thread flavor) |
| `serde` / `serde_json` | Serialization for UDS protocol |
| `clap` / `clap_complete` | CLI parsing and shell completions |
| `config` | Hierarchical config loading |
| `anyhow` | Error handling |
| `tracing` / `tracing-subscriber` | Structured logging |
| `uuid` | Command ID generation |
| `chrono` | Timestamps |
| `portable-pty` | Cross-platform PTY creation |
| `parking_lot` | `RwLock` for VTTY buffer |
| `crossterm` | Local terminal display |
| `libc` | Raw Unix syscalls for daemonization and signals |
| `dirs` | Standard directories |
| `dashmap` | Concurrent hash map |
| `async-trait` | Async trait support |
| `unicode-width` | Terminal column width calculation |
| `regex` | Pattern matching in display/keybindings |

---

## 7. Extension Points

### Adding a New IPC Command

1. Add variant to `ControlCommand` enum in `src/ipc/protocol.rs`.
2. Add handling in `src/ipc/server.rs`.
3. Add CLI handler in `src/cli/commands/ipc.rs`.
4. Document in README and configuration reference.

### Adding a New Handle Sink

1. Implement the `Sink` trait in `src/handles/`.
2. Add the sink type name to the factory logic.
3. Document in the configuration reference.

---

## 8. Testing Strategy

- **Unit tests**: VTTY parser, buffer operations, config merging, key encoding.
- **Integration tests**: Spawn command, send keys via UDS, assert VTTY contents.
- **Instance tests**: Start two instances, verify `vrl list` shows both, verify `vrl stop` shuts one down.
- **Platform tests**: CI matrix for Linux, macOS, Windows.
