# vrl / vrunner User Manual

> **vrl** — A local-first virtual terminal runner (UDS IPC). **vrunner** — The same core, exposed as an HTTP server with web dashboard and REST API.

This manual is the comprehensive reference for both **vrl** and **vrunner** — two binaries built from the same `vrl_core` library. It covers every feature, configuration option, API endpoint, and workflow. Whether you are running your first command or integrating vrunner into a CI pipeline, this document has the answer.

---

## Table of Contents

### [Part I — Getting Started](#part-i--getting-started)
- [1.1 What is vrl / vrunner?](#11-what-is-vrl--vrunner)
- [1.2 vrl vs vrunner — Choosing the Right Binary](#12-vrl-vs-vrunner--choosing-the-right-binary)
- [1.3 Installation](#13-installation)
- [1.4 First Run](#14-first-run)
- [1.5 Key Concepts](#15-key-concepts)
- [1.6 CLI Quick Reference](#16-cli-quick-reference)

### [Part II — Everyday Use](#part-ii--everyday-use)
- [2.1 Running Commands](#21-running-commands)
- [2.2 Viewing Terminal Output](#22-viewing-terminal-output)
- [2.3 Web Admin Interface — vrunner only](#23-web-admin-interface--vrunner-only)
- [2.4 Sending Keystrokes](#24-sending-keystrokes)
- [2.5 Managing Commands](#25-managing-commands)
- [2.6 Configuration](#26-configuration)
- [2.7 Configuration Profiles](#27-configuration-profiles)
- [2.8 Environment Variables](#28-environment-variables)
- [2.9 Logging](#29-logging)
- [2.10 Display Modes](#210-display-modes)

### [Part III — Advanced Topics](#part-iii--advanced-topics)
- [3.1 Interactive Display](#31-interactive-display)
- [3.2 Keyboard Shortcuts Reference](#32-keyboard-shortcuts-reference)
- [3.3 Mouse Support](#33-mouse-support)
- [3.4 Search and Copy/Paste](#34-search-and-copypaste)
- [3.5 Split-Pane Display](#35-split-pane-display)
- [3.6 Retain on Exit and Purge](#36-retain-on-exit-and-purge)
- [3.7 Tabs Feature](#37-tabs-feature)
- [3.8 Remote Access and TLS — vrunner only](#38-remote-access-and-tls--vrunner-only)
- [3.9 Daemon Mode](#39-daemon-mode)
- [3.10 Multi-Instance Management](#310-multi-instance-management)
- [3.11 Certificate Management — vrunner only](#311-certificate-management--vrunner-only)
- [3.12 Exit Handlers and Timeouts](#312-exit-handlers-and-timeouts)
- [3.13 Snapshots and Diffs — vrunner only](#313-snapshots-and-diffs--vrunner-only)
- [3.14 WebSocket Protocol — vrunner only](#314-websocket-protocol--vrunner-only)
- [3.15 Incremental Diff Protocol — vrunner only](#315-incremental-diff-protocol--vrunner-only)
- [3.16 Hooks](#316-hooks)

### [Part IV — API Reference (vrunner only)](#part-iv--api-reference-vrunner-only)
- [4.1 REST API Overview](#41-rest-api-overview)
- [4.2 Command Endpoints](#42-command-endpoints)
- [4.3 VTTY Endpoints](#43-vtty-endpoints)
- [4.4 Mouse Endpoints](#44-mouse-endpoints)
- [4.5 Snapshot Endpoints](#45-snapshot-endpoints)
- [4.6 Instance Endpoints](#46-instance-endpoints)
- [4.7 Certificate Endpoints](#47-certificate-endpoints)
- [4.8 Log Endpoints](#48-log-endpoints)
- [4.9 Handle Endpoints](#49-handle-endpoints)

### [Part V — Security (vrunner only)](#part-v--security-vrunner-only)
- [5.1 Authentication](#51-authentication)
- [5.2 TLS Encryption](#52-tls-encryption)
- [5.3 CORS Policy](#53-cors-policy)
- [5.4 Token Management](#54-token-management)
- [5.5 Certificate-Based Access Control](#55-certificate-based-access-control)
- [5.6 Security Best Practices](#56-security-best-practices)

### [Part VI — For Contributors](#part-vi--for-contributors)
- [6.1 Building from Source](#61-building-from-source)
- [6.2 Code Organization](#62-code-organization)
- [6.3 Testing](#63-testing)
- [6.4 Architecture Decision Records](#64-architecture-decision-records)

### [Appendices](#appendices)
- [A. Comparison with Alternatives](#a-comparison-with-alternatives)
- [B. Troubleshooting](#b-troubleshooting)
- [C. Use Cases](#c-use-cases)
- [D. Cookbook](#d-cookbook)
- [E. Video Storyboard](#e-video-storyboard)
- [F. Version Upgrade Guide](#f-version-upgrade-guide)

---

# Part I — Getting Started

## 1.1 What is vrl / vrunner?

vrl executes commands inside virtual TTYs (pseudo-terminals) and exposes them through a web API and built-in admin dashboard. Unlike tools that wrap processes directly, vrl gives child processes full terminal capabilities — ANSI colors, cursor movement, interactive keyboard input, mouse events — while keeping the local terminal completely silent unless you opt in.

The key architectural idea is the **separation between starting a command and interacting with it**. A command can be started from the CLI, the web UI, or a script calling the API. Once running, it can be monitored and controlled from any of those interfaces interchangeably. This makes vrl suitable for scenarios where a command is launched from one place (a CI script) and observed from another (a web dashboard).

vrl is a single statically-linked binary with no runtime dependencies beyond the OS. The admin UI is embedded directly into the binary using `rust_embed`, so there are no separate assets to deploy or serve.

### Who is vrl / vrunner for?

| Role | How vrl helps |
|------|-------------------|
| Web developer | Run frontend + backend + database services, monitor all outputs from one dashboard |
| DevOps engineer | Expose build logs through the web UI for real-time debugging, manage remote services |
| CI/CD engineer | Run terminal-aware tests with full PTY support, capture output for debugging |
| System administrator | Manage services on headless machines through a browser with TLS and auth |
| Pair programmers | Share a terminal session between developers via the web interface |

### What makes vrl / vrunner different?

vrl is **web-first by design**. While it supports local terminal display, its primary interface is the HTTP API and the admin dashboard. This means you can run vrl on a headless server and interact with it entirely from a browser. The embedded admin UI requires no separate build step or asset pipeline — it ships inside the binary.

## 1.2 vrl vs vrunner — Choosing the Right Binary

The project provides **two binaries** built from the same `vrl_core` library. They share all core functionality (VTTY engine, process management, configuration, interactive display, daemon mode, hooks, handles) but differ in how they expose that functionality:

### Overview

| Aspect | **vrl** (local) | **vrunner** (HTTP) |
|--------|------------------|---------------------|
| Transport | Unix Domain Socket (UDS) | HTTP on port 9090 |
| Startup | Fast, minimal overhead | Starts HTTP server, TLS if configured |
| Web dashboard | No | Yes — embedded SPA at `/admin` |
| REST API | No | Yes — 30+ endpoints at `/api/` |
| WebSocket | No | Yes — VTTY streaming + log streaming |
| TLS encryption | No | Yes — built-in, auto-generated certs |
| Authentication | Not needed (local only) | Yes — bearer token, certificate pool |
| CORS policy | N/A | Yes — configurable via config |
| Remote access | No (local socket only) | Yes — bind to any interface |
| Certificates | No | Yes — per-command access control |
| Interactive display | Yes (`--display`) | Yes (`--display`) |
| Daemon mode | Yes (`--daemon`) | Yes (`--daemon`) |
| Config profiles | Yes | Yes |
| Hooks | Yes | Yes |
| Handles | Yes | Yes |
| VTTY engine | Same | Same |
| CLI subcommands | Same (`list`, `stop`, `spawn`, etc.) | Same |
| Binary name | `vrl` | `vrunner` |

### When to use **vrl**

- Local development workflows where you want fast startup
- Scripting and automation where you don't need HTTP
- CI pipelines that use `--display` to mirror output to the terminal
- Any scenario where only local access is needed and you want minimal overhead

### When to use **vrunner**

- You need a **web dashboard** to monitor processes from a browser
- You need a **REST API** for programmatic control from scripts or other services
- You need **remote access** to manage processes on a headless server
- You need **WebSocket streaming** for real-time terminal output
- You need **TLS** and **authentication** for secure remote management
- You need **per-command certificates** for access control
- Pair programming — multiple developers viewing the same terminal sessions

### How they share configuration

Both binaries read the same configuration files (`vrl.yaml`, `~/.config/vrl/config.yaml`, etc.) and support the same config schema. However, **vrl ignores web-specific settings** (`server`, `security`, `tls`, `web` sections) since it does not run an HTTP server. Similarly, **vrunner supports all config sections** including web-specific ones.

## 1.3 Installation

### From Source (Cargo)

```bash
git clone https://github.com/nkh/K.git
cd K

# Build vrl (local UDS binary)
cargo build --release --bin vrl
# Binary is at target/release/vrl

# Build vrunner (HTTP server + web dashboard binary)
cargo build --release --bin vrunner
# Binary is at target/release/vrunner
```

### Build Both at Once

```bash
cargo build --release
# Both binaries are at target/release/vrl and target/release/vrunner
```

### System-Wide Install

```bash
# Install vrl only
cargo install --bin vrl --path .

# Install vrunner only
cargo install --bin vrunner --path .

# Install both
cargo install --path .
```

### Man Pages

```bash
cp man/vrl.1 /usr/local/share/man/man1/
cp man/vrunner.1 /usr/local/share/man/man1/
man vrl
man vrunner
```

### Prebuilt Binaries

Download from the [Releases](https://github.com/nkh/K/releases) page (if available).

## 1.4 First Run

### First Run with vrl (local UDS)

**Step 1: Start vrl with a command**

```bash
vrl -- htop
```

This runs `htop` inside a virtual TTY. The VTTY is connected via a Unix Domain Socket — no HTTP server is started. The command runs until it exits (or use `Ctrl+\` to detach).

**Step 2: View it in the terminal**

```bash
vrl --display -- htop
```

This mirrors the VTTY output to your local terminal. Use the interactive display to send keystrokes, switch commands, and more.

**Step 3: Start in idle mode**

```bash
vrl
```

vrl starts with no command running. Use `vrl spawn` from another terminal to add commands:

```bash
# From another terminal
vrl spawn htop
vrl spawn -- cargo run
```

### First Run with vrunner (HTTP server)

**Step 1: Start vrunner**

```bash
vrunner
```

This starts an HTTP server on `http://127.0.0.1:9090`. No command is running yet — the instance is idle and ready to receive API requests or web UI connections.

**Step 2: Verify it works**

```bash
curl http://127.0.0.1:9090/api/commands
```

Expected response:
```json
{"status":"ok","data":[],"error":null}
```

**Step 3: Open the dashboard**

Open `http://127.0.0.1:9090/admin` in your browser. You will see the admin dashboard with an empty command list.

**Step 4: Spawn a command**

```bash
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "htop", "args": []}'
```

Expected response:
```json
{"status":"ok","data":{"id":"550e8400-e29b-41d4-a716-446655440000"},"error":null}
```

The command now appears in the web dashboard. Click on it to view the live terminal output.

**Step 5: Run a command at startup**

Stop vrunner (`Ctrl+C`) and try:

```bash
vrunner --display -- htop
```

This runs `htop` inside a virtual TTY and mirrors the output to your local terminal. Press `Ctrl+\` to quit the display while keeping the server running.

## 1.5 Key Concepts

> **Shared feature** — These concepts apply equally to both **vrl** and **vrunner**.

### VTTY (Virtual TTY)

A VTTY is an in-memory terminal emulator that receives raw PTY output from a child process. It maintains a 2D grid of cells (character + attributes), a scrollback buffer, cursor position, and terminal state (alternate screen, scroll regions, mouse tracking). Each command spawned by vrl/vrunner gets its own VTTY.

### Command Manager

The `CommandManager` is the central registry of all running commands. It stores `CommandHandle` objects in a concurrent `DashMap`, keyed by UUID. When a command exits, its handle is either removed (default) or retained (with `--retain-on-exit` or the API `retain_on_exit` field). When the last command is removed from the manager, the binary exits (in display and headless mode). Retained commands keep the display and server alive.

### Display Modes

Both vrl and vrunner operate in several modes depending on the flags used:

| Mode | Flags | Behavior |
|------|-------|----------|
| Headless | *(default)* | Silent execution, server/socket runs, waits for shutdown signal |
| Display | `--display` | Mirrors VTTY to local terminal, exits when command exits |
| Monitor | `--display-all` | Stays in display mode after command exits, switches to next |
| Daemon | `--daemon` | Background process, double-forks, detached from terminal |

### The `--` Separator

The `--` separator is required when passing a command to vrl or vrunner. Everything before `--` is a vrl/vrunner flag; everything after is the child command:

```bash
vrl --port 3000 --display -- python -m http.server 8000
#     ^^^^^^^^^^^ ^^^^^^^^^^^ ^^ ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
#     vrl opts  vrl opt  child command + args

vrunner --port 3000 --display -- python -m http.server 8000
#        ^^^^^^^^^^^ ^^^^^^^^^^^ ^^ ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
#        vrunner opts  vrunner opt  child command + args
```

## 1.6 CLI Quick Reference

### vrl (local UDS binary)

```
vrl [OPTIONS] [-- COMMAND [ARGS...]]
vrl list
vrl stop <PID>
vrl spawn [OPTIONS] CMD [ARGS...]
vrl freeze <PID>
vrl thaw <PID>
vrl resize <TARGET> [--rows N] [--cols M]
vrl purge [TARGET]
vrl list-vrl
vrl list-commands
vrl stop-command <TARGET>
vrl config-check
```

### vrunner (HTTP server binary)

```
vrunner [OPTIONS] [-- COMMAND [ARGS...]]
vrunner list
vrunner stop <PID>
vrunner spawn [OPTIONS] CMD [ARGS...]
vrunner freeze <PID>
vrunner thaw <PID>
vrunner resize <TARGET> [--rows N] [--cols M]
vrunner purge [TARGET]
vrunner cert generate|list|show|remove
vrunner list-vrl
vrunner list-commands
vrunner stop-command <TARGET>
vrunner config-check
```

> **Note:** Both binaries share the same CLI subcommand interface. The `cert` subcommand is only available in vrunner since it manages HTTP API certificates.

For the complete CLI reference, see the man page (`man vrl` or `man vrunner`) or run `vrl --help` / `vrunner --help`.

---

# Part II — Everyday Use

## 2.1 Running Commands

> **Shared feature** — Both vrl and vrunner can run commands at startup and via `spawn`. Web UI and API methods are **vrunner only**.

### At Startup

Use the `--` separator to pass a command at launch:

```bash
# Run a development server (vrl or vrunner)
vrl --display -- vim notes.txt
vrunner --port 3000 -- npm run dev

# Run with local terminal display
vrl --display -- vim notes.txt

# Run in the background as a daemon
vrl --daemon -- my-long-running-script.sh
vrunner --daemon -- my-long-running-script.sh

# Run with a custom terminal size
vrl --vtty-cols 120 -- python -m http.server 8000

# Send initial keystrokes after the command starts
vrl --send-keys "ls<Enter>" -- bash

# Run with per-command exit options
vrl --retain-on-exit --snapshot-on-exit /tmp/build.log -- cargo build --release

# Capture a command's output and exit when it finishes
vrl --snapshot-on-exit /tmp/htop-output.txt --display -- htop
```

### Via the Web UI — vrunner only

1. Open `http://127.0.0.1:9090/admin` in your browser.
2. Use the command spawn interface to enter a command and optional arguments.
3. The command starts inside a new VTTY; its entry appears in the command list.
4. Click on the command to view its terminal output.

### Via curl (API) — vrunner only

```bash
# Start a simple command
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "htop", "args": []}'

# Start with arguments
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "python", "args": ["-m", "http.server", "8000"]}'

# Start with exit handlers
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "cargo", "args": ["test"], "on_exit": "notify-send OK", "on_error": "notify-send FAIL"}'
```

### Via `vrl spawn` / `vrunner spawn` (CLI)

The `spawn` subcommand discovers running instances and sends a spawn request:

```bash
# Auto-selects the only running instance
vrl spawn htop

# With arguments
vrl spawn python -m http.server 8000

# Target a specific instance
vrl --target 12345 spawn npm run dev

# With environment variables
vrl spawn --env RUST_LOG=debug -- cargo run

# With a custom terminal size
vrl spawn --rows 50 --cols 160 -- vim file.txt
```

> With **vrl**, the `spawn` subcommand communicates via UDS. With **vrunner**, it communicates via HTTP to the running server.

### Via WebSocket — vrunner only

```javascript
const ws = new WebSocket('ws://127.0.0.1:9090/api/commands/550e8400.../ws');
// Send keystrokes
ws.send(JSON.stringify({ type: 'keys', keys: 'ls -la\r' }));
// Resize terminal
ws.send(JSON.stringify({ type: 'resize', rows: 40, cols: 120 }));
```

## 2.2 Viewing Terminal Output

> **Shared feature** — Both vrl and vrunner support local VTTY display. Web-based viewing is **vrunner only**.

### Local VTTY Display

```bash
vrl --display -- htop
```

The VTTY contents are mirrored to stdout at the refresh interval (`--refresh-ms`, default 100ms). The display renders the raw ANSI output including colors and cursor positioning.

### Web Admin VTTY Viewer — vrunner only

The admin dashboard provides a full-featured terminal viewer with:

- **Real-time streaming** via the incremental diff WebSocket protocol
- **Keyboard input** — click the terminal pane to focus, then type
- **Mouse interaction** — wheel scrolling, click forwarding, drag selection
- **Scrollback navigation** — mouse wheel to scroll up through history
- **Search** — `Ctrl+F` to search within the output buffer
- **Auto-resize** — terminal fits the available panel space

### VTTY API Endpoints — vrunner only

Three endpoints provide VTTY content at different levels:

```bash
ID="550e8400-e29b-41d4-a716-446655440000"

# Full ANSI content (raw escape sequences)
curl http://127.0.0.1:9090/api/commands/$ID/vtty

# HTML-rendered content (with cursor, dimensions, scrollback count)
curl http://127.0.0.1:9090/api/commands/$ID/vtty/html

# HTML with scrollback offset (for browsing history)
curl "http://127.0.0.1:9090/api/commands/$ID/vtty/html?scrollback_offset=10"

# Paginated plain text
curl "http://127.0.0.1:9090/api/commands/$ID/vtty/partial?offset=0&limit=50"
```

### Direct Command URLs — vrunner only

Navigate to `http://localhost:9090/<command_name>` to jump straight to a command's terminal. For example, `/htop` opens the VTTY viewer for a command named `htop`. If multiple commands share the same name, a picker list is shown.

## 2.3 Web Admin Interface — vrunner only

> **vrunner only** — The web admin interface is not available in vrl.

The admin dashboard is a single-page application embedded in the vrunner binary and served at `/admin`. It is split into three files — `index.html`, `style.css`, and `app.js` — and communicates with the REST API and WebSocket endpoints. All static assets are embedded at compile time via `rust-embed`; no external CDN or build step is required.

### Layout

The top bar is organized into three button groups:

- **Left group** — Add Panel (spawn), Pause/Run toggle, Kill All
- **Center group** — Font size controls (A-/A+), resize to fit, alternate screen buffer selector
- **Right group** — Auth token input, documentation link, keyboard shortcuts (`?`), theme toggle (sun/moon)

The layout uses a consistent button sizing system with four variants: `btn-xs` (compact toolbars), `btn-sm` (secondary actions), `btn` (default primary), and `btn-primary`/`btn-danger` (color variants). The center group collapses on mobile viewports.

- **Top bar** — grouped button layout (left/center/right), theme toggle, shortcuts
- **Sidebar** — command list with name, PID, status, runtime, and context menu actions
- **Panel tab bar** — tab strip for multi-panel layouts; right-click for context menu
- **Main pane** — VTTY terminal viewer with per-panel font size, copy, export, and selection mode
- **Bottom bar** — connection quality indicator (latency + reconnect count), scrollback indicator

### Features

#### Terminal Interaction

| Feature | Description |
|---------|-------------|
| Real-time VTTY viewer | Incremental diff WebSocket protocol, 1-second HTTP polling fallback |
| Direct keyboard input | Click terminal to focus, type to send keystrokes |
| Mouse event forwarding | Clicks, drags, and wheel events forwarded to child process |
| Mouse wheel scrollback | Scroll through command history; smart routing to app or scrollback |
| Terminal search | `Ctrl+F` to search within the output buffer |
| Scroll-to-Bottom | Floating button when scrolled up |
| Selection mode | Toggle to enable native text selection when mouse tracking is active (`Ctrl+Shift+S`, `Alt+S`) |
| Copy to clipboard | `Ctrl+Shift+C` copies selected text; falls back to full buffer when no selection |
| Per-panel font size | A-/A+ buttons in each panel header; persisted to localStorage |
| Persistent scrollback | Scrollback offset saved to sessionStorage; restored on re-select |
| Export output | Download terminal buffer as `.txt` |

#### Command Management

| Feature | Description |
|---------|-------------|
| Command sidebar | Name, arguments, PID, status, runtime with search/filter |
| Batch Kill All | One-click terminate all running commands |
| Pause / Run | Freeze/thaw current command (SIGSTOP/SIGCONT) |
| Retained VTTY display | Exited commands shown with status and purge option |
| Delete retained VTTYs | Purge button on exited commands in the sidebar |
| Incremental DOM updates | Command list polling skips redundant DOM rebuilds when state is unchanged |

#### Context Menu and Accessibility

| Feature | Description |
|---------|-------------|
| Context menu (sidebar) | Right-click commands for kill, freeze/thaw, copy URL, new tab |
| Context menu (panels) | Right-click panel headers for Copy URL, Pause/Resume, Kill, Remove Panel |
| Keyboard-accessible menu | `Shift+F10` opens menu, arrow keys navigate, Enter activates, Escape closes |
| ARIA roles | `role=menu` and `role=menuitem` on context menu elements |
| XSS-safe handlers | All context menu actions use `addEventListener` — no inline `onclick` injection |

#### Focus and Keyboard

| Feature | Description |
|---------|-------------|
| Focus management | Modals trap focus (Tab/Shift+Tab wrap around), restore focus on close |
| Escape to dismiss | Consistent Escape key dismissal for all modals and overlays |
| Keyboard shortcuts | Press `?` for the shortcuts panel showing all keybindings |

#### Connection and Theming

| Feature | Description |
|---------|-------------|
| Connection quality | Bottom bar shows WebSocket latency (color-coded green/yellow/red) and reconnect count |
| Auto-reconnect | WebSocket re-establishes after network interruptions |
| Light theme | Toggle between dark and light themes; respects `prefers-color-scheme` by default |
| Theme persistence | Theme choice saved to localStorage |
| VTTY theme-aware | Terminal background adapts to active theme |
| Responsive layout | Collapsible sidebar on narrow viewports |
| Browser notifications | Desktop notification on command exit |

#### Real-Time Log Streaming

| Feature | Description |
|---------|-------------|
| WebSocket log stream | Connects to `/api/ws/logs` when log view is active; appends entries in real-time |
| HTTP fallback | Initial log load and search filtering use HTTP endpoint |
| Transport indicator | Log toolbar shows current transport (ws/http) |
| Auto-scroll | Automatically scrolls to bottom when already at the bottom |
| Exponential backoff | Reconnects with 1s–30s backoff on disconnect |

## 2.4 Sending Keystrokes

> **Shared feature** — Both vrl and vrunner support sending keystrokes. API and WebSocket methods are **vrunner only**.

### Via API — vrunner only

```bash
ID="550e8400-e29b-41d4-a716-446655440000"

# Send text
curl -X POST http://127.0.0.1:9090/api/commands/$ID/keys \
  -H "Content-Type: application/json" \
  -d '{"keys": "ls -la\r"}'

# Send special keys
curl -X POST http://127.0.0.1:9090/api/commands/$ID/keys \
  -H "Content-Type: application/json" \
  -d '{"keys": "\x03"}'   # Ctrl+C
```

### Common Escape Sequences

| Sequence | Key |
|----------|-----|
| `\x03` | Ctrl+C (SIGINT) |
| `\x04` | Ctrl+D (EOF) |
| `\x1b` | Escape |
| `\r` | Enter |
| `\t` | Tab |
| `\x7f` | Backspace |
| `\x1b[A` | Up arrow |
| `\x1b[B` | Down arrow |
| `\x1b[C` | Right arrow |
| `\x1b[D` | Left arrow |

### Via WebSocket — vrunner only

```javascript
ws.send(JSON.stringify({ type: 'keys', keys: 'q' }));
ws.send(JSON.stringify({ type: 'keys', keys: ':q!\r' }));  // quit vim
```

### Via Web UI — vrunner only

Click the terminal pane to capture keyboard focus, then type normally. The keystrokes are forwarded to the child process in real time.

### Via Interactive Display — vrl / vrunner

When using `--display`, type directly in the terminal. Keystrokes are forwarded to the child process in real time.

## 2.5 Managing Commands

> **Shared feature** — Both vrl and vrunner support CLI-based command management. API methods are **vrunner only**.

### Listing Commands

```bash
# Via CLI (all instances)
vrl list

# Via API (vrunner only)
curl http://127.0.0.1:9090/api/commands
```

### Killing Commands

```bash
# Via API (by command UUID) — vrunner only
curl -X POST http://127.0.0.1:9090/api/commands/$ID/kill \
  -H "Content-Type: application/json" -d '{}'

# Via API (by OS PID) — vrunner only
curl -X POST http://127.0.0.1:9090/api/commands/kill-pid/12345

# Via CLI (queries all instances)
vrl stop 12345

# Via CLI subcommand
vrl stop-command 12345
```

### Freeze/Thaw

```bash
# Freeze (SIGSTOP — pauses the process)
vrl freeze $ID

# Thaw (SIGCONT — resumes the process)
vrl thaw $ID

# Via API (vrunner only)
curl -X POST http://127.0.0.1:9090/api/commands/$ID/freeze
curl -X POST http://127.0.0.1:9090/api/commands/$ID/thaw
```

### Resizing

```bash
# Via CLI (auto-detects terminal size if omitted)
vrl resize htop --rows 50 --cols 160

# Via API (vrunner only)
curl -X POST http://127.0.0.1:9090/api/commands/$ID/resize \
  -H "Content-Type: application/json" \
  -d '{"rows": 50, "cols": 160}'
```

Valid ranges: rows 1–200, cols 1–500. The child process receives a `SIGWINCH` signal so terminal-aware applications adjust their layout.

### Purging Retained Commands

When `--retain-on-exit` is enabled, exited commands remain in memory. Purge them to free resources:

```bash
# Via CLI (purges the only exited command, or specify target)
vrl purge

# Via CLI (by ID or name)
vrl purge 550e8400

# Via API (vrunner only)
curl -X DELETE http://127.0.0.1:9090/api/commands/$ID

# Via Web UI (vrunner only) — click the purge button on an exited command in the sidebar
```

## 2.6 Configuration

> **Shared feature** — Both vrl and vrunner read the same configuration files. Web-specific sections (`server`, `security`, `tls`, `web`) are only used by vrunner.

Both binaries read configuration from multiple sources in order of increasing precedence:

```
Built-in defaults → Global config → Local config → Explicit config file → CLI flags
```

| Priority | Source | Path |
|----------|--------|------|
| Lowest | Built-in defaults | Compiled into the binary |
| Low | Global config | `~/.config/vrl/config.yaml` (or `.toml`/`.json`) |
| Medium | Local config | `./vrl.yaml` (or `.toml`/`.json`) |
| High | Explicit config | Any path via `-c <FILE>` |
| Highest | CLI flags | Command-line arguments |

### Example `vrl.yaml`

```yaml
server:
  bind: "127.0.0.1"
  port: 9090

security:
  require_auth: false
  token_file: "~/.config/vrl/token"

tls:
  enabled: false

vtty:
  rows: 24
  cols: 80
  term: "xterm-256color"
  scrollback: 5000
  truecolor: true
  mouse: false

display:
  enabled: false
  refresh_ms: 100

daemon:
  enabled: false
  stdout_file: "/tmp/vrl.out"
  stderr_file: "/tmp/vrl.err"

web:
  update_mode: "push"
  dirty_check_ms: 200
  default_poll_ms: 500

interactive:
  tabs: false
  keybindings:
    next_command: "ctrl+right"
    prev_command: "ctrl+left"
    toggle_log: "ctrl+l"
    spawn_command: "f12"
    show_help: "ctrl+h"
    quit: null

default_exit:
  exit:
    on_exit: null
    on_error: null
    timeout_secs: 10

environment:
  variables: {}

profiles: {}

handles: []
```

> **Binary-specific config sections:**
> - **Both**: `vtty`, `display`, `daemon`, `interactive`, `default_exit`, `environment`, `profiles`, `handles`
> - **vrunner only**: `server`, `security`, `tls`, `web`

For the complete configuration reference with all fields, types, and CLI flag mappings, see [docs/configuration.md](docs/configuration.md).

## 2.7 Configuration Profiles

> **Shared feature** — Both vrl and vrunner support configuration profiles.

Profiles let you define named sets of configuration values. When selected, only the fields present in the profile override the base configuration:

```yaml
profiles:
  development:
    vtty:
      rows: 40
      cols: 120
    display:
      enabled: true
    environment:
      variables:
        RUST_LOG: "debug"

  production:
    server:
      bind: "0.0.0.0"
      port: 443
    security:
      require_auth: true
    tls:
      enabled: true
```

```bash
# Use a profile
vrl --profile development -- cargo run

# Combine with CLI flag overrides
vrl --profile production --port 8443 -- ./my-server
```

CLI flags always take final precedence over both the base config and the profile.

## 2.8 Environment Variables

> **Shared feature** — Both vrl and vrunner support environment variable control via config and CLI.

Both binaries provide three layers of environment variable control:

```
CLI --env flags / API "env" field  (highest — always wins)
        ↓
Config environment.variables     (global defaults)
        ↓
--no-env clears config env vars   (but CLI/API env vars still apply)
```

The `TERM` variable is always set to the configured `vtty.term` value (default: `xterm-256color`).

### Examples

```yaml
# Config file — global defaults for all commands
environment:
  variables:
    RUST_LOG: "info"
    NODE_ENV: "development"
```

```bash
# CLI — override for this command only
vrl spawn --env RUST_LOG=debug -- cargo run

# API — per-command override (vrunner only)
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "./my-app", "env": {"RUST_LOG": "debug"}}'

# Clean environment (ignore config env vars)
vrl spawn --no-env --env PATH=/usr/bin -- ./my-script.sh
```

## 2.9 Logging

> **Shared feature** — Both vrl and vrunner support command logging via CLI flags. API log access and WebSocket log streaming are **vrunner only**.

### Command Log

```bash
# Log to terminal
vrl --log -- my-command

# Log to file
vrl --log-file /var/log/vrl.log -- my-command
```

### Reading Logs via API — vrunner only

```bash
# All log entries
curl http://127.0.0.1:9090/api/log

# With search filter and pagination
curl "http://127.0.0.1:9090/api/log?search=spawn&offset=0&limit=50"
```

### Raw PTY Logging

The `--log-pty-raw` flag records the raw bytes received from each child PTY read. This is useful for debugging terminal output issues or replaying sessions:

```bash
vrl --log-pty-raw /tmp/pty-raw.log -- my-command
```

Each line records one `read()` call with an elapsed-time stamp and escaped bytes. Printable ASCII is shown as-is; non-printable bytes are escaped as `\xHH`. The resulting log can be replayed step-by-step with tools like `ansi-replay`.

### Real-Time Log Streaming — vrunner only

```javascript
const ws = new WebSocket('ws://127.0.0.1:9090/api/ws/logs');
ws.onmessage = (e) => {
  const msg = JSON.parse(e.data);
  if (msg.type === 'log_entry') console.log('[LOG]', msg.data);
};
```

## 2.10 Display Modes

> **Shared feature** — Both vrl and vrunner support the same display mode flags.

The display flags control whether and how VTTY output appears on your local terminal. Each mode serves a different workflow — from silent background execution to persistent monitoring of multiple commands. For the interactive display internals (keybindings, overlays, state machine), see [3.1 Interactive Display](#31-interactive-display).

### Overview

| Flag | Short | Mode | Display closes when… | After close… |
|------|-------|------|-----------------------|--------------|
| *(none)* | — | Headless | N/A (no display) | Instance runs until shutdown signal |
| `--display` | `-D` | Display | The initial/main command exits | Instance continues running in the background |
| `--display-all` | `-s` | Monitor | All commands have exited and none have `retain-on-exit` set | Instance exits |
| `--no-display` | — | Explicit off | N/A (no display) | Same as headless; overrides config |
| `--tabs` | — | Monitor + tab bar | Same as `--display-all` | Same as `--display-all` |

### No display flag (default)

When no display flag is given, the vrl/vrunner instance runs **headless** — it starts the server or socket, spawns the command (if any), and runs silently in the background:

```bash
# Headless — VTTY output is not shown on the terminal
vrl -- cargo build --release
vrunner --port 3000 -- npm run dev
```

- The instance runs in the background with no terminal output
- The instance continues running after the command exits (if `--retain-on-exit` is set) or exits when the last command is removed
- Useful for daemon mode, scripting, CI pipelines, or when you only need the web UI (vrunner)

> **Tip:** Combine headless mode with `--daemon` for fully detached background execution.

### `--display` (`-D`)

Shows the VTTY output on the local terminal in real-time. The display is tied to the **initial/main command** — when that command exits, the display immediately closes:

```bash
# Display closes when `cargo test` finishes
vrl --display -- cargo test

# Display closes when `htop` exits (e.g. q or F10)
vrl -D -- htop

# Display closes when the command exits, but vrunner keeps running
vrunner --display -- npm run dev
```

- VTTY output is mirrored to stdout at the refresh interval (`--refresh-ms`, default 100ms)
- Keystrokes are forwarded to the child process in real-time
- When the initial/main command exits, the display **immediately closes** and returns to the shell prompt
- The vrl/vrunner instance **continues running** in the background (server or socket stays active)
- Useful when you want to see a specific command's output but don't need to keep monitoring after it finishes

> **Tip:** Use `--retain-on-exit` to keep the command's VTTY in memory after it exits. The VTTY content remains accessible via the web UI (vrunner) or `vrl list`.

### `--display-all` (`-s`)

Shows the VTTY output on the local terminal in real-time and enters **monitor mode** — the display stays open and automatically switches between commands:

```bash
# Monitor mode — display stays open, switches to next command
vrl --display-all -- cargo test

# Short form
vrl -s -- cargo test

# Monitor mode with retain-on-exit keeps display alive
vrl --retain-on-exit --display-all -- cargo build --release
```

- VTTY output is mirrored to the terminal, same as `--display`
- When the current command exits, the display **automatically switches** to the next running command
- If no commands are running, shows a "waiting for commands" message and keeps waiting
- The display **only exits when all commands have exited** and none have `retain-on-exit` set
- Keystrokes are forwarded to the currently displayed command
- Useful for monitoring multiple commands as they start and stop, or for long-running sessions where you want the display to persist

> **Tip:** Combine with `--tabs` to add a tab bar at the bottom of the display for quick command switching (see below).

### `--no-display`

Explicitly disables the local terminal display, overriding any configuration file settings:

```bash
# Force headless even if config sets display.enabled: true
vrl --no-display -- cargo test
```

- Disables terminal output regardless of `display.enabled` in the config file
- Useful in scripts and CI pipelines to ensure clean output
- CLI flags always take precedence over config, but `--no-display` makes the intent explicit

### `--tabs`

Shows a tab bar at the bottom of the display and enables monitor mode behavior:

```bash
# Tab bar with monitor mode
vrl --tabs --display-all -- cargo test

# Tab bar implies display-all, so this is equivalent
vrl --tabs -- cargo test

# With mouse support for tab clicking
vrl --tabs --mouse -- cargo test
```

- Renders a tab bar at the bottom of the terminal showing all running commands
- Click tabs (with `--mouse`) or use `Ctrl+Right`/`Ctrl+Left` to switch between commands
- **Implies `--display-all` behavior** — the display stays open in monitor mode
- The tab bar updates dynamically as commands start and stop
- Useful when running multiple commands and wanting to quickly switch between them

### Comparison Examples

The following examples demonstrate the practical differences between the modes:

```bash
# Scenario 1: Build and immediately return to shell
vrl --display -- cargo build --release
# → Shows build output. When build finishes, display closes.
# → vrl instance exits (no other commands running).

# Scenario 2: Build and keep monitoring
vrl --display-all -- cargo build --release
# → Shows build output. When build finishes, display stays open.
# → Shows "waiting for commands" until another command starts or instance is stopped.

# Scenario 3: Build multiple targets with tab switching
cargo build --release & cargo test &
vrl --tabs -- cargo build --release
# → Shows tab bar. Switch between build and test output.
# → Display stays open until all commands exit.

# Scenario 4: Silent background with web UI only
vrunner --port 3000 -- npm run dev
# → No terminal output. Access via http://127.0.0.1:3000/admin.
```

### Interaction with `--retain-on-exit`

The `--retain-on-exit` flag affects when the display and instance decide to exit:

| Mode | Command exits without `--retain-on-exit` | Command exits with `--retain-on-exit` |
|------|------------------------------------------|--------------------------------------|
| `--display` | Display closes; instance continues if other commands exist | Display closes; instance continues (retained VTTY keeps it alive) |
| `--display-all` | Display switches to next command, or shows "waiting" | Display switches to next command; retained VTTY keeps instance alive |
| Headless | Instance continues if other commands exist | Instance continues (retained VTTY keeps it alive) |

A retained command's VTTY is kept in memory and displayed as an exited entry in the command list. The instance will not exit while any retained commands exist, regardless of display mode.

---

# Part III — Advanced Topics

## 3.1 Interactive Display

> **Shared feature** — Both vrl and vrunner support the interactive terminal-based display mode.

The interactive display mode provides a terminal-based UI for monitoring and controlling commands. It is enabled with `--display` and optionally `--display-all` and `--tabs`:

```bash
# View a single command's output (exits when command finishes)
vrl --display -- htop

# Stay active after command exits, switching to next command
vrl --display-all -- htop

# Show tab bar for command switching
vrl --tabs --display-all -- htop

# Combine with mouse support
vrl --display-all --tabs --mouse -- cargo test
```

### Display Loop States

The display loop operates in these states:

```
                    ┌──────────────────────────────────────┐
                    │          DISPLAY LOOP STATES          │
                    └──────────────────────────────────────┘

  ┌──────────┐    command      ┌──────────┐    command     ┌──────────┐
  │  Active  │────exits───────>│ Monitor  │────exits──────>│   Exit   │
  │          │<──selects───────│          │<──selects──────│   loop   │
  │forward   │                 │read-only │                 │terminates│
  │keystrokes│                 │display   │                 │          │
  └────┬─────┘                 └────┬─────┘                 └──────────┘
       │                            │
       │ Ctrl+L                     │ Ctrl+L
       ▼                            ▼
  ┌──────────┐                 ┌──────────┐
  │   Log    │                 │   Log    │
  │ Overlay  │                 │ Overlay  │
  └────┬─────┘                 └────┬─────┘
       │ Ctrl+H / q/Esc            │ q/Esc
       ▼                            │
  ┌──────────┐                      │
  │   Help   │                      │
  │ Overlay  │                      │
  └──────────┘                      │
       │ any key                    │
       ▼                            │
  (returns to                       │
   previous state)                  │
                                    │
  ┌──────────┐    Esc/Enter         │
  │  Context │<─────────────────────┘
  │   Menu   │   right-click
  │(Kill/    │
  │Purge/    │   Ctrl+F             ┌──────────┐
  │Restart)  │────────────────────>│  Search  │
  └──────────┘                      │ Overlay  │
                                    └────┬─────┘
                                         │ Esc
                                         ▼
                                    (returns to
                                     previous state)
```

**State transitions:**

1. **Active** — A command is selected; keystrokes are forwarded to the child process
2. **Monitor** — No direct child; VTTY output is displayed read-only from other commands
3. **Overlay** — A temporary overlay (log, help, spawn prompt) is shown on top of the VTTY
4. **Context Menu** — Right-click context menu for command management actions

When a command exits in active mode, the display automatically transitions to monitor mode (if other commands exist) or exits (if no commands remain). If `--retain-on-exit` was used on a command, that command stays in the manager after exiting, which keeps the display alive even in `display_all` mode. When all commands have been removed (none retained), the binary exits regardless of `display_all`.

## 3.2 Keyboard Shortcuts Reference

> **Shared feature** — Both vrl and vrunner support the same interactive keyboard shortcuts.

### Default Keybindings

| Shortcut | Action | Mode |
|----------|--------|------|
| `Ctrl+Right` | Switch to next command | display-all |
| `Ctrl+Left` | Switch to previous command | display-all |
| `Ctrl+L` | Toggle command log overlay | always |
| `F12` | Spawn a new command | always |
| `Ctrl+H` | Show help overlay | always |
| `Ctrl+\` | Quit display | always |
| `Ctrl+F` | Search in terminal buffer | active |
| `Ctrl+S` | Toggle split-pane | always |
| `Ctrl+C` | Interrupt (SIGINT) child | active |
| Right-click | Context menu | always |

### Customizing Keybindings

```yaml
interactive:
  keybindings:
    next_command: "ctrl+right"
    prev_command: "ctrl+left"
    toggle_log: "ctrl+l"
    spawn_command: "f12"
    show_help: "ctrl+h"
    quit: null          # null disables the binding
    kill_command: "ctrl+k"   # disabled by default
    toggle_pause: "ctrl+p"   # disabled by default
```

### Supported Key Name Formats

- **Control keys**: `ctrl+a` through `ctrl+z`, `ctrl+left`, `ctrl+right`, `ctrl+up`, `ctrl+down`
- **Alt/Meta**: `alt+a` through `alt+z`, `alt+0` through `alt+9`
- **Shift + arrows**: `shift+left`, `shift+right`, `shift+up`, `shift+down`, `shift+tab`
- **Function keys**: `f1` through `f12`
- **Special keys**: `enter`, `tab`, `backspace`, `delete`, `insert`, `home`, `end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right`, `esc`, `space`

## 3.3 Mouse Support

### CLI Interactive Display — vrl / vrunner

> **Shared feature** — Both binaries support mouse forwarding in the interactive display.

Enable mouse forwarding to child processes with `--mouse`:

```bash
vrl --display --mouse -- htop
```

When enabled, mouse events (clicks, drags, wheel) from the terminal are captured and forwarded to the child process using SGR (?1006) encoding. This allows mouse-aware applications (htop, vim, mc, tmux) to receive mouse input as if running directly in the terminal.

Mouse events in the **tab bar** area always control the display (tab switching), regardless of the `--mouse` flag. Only mouse events within the VTTY display area are forwarded to the child.

### Web UI Mouse Interaction — vrunner only

The web admin interface provides full mouse interaction:

- **Click to focus** — Click the terminal pane to capture keyboard input (shown with a blue outline)
- **Wheel scrolling** — Mouse wheel scrolls through scrollback history
- **Smart wheel routing** — If the child application has mouse tracking enabled, wheel events go to the application; otherwise, they scroll the view
- **Scrollback indicator** — A yellow "SCROLLBACK" label appears in the bottom bar when scrolled up; a floating button returns to live output
- **Mouse event forwarding** — Click, drag, and wheel events are forwarded to the child process via `POST /api/commands/:id/mouse`

## 3.4 Search and Copy/Paste

### Search in Interactive Display — vrl / vrunner

> **Shared feature** — Both binaries support search in the interactive display.

Press `Ctrl+F` to open the search bar. Type a search term and press Enter to find the next match. Matches are highlighted in the terminal output.

### Search in Web UI — vrunner only

Press `Ctrl+F` inside the web terminal viewer. A search bar appears at the top of the terminal pane.

### Copy/Paste in Interactive Display — vrl / vrunner

> **Shared feature** — Both binaries support copy/paste in the interactive display.

When mouse support is enabled (`--mouse`), you can select text in the terminal display:

1. Click and drag to select text
2. The selection is automatically copied to the system clipboard
3. Use the terminal's paste mechanism to insert the copied text

## 3.5 Split-Pane Display

> **Shared feature** — Both vrl and vrunner support split-pane mode.

Press `Ctrl+S` in the interactive display to toggle split-pane mode. This divides the terminal area horizontally, showing two VTTYs side by side. Use `Ctrl+Left`/`Ctrl+Right` to switch which pane is active. Each pane shows a different command's output.

```bash
vrl --display-all -- cargo test
# Spawn a second command via F12, then press Ctrl+S for split view
```

## 3.6 Retain on Exit and Purge

> **Shared feature** — Both vrl and vrunner support retain-on-exit. API/web purge is **vrunner only**.

By default, when a command exits its VTTY buffer is discarded and the binary exits (in display mode). The `--retain-on-exit` flag is a **per-command option** that keeps the buffer in memory, allowing you to inspect the final output:

```bash
# Keep the VTTY buffer after cargo test finishes
vrl --retain-on-exit --display-all -- cargo test

# Different commands can have different retain settings
vrl --retain-on-exit --snapshot-on-exit /tmp/test-output.txt -- cargo test
```

When a command with `--retain-on-exit` finishes:
- It remains visible in the tab bar with an `[EXITED]` status
- The VTTY buffer stays in memory for inspection
- In the web UI (vrunner), exited commands show a purge button in the sidebar
- The display loop stays active (does not exit) because the retained command is still in the manager

**Important:** `--retain-on-exit` is per-command. It only affects the command specified on the CLI. Future commands spawned via the API or F12 use their own `retain_on_exit` setting (passed in the spawn request body).

### Snapshot on Exit

> **Shared feature** — Both vrl and vrunner support snapshot-on-exit.

The `--snapshot-on-exit <FILE>` flag saves the VTTY buffer (including scrollback) to a file as plain text when the command exits. This is useful for capturing test output, build logs, or any command's final state:

```bash
# Save htop's final screen to a file
vrl --snapshot-on-exit /tmp/htop-output.txt --display -- htop
```

The output includes all scrollback lines followed by the visible screen rows, with each line trimmed of trailing whitespace. The snapshot is taken after the process exits but before the command is removed from the manager.

### Purging Retained Commands

Remove retained commands to free memory:

```bash
# CLI: purge the only exited command, or specify by ID/name
vrl purge
vrl purge 550e8400

# API: delete by command ID (vrunner only)
curl -X DELETE http://127.0.0.1:9090/api/commands/$ID

# Web UI: click the purge button on an exited command (vrunner only)
```

## 3.7 Tabs Feature

> **Shared feature** — Both vrl and vrunner support the tab bar in interactive display.

The `--tabs` flag enables a tab bar at the top of the interactive display listing all running commands:

```bash
vrl --tabs --display-all -- cargo test
```

Features:
- Each tab shows the command name and status (running/frozen/exited)
- Click a tab to switch to that command
- `Ctrl+Left`/`Ctrl+Right` navigate between tabs
- Tabs automatically update when commands spawn or exit
- Retained (exited) commands appear with `[EXITED]` status

## 3.8 Remote Access and TLS — vrunner only

> **vrunner only** — vrl uses local UDS and has no HTTP server, TLS, or remote access capabilities.

### Quick Remote Setup

```bash
vrunner --remote --tls -- my-command
```

This single command:
- Binds to `0.0.0.0` (accepts connections from any interface)
- Enables bearer token authentication (auto-generates a token)
- Enables TLS with self-signed certificates (auto-generates)

### Step-by-Step Setup

1. **Start the server**: `vrunner --bind 0.0.0.0 --port 8080 --auth --tls -- my-command`
2. **Get the token**: `cat ~/.config/vrl/token`
3. **Get the certificate**: `cat ~/.config/vrl/cert.pem`
4. **Connect from remote**:
   ```bash
   curl --cacert /path/to/cert.pem \
        -H "Authorization: Bearer <token>" \
        https://server:8080/api/commands
   ```

### Custom TLS Certificates

```bash
vrunner --tls \
  --cert-file /etc/ssl/certs/vrl.crt \
  --key-file /etc/ssl/private/vrl.key \
  --remote -- my-command
```

Or in the config file:
```yaml
tls:
  enabled: true
  cert_file: "/etc/ssl/certs/vrl.crt"
  key_file: "/etc/ssl/private/vrl.key"
```

## 3.9 Daemon Mode

> **Shared feature** — Both vrl and vrunner support daemon mode.

Run as a background process:

```bash
vrl --daemon -- my-command
vrunner --daemon -- my-command
```

In daemon mode:
- A double-fork detaches the process from the controlling terminal
- stdin is closed; stdout/stderr redirect to files (default: `/tmp/vrl.out`, `/tmp/vrl.err`)
- The `--display` option is automatically disabled
- With vrunner, the HTTP server continues running for API/web access

```bash
# Custom output files
vrl --daemon --stdout-file /var/log/vrl/stdout \
  --stderr-file /var/log/vrl/stderr -- my-command

# Manage a daemon
vrl list                          # find the PID
vrl stop <pid>                    # stop the instance
curl http://127.0.0.1:9090/api/commands  # API still works (vrunner only)
```

## 3.10 Multi-Instance Management

> **Shared feature** — Both vrl and vrunner support multiple instances. HTTP-specific features apply to vrunner only.

Multiple instances can run simultaneously:

```bash
# Dev instance — vrl (local UDS, no port needed)
vrl -- daemon

# Staging — vrunner on port 9090 with TLS
vrunner --port 9090 --tls -- daemon

# Production — vrunner on port 443
vrunner --port 443 --tls --remote -- daemon
```

```bash
# List all instances with their commands
vrl list

# Stop a specific instance
vrl stop 12345

# Each instance can use a different config
vrl -c ./configs/dev.yaml -- daemon
vrunner -c ./configs/prod.yaml --port 443 -- daemon
```

## 3.11 Certificate Management — vrunner only

> **vrunner only** — Certificates provide per-command access control for the HTTP API. Not applicable to vrl.

Certificates provide per-command access isolation. Each certificate in the pool can be bound to running commands, ensuring only clients with the correct bearer token can interact with them.

```bash
# Generate a certificate
vrunner cert generate my-application

# List all certificates
vrunner cert list

# Show certificate details including bearer token
vrunner cert show my-application

# Remove a certificate
vrunner cert remove my-application

# Use the token in API requests
TOKEN=$(vrunner cert show my-application | grep -oP 'Token:\s*\K\S+')
curl -H "Authorization: Bearer $TOKEN" http://localhost:9090/api/commands/$ID/vtty

# Bind a command to a certificate at spawn time
curl -X POST http://localhost:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "node", "args": ["server.js"], "certificate": "my-application"}'
```

For the complete certificate management guide, see [docs/certificates.md](docs/certificates.md).

## 3.12 Exit Handlers and Timeouts

> **Shared feature** — Both vrl and vrunner support exit handlers via CLI. API-based exit handlers are **vrunner only**.

Exit handlers run external commands when a child process exits:

```bash
# Via CLI
vrl --on-exit "notify-send Done" --on-error "notify-send Error" --exit-timeout 20 -- cargo test

# Via API (vrunner only)
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "cargo", "args": ["test"], "on_exit": "notify-send OK", "on_error": "notify-send FAIL", "exit_timeout": 30}'
```

### Graceful Shutdown Sequence

When a command is killed via the API or CLI:
1. **SIGINT** is sent (giving the process a chance to exit cleanly)
2. Wait up to `exit_timeout` seconds (default: 10)
3. If still running, **SIGKILL** is sent

This two-phase approach prevents data corruption in processes that need to flush buffers or close connections.

## 3.13 Snapshots and Diffs — vrunner only

> **vrunner only** — API-based snapshot and diff operations require the HTTP server.

Store named VTTY buffer snapshots and compute cell-level diffs for testing and debugging:

```bash
ID="550e8400-e29b-41d4-a716-446655440000"

# Store a snapshot
curl -X POST http://127.0.0.1:9090/api/commands/$ID/snapshot \
  -H "Content-Type: application/json" \
  -d '{"name": "after-build"}'

# List snapshots
curl http://127.0.0.1:9090/api/commands/$ID/snapshots

# Compute diff against a snapshot
curl -X POST http://127.0.0.1:9090/api/commands/$ID/diff \
  -H "Content-Type: application/json" \
  -d '{"name": "after-build"}'

# Delete a snapshot
curl -X DELETE http://127.0.0.1:9090/api/commands/$ID/snapshots/after-build
```

## 3.14 WebSocket Protocol — vrunner only

> **vrunner only** — WebSocket endpoints are part of the HTTP server and not available in vrl.

vrunner provides two WebSocket endpoints for real-time streaming:

### VTTY WebSocket — `ws://host:port/api/commands/:id/ws`

Bidirectional connection for terminal output and keyboard input.

### Log WebSocket — `ws://host:port/api/ws/logs`

Read-only stream of command log entries.

### Connection Examples

```javascript
// VTTY WebSocket
const ws = new WebSocket('ws://127.0.0.1:9090/api/commands/550e8400.../ws');

ws.onmessage = (e) => {
  const msg = JSON.parse(e.data);
  switch (msg.type) {
    case 'connected': break;
    case 'vtty_full': renderTerminal(msg.data.html); break;
    case 'vtty_diff': applyDiff(msg.data); break;
    case 'command_ended': ws.close(); break;
    case 'error': console.error(msg.message); break;
  }
};

// Send keystrokes
ws.send(JSON.stringify({ type: 'keys', keys: 'ls -la\r' }));

// Resize terminal
ws.send(JSON.stringify({ type: 'resize', rows: 40, cols: 120 }));

// Keepalive
ws.send(JSON.stringify({ type: 'ping' }));
```

### TLS and Auth

```javascript
// With TLS
const ws = new WebSocket('wss://127.0.0.1:9090/api/commands/.../ws');

// With auth (token as query parameter)
const ws = new WebSocket('wss://host:9090/api/commands/.../ws?token=YOUR_TOKEN');
```

For the complete WebSocket message specification, see [docs/websocket.md](docs/websocket.md).

## 3.15 Incremental Diff Protocol — vrunner only

> **vrunner only** — The incremental diff protocol is used by the VTTY WebSocket, which is part of the HTTP server.

The VTTY WebSocket uses an incremental diff protocol to minimize bandwidth. Instead of sending the full terminal HTML on every update, only changed cells are transmitted.

### Phase 1 — Initial Snapshot

On connection, the server sends `vtty_full` with the complete terminal state:
```json
{
  "type": "vtty_full",
  "data": {
    "id": "550e8400-...",
    "html": "<span>...</span>",
    "cursor": {"row": 5, "col": 12},
    "dimensions": {"rows": 24, "cols": 80},
    "alternate_screen": false
  }
}
```

### Phase 2 — Incremental Diffs

The server polls each VTTY every 200ms, computes a cell-level diff (character, RGB colors, attributes), and sends only the changed cells:
```json
{
  "type": "vtty_diff",
  "data": {
    "id": "550e8400-...",
    "diff": {
      "width": 80, "height": 24, "changed_count": 3,
      "cells": [
        {"row": 5, "col": 10, "ch": "A", "fg": [255,255,255], "bg": [0,0,0], "bold": false}
      ]
    },
    "cursor": {"row": 5, "col": 13}
  }
}
```

### Phase 3 — Resynchronization

If the client falls behind (broadcast lag), the server automatically sends a new `vtty_full` to resynchronize.

## 3.16 Hooks

> **Shared feature** — Both vrl and vrunner support hooks.

Hooks are commands that run at specific points in the command lifecycle. They are configured in the config file under the `hooks` section. For details, see [docs/hooks.md](docs/hooks.md).

---

# Part IV — API Reference (vrunner only)

> **vrunner only** — All REST API endpoints require the HTTP server. vrl uses UDS IPC and does not expose these endpoints.

## 4.1 REST API Overview

All API endpoints are prefixed with `/api/`. Responses use a standard JSON envelope:

```json
{"status": "ok", "data": {...}, "error": null}
{"status": "error", "data": null, "error": "Description of the error"}
```

When authentication is enabled, include the header: `Authorization: Bearer <token>`.

## 4.2 Command Endpoints

### `GET /api/commands`

List all running commands.

```bash
curl http://127.0.0.1:9090/api/commands
```

Response:
```json
{
  "status": "ok",
  "data": [
    {
      "id": "550e8400-...",
      "name": "htop",
      "args": [],
      "pid": 12345,
      "alive": true,
      "runtime_secs": 42.5,
      "exit_code": null,
      "exit_time_secs": null,
      "status": "running",
      "certificate": null,
      "exit": {"on_exit": "", "on_error": "", "exit_timeout": 10, "retain_on_exit": false, "snapshot_on_exit": null}
    }
  ],
  "error": null
}
```

### `GET /api/commands/lookup/:name`

Look up commands by name. Matches both the full path and the basename.

```bash
curl http://127.0.0.1:9090/api/commands/lookup/htop
```

Response:
```json
{
  "status": "ok",
  "data": [
    {"id": "550e8400-...", "name": "htop", "args": [], "pid": 12345, "alive": true, "runtime_secs": 42.5, "certificate": null}
  ],
  "error": null
}
```

### `POST /api/commands`

Start a new command.

```bash
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "cmd": "htop",
    "args": [],
    "rows": 24,
    "cols": 80,
    "env": {"RUST_LOG": "debug"},
    "no_env": false,
    "on_exit": null,
    "on_error": null,
    "exit_timeout": 10,
    "certificate": null,
    "retain_on_exit": false,
    "snapshot_on_exit": null,
    "profile": null
  }'
```

All fields except `cmd` are optional. Returns the command ID.

Response:
```json
{"status": "ok", "data": {"id": "550e8400-e29b-41d4-a716-446655440000"}, "error": null}
```

### `POST /api/commands/:id/keys`

Send keystrokes to a command.

```bash
curl -X POST http://127.0.0.1:9090/api/commands/$ID/keys \
  -H "Content-Type: application/json" \
  -d '{"keys": "q"}'
```

Response:
```json
{"status": "ok", "data": {"id": "550e8400-...", "sent": 1}, "error": null}
```

### `POST /api/commands/:id/kill`

Kill a running command. Sends SIGINT first, then SIGKILL after `exit_timeout` seconds.

```bash
curl -X POST http://127.0.0.1:9090/api/commands/$ID/kill \
  -H "Content-Type: application/json" \
  -d '{"signal": "SIGTERM"}'
```

Response:
```json
{"status": "ok", "data": {"id": "550e8400-..."}, "error": null}
```

### `POST /api/commands/:id/freeze`

Freeze (SIGSTOP) a running command.

```bash
curl -X POST http://127.0.0.1:9090/api/commands/$ID/freeze
```

Response:
```json
{"status": "ok", "data": {"id": "550e8400-...", "frozen": true}, "error": null}
```

### `POST /api/commands/:id/thaw`

Thaw (SIGCONT) a frozen command.

```bash
curl -X POST http://127.0.0.1:9090/api/commands/$ID/thaw
```

Response:
```json
{"status": "ok", "data": {"id": "550e8400-...", "frozen": false}, "error": null}
```

### `POST /api/commands/:id/resize`

Resize a command's virtual terminal. The child process receives a `SIGWINCH` signal.

```bash
curl -X POST http://127.0.0.1:9090/api/commands/$ID/resize \
  -H "Content-Type: application/json" \
  -d '{"rows": 50, "cols": 160}'
```

Response:
```json
{"status": "ok", "data": {"id": "550e8400-...", "rows": 50, "cols": 160}, "error": null}
```

Valid ranges: rows 1-200, cols 1-500.

### `POST /api/commands/kill-pid/:pid`

Kill a command by its OS process ID.

```bash
curl -X POST http://127.0.0.1:9090/api/commands/kill-pid/12345
```

Response:
```json
{"status": "ok", "data": {"pid": 12345}, "error": null}
```

### `DELETE /api/commands/:id`

Purge a retained (exited) command from the manager. Permanently discards the VTTY buffer and all associated state.

```bash
curl -X DELETE http://127.0.0.1:9090/api/commands/$ID
```

Response:
```json
{"status": "ok", "data": {"id": "550e8400-...", "purged": true}, "error": null}
```

## 4.3 VTTY Endpoints

### `GET /api/commands/:id/vtty`

Get full VTTY contents as raw ANSI text.

```bash
curl http://127.0.0.1:9090/api/commands/$ID/vtty
```

Response:
```json
{"status": "ok", "data": {"id": "550e8400-...", "content": "\x1b[1mhello\x1b[0m\r\nworld"}, "error": null}
```

### `GET /api/commands/:id/vtty/html`

Get VTTY contents as rendered HTML. Supports optional `scrollback_offset` query parameter for browsing history.

```bash
curl http://127.0.0.1:9090/api/commands/$ID/vtty/html
curl "http://127.0.0.1:9090/api/commands/$ID/vtty/html?scrollback_offset=10"
```

Response:
```json
{
  "status": "ok",
  "data": {
    "id": "550e8400-...",
    "html": "<span style=\"...\">...</span>",
    "cursor": {"row": 5, "col": 12},
    "dimensions": {"rows": 24, "cols": 80},
    "scrollback_lines": 142,
    "scrollback_offset": 0,
    "alternate_screen": false,
    "mouse_tracking": false,
    "mouse_sgr": false
  },
  "error": null
}
```

### `GET /api/commands/:id/vtty/buffer?screen=current|main|alt`

Get VTTY buffer as HTML. The `screen` query parameter selects which buffer to render: `current` (auto-detects main or alternate), `main` (primary buffer), or `alt` (alternate screen buffer).

```bash
curl http://127.0.0.1:9090/api/commands/$ID/vtty/buffer
curl "http://127.0.0.1:9090/api/commands/$ID/vtty/buffer?screen=alt"
```

Response:
```json
{
  "status": "ok",
  "data": {
    "id": "550e8400-...",
    "screen": "current",
    "html": "<span>...</span>",
    "alternate_screen": false,
    "dimensions": {"rows": 24, "cols": 80}
  },
  "error": null
}
```

### `GET /api/commands/:id/vtty/changed`

Check if a VTTY has changed since the last poll. Used by clients implementing efficient polling to skip unnecessary full re-renders.

```bash
curl http://127.0.0.1:9090/api/commands/$ID/vtty/changed
```

Response:
```json
{
  "status": "ok",
  "data": {"id": "550e8400-...", "changed": true},
  "error": null
}
```

### `GET /api/commands/:id/vtty/partial?offset=N&limit=N`

Get paginated plain-text VTTY content.

```bash
curl "http://127.0.0.1:9090/api/commands/$ID/vtty/partial?offset=0&limit=50"
```

Response:
```json
{"status": "ok", "data": {"id": "550e8400-...", "offset": 0, "limit": 50, "content": "line 1\nline 2\n..."}, "error": null}
```

## 4.4 Mouse Endpoints

### `POST /api/commands/:id/mouse`

Forward a mouse event to a command. The event is encoded as an SGR sequence and written to the child PTY.

```bash
curl -X POST http://127.0.0.1:9090/api/commands/$ID/mouse \
  -H "Content-Type: application/json" \
  -d '{"event": "press", "button": 0, "x": 20, "y": 10}'
```

| Field | Type | Description |
|-------|------|-------------|
| `event` | string | Event type: `press`, `release`, `motion`, `wheel_up`, `wheel_down` |
| `button` | number | Button code: 0=left, 1=middle, 2=right |
| `x` | number | Column position (1-based) |
| `y` | number | Row position (1-based) |

Response:
```json
{"status": "ok", "data": {"id": "550e8400-...", "forwarded": true}, "error": null}
```

## 4.5 Snapshot Endpoints

### `POST /api/commands/:id/snapshot`

Store a named VTTY buffer snapshot.

```bash
curl -X POST http://127.0.0.1:9090/api/commands/$ID/snapshot \
  -H "Content-Type: application/json" \
  -d '{"name": "after-build"}'
```

Response:
```json
{
  "status": "ok",
  "data": {
    "id": "550e8400-...",
    "name": "after-build",
    "command_name": "cargo",
    "command_args": ["test"],
    "pid": 12345,
    "rows": 24,
    "cols": 80,
    "timestamp": 1700000000
  },
  "error": null
}
```

### `GET /api/commands/:id/snapshots`

List all snapshots for a command.

```bash
curl http://127.0.0.1:9090/api/commands/$ID/snapshots
```

Response:
```json
{
  "status": "ok",
  "data": [
    {"name": "after-build", "command_name": "cargo", "pid": 12345, "rows": 24, "cols": 80, "timestamp": 1700000000}
  ],
  "error": null
}
```

### `POST /api/commands/:id/diff`

Compute cell-level diff against a stored snapshot.

```bash
curl -X POST http://127.0.0.1:9090/api/commands/$ID/diff \
  -H "Content-Type: application/json" \
  -d '{"name": "after-build"}'
```

Response:
```json
{
  "status": "ok",
  "data": {
    "id": "550e8400-...",
    "name": "after-build",
    "width": 80,
    "height": 24,
    "changed_count": 3,
    "cells": [
      {"row": 5, "col": 10, "ch": "A", "fg": [255,255,255], "bg": [0,0,0], "bold": false}
    ]
  },
  "error": null
}
```

### `DELETE /api/commands/:id/snapshots/:name`

Delete a stored snapshot.

```bash
curl -X DELETE http://127.0.0.1:9090/api/commands/$ID/snapshots/after-build
```

Response:
```json
{"status": "ok", "data": {"id": "550e8400-...", "name": "after-build"}, "error": null}
```

## 4.6 Instance Endpoints

### `GET /api/info`

Get instance info (command count, certificate count, auth status).

```bash
curl http://127.0.0.1:9090/api/info
```

Response:
```json
{
  "status": "ok",
  "data": {
    "command_count": 3,
    "certificate_count": 1,
    "certificates": ["frontend-team"],
    "auth_enabled": true,
    "web": {"update_mode": "push", "dirty_check_ms": 200, "default_poll_ms": 500}
  },
  "error": null
}
```

### `POST /api/shutdown`

Gracefully shut down the vrunner instance. Drains connections for 2 seconds, then terminates.

```bash
curl -X POST http://127.0.0.1:9090/api/shutdown
```

Response:
```json
{"status": "ok", "data": {"shutting_down": true}, "error": null}
```

### `GET /api/commands/:id/ws` (WebSocket)

Upgrade to a WebSocket for real-time VTTY streaming. See [Section 3.14](#314-websocket-protocol--vrunner-only) for the full protocol specification.

## 4.7 Certificate Endpoints

### `GET /api/certificates`

List all certificates in the pool. Only the first 16 characters of each derived token are returned.

```bash
curl http://127.0.0.1:9090/api/certificates
```

Response:
```json
{
  "status": "ok",
  "data": [
    {"name": "frontend-team", "cert_file": "...", "key_file": "...", "token_preview": "a1b2c3d4e5f6g7h8"}
  ],
  "error": null
}
```

## 4.8 Log Endpoints

### `GET /api/log`

Get command log entries with optional search and pagination.

```bash
curl "http://127.0.0.1:9090/api/log?search=spawn&offset=0&limit=50"
```

Response:
```json
{
  "status": "ok",
  "data": [
    {"timestamp": "2024-01-15T10:30:00Z", "action": "spawn", "command": "cargo test", "pid": 12345}
  ],
  "error": null
}
```

### `GET /api/ws/logs` (WebSocket)

Upgrade to a WebSocket for real-time log streaming.

## 4.9 Handle Endpoints

### `GET /api/commands/:id/handles`

List output handles for a command.

```bash
curl http://127.0.0.1:9090/api/commands/$ID/handles
```

### `POST /api/commands/:id/handles`

Add an output handle to a command.

```bash
curl -X POST http://127.0.0.1:9090/api/commands/$ID/handles \
  -H "Content-Type: application/json" \
  -d '{"name": "stdout", "type": "file", "path": "/tmp/output.log"}'
```

---

# Part V — Security (vrunner only)

> **vrunner only** — Security features (authentication, TLS, CORS, certificates) are part of the HTTP server. vrl uses local UDS and does not need these protections.

## 5.1 Authentication

By default, vrunner binds to `127.0.0.1` with no authentication. This is safe for local use because any process that can reach localhost already has shell access.

### Enabling Auth

```bash
# Explicit
vrunner --auth -- my-command

# Implicit (via --remote)
vrunner --remote -- my-command
```

When auth is enabled, a 256-bit random token is generated and saved to `~/.config/vrl/token` (with 0600 permissions). Include it in all API requests:

```bash
TOKEN=$(cat ~/.config/vrl/token)
curl -H "Authorization: Bearer $TOKEN" http://localhost:9090/api/commands
```

### WebSocket Auth

Pass the token as a query parameter:
```javascript
const ws = new WebSocket('wss://host:9090/api/commands/.../ws?token=YOUR_TOKEN');
```

## 5.2 TLS Encryption

### Self-Signed Certificates

```bash
vrunner --tls -- my-command
```

Certificates are auto-generated on first use and saved to `~/.config/vrl/` (cert.pem + key.pem @ 0600 permissions). The certificate includes SAN entries for `localhost`, `127.0.0.1`, and `::1`.

### Custom Certificates

```bash
vrunner --tls --cert-file /etc/ssl/certs/vrl.crt --key-file /etc/ssl/private/vrl.key -- my-command
```

### Connecting with TLS

```bash
# With curl
curl --cacert ~/.config/vrl/cert.pem https://localhost:9090/api/commands

# Skip verification (not recommended)
curl -k https://localhost:9090/api/commands
```

## 5.3 CORS Policy

vrunner uses `tower-http` CORS middleware to control cross-origin access to the API and admin interface. The CORS policy is configurable through the `security.cors` field in the configuration file.

### Configuration

```yaml
security:
  cors:
    policy: "any"    # default: allow all origins
```

### Policy Values

| Value | Behavior |
|-------|----------|
| `"any"` | Allow all origins. Sets `Access-Control-Allow-Origin: *`. This is the default for backward compatibility. Suitable for local development where the browser and API are on the same machine. |
| `"none"` | Block all cross-origin requests. No `Access-Control-Allow-Origin` header is set. The admin interface still works when served from the same origin. |
| Comma-separated origins | Allow only the listed origins. Example: `"https://myapp.example.com,https://admin.example.com"`. Each origin must include the scheme (`http` or `https`). |

### Examples

Allow all origins (default):
```yaml
security:
  cors:
    policy: "any"
```

Block cross-origin requests:
```yaml
security:
  cors:
    policy: "none"
```

Allow specific origins:
```yaml
security:
  cors:
    policy: "https://dashboard.example.com,https://ci.example.com"
```

### CORS and Authentication

When both authentication and CORS are enabled, the `Authorization` header is exposed to allowed origins via `Access-Control-Expose-Headers`. If you restrict CORS to specific origins, ensure those origins are listed in the policy so that the browser can read the auth headers from API responses.

### Recommendation

For production deployments where the admin interface is accessed from a different origin than the API server, set the CORS policy to the specific origin(s) that need access. For localhost development, the default `"any"` policy is appropriate.

## 5.4 Token Management

- Tokens are 256-bit random values generated with `rand` crate
- Stored in `~/.config/vrl/token` with 0600 permissions
- Reused across restarts (only regenerated if the file is deleted)
- Certificate-derived tokens are computed as SHA-256 of the certificate PEM, hex-encoded

## 5.5 Certificate-Based Access Control

Certificates in the pool can be bound to individual commands. When a command is certificate-bound, only API requests bearing the matching derived token can interact with it.

```bash
# Generate a certificate
vrunner cert generate my-app

# Spawn a command bound to that certificate
curl -X POST http://localhost:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "node", "args": ["server.js"], "certificate": "my-app"}'

# Only requests with the certificate's token can access this command
TOKEN=$(vrunner cert show my-app | grep -oP 'Token:\s*\K\S+')
curl -H "Authorization: Bearer $TOKEN" http://localhost:9090/api/commands/$ID/vtty
```

## 5.6 Security Best Practices

- **Local use** — No auth needed; localhost binding is safe
- **Remote use** — Always use `--remote` (enables auth) and `--tls`
- **Token security** — Protect `~/.config/vrl/token` like a password; use file permissions (0600)
- **Certificate management** — Delete unused certificates; rotate certificate tokens periodically
- **TLS certificates** — Use CA-signed certificates (e.g., Let's Encrypt) for production; self-signed only for development
- **Daemon mode** — Ensure log files are in secure directories; do not expose stdout/stderr files
- **Network exposure** — Only bind to `0.0.0.0` when remote access is needed; use a firewall to restrict access
- **No sandboxing** — Child processes are not sandboxed; only run trusted commands

---

# Part VI — For Contributors

## 6.1 Building from Source

```bash
# Clone
git clone https://github.com/nkh/K.git
cd K

# Build both binaries
cargo build --release

# Build vrl only
cargo build --release --bin vrl

# Build vrunner only
cargo build --release --bin vrunner

# Run tests
cargo test

# Run clippy
cargo clippy

# Check formatting
cargo fmt --check
```

## 6.2 Code Organization

```
src/
├── main.rs              # Binary entry point, CLI parsing, async runtime
├── lib.rs               # Library crate root (vrl_core)
├── cli/                 # CLI argument parsing and subcommands
│   ├── args.rs          # clap derive structs (Cli, Commands)
│   └── subcommands.rs   # Subcommand handlers (list, stop, spawn, etc.)
├── config/              # Configuration loading and schema
│   ├── schema.rs        # Typed config structs (serde)
│   ├── loader.rs        # Multi-source config discovery
│   ├── merge.rs         # Precedence-based override logic
│   └── profiles.rs      # Named profile resolution
├── process/             # Process management
│   ├── manager.rs       # CommandManager (DashMap of CommandHandles)
│   ├── spawner.rs       # PTY creation, mpsc channel bridge
│   └── handle.rs        # Per-command state and lifecycle
├── vtty/                # Virtual terminal emulator
│   ├── emulator.rs      # Terminal state machine (CSI, OSC, DCS)
│   ├── parser.rs        # Streaming ANSI parser
│   ├── buffer.rs        # 2D cell grid with scrollback and diff
│   ├── renderer.rs      # ANSI, HTML, and plain-text serialization
│   └── display.rs       # Local terminal rendering via crossterm
├── web/                 # HTTP server and admin UI (vrunner only)
│   ├── server.rs        # TCP/TLS binding, graceful shutdown
│   ├── router.rs        # Axum route table
│   ├── state.rs         # AppState (shared state)
│   ├── auth.rs          # Bearer token authentication
│   ├── tls.rs           # TLS certificate management
│   ├── middleware.rs     # CORS, auth, logging middleware
│   ├── static_assets.rs # rust_embed admin UI
│   ├── certs.rs         # Certificate pool (named certs + derived tokens)
│   └── handlers/        # API handler modules
├── interactive/         # Interactive terminal display
│   ├── display.rs       # Display loop, overlays, context menu
│   ├── actions.rs       # Display actions
│   └── keybinding.rs    # Key name parser
├── daemon/              # Unix daemonization (double-fork)
├── instance/            # Instance registry (pidfiles)
├── handles/             # Extensible file descriptor routing
└── logging/             # Command logger

static/admin/            # Embedded admin SPA (HTML/CSS/JS, vrunner only)
docs/                    # Documentation
man/                     # Unix man pages
examples/                # Example configuration files
tests/                   # Unit and integration tests
```

## 6.3 Testing

```bash
# Run all tests (246 unit + 4 integration + 2 doc)
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test integration_test

# Run VTTY regression tests
cd tests/vtty && bash run_tests.sh
```

## 6.4 Architecture Decision Records

### ADR-001: Broadcast over Notify for VTTY events

**Decision**: Use `tokio::sync::broadcast` channels for VTTY change notifications.

**Rationale**: `broadcast` allows multiple WebSocket connections to receive the same VTTY updates without each needing its own polling loop. When all receivers are dropped (no WebSocket clients), the sender does not block. `notify` was considered but rejected because it loses signals if no waiter is present at the time of notification.

### ADR-002: AsyncFd over stdin for PTY reads

**Decision**: Use a blocking thread with `mpsc::channel` to bridge synchronous PTY reads to the async runtime.

**Rationale**: `portable-pty` provides synchronous `Read`/`Write` operations. Rather than using `tokio::fs::File` or `AsyncFd` (which has platform-specific behavior), a dedicated blocking thread reads from the PTY and sends chunks through a bounded `mpsc::channel`. A single async receiver task feeds data to the VTTY emulator. This approach is simple, portable, and avoids complexity.

### ADR-003: std::process::exit for shutdown

**Decision**: Use `std::process::exit(0)` for final shutdown after graceful cleanup.

**Rationale**: After the web server shuts down gracefully (draining connections, closing listeners), there is no further work to do. Using `std::process::exit` bypasses any remaining drop code and ensures immediate process termination. The tokio runtime is already shut down at this point, so normal cleanup is complete.

### ADR-004: rust_embed for admin UI assets

**Decision**: Embed the admin SPA using `rust_embed` rather than serving static files from disk.

**Rationale**: Embedding assets produces a single binary with zero external dependencies. The admin UI is always available at `/admin` without any file serving configuration. `rust_embed` is lightweight, has no runtime dependencies, and supports hot-reloading in development mode.

### ADR-005: watch::channel for exit notification

**Decision**: Use `tokio::sync::watch` (never loses notifications) instead of `tokio::sync::Notify` (can lose signals).

**Rationale**: When a child process exits, the process waiter thread must reliably signal the main loop. `Notify::notify()` is a no-op if no one is waiting, which means the signal could be lost if the main loop is between poll cycles. `watch::Sender::send()` always updates the value, so any subsequent `watch::Receiver::changed()` will detect it.

---

# Appendices

## A. Comparison with Alternatives

| Feature | vrl / vrunner | tmux | screen | mprocs | gotty | wetty |
|---------|---------|------|--------|--------|-------|-------|
| Web dashboard | Embedded SPA (vrunner) | No | No | Web UI | Web UI | Web UI |
| REST API | 30+ endpoints (vrunner) | No | No | No | Limited | Limited |
| WebSocket streaming | Incremental diff (vrunner) | No | No | No | Yes | Yes |
| Per-command auth | Certificate pool (vrunner) | No | No | No | No | SSH |
| TLS | Built-in (vrunner) | No | No | No | Built-in | SSH |
| Daemon mode | Double-fork | Server mode | Detach | No | No | No |
| Multi-instance | Yes | Yes | Yes | No | No | No |
| Language | Rust | C | C | Go | Go | Node.js |
| Terminal emulation | Full VTTY | Full PTY | Full PTY | PTY | PTY | PTY |
| Configuration | YAML/TOML/JSON | .tmux.conf | .screenrc | CLI only | CLI only | CLI only |
| Mouse support | Full | Yes | Limited | No | No | Yes |
| Snapshots/diffs | Yes (vrunner) | No | No | No | No | No |
| CI/CD integration | API-first (vrunner) | CLI-only | CLI-only | CLI-only | CLI-only | CLI-only |
| Local UDS mode | Yes (vrl) | No | No | No | No | No |

**When to choose vrl over alternatives:**

- You need a **local-first** terminal runner with fast startup and no HTTP overhead
- You want **UDS-based IPC** for local-only process management

**When to choose vrunner over alternatives:**

- You need a **web API** to programmatically manage terminal processes
- You want a **single binary** with no external dependencies (not even Node.js)
- You need **per-command access control** via certificates
- You want **real-time monitoring** from a browser without installing anything on the client
- You need **CI/CD integration** where a script starts a command and another monitors it

## B. Troubleshooting

### vrl / vrunner won't start

**Port already in use (vrunner only):**
```bash
lsof -i :9090
vrunner --port 9091
```

### Connection refused (vrunner only)

**Instance not running:**
```bash
vrl list          # Check if any instances are running
vrunner --bind 0.0.0.0  # If remote connections needed
```

### TLS certificate errors (vrunner only)

**Self-signed cert not trusted:**
```bash
curl --cacert ~/.config/vrl/cert.pem https://localhost:9090/api/commands
```

### Authentication failures (vrunner only)

**Missing token in requests:**
```bash
TOKEN=$(cat ~/.config/vrl/token)
curl -H "Authorization: Bearer $TOKEN" http://localhost:9090/api/commands
```

### VTTY output appears empty

**Output in scrollback or command hasn't started:**
```bash
# Check scrollback (vrunner only)
curl http://localhost:9090/api/commands/$ID/vtty/html | jq '.data.scrollback_lines'
# Check command status (vrunner only)
curl http://localhost:9090/api/commands | jq '.data[].status'
```

### WebSocket disconnections (vrunner only)

**Auto-reconnect is built into the admin UI.** If using a custom client, implement reconnection logic with exponential backoff. The server sends a `pong` response to `ping` messages for keepalive.

### PTY allocation failures

**System limit reached:** Increase the max PTY count:
```bash
# Check current limit
sysctl kernel.pty.max
# Increase (temporary)
sudo sysctl -w kernel.pty.max=4096
```

### Build failures

**Disk space:** A full debug build requires approximately 2GB. Clean old artifacts:
```bash
cargo clean
cargo build --release
```

## C. Use Cases

### Development Server Orchestration

```bash
# Start vrunner in idle mode
vrunner --daemon --log --log-file /tmp/vrl.log

# Spawn services
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "npm", "args": ["run", "dev:frontend"]}'
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "cargo", "args": ["run"]}'
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "npm", "args": ["run", "watch:db"]}'

# Monitor all three at http://127.0.0.1:9090/admin
```

### CI/CD Pipeline Monitoring

```bash
# Start secure vrunner instance on CI server
vrunner --remote --tls --port 8080 --daemon

# CI script spawns build
TOKEN=$(cat ~/.config/vrl/token)
JOB_ID=$(curl -s -X POST https://localhost:8080/api/commands \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"cmd": "./run-tests.sh", "args": ["--verbose"]}' | jq -r '.data.id')

# Poll for completion
while true; do
  STATUS=$(curl -s -H "Authorization: Bearer $TOKEN" \
    "https://localhost:8080/api/commands" \
    | jq -r ".data[] | select(.id == \"$JOB_ID\") | .status")
  [ "$STATUS" != "running" ] && break
  sleep 5
done

# Retrieve build output
curl -s -H "Authorization: Bearer $TOKEN" \
  "https://localhost:8080/api/commands/$JOB_ID/vtty" | jq -r '.data.content'
```

### Remote Server Administration

```bash
# On the remote server
vrunner --remote --tls --port 443 --daemon

# From your local machine — run interactive commands
curl --cacert /path/to/cert.pem \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -X POST https://remote.example.com/api/commands \
  -d '{"cmd": "htop"}'
```

### Pair Programming

```bash
# Developer 1 starts the server
vrunner --port 8080 --daemon

# Developer 1 starts a shared session
SHARED_ID=$(curl -s -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "vim", "args": ["notes.txt"]}' | jq -r '.data.id')

# Developer 2 opens http://server:8080/admin and views the shared session
# Both developers can type and see each other's input in real time
```

### Long-Running Background Tasks

```bash
# Start vrunner in daemon mode
vrunner --daemon

# Run a long job
JOB_ID=$(curl -s -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "python", "args": ["process_data.py", "--input", "data.csv"]}' \
  | jq -r '.data.id')

# Disconnect — the job keeps running
# Reconnect later and check progress
curl "http://127.0.0.1:9090/api/commands/$JOB_ID/vtty/partial?offset=0&limit=20"
```

### Local Development with vrl

```bash
# Start vrl with a local-only UDS connection
vrl --display-all -- cargo test

# The VTTY output is mirrored to your terminal
# No HTTP server is started — fast startup, minimal overhead
```

## D. Cookbook

See [docs/cookbook/](docs/cookbook/) for step-by-step recipes:

- [Run a dev server with hot reload](docs/cookbook/dev-server.md)
- [Monitor multiple services](docs/cookbook/multi-service.md)
- [CI pipeline with vrunner](docs/cookbook/ci-pipeline.md)
- [Pair programming setup](docs/cookbook/pair-programming.md)
- [Remote access via TLS](docs/cookbook/remote-tls.md)

## E. Video Storyboard

See [docs/storyboard.md](docs/storyboard.md) for the vrl / vrunner introduction video storyboard.

## F. Version Upgrade Guide

vrl follows semantic versioning. Upgrades are generally safe, but review these notes for breaking changes.

### Upgrading from Source

```bash
cd K  # your clone directory
git pull origin main
cargo build --release
```

If you installed system-wide:
```bash
cargo install --bin vrl --path .
cargo install --bin vrunner --path .
```

### Upgrade Checklist

1. **Stop all running instances** before upgrading:
   ```bash
   vrl list          # find all PIDs
   vrl stop <pid>    # stop each one
   ```

2. **Check for config changes**: Review the changelog for any configuration field renames or removals. If the config validation catches issues, run `vrl config-check` before starting.

3. **Reinstall man pages**: If the man pages changed, copy the new versions:
   ```bash
   cp man/vrl.1 /usr/local/share/man/man1/
   cp man/vrunner.1 /usr/local/share/man/man1/
   mandb  # rebuild the man page index
   ```

4. **Test with a simple command**: After upgrading, verify basic functionality:
   ```bash
   vrl -- echo "hello"
   vrunner -- echo "hello"
   ```

### Configuration Migration

If a config field was renamed between versions, a warning will be logged at startup. The old field name is still accepted but deprecated. Update your config files to use the new names:

```bash
# Validate your config without starting the server
vrl config-check -c ./my-config.yaml
```

### Breaking Changes

Breaking changes are documented in the commit history with the `BREAKING` prefix. As of the current version, there are no breaking changes from the initial release.

### Rollback

If you need to roll back to a previous version:
```bash
git checkout <previous-tag>
cargo build --release
```

---

*For the complete configuration reference, see [docs/configuration.md](docs/configuration.md).*
*For the technical architecture, see [docs/architecture.md](docs/architecture.md).*
*For the WebSocket protocol specification, see [docs/websocket.md](docs/websocket.md).*
