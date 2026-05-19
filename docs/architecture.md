# vrunner Architecture Overview

This document describes the high-level architecture of **vrunner**, a Rust-based virtual terminal runner with a web control plane, daemon mode, and instance registry.

---

## 1. Design Principles

1. **Silent by Default** — The local terminal is not a log sink unless explicitly requested.
2. **Separation of Concerns** — CLI parsing, config loading, process management, terminal emulation, web serving, and instance tracking are distinct modules.
3. **Extensibility** — New web API commands, handle sinks, and VTTY backends can be added without modifying core logic.
4. **Async-First** — Built on `tokio` to handle concurrent connections, processes, and I/O loops.
5. **Multi-Instance Awareness** — The tool manages not only commands but also peer `vrunner` processes on the same machine.

---

## 2. System Context

```
┌─────────────────────────────────────────────────────────────────────┐
│                         vrunner binary                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐  │
│  │   CLI       │  │   Config    │  │   Instance│  │   Daemon  │  │
│  │   Parser    │  │   Loader    │  │   Registry  │  │   Mode    │  │
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
│              │      Web Server          │                            │
│              │   (axum / tokio)         │                            │
│              └───────────┬────────────┘                            │
│                          │                                         │
│              ┌───────────▼────────────┐                            │
│              │   HTTP Clients         │                            │
│              │  (Browser / curl /     │                            │
│              │   other vrunner CLIs)  │                            │
│              └────────────────────────┘                            │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Module Breakdown

### 3.1 CLI Entry Point (`src/main.rs` + `src/cli/`)

Uses `clap` with derive macros. Supports:
- Options before `--` (vrunner flags)
- Command + args after `--` (child process)
- Subcommands: `list`, `stop <PID>`

**`src/cli/args.rs`** defines:
- `Cli` struct with `#[command(trailing_var_arg = true)]`
- `Commands` enum for `List` and `Stop`
- Fields: `config`, `bind`, `port`, `daemon`, `display`, `no_display`, `log`, `log_file`, `vtty_rows`, `vtty_cols`, `cmd_args`

### 3.2 Configuration Layer (`src/config/`)

| Component | Responsibility |
|-----------|-------------|
| `loader.rs` | Discovers global + local YAML configs. |
| `schema.rs` | Typed structs including new sections: `DisplayConfig`, `CommandLogConfig`, `DaemonConfig`. |
| `merge.rs` | Override logic: CLI > local > global. |

### 3.3 Instance Registry (`src/instance/`)

Manages a directory of JSON pidfiles (`~/.local/share/vrunner/instances/<PID>.json`).

| Component | Responsibility |
|-----------|-------------|
| `registry.rs` | `InstanceRegistry`: register, unregister, list, get. Validates liveness via `sysinfo`. |
| `info.rs` | `InstanceInfo`: serializable metadata about a running vrunner process. |

### 3.4 Daemon Mode (`src/daemon/`)

| Component | Responsibility |
|-----------|-------------|
| `mod.rs` | Platform dispatch. |
| `unix.rs` | Uses `daemonize` crate to detach from TTY, redirect stdout/stderr to files. |

### 3.5 Command Logger (`src/logging/`)

| Component | Responsibility |
|-----------|-------------|
| `command_log.rs` | `CommandLogger`: thread-safe logger that writes to screen, file, or both. Used by the web layer to audit API calls. |

### 3.6 Process Management (`src/process/`)

| Component | Responsibility |
|-----------|-------------|
| `manager.rs` | `CommandManager`: owns `DashMap<CommandId, CommandHandle>`. Spawns, lists, kills. Injects `CommandLogger`. |
| `spawner.rs` | Platform-specific PTY creation via `portable-pty`. |
| `handle.rs` | `CommandHandle`: per-command state (ID, PID, name, VTTY reference). |

### 3.7 VTTY Emulator (`src/vtty/`)

| Component | Responsibility |
|-----------|-------------|
| `emulator.rs` | Terminal state machine. |
| `parser.rs` | ANSI sequence tokenizer. |
| `buffer.rs` | 2D cell grid with color attributes. |
| `renderer.rs` | Serialize buffer to ANSI or HTML. |
| `display.rs` | `TerminalDisplay`: renders the buffer to the local terminal using `crossterm` (only when `--display` is active). |

### 3.8 Handle System (`src/handles/`)

Extensible file descriptor routing.

| Component | Responsibility |
|-----------|-------------|
| `registry.rs` | `HandleRegistry`: map of name -> sink. |
| `sink.rs` | `Sink` trait. |
| `file_sink.rs`, `vtty_sink.rs`, `null_sink.rs` | Implementations. |

### 3.9 Web Server (`src/web/`)

| Component | Responsibility |
|-----------|-------------|
| `server.rs` | Binds TCP socket, starts `axum::serve()`. |
| `router.rs` | Route table including `/api/shutdown`. Designed for easy extension. |
| `handlers/` | One module per endpoint group. |
| `middleware.rs` | CORS, request logging, JSON error envelopes. |
| `static_assets.rs` | Embedded admin SPA via `rust-embed`. |

#### Route Table

```rust
pub fn create_router(manager: Arc<CommandManager>) -> Router {
    Router::new()
        .route("/api/commands", get(list_commands).post(start_command))
        .route("/api/commands/:id/keys", post(send_keys))
        .route("/api/commands/:id/kill", post(kill_command))
        .route("/api/commands/:id/vtty", get(get_vtty_full))
        .route("/api/commands/:id/vtty/partial", get(get_vtty_partial))
        .route("/api/commands/:id/handles", get(list_handles).post(add_handle))
        .route("/api/shutdown", post(shutdown))
        .route("/admin", get(admin_page))
        .route("/admin/*path", get(admin_assets))
        .with_state(manager)
}
```

### 3.10 Admin Interface (`static/admin/`)

Lightweight SPA served from embedded assets. Communicates with the REST API.

---

## 4. Data Flow

### 4.1 Starting an Instance

```
CLI: vrunner --port 9090 --daemon -- htop
       │
       ▼
┌──────────────┐
│ CLI Parser   │──► Extract vrunner flags + child command
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
│ Daemonize?   │──► If --daemon, detach from TTY
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
│ Manager      │
└──────────────┘
       │
       ▼
┌──────────────┐
│ Web Server   │──► Start axum on 127.0.0.1:9090
└──────────────┘
```

### 4.2 Listing Instances

```
CLI: vrunner list
       │
       ▼
┌──────────────┐
│ Instance     │──► Read all pidfiles from ~/.local/share/vrunner/instances/
│ Registry     │──► Filter out stale entries (dead PIDs)
└──────────────┘
       │
       ▼
   Print table
```

### 4.3 Stopping an Instance

```
CLI: vrunner stop 12345
       │
       ▼
┌──────────────┐
│ Instance     │──► Read 12345.json from registry
│ Registry     │──► Get bind address and port
└──────────────┘
       │
       ▼
┌──────────────┐
│ HTTP Client  │──► POST http://<bind>:<port>/api/shutdown
└──────────────┘
```

---

## 5. Concurrency Model

- **Tokio Runtime**: Multi-threaded scheduler.
- **Command Manager**: `Arc<DashMap<CommandId, CommandHandle>>` for lock-free reads.
- **Per-Command Tasks**: `pty_reader`, `stdin_writer`, `handle_writers`, `process_waiter`.
- **Web Handlers**: Stateless, borrow `Arc<CommandManager>` from axum state.
- **Command Logger**: `Arc<CommandLogger>` shared between web handlers and process manager.

---

## 6. Crate Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime |
| `axum` | HTTP server |
| `reqwest` | HTTP client (for `vrunner stop`) |
| `serde` / `serde_json` | Serialization |
| `clap` | CLI parsing |
| `config` | Hierarchical config loading |
| `thiserror` / `anyhow` | Error handling |
| `tracing` | Structured logging |
| `uuid` | Command ID generation |
| `chrono` | Timestamps |
| `portable-pty` | Cross-platform PTY creation |
| `crossterm` | Local terminal display |
| `daemonize` | Unix daemonization |
| `dirs` | Standard directories |
| `sysinfo` | Process liveness checks |
| `rust-embed` | Embed admin assets |
| `dashmap` | Concurrent hash map |

---

## 7. Extension Points

### Adding a New Web Command

1. Write handler in `src/web/handlers/<domain>.rs`.
2. Add route in `src/web/router.rs`.
3. Document in README and requirements.

### Adding a New Handle Sink

1. Implement `Sink` trait.
2. Add variant to config enum.
3. Update factory in `src/handles/registry.rs`.

---

## 8. Testing Strategy

- **Unit tests**: VTTY parser, config merging, key encoding.
- **Integration tests**: Spawn command, send keys via API, assert VTTY contents.
- **Instance tests**: Start two instances on different ports, verify `vrunner list` shows both, verify `vrunner stop` shuts one down.
- **Platform tests**: CI matrix for Linux, macOS, Windows.


## Module Implementation Status

### Module 1: VTTY Core — COMPLETE ✅

**Files implemented:**
- `src/vtty/error.rs` — Error types for VTTY operations
- `src/vtty/cell.rs` — `Cell` struct with full attribute support (fg, bg, bold, italic, underline, blink, reverse, invisible, strikethrough)
- `src/vtty/color.rs` — 256-color palette, 6x6x6 color cube, grayscale ramp, RGB roundtrip
- `src/vtty/buffer.rs` — 2D cell grid with scrollback, insert/delete lines/cells, clear operations
- `src/vtty/parser.rs` — Streaming ANSI parser with state machine (CSI, OSC, DCS, simple escapes)
- `src/vtty/emulator.rs` — Full terminal emulator:
  - Cursor movement (CUP, CUU, CUD, CUF, CUB, CNL, CPL, CHA)
  - Erase operations (ED, EL)
  - Line operations (IL, DL, DCH, ICH)
  - Scroll (SU, SD)
  - SGR attributes (colors 16/256/truecolor, bold, italic, underline, blink, reverse, invisible, strikethrough)
  - DEC private modes (cursor visibility, alternate screen, auto-wrap, origin mode)
  - Save/restore cursor (DECSC/DECRC)
  - Scroll regions (DECSTBM)
  - Full reset (RIS)
  - Buffer read API: `contents_plain()`, `contents_ansi()`, `partial()`, `snapshot()`

**Tests:** 15 unit tests covering text rendering, colors, cursor movement, scrolling, save/restore, alternate screen, insert/delete, resize.

**Key design decisions:**
- Hand-written state machine parser rather than regex (handles streaming correctly)
- `RwLock<Buffer>` for concurrent read access from web API while emulator writes
- Clone-on-read for snapshot API
- Truecolor (24-bit RGB) as internal representation, with 256-color conversion for compatibility
