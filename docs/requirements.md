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

### 2.6 Multiple Instances

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-23 | Multiple `vrunner` instances must be able to run in parallel on the same machine, each bound to a different port. | Must |
| FR-24 | Each instance must register itself in a shared instance registry (e.g., pidfiles in `~/.local/share/vrunner/instances/`). | Must |
| FR-25 | The registry must store: PID, port, bind address, start time, daemon status, display status, and the running command. | Must |
| FR-26 | Stale pidfiles (process no longer alive) must be automatically cleaned up on `list`. | Should |

### 2.7 Instance Listing

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-27 | The CLI subcommand `vrunner list` must display all running `vrunner` instances. | Must |
| FR-28 | The output must include: PID, port, bind address, daemon flag, display flag, and the command being run (or "(idle)"). | Must |
| FR-29 | The listing must work regardless of whether the instances were started normally or as daemons. | Must |

### 2.8 Instance Shutdown

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-30 | The CLI subcommand `vrunner stop <PID>` must send a graceful shutdown request to the specified instance. | Must |
| FR-31 | The shutdown command must use the instance's PID to identify it (read from the registry). | Must |
| FR-32 | The shutdown mechanism should first attempt an HTTP POST to `/api/shutdown`; if that fails, the CLI may warn the user to use `kill <PID>`. | Should |

### 2.9 File Handle Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-33 | `vrunner` must be able to provide additional file descriptors (handles) to the running command beyond stdin/stdout/stderr. | Should |
| FR-34 | Additional handles may be routed to the VTTY or to dedicated log files managed by `vrunner`. | Should |

### 2.10 VTTY Configuration

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-35 | VTTY parameters must be read from a configuration file (YAML). | Must |
| FR-36 | Configurable VTTY properties: rows, columns, term type, scrollback, color support. | Must |
| FR-37 | Support both global (`~/.config/vrunner/config.yaml`) and local (`./vrunner.yaml`) config files. | Should |
| FR-38 | Local config overrides global config values. | Should |

### 2.11 Web Server & API

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-39 | `vrunner` must start an embedded HTTP server on a configurable address/port (default: `127.0.0.1:8080`). | Must |
| FR-40 | The server must accept both GET and POST requests. | Must |
| FR-41 | All JSON responses must include a standard envelope: `{ "status": "ok|error", "data": ..., "error": "..." }`. | Should |
| FR-42 | Mandatory endpoints: list commands, start command, send keys, kill command, get VTTY, get partial VTTY, shutdown instance. | Must |
| FR-43 | The architecture must allow new web commands to be added with minimal boilerplate. | Must |

### 2.12 Administrative Web Interface

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-44 | The `/admin` page must be served from embedded static assets. | Should |
| FR-45 | The admin page must support: listing commands, viewing VTTY, sending keystrokes, starting/stopping commands. | Must |

## 3. Non-Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| NFR-01 | Written in Rust (edition 2021). | Must |
| NFR-02 | Cross-platform: Linux, macOS, Windows. | Must |
| NFR-03 | Binary size should be reasonable (< 10 MB stripped). | Should |
| NFR-04 | Web server must be asynchronous (`tokio` + `axum`). | Must |
| NFR-05 | VTTY emulation must support ANSI/VT100 and 256-color mode. | Must |
| NFR-06 | Comprehensive error messages in API responses and CLI output. | Should |

## 4. Out of Scope

- Authentication / authorization.
- TLS/HTTPS termination (use a reverse proxy).
- Persistent sessions across `vrunner` restarts.
- Clustering or multi-node process distribution.

## 5. Glossary

- **VTTY**: Virtual TTY. A pseudo-terminal pair managed by `vrunner`.
- **Handle**: An additional file descriptor passed to a child process.
- **Command ID**: A unique identifier assigned to each running command instance.
- **Instance**: A single running `vrunner` process with its own web server and registry entry.
