# vrc Documentation

A virtual terminal runner with two deployment modes: a local-only **vrc** binary using UDS IPC, and a remote-capable **vrw** binary with an HTTP server, web admin dashboard, WebSocket streaming, TLS, and REST API. Both share the same core library — VTTY emulator, process manager, configuration, interactive display, handles, hooks, and daemon — so everything you learn for one transfers to the other.

## Overview

vrc is a project that ships two binaries, each backed by the same `vrc_core` library:

### vrc (default feature `vrc`)

- **UDS IPC control plane** — communicate over a Unix domain socket.
- **Local-only** — designed for single-machine, single-user workflows.
- **No HTTP server** — minimal attack surface, ideal for headless servers and CI.
- Use when you need fast, lightweight terminal multiplexing on one host.

### vrw (feature `vrw`)

- **HTTP server on port 9090** — REST API to spawn, list, inspect, and kill commands.
- **Web admin dashboard** — full browser-based UI with terminal streaming, sidebar, panels, themes, search, and more.
- **WebSocket streaming** — real-time terminal output delivered to any connected client.
- **TLS support** — serve over HTTPS with configurable certificates for secure remote access.
- Use when you need remote access, multi-user collaboration, or a web-based terminal dashboard.

### Shared Core (`vrc_core`)

Both binaries share:

- VTTY emulator (xterm-compatible terminal state)
- Process manager (spawn, signal, retain, exit handling)
- Configuration system (file, CLI flags, environment variables)
- Interactive display (tabs, split-pane, search, mouse)
- Handles & event hooks (on spawn, exit, error, kill)
- Daemon mode

## Where to Start

| You want to... | Read this first |
|----------------|-----------------|
| Learn vrc from scratch | [Getting Started](tutorials/getting-started.md) |
| Run your first command in 5 minutes | [Getting Started, Lesson 1](tutorials/getting-started.md#lesson-1-your-first-command) |
| Access terminals from a browser | [Web Dashboard Guide](how-to-guides/web-dashboard.md) |
| Explore the REST API | [API Usage Guide](how-to-guides/api-usage.md) |
| Look up a config option | [Configuration](configuration.md) |
| Understand the architecture | [Architecture](explanation/architecture.md) |
| Troubleshoot a problem | [FAQ](faq.md) |

## Document Index

### Tutorials
| Document | Description |
|----------|-------------|
| [Getting Started](tutorials/getting-started.md) | Lessons: install, first command, config, display, CLI commands |

### How-To Guides
| Document | Description |
|----------|-------------|
| [Running Commands](how-to-guides/running-commands.md) | Spawn commands via CLI and UDS IPC |
| [Daemon Mode](how-to-guides/daemon-mode.md) | Run vrc as a background service |
| [Interactive Display](how-to-guides/interactive-display.md) | Local TUI with tabs, search, split-pane, mouse support |
| [Configuration Profiles](how-to-guides/configuration-profiles.md) | Named presets for dev, staging, production |
| [Snapshots and Diffs](how-to-guides/snapshots-diffs.md) | Capture and compare terminal state |
| [Environment Variables](how-to-guides/environment-variables.md) | Three-layer environment variable control |
| [Event Hooks](how-to-guides/hooks.md) | Run shell commands on spawn, exit, error, kill events |
| [Web Dashboard Guide](how-to-guides/web-dashboard.md) | Using the browser-based admin UI with vrw |
| [API Usage Guide](how-to-guides/api-usage.md) | REST API endpoints and examples for vrw |
| [TLS & Certificates](how-to-guides/certificates.md) | Configure HTTPS/TLS for remote access |
| [Remote Access with TLS](how-to-guides/remote-tls.md) | End-to-end guide for secure remote vrw |
| [Dev Server Workflow](how-to-guides/dev-server.md) | Use vrw as a persistent dev server |
| [Pair Programming](how-to-guides/pair-programming.md) | Share a terminal session with collaborators |
| [Multi-Service Orchestration](how-to-guides/multi-service.md) | Manage multiple services through the web dashboard |
| [CI Pipeline Integration](how-to-guides/ci-pipeline.md) | Use vrw in automated CI/CD pipelines |

### Reference
| Document | Description |
|----------|-------------|
| [Configuration](configuration.md) | All config fields, CLI flags, types, defaults, and precedence |
| [CLI Reference](reference/cli.md) | All flags, subcommands, and key notation |
| [Keybindings](reference/keybindings.md) | Default and customizable keyboard shortcuts |
| [API Reference](api.md) | Complete REST API specification for vrw |
| [WebSocket Protocol](websocket.md) | WebSocket message format and streaming protocol |
| [Web UI Guide](web-ui/introduction.md) | Web admin dashboard components and usage |
| [Usage](usage.md) | Practical guide to using vrc |

### Explanation
| Document | Description |
|----------|-------------|
| [Architecture](explanation/architecture.md) | System design, module breakdown, data flow, concurrency model |
| [Comparison](explanation/comparison.md) | Feature matrix: vrc vs tmux, screen, mprocs |
| [Security Model](explanation/security-model.md) | Authentication, TLS, and access control for vrw |
| [Lifecycle Policy](explanation/lifecycle-policy.md) | Start, retain, and exit behavior |

### Other
| Document | Description |
|----------|-------------|
| [FAQ](faq.md) | Frequently asked questions |
