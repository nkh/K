# vrl / vrunner

A virtual terminal runner and process orchestrator. Run commands in pseudo-terminals, monitor them through a local terminal display or a web dashboard, and control them via Unix Domain Socket or HTTP API — from a single statically-linked binary with under 5ms startup.

This repository contains two binaries built from a shared codebase, selected at compile time via Cargo features:

| Binary | Feature Flag | IPC Mechanism | Target Use Case |
|--------|-------------|---------------|-----------------|
| **vrl** | `vrl` (default) | Unix Domain Sockets | Lightweight local CLI tooling, scripts, pipelines |
| **vrunner** | `vrunner` | HTTP + WebSocket | Remote access, web dashboard, multi-user scenarios |

Both binaries share the same VTTY emulator, process manager, configuration system, interactive display, and handle infrastructure. They differ only in how clients communicate with running instances: **vrl** uses UDS IPC for local-only communication, while **vrunner** exposes a full HTTP API with an embedded web admin UI.

## Features

### Shared (both binaries)

- **Interactive Display** — Full terminal UI with tab bar, search, copy/paste, split-pane, and scrollback navigation
- **Daemon Mode** — Background execution with double-fork, detachable from the terminal
- **Multi-Instance** — Run multiple instances, discover and manage them from the CLI
- **Configuration** — YAML/TOML/JSON with 3-layer precedence, named profiles, environment variable control
- **Advanced VTTY** — Scrollback, search, mouse support, alternate screen, 256/truecolor
- **Process Control** — Freeze/thaw (SIGSTOP/SIGCONT), graceful shutdown with timeouts, exit handlers, per-command retain/snapshot on exit, initial keystroke injection

### vrl-specific

- **UDS IPC** — All inter-instance communication uses Unix Domain Sockets with length-prefixed JSON protocol. No HTTP server, no network binding, no TLS overhead. Sockets use `0600` permissions for filesystem-based security.

### vrunner-specific

- **HTTP API** — RESTful API for spawning commands, sending keystrokes, reading VTTY output, managing certificates, and streaming logs
- **Web Admin Dashboard** — Embedded single-page application served at `/admin` with real-time VTTY viewer, command management, theme switching, search, and keyboard shortcuts
- **WebSocket Streaming** — Incremental diff protocol for efficient real-time terminal updates and log streaming
- **TLS / Remote Access** — Optional TLS encryption with auto-generated self-signed certificates, bearer token authentication, and CORS configuration
- **Certificate-Based Access Control** — Per-command certificate pool for fine-grained client authorization
- **Screenshot Rendering** — Server-side PNG rendering of terminal output via the API

## Quick Start

### Building vrl (default)

```bash
git clone https://github.com/nkh/K.git
cd K
cargo build --release
# Binary at target/release/vrl
```

### Building vrunner

```bash
git clone https://github.com/nkh/K.git
cd K
cargo build --release --features vrunner
# Binary at target/release/vrunner
```

### Building both

```bash
cargo build --release --features "vrl,vrunner"
# Both binaries at target/release/vrl and target/release/vrunner
```

### Using vrl

```bash
# Run a command
vrl -- htop

# Run with local terminal display
vrl --display -- htop

# Send initial keystrokes and retain buffer after exit
vrl --retain-on-exit --send-keys "ls<Enter>" -- bash

# Run as a background daemon
vrl --daemon -- npm run dev

# List running instances
vrl list

# Stop an instance
vrl stop <pid>
```

### Using vrunner

```bash
# Start the server (HTTP on port 9090)
vrunner

# Start with a command at launch
vrunner -- htop

# Open the web dashboard
# Navigate to http://127.0.0.1:9090/admin

# Spawn a command via CLI
vrunner spawn htop

# Spawn via API
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "htop", "args": []}'

# Enable TLS and remote access
vrunner --remote --tls -- my-command
```

## Installation

### From Source (vrl)

```bash
git clone https://github.com/nkh/K.git
cd K
cargo build --release --features vrl
# Binary at target/release/vrl
```

### From Source (vrunner)

```bash
git clone https://github.com/nkh/K.git
cd K
cargo build --release --features vrunner
# Binary at target/release/vrunner
```

### System-Wide Install (vrl)

```bash
cargo install --path . --features vrl
```

### System-Wide Install (vrunner)

```bash
cargo install --path . --features vrunner
```

### Man Pages

```bash
# vrl
cp man/vrl.1 /usr/local/share/man/man1/
man vrl

# vrunner
cp man/vrunner.1 /usr/local/share/man/man1/
man vrunner
```

## Documentation

Documentation is organized using the [Diataxis framework](https://diataxis.fr/) into four quadrants. Start at the **[documentation index](docs/index.md)** to find what you need.

| Quadrant | Description | Start here |
|----------|-------------|------------|
| [Tutorials](docs/tutorials/getting-started.md) | Hands-on lessons for new users | Lesson 1: Your First Command |
| [How-To Guides](docs/how-to-guides/) | Task-oriented recipes for specific goals | Pick the task you want |
| [Reference](docs/configuration.md) | Authoritative specs: config, CLI, protocol | Look up a value |
| [Explanation](docs/explanation/architecture.md) | Concepts, architecture, design decisions | Understand why |

**Other resources:**

| Document | Description |
|----------|-------------|
| [FAQ](docs/faq.md) | Frequently asked questions |
| [User Manual](MANUAL.md) | Comprehensive all-in-one reference |
| [docs/examples/](docs/examples/) | Complete example configuration files |
| [man/vrl.1](man/vrl.1) | vrl Unix manpage |
| [man/vrunner.1](man/vrunner.1) | vrunner Unix manpage |

## Architecture

Both binaries are built from the `vrl_core` library crate. The `vrl` feature is the default and compiles only the UDS IPC path. The `vrunner` feature additionally pulls in the HTTP stack (Axium, reqwest, rustls, rust-embed) and the embedded web admin UI.

Shared modules (available to both binaries): `cli/`, `config/`, `daemon/`, `handles/`, `hooks/`, `instance/`, `interactive/`, `ipc/`, `logging/`, `process/`, `vtty/`.

vrl-specific: `ipc/` (UDS server/client).

vrunner-specific: `web/` (HTTP server, REST handlers, WebSocket, TLS, auth, static assets).

For the full architecture overview, see [docs/explanation/architecture.md](docs/explanation/architecture.md).

## License

Dual-licensed under **GPL-3.0-or-later** or **Artistic-2.0** — see [LICENSE](LICENSE) for the full text.
