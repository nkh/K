# vrunner

A virtual terminal runner and process orchestrator with a web-first control plane. Run commands in pseudo-terminals, monitor them through a built-in web dashboard, and control them via a REST API — all from a single statically-linked binary.

## Features

- **Web Admin Dashboard** — Built-in SPA at `/admin` with real-time VTTY streaming, keyboard/mouse input, command management, light/dark theme, keyboard-accessible context menus, and connection quality indicator
- **Interactive Display** — Full terminal UI with tab bar, search, copy/paste, split-pane, and scrollback navigation
- **REST API** — 30+ endpoints for spawning, killing, resizing, snapshotting, and inspecting commands
- **WebSocket Streaming** — Incremental diff protocol for low-bandwidth terminal output push
- **TLS & Auth** — Self-signed certificates, custom certs, bearer token auth, per-command certificate isolation
- **Daemon Mode** — Background execution with double-fork, detachable from the terminal
- **Multi-Instance** — Run multiple vrunner servers, discover and manage them from the CLI
- **Configuration** — YAML/TOML/JSON with 3-layer precedence, named profiles, environment variable control
- **Advanced VTTY** — Scrollback, search, mouse support, sixel images, alternate screen, 256/truecolor
- **Process Control** — Freeze/thaw (SIGSTOP/SIGCONT), graceful shutdown with timeouts, exit handlers, per-command retain/snapshot on exit, initial keystroke injection

## Quick Start

```bash
# Build from source
cargo build --release

# Run idle (waits for API/web commands)
vrunner

# Run a command with web dashboard
vrunner -- htop

# Run with local terminal display and snapshot output on exit
vrunner --display --snapshot-on-exit /tmp/htop-output.txt -- htop

# Send initial keystrokes and retain buffer after exit
vrunner --retain-on-exit --send-keys "ls<Enter>" -- bash

# Run as a background daemon
vrunner --daemon -- npm run dev

# Accept remote connections securely
vrunner --remote --tls -- my-server

# Spawn a command via API
curl -X POST http://localhost:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "htop", "args": []}'
```

Open `http://localhost:9090/admin` in your browser to access the dashboard.

## Installation

### From Source

```bash
git clone https://github.com/nkh/K.git
cd K
cargo build --release
# Binary at target/release/vrunner
```

### System-Wide Install

```bash
cargo install --path .
```

### Man Pages

```bash
cp man/vrunner.1 /usr/local/share/man/man1/
man vrunner
```

## Documentation

Documentation is organized using the [Diataxis framework](https://diataxis.fr/) into four quadrants. Start at the **[documentation index](docs/index.md)** to find what you need.

| Quadrant | Description | Start here |
|----------|-------------|------------|
| [Tutorials](docs/tutorials/getting-started.md) | Hands-on lessons for new users | Lesson 1: Your First Command |
| [How-To Guides](docs/how-to-guides/) | Task-oriented recipes for specific goals | Pick the task you want |
| [Reference](docs/configuration.md) | Authoritative specs: config, API, CLI, protocol | Look up a value |
| [Explanation](docs/explanation/architecture.md) | Concepts, architecture, design decisions | Understand why |

**Other resources:**

| Document | Description |
|----------|-------------|
| [FAQ](docs/faq.md) | 50+ frequently asked questions |
| [User Manual](MANUAL.md) | Comprehensive all-in-one reference |
| [docs/examples/](docs/examples/) | Complete example configuration files |
| [man/vrunner.1](man/vrunner.1) | Unix manpage (comprehensive CLI reference) |
| [man/vrunnerctrl.1](man/vrunnerctrl.1) | CLI controller and API reference manpage |

## Use Cases

- **Development server orchestration** — Run frontend, backend, and database services in separate VTTYs, monitor them from a single web dashboard
- **CI/CD pipeline monitoring** — Expose build logs through the web UI or API for real-time debugging of failed builds
- **Remote server administration** — Securely manage services on headless machines through a browser with TLS and auth
- **Pair programming** — Share a terminal session between developers via the web interface
- **Long-running background tasks** — Run tasks that outlive your SSH session without screen or tmux

## License

Dual-licensed under **GPL-3.0-or-later** or **Artistic-2.0** — see [LICENSE](LICENSE) for the full text.
