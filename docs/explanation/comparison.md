# Comparison with Alternatives

This document compares vrunner against the most common tools that overlap with its
capabilities: multiplexers (tmux, screen), process monitors (mprocs), and web-based
terminal sharing tools (gotty, wetty). It presents a feature-by-feature matrix,
highlights architectural differences, and offers guidance on when vrunner—or one of
its competitors—is the better choice. Read this if you are evaluating whether vrunner
fits your use case or if you are migrating from an existing tool.

---

## Feature Comparison Matrix

The table below covers 35 features across six tools. A **✓** means the feature is
fully supported; **~** means partial or limited support; **✗** means not supported;
and **—** means not applicable.

| # | Feature | vrunner | tmux | screen | mprocs | gotty | wetty |
|---|---|---:|---:|---:|---:|---:|---:|
| 1 | Web-based terminal UI | ✓ | ✗ | ✗ | ✗ | ✓ | ✓ |
| 2 | Native terminal UI (in-shell) | — | ✓ | ✓ | ✓ | — | — |
| 3 | HTTP REST API for management | ✓ | ✗ | ✗ | ✗ | ~ | ~ |
| 4 | WebSocket streaming | ✓ | ✗ | ✗ | ✗ | ✓ | ✓ |
| 5 | Multi-instance management | ✓ | ✓ | ✓ | ✓ | ✗ | ✗ |
| 6 | Named instances | ✓ | ✓ (sessions) | ✓ (sessions) | ✗ | ✗ | ✗ |
| 7 | Daemon/background mode | ✓ | ✓ | ✓ | ✗ | ~ | ~ |
| 8 | Auto-generate TLS certificates | ✓ | ✗ | ✗ | ✗ | ~ | ✗ |
| 9 | Custom TLS certificates | ✓ | ✗ | ✗ | ✗ | ✓ | ✓ |
| 10 | Token-based authentication | ✓ | ✗ | ✗ | ✗ | ✓ | ~ |
| 11 | Per-command auth (cert pool) | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 12 | CORS support | ✓ | — | — | — | ~ | ✗ |
| 13 | Incremental diff streaming | ✓ | — | — | — | ✗ | ✗ |
| 14 | Full HTML snapshot (initial load) | ✓ | — | — | — | ✓ | ✓ |
| 15 | Terminal resize support | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 16 | Send keystrokes via API | ✓ | — | — | — | ~ | ~ |
| 17 | Snapshot-on-exit | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 18 | Retain-on-exit | ✓ | ~ (linger) | ~ (linger) | ✗ | ✗ | ✗ |
| 19 | Headless mode | ✓ | ✓ | ✓ | ~ | ✗ | ✗ |
| 20 | Monitor mode (buffering) | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 21 | Graceful shutdown signaling | ✓ | ✓ | ✓ | ✗ | ~ | ~ |
| 22 | Last-command-standing lifecycle | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 23 | Extensible handle sinks | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 24 | Extensible web commands | ✓ | ~ (plugins) | ✗ | ✗ | ✗ | ✗ |
| 25 | Pure-Rust / memory-safe | ✓ | ✗ (C) | ✗ (C) | ✓ (Go) | ✓ (Go) | ✓ (Node) |
| 26 | No system TLS dependency | ✓ | — | — | ✓ | ✓ | ✗ (OpenSSL) |
| 27 | Cross-platform PTY | ✓ | ~ (Unix only) | ~ (Unix only) | ~ (Unix only) | ~ (Unix only) | ~ (Unix only) |
| 28 | Single binary distribution | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| 29 | Per-instance configuration | ✓ | ✓ | ~ | ✗ | ✗ | ✗ |
| 30 | Process auto-restart | ✗ | ~ | ~ | ✗ | ✗ | ✗ |
| 31 | Scrollback buffer in UI | ✓ | ✓ | ✓ | ✓ | ~ | ~ |
| 32 | Search in terminal output | ~ | ✓ | ✓ | ✗ | ✗ | ✗ |
| 33 | Split panes / windows | ✗ | ✓ | ✓ | ~ | ✗ | ✗ |
| 34 | Session sharing (multi-client) | ✓ | ✓ | ✓ | ✗ | ~ | ✓ |
| 35 | Scripting / CI integration | ✓ | ✓ | ✓ | ~ | ✓ | ~ |

### Legend

- **✓** — Fully supported, production-ready
- **~** — Partial support or limited implementation
- **✗** — Not supported
- **—** — Not applicable to the tool's paradigm

---

## Key Architectural Differences

Beyond individual features, the tools differ fundamentally in how they are
architected. The table below summarizes these structural distinctions:

| Dimension | vrunner | tmux / screen | mprocs | gotty / wetty |
|---|---|---|---|---|
| **Communication** | HTTP + WebSocket | Unix socket / pipe | stdin/stdout of parent | HTTP + WebSocket |
| **State storage** | In-process memory (DashMap) | Server process memory | In-process memory | In-process memory |
| **Extensibility** | Trait-based plugins (handle sinks, web commands) | C plugins (tmux), scripts | Closed | Closed |
| **Deployment** | Single binary, optional daemon | System package / compile | Single binary | Single binary / npm |
| **Scaling** | Single node, multi-instance | Single node, multi-session | Single node, single session | Single node, single command |
| **Remote access** | Built-in (TLS + auth) | SSH tunnel required | Not designed for remote | Built-in (TLS + auth) |
| **Process model** | One daemon, N child PTYs | One server, N PTYs | One process, N children | One process, 1 child |
| **Shutdown model** | Last-command-standing | Manual detach/kill | Parent exit kills all | Parent exit kills child |
| **Configuration** | TOML + env + CLI flags | `.tmux.conf` / `.screenrc` | CLI flags only | CLI flags only |

---

## When to Choose vrunner

vrunner is the right tool when **one or more** of the following conditions hold:

### You Need a Web-Based Terminal with Zero Setup

You want to share a terminal session in a browser without installing SSH, setting
up reverse tunnels, or configuring firewalls. vrunner's self-signed TLS and
localhost binding mean you can start a secure terminal in one command:

```bash
vrunner run --tls --auth-token my-secret -- web-server --port 3000
```

### You Need a REST API Alongside the Terminal

Your automation pipeline needs to start, stop, and query processes via HTTP. With
vrunner you get a full JSON API (`POST /api/commands`, `GET /api/instances`,
`DELETE /api/commands/:id`) alongside the live terminal stream.

### You Need Fine-Grained Lifecycle Control

You need per-command options like `--retain-on-exit` (keep the command entry and
scrollback after exit), `--snapshot-on-exit` (capture a final HTML snapshot), or
`--send-keys` (inject keystrokes programmatically). These are first-class features
in vrunner, not workarounds.

### You Need Secure Remote Access Without OpenSSL

Your deployment environment does not have OpenSSL installed (e.g., minimal Docker
images, air-gapped networks). vrunner uses pure-Rust `rustls` and can auto-generate
certificates via `rcgen`—no external dependencies.

### You Need to Embed Terminal Access in a Larger Application

You are building a management dashboard or orchestration tool and need to embed
terminal access. vrunner's extensible handle sink system lets you pipe terminal
output to databases, log aggregators, or alerting systems alongside the
WebSocket stream.

### You Run Long-Running Processes in CI or Headless Servers

Your CI pipeline spawns background processes (databases, message brokers,
development servers) that need to be monitored and occasionally interacted with.
vrunner's daemon mode with last-command-standing lifecycle keeps the daemon alive
only as long as needed.

---

## When to Choose Alternatives

### Choose tmux When…

- **You want an in-terminal multiplexer.** tmux's split panes, windows, and
  keyboard-driven workflow are unmatched for daily development inside a terminal.
- **You don't need a web UI.** If all interaction happens over SSH, tmux is
  mature, fast, and ubiquitous.
- **You need session sharing over SSH.** Multiple users can attach to the same
  tmux session natively.
- **You are on a shared Unix server.** tmux is almost certainly already installed.

### Choose screen When…

- **You are on a legacy system** where tmux is not available but screen is.
- **You prefer screen's simpler configuration syntax.**
- **You need wide compatibility** with very old Unix systems.

### Choose mprocs When…

- **You want a simple, TUI-based process monitor** for running multiple commands
  side by side during development.
- **You don't need daemon mode, web access, or REST APIs.** mprocs is designed for
  interactive use in a single terminal window.
- **You want a zero-configuration experience.** mprocs reads a `mprocs.yaml` file
  and displays processes immediately.

### Choose gotty When…

- **You need a lightweight, Go-based terminal sharer** for a single command.
- **Your environment already has Go** and you prefer a Go toolchain.
- **You don't need multi-instance management or REST APIs.** gotty wraps a single
  command and serves it over WebSocket.

### Choose wetty When…

- **You are already running a Node.js ecosystem** and want a terminal sharer that
  integrates with your existing npm-based deployment.
- **You need integration with express.js or socket.io** middleware.
- **You are comfortable with OpenSSL dependencies** and want mature Node.js TLS
  handling.

---

## Summary

vrunner occupies a unique niche: it is the only tool that combines **web-based
terminal streaming**, **REST API management**, **incremental diff optimization**,
**per-command lifecycle options**, and **pure-Rust security** in a single binary.
If your needs align with any of the "When to Choose vrunner" scenarios above,
it provides capabilities that no other single tool can match. If you only need
terminal multiplexing in an SSH session, tmux remains the gold standard.

---

*This document is part of the [Diátaxis](https://diataxis.fr/) documentation framework
for vrunner. See the [explanation index](./) for related topics.*
