# vrunner Architecture Overview

This document describes the high-level architecture of **vrunner**, a Rust-based virtual terminal runner with a web control plane, TLS encryption, optional authentication, daemon mode, and instance registry.

---

## 1. Design Principles

1. **Silent by Default** — The local terminal is not a log sink unless explicitly requested.
2. **Secure by Default** — Binds to localhost with no auth; remote access requires explicit opt-in.
3. **Separation of Concerns** — CLI parsing, config loading, process management, terminal emulation, web serving, security, and instance tracking are distinct modules.
4. **Extensibility** — New web API commands, handle sinks, and VTTY backends can be added without modifying core logic.
5. **Async-First** — Built on `tokio` to handle concurrent connections, processes, and I/O loops.
6. **Multi-Instance Awareness** — The tool manages not only commands but also peer `vrunner` processes on the same machine.

---

## 2. System Context

```
┌─────────────────────────────────────────────────────────────────────┐
│                         vrunner binary                               │
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
│              │    AppState            │                            │
│              │  (Axum shared state)   │                            │
│              └───────────┬────────────┘                            │
│                          │                                         │
│         ┌────────────────┼────────────────┐                       │
│         │                │                │                       │
│  ┌──────▼──────┐  ┌─────▼─────┐  ┌──────▼──────┐                │
│  │   Auth      │  │   TLS     │  │  Middleware  │                │
│  │  Middleware │  │  (HTTPS)  │  │ (CORS, Log) │                │
│  └─────────────┘  └───────────┘  └─────────────┘                │
│                          │                                         │
│              ┌───────────▼────────────┐                            │
│              │      Web Server        │                            │
│              │   (axum / tokio)       │                            │
│              └───────────┬────────────┘                            │
│                          │                                         │
│              ┌───────────▼────────────┐                            │
│              │   HTTP(S) Clients      │                            │
│              │  (Browser / curl /     │                            │
│              │   other vrunner CLIs)  │                            │
│              └────────────────────────┘                            │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Module Breakdown

### 3.1 CLI Entry Point (`src/main.rs` + `src/cli/`)

`main.rs` is a thin binary wrapper that parses CLI arguments, loads configuration, optionally daemonizes, and delegates to the library crate. `src/lib.rs` is the single source of truth for all `mod` declarations.

Uses `clap` with derive macros. Supports:
- Options before `--` (vrunner flags)
- Command + args after `--` (child process)
- Subcommands: `list`, `stop <PID>`

**`src/cli/args.rs`** defines:
- `Cli` struct with `#[command(trailing_var_arg = true)]`
- `Commands` enum for `List` and `Stop`
- `Cli::apply_overrides()` method that applies CLI flags over loaded configuration
- Complete CLI coverage for all config entries (see [docs/configuration.md](configuration.md))

### 3.2 Configuration Layer (`src/config/`)

| Component | Responsibility |
|-----------|-------------|
| `loader.rs` | Discovers global (`~/.config/vrunner/config.yaml`) and local (`./vrunner.yaml`) YAML configs, plus any CLI-specified path. |
| `schema.rs` | Typed structs with serde: `Config`, `ServerConfig`, `SecurityConfig`, `TlsConfig`, `VttyConfig`, `DisplayConfig`, `CommandLogConfig`, `DaemonConfig`, `HandleConfig`. |
| `merge.rs` | Override logic: local config overrides global config. CLI flags applied on top. |

### 3.3 Application State (`src/web/state.rs`)

The `AppState` struct holds all shared state for the web server, passed through Axum's `State<AppState>` extractor:

```rust
pub struct AppState {
    pub manager: Arc<CommandManager>,
    pub shutdown_tx: broadcast::Sender<()>,
    pub auth_token: Option<String>,  // None = no auth required
}
```

This replaces the previous global mutable static pattern, providing clean dependency injection and thread-safe access.

### 3.4 Security (`src/web/auth.rs`, `src/web/middleware.rs`)

| Component | Responsibility |
|-----------|-------------|
| `auth.rs` | `AuthManager`: loads or generates a 256-bit random bearer token. Token file is created with `0600` permissions. |
| `middleware.rs` | `auth_middleware`: validates `Authorization: Bearer <token>` header when auth is enabled. When `auth_token` is `None`, all requests pass through. |

### 3.5 TLS (`src/web/tls.rs`)

| Component | Responsibility |
|-----------|-------------|
| `tls.rs` | `TlsManager`: loads existing PEM certificates or generates self-signed certificates via `rcgen`. Certificates include SAN entries for `localhost`, `127.0.0.1`, and `::1`. Private key files are created with `0600` permissions. |

### 3.6 Instance Registry (`src/instance/`)

Manages a directory of JSON pidfiles (`~/.local/share/vrunner/instances/<PID>.json`).

| Component | Responsibility |
|-----------|-------------|
| `registry.rs` | `InstanceRegistry`: register, unregister, list, stop. Validates liveness via `sysinfo`. Auto-cleans stale pidfiles. |
| `info.rs` | `InstanceInfo`: serializable metadata (PID, port, bind, start time, daemon/display flags, command). |

### 3.7 Daemon Mode (`src/daemon/`)

| Component | Responsibility |
|-----------|-------------|
| `mod.rs` | Platform dispatch. |
| `unix.rs` | Custom double-fork daemonization using raw `libc` calls (`fork`, `setsid`, fd redirection). Called before tokio runtime to avoid conflicts with async signal handling. |

### 3.8 Command Logger (`src/logging/`)

| Component | Responsibility |
|-----------|-------------|
| `command_log.rs` | `CommandLogger`: thread-safe logger that writes to screen, file, or both. Used by the web layer to audit API calls. |

### 3.9 Process Management (`src/process/`)

| Component | Responsibility |
|-----------|-------------|
| `manager.rs` | `CommandManager`: owns `DashMap<CommandId, CommandHandle>`. Spawns, lists, kills. Injects `CommandLogger`. |
| `spawner.rs` | Platform-specific PTY creation via `portable-pty`. Uses `mpsc::channel` bridge between synchronous PTY reads and async VTTY writes — a blocking thread reads from the PTY and sends chunks through the channel, while a single async receiver task feeds them to the VTTY emulator. |
| `handle.rs` | `CommandHandle`: per-command state (ID, PID, name, VTTY reference). |

### 3.10 VTTY Emulator (`src/vtty/`)

| Component | Responsibility |
|-----------|-------------|
| `emulator.rs` | Terminal state machine supporting cursor movement, erase operations, scroll, SGR attributes (16/256/truecolor), DEC private modes, alternate screen, save/restore cursor, scroll regions. |
| `parser.rs` | Streaming ANSI parser with state machine (CSI, OSC, DCS, simple escapes). |
| `buffer.rs` | 2D cell grid with scrollback, insert/delete lines/cells, clear operations. |
| `renderer.rs` | Serialize buffer to ANSI, HTML, or plain text. |
| `display.rs` | `TerminalDisplay`: renders the buffer to the local terminal using `crossterm` (only when `--display` is active). |

### 3.11 Handle System (`src/handles/`)

Extensible file descriptor routing.

| Component | Responsibility |
|-----------|-------------|
| `registry.rs` | `HandleRegistry`: map of name to sink. |
| `sink.rs` | `Sink` trait (async). |
| `file_sink.rs`, `vtty_sink.rs`, `null_sink.rs` | Implementations. |

### 3.12 Web Server (`src/web/`)

| Component | Responsibility |
|-----------|-------------|
| `server.rs` | Binds TCP socket, starts `axum_server`. Supports both HTTP and TLS (HTTPS) modes. Manages graceful shutdown via signal handlers. |
| `router.rs` | Route table with `AppState` injection. All handlers receive `State<AppState>`. |
| `handlers/` | One module per endpoint group: `commands.rs`, `keys.rs`, `vtty.rs`, `admin.rs`, `handles.rs`. |
| `middleware.rs` | CORS, authentication, request logging, JSON error envelopes. |
| `certs.rs` | `CertificateStore`: manages a pool of named certificates for per-command access control. Generates self-signed certs via `rcgen`, derives bearer tokens from certificate content via SHA-256. |
| `static_assets.rs` | Embedded admin SPA via `rust-embed`. |

#### Route Table

```rust
pub fn create_router(state: AppState) -> Router {
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
        .with_state(state)
}
```

### 3.13 Admin Interface (`static/admin/`)

Lightweight SPA served from embedded assets. Communicates with the REST API.

### 3.14 Certificate System (`src/web/certs.rs`)

The certificate system provides a pool of named certificates that can be bound to individual commands for per-command access control.

| Component | Description |
|-----------|-------------|
| `CertificateStore` | Thread-safe pool (`DashMap<String, CertificateEntry>`) holding named certificate/key pairs. Shared via `AppState`. Initialized from config entries and CLI `--certificate` flags. |
| `CertificateEntry` | A named cert/key pair with a derived bearer token. The token is computed as `SHA-256(PEM certificate)`, hex-encoded. This allows clients to authenticate using the token derived from the certificate content. |
| `CommandHandle.certificate` | Optional field on each running command. When set, only API requests bearing the matching derived token can interact with that command's endpoints. Unbound commands follow the normal auth rules. |
| CLI subcommands | `cert generate <name>` creates a self-signed cert via `rcgen` and adds it to the pool. `cert list`, `cert show <name>`, and `cert remove <name>` manage the pool. |

**Per-command access control flow:**

```
API Request → Auth Middleware
                  │
         ┌────────┴────────┐
         │                 │
   Command is cert-bound   Command is unbound
         │                 │
   Check cert-derived     Follow normal auth rules
   token in header        (bearer token or no auth)
         │                 │
   ┌─────┴─────┐          │
   │           │          │
  Match     No match      │
   │           │          │
  Allow    403 Forbidden  │
```

---

## 4. Data Flow

### 4.1 Starting an Instance

```
CLI: vrunner --port 9090 --tls -- htop
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
│ TLS Manager  │──► Load or generate self-signed certs (if --tls)
└──────────────┘
       │
       ▼
┌──────────────┐
│ Certificate  │──► Initialize cert pool from config + CLI --certificate flags
│ Store        │──► Derive bearer tokens (SHA-256 of cert PEM) for each entry
└──────────────┘
       │
       ▼
┌──────────────┐
│ Auth Manager │──► Load or generate bearer token (if --auth)
└──────────────┘
       │
       ▼
┌──────────────┐
│ Web Server   │──► Start axum_server on 127.0.0.1:9090 (HTTP or HTTPS)
│              │──► Apply auth + CORS + logging middleware
└──────────────┘
```

### 4.2 Listing Instances

```
CLI: vrunner list
       │
       ▼
┌──────────────┐
│ Instance     │──► Read all pidfiles from ~/.local/share/vrunner/instances/
│ Registry     │──► Filter out stale entries (dead PIDs via sysinfo)
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
│ HTTP Client  │──► POST http(s)://<bind>:<port>/api/shutdown
└──────────────┘
```

---

## 5. Concurrency Model

- **Tokio Runtime**: Multi-threaded scheduler, started after daemonization (if applicable).
- **AppState**: Shared via `Arc` and Axum's `State` extractor — no global mutable state.
- **Command Manager**: `Arc<DashMap<CommandId, CommandHandle>>` for lock-free reads.
- **Per-Command Tasks**: `pty_reader` (blocking thread → mpsc channel), `stdin_writer`, `handle_writers`, `process_waiter`.
- **Sync/Async Bridge**: PTY reads happen on a blocking thread (`portable-pty` provides synchronous `Read`/`Write`). Data is sent through a bounded `tokio::sync::mpsc::channel(64)` to a single async receiver task that feeds the VTTY emulator.
- **Web Handlers**: Stateless, borrow `State<AppState>` from Axum.
- **Command Logger**: `Arc<CommandLogger>` shared between web handlers and process manager.
- **Shutdown**: `broadcast::Sender<()>` distributed via `AppState`. Signal handler sends on the channel; server listens and triggers graceful shutdown.

---

## 6. Security Architecture

### Authentication Flow

```
Request → CORS middleware → Auth middleware → Handler
                                 │
                    ┌────────────┴────────────┐
                    │                         │
              auth_token is None        auth_token is Some
              (localhost mode)         (remote mode)
                    │                         │
              Pass through          Check Authorization header
                                       │
                              ┌────────┴────────┐
                              │                 │
                           Valid token     Missing/invalid
                              │                 │
                          Pass through    401 Unauthorized
```

### TLS Setup

```
--tls flag → TlsManager::load_or_generate_config()
                    │
            ┌───────┴────────┐
            │                │
      Certs exist      Certs missing
            │                │
      Load PEM files   Generate via rcgen
            │          (CN=vrunner, SAN=localhost)
            │                │
            │          Save to ~/.config/vrunner/
            │          (cert.pem + key.pem @ 0600)
            │                │
            └───────┬────────┘
                    │
            Build rustls::ServerConfig
            (no client auth — bearer token handles auth)
                    │
            axum_server::bind_rustls()
```

---

## 7. Crate Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime |
| `axum` | HTTP server framework |
| `axum-server` | HTTP/TLS server with graceful shutdown |
| `reqwest` | HTTP client (for `vrunner stop`) |
| `rustls` / `rustls-pemfile` | TLS (no OpenSSL dependency) |
| `rcgen` | Self-signed certificate generation |
| `rand` | Cryptographically random token generation |
| `serde` / `serde_json` | Serialization |
| `clap` | CLI parsing |
| `config` | Hierarchical config loading |
| `anyhow` | Error handling |
| `tracing` | Structured logging |
| `uuid` | Command ID generation |
| `chrono` | Timestamps |
| `portable-pty` | Cross-platform PTY creation |
| `parking_lot` | `RwLock` for VTTY buffer |
| `crossterm` | Local terminal display |
| `libc` | Raw Unix syscalls for daemonization |
| `dirs` | Standard directories |
| `sysinfo` | Process liveness checks |
| `rust-embed` | Embed admin assets |
| `dashmap` | Concurrent hash map |
| `tower-http` | CORS middleware |
| `sha2` | SHA-256 hashing for certificate token derivation |
| `hex` | Hex encoding for certificate tokens |

---

## 8. Extension Points

### Adding a New Web Command

1. Write handler in `src/web/handlers/<domain>.rs`.
2. Add route in `src/web/router.rs` (the handler receives `State<AppState>`).
3. Document in README and configuration reference.

### Adding a New Handle Sink

1. Implement the `Sink` trait in `src/handles/`.
2. Add the sink type name to the factory logic.
3. Document in the configuration reference.

---

## 9. Testing Strategy

- **Unit tests**: VTTY parser, buffer operations, config merging, key encoding.
- **Integration tests**: Spawn command, send keys via API, assert VTTY contents.
- **Instance tests**: Start two instances on different ports, verify `vrunner list` shows both, verify `vrunner stop` shuts one down.
- **Platform tests**: CI matrix for Linux, macOS, Windows.
- **Security tests**: Verify auth middleware blocks unauthenticated requests when enabled.
