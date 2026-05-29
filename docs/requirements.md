# vrunner Requirements Document

## 1. Overview

**vrunner** is a Rust-based virtual terminal runner and process orchestrator. It executes commands inside virtual TTYs (VTTYs), exposes their output via a web interface, and allows remote control through HTTP APIs. By default, it produces no local terminal output, operating silently until explicitly configured otherwise.

## 2. Functional Requirements

### 2.1 Core Execution

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-01 | The binary must be named `vrunner`. | Must |
| FR-02 | Accept a command name and its arguments as CLI arguments and execute it. | Must |
| FR-03 | If no command is provided at startup, `vrunner` must idle and wait for commands exclusively through the web interface. | Must |
| FR-04 | Each executed command must be provided with a virtual TTY (VTTY) for terminal-aware programs. | Must |
| FR-05 | The VTTY must forward stdout and stderr from the child process. | Must |
| FR-06 | The VTTY must accept stdin (keyboard input) forwarded from the web API or local terminal. | Must |
| FR-07 | **By default, `vrunner` must not write anything to the local terminal screen.** All output is internal or served via the web API. | Must |
| FR-08 | The default silent behavior may be overridden via the configuration file or CLI flags. | Must |

### 2.2 CLI Argument Separation

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-09 | The CLI must use `--` as a separator between `vrunner` options and the command to run with its own options. | Must |
| FR-10 | Example: `vrunner --port 9090 --display -- python -m http.server 8000`. Here `--port` and `--display` belong to `vrunner`; `python -m http.server 8000` is the child command. | Must |

### 2.3 Local VTTY Display

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-11 | A CLI flag `--display` must enable real-time mirroring of the active VTTY onto the local terminal screen. | Must |
| FR-12 | A CLI flag `--no-display` must explicitly disable local display, even if enabled in the config file. | Must |
| FR-13 | The display refresh rate must be configurable (default: 100ms). | Should |
| FR-14 | The display must render ANSI colors using the local terminal's capabilities. | Should |

### 2.4 Command Logging

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-15 | A CLI flag `--log` must enable logging of all received web API commands to the local terminal (stdout). | Must |
| FR-16 | A CLI flag `--log-file <FILE>` must write command logs to the specified file instead of (or in addition to) the screen. | Must |
| FR-17 | Each log entry must include a timestamp, the command name, and relevant parameters. | Should |
| FR-18 | Logging must be configurable via the config file under a `command_log` section. | Should |

### 2.5 Daemon Mode

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-19 | A CLI flag `--daemon` must detach `vrunner` from the controlling terminal and run it as a background process. | Must |
| FR-20 | In daemon mode, `vrunner` must have no local display, no stdin interaction, and no terminal attachment. | Must |
| FR-21 | Daemon mode must redirect stdout and stderr to configurable log files (default: `/tmp/vrunner.out` and `/tmp/vrunner.err`). | Should |
| FR-22 | Daemon mode is only required on Unix-like systems; on Windows it may print an error. | Should |
| FR-23 | Daemonization must occur before the tokio runtime starts to avoid conflicts with async signal handling. | Should |

### 2.6 Multiple Instances

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-24 | Multiple `vrunner` instances must be able to run in parallel on the same machine, each bound to a different port. | Must |
| FR-25 | Each instance must register itself in a shared instance registry (e.g., pidfiles in `~/.local/share/vrunner/instances/`). | Must |
| FR-26 | The registry must store: PID, port, bind address, start time, daemon status, display status, and the running command. | Must |
| FR-27 | Stale pidfiles (process no longer alive) must be automatically cleaned up on `list`. | Should |

### 2.7 Instance Listing

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-28 | The CLI subcommand `vrunner list` must display all running `vrunner` instances. | Must |
| FR-29 | The output must include: PID, port, bind address, daemon flag, display flag, and the command being run (or "(idle)"). | Must |
| FR-30 | The listing must work regardless of whether the instances were started normally or as daemons. | Must |

### 2.8 Instance Shutdown

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-31 | The CLI subcommand `vrunner stop <PID>` must send a graceful shutdown request to the specified instance. | Must |
| FR-32 | The shutdown command must use the instance's PID to identify it (read from the registry). | Must |
| FR-33 | The shutdown mechanism should first attempt an HTTP POST to `/api/shutdown`; if that fails, the CLI may warn the user to use `kill <PID>`. | Should |

### 2.9 File Handle Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-34 | `vrunner` must be able to provide additional file descriptors (handles) to the running command beyond stdin/stdout/stderr. | Should |
| FR-35 | Additional handles may be routed to the VTTY or to dedicated log files managed by `vrunner`. | Should |

### 2.10 VTTY Configuration

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-36 | VTTY parameters must be read from a configuration file (YAML). | Must |
| FR-37 | Configurable VTTY properties: rows, columns, term type, scrollback, color support, mouse forwarding. | Must |
| FR-38 | Support both global (`~/.config/vrunner/config.yaml`) and local (`./vrunner.yaml`) config files. | Should |
| FR-39 | Local config overrides global config values. | Should |
| FR-40 | Every configuration entry must have a corresponding CLI flag for override. | Must |

### 2.11 Web Server & API

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-41 | `vrunner` must start an embedded HTTP server on a configurable address/port (default: `127.0.0.1:8080`). | Must |
| FR-42 | The server must accept both GET and POST requests. | Must |
| FR-43 | All JSON responses must include a standard envelope: `{ "status": "ok|error", "data": ..., "error": "..." }`. | Should |
| FR-44 | Mandatory endpoints: list commands, start command, send keys, kill command, get VTTY, get partial VTTY, shutdown instance. | Must |
| FR-45 | The architecture must allow new web commands to be added with minimal boilerplate. | Must |

### 2.12 Administrative Web Interface

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-46 | The `/admin` page must be served from embedded static assets. | Should |
| FR-47 | The admin page must support: listing commands, viewing VTTY, sending keystrokes, starting/stopping commands. | Must |

### 2.13 Authentication

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-48 | By default (localhost binding), no authentication must be required. | Must |
| FR-49 | When bound to a non-localhost address, authentication must be available via a CLI flag (`--auth`) or a convenience flag (`--remote`). | Must |
| FR-50 | Authentication must use bearer tokens in the `Authorization` header. | Must |
| FR-51 | When auth is enabled and no token file exists, a cryptographically random 256-bit token must be generated and saved. | Must |
| FR-52 | The token file must have restrictive permissions (`0600` on Unix). | Should |

### 2.14 TLS Encryption

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-53 | TLS (HTTPS) must be available via a CLI flag (`--tls`). | Must |
| FR-54 | When TLS is enabled and no certificates exist, self-signed certificates must be automatically generated. | Must |
| FR-55 | Self-signed certificates must include SAN entries for `localhost`, `127.0.0.1`, and `::1`. | Must |
| FR-56 | The private key file must have restrictive permissions (`0600` on Unix). | Must |
| FR-57 | Custom certificate and key paths must be supported via CLI flags (`--cert-file`, `--key-file`). | Should |
| FR-58 | TLS must use `rustls` (no OpenSSL dependency). | Should |

### 2.15 Certificate Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-59 | vrunner must support a pool of named certificates for per-command access control. | Must |
| FR-60 | Certificates can be generated via CLI (`vrunner cert generate <name>`) or configured in the config file. | Must |
| FR-61 | Each certificate in the pool must have a derived bearer token (SHA-256 of the certificate PEM). | Must |
| FR-62 | When starting a command, a certificate name can be specified to bind the command to that certificate. | Must |
| FR-63 | Only clients presenting the bound certificate's derived token can interact with the command's endpoints. | Must |
| FR-64 | Unbound commands are accessible to any authenticated client (or unauthenticated on localhost). | Must |
| FR-65 | Different vrunner instances can have completely different certificate pools. | Must |
| FR-66 | The certificate pool must be configurable via YAML config file. | Should |

### 2.16 WebSocket Incremental Diff Protocol

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-67 | The VTTY WebSocket must use an incremental diff protocol that transmits only changed cells instead of the full buffer on every update. | Must |
| FR-68 | Upon WebSocket connection, the server must send an initial full snapshot (`vtty_full`) with the complete terminal HTML, cursor position, and dimensions. | Must |
| FR-69 | Subsequent updates must be sent as `vtty_diff` messages containing only the cells that changed (character, colors, and text attributes). | Must |
| FR-70 | If the client falls behind (broadcast lag), the server must automatically send a new `vtty_full` message to resynchronize. | Should |
| FR-71 | The diff computation must compare cell-by-cell including character value, foreground/background RGB colors, and text attributes (bold, italic, underline, etc.). | Must |

### 2.17 Snapshot and Diff API

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-72 | vrunner must support storing named snapshots of a command's VTTY buffer via `POST /api/commands/{id}/snapshot`. | Must |
| FR-73 | Each snapshot must include metadata: name, command name, command arguments, PID, timestamp, and wall-clock runtime. | Must |
| FR-74 | vrunner must support listing all snapshots for a command via `GET /api/commands/{id}/snapshots`. | Must |
| FR-75 | vrunner must support computing a cell-level diff between the current buffer and a stored snapshot via `POST /api/commands/{id}/diff`. | Must |
| FR-76 | vrunner must support deleting snapshots via `DELETE /api/commands/{id}/snapshots/:name`. | Must |
| FR-77 | All snapshots for a command must be automatically cleaned up when the command is killed. | Should |

### 2.18 Kill by PID

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-78 | vrunner must support killing individual commands by their OS process ID via `POST /api/commands/kill-pid/:pid`. | Must |
| FR-79 | The `vrunner stop <pid>` CLI command must first attempt to kill a command with that PID on any running instance before falling back to instance shutdown. | Should |

### 2.19 Enhanced Instance Listing

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-80 | `vrunner list` must contact each running instance via HTTP to retrieve its active commands. | Should |
| FR-81 | The list output must include command name, arguments, PID, and certificate binding for each command on each instance. | Should |
| FR-82 | Unreachable instances must be clearly indicated in the list output. | Should |

### 2.20 Admin Interface Enhancements

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-83 | The admin interface must include a Pause/Run button to freeze and thaw the currently selected command. | Must |
| FR-84 | The admin interface must use 1-second HTTP polling as a fallback when WebSocket is not available. | Should |
| FR-85 | The admin interface must auto-select the first available command when no command is selected. | Should |
| FR-86 | The admin interface topbar layout must be responsive and not overflow on narrow screens. | Should |

## 3. Non-Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| NFR-01 | Written in Rust (edition 2021). | Must |
| NFR-02 | Cross-platform: Linux, macOS, Windows. | Must |
| NFR-03 | Binary size should be reasonable (< 10 MB stripped). | Should |
| NFR-04 | Web server must be asynchronous (`tokio` + `axum`). | Must |
| NFR-05 | VTTY emulation must support ANSI/VT100 and 256-color mode. | Must |
| NFR-06 | Comprehensive error messages in API responses and CLI output. | Should |
| NFR-07 | No global mutable state — all shared state must be injected via function parameters or Axum state. | Should |
| NFR-08 | Zero compiler warnings (`cargo check` and `cargo clippy`). | Should |

## 4. Out of Scope

- Persistent sessions across `vrunner` restarts.
- Clustering or multi-node process distribution.
- Public CA-signed certificates in the certificate pool (use `--cert-file`/`--key-file` for the instance TLS cert, or a reverse proxy for public CA certs).
- Client certificate authentication (bearer token auth is used instead).

## 5. Glossary

- **VTTY**: Virtual TTY. A pseudo-terminal pair managed by `vrunner`.
- **Handle**: An additional file descriptor passed to a child process.
- **Command ID**: A unique identifier (UUID) assigned to each running command instance.
- **Instance**: A single running `vrunner` process with its own web server and registry entry.
- **Bearer Token**: A secret string used in the `Authorization` header to authenticate API requests.
- **Self-Signed Certificate**: An X.509 certificate signed by vrunner itself (not by a public CA), distributed to authorized clients out of band.
- **Incremental Diff Protocol**: A bandwidth optimization for the VTTY WebSocket that transmits only changed cells rather than the full terminal buffer on each update.
- **Snapshot**: A named, point-in-time capture of a command's VTTY buffer contents, stored in memory with metadata for later comparison via diffs.
