# vrc User Guide

A practical guide to using vrc for common tasks. This document covers the web administrative interface, CLI controller, and direct HTTP API access via curl and other tools.

---

## Table of Contents

1. [Concepts Overview](#concepts-overview)
2. [Getting Started](#getting-started)
3. [Running Commands](#running-commands)
   - [Running a Command on Startup](#running-a-command-on-startup)
   - [Spawning Commands via the Web UI](#spawning-commands-via-the-web-ui)
   - [Spawning Commands via curl](#spawning-commands-via-curl)
   - [Spawning Commands via the CLI](#spawning-commands-via-the-cli)
4. [Viewing Terminal Output](#viewing-terminal-output)
   - [Local VTTY Display](#local-vtty-display)
   - [Web Admin VTTY Viewer](#web-admin-vtty-viewer)
   - [VTTY API Endpoints](#vtty-api-endpoints)
   - [WebSocket Real-Time Streaming](#websocket-real-time-streaming)
   - [Incremental VTTY Diff Protocol](#incremental-vtty-diff-protocol)
5. [Sending Keystrokes](#sending-keystrokes)
6. [Managing Running Commands](#managing-running-commands)
   - [Listing Commands](#listing-commands)
   - [Killing Commands](#killing-commands)
   - [Kill by PID](#kill-by-pid)
   - [Resizing the Terminal](#resizing-the-terminal)
7. [Freezing and Thawing Commands](#freezing-and-thawing-commands)
8. [Snapshot and Diff](#snapshot-and-diff)
   - [Storing Snapshots](#storing-snapshots)
   - [Listing Snapshots](#listing-snapshots)
   - [Computing Diffs](#computing-diffs)
   - [Deleting Snapshots](#deleting-snapshots)
9. [Exit Handlers and Timeouts](#exit-handlers-and-timeouts)
   - [Per-Command Exit Handlers via API](#per-command-exit-handlers-via-api)
   - [Default Exit Handlers via Config and CLI](#default-exit-handlers-via-config-and-cli)
   - [Graceful Shutdown with Timeout](#graceful-shutdown-with-timeout)
10. [Viewing Logs](#viewing-logs)
   - [Real-Time Log Streaming via WebSocket](#real-time-log-streaming-via-websocket)
11. [Certificate-Based Access Control](#certificate-based-access-control)
12. [Remote Access and TLS](#remote-access-and-tls)
13. [Daemon Mode](#daemon-mode)
14. [Interactive Display](#interactive-display)
15. [Multi-Instance Management](#multi-instance-management)
16. [Configuration File Reference](#configuration-file-reference)
17. [Common Use Cases](#common-use-cases)
    - [Development Server Orchestration](#development-server-orchestration)
    - [CI/CD Pipeline Runner](#cicd-pipeline-runner)
    - [Remote Server Administration](#remote-server-administration)
    - [Pair Programming and Collaboration](#pair-programming-and-collaboration)
    - [Long-Running Background Tasks](#long-running-background-tasks)
18. [Troubleshooting](#troubleshooting)

---

## Concepts Overview

vrc is a process manager that runs commands inside virtual TTYs (VTTYs) and exposes them through a web API. Rather than wrapping processes directly, vrc creates pseudo-terminals, giving child processes full terminal capabilities including ANSI colors, cursor movement, and interactive keyboard input.

The key architectural concept is the separation between **starting a command** and **interacting with it**. A command can be started from the CLI, the web UI, or the API. Once running, it can be monitored and controlled from any of those interfaces interchangeably. This makes vrc suitable for scenarios where a command needs to be started from one place (like a CI script) and monitored from another (like a web dashboard).

vrc supports three controllers plus a real-time streaming layer:
- **CLI** — direct command-line invocation for starting, listing, and stopping instances
- **Web Admin** — a browser-based dashboard at `/` or `/admin` for managing commands visually. Supports direct command-name URLs like `/htop` to jump straight to a command's terminal.
- **HTTP API** — a RESTful API for programmatic access from scripts, curl, or custom clients
- **WebSocket API** — real-time bidirectional streaming for terminal output and log entries, eliminating the need for polling

All controllers communicate with the same vrc instance. The CLI subcommands (`list`, `stop`, `cert`) connect to running instances over HTTP to perform management operations. WebSocket connections upgrade from HTTP and provide push-based updates for lower latency.

---

## Getting Started

### Installation

Build from source using Cargo:

```bash
git clone https://github.com/yourusername/vrc.git
cd vrc
cargo build --release
# Binary is at target/release/vrc
```

Or install system-wide:

```bash
cargo install --path .
```

### First Run

Start vrc in its simplest form — idle mode on localhost with no command:

```bash
vrc
```

This starts an HTTP server on `http://127.0.0.1:9090`. No commands are running yet; the instance is ready to receive API requests or web UI connections. You can verify it is working by listing commands:

```bash
curl http://127.0.0.1:8080/api/commands
```

Response:
```json
{
  "status": "ok",
  "data": [],
  "error": null
}
```

### With a Command

Run a command immediately and start the web server alongside it:

```bash
vrc -- htop
```

vrc spawns `htop` inside a virtual TTY, starts the HTTP server, and waits. You can open the web admin at `http://127.0.0.1:8080/admin` to see htop's terminal output, or use curl to interact with it programmatically.

### Getting Help

vrc includes built-in help via clap:

```bash
vrc --help
vrc -h
vrc cert --help
vrc cert generate --help
```

These display all available options, subcommands, and their descriptions. The help text is the authoritative reference for CLI flags.

---

## Running Commands

### Running a Command on Startup

Use the `--` separator to pass a command to vrc at launch. Everything after `--` is treated as the child command and its arguments:

```bash
# Run a development server
vrc --port 3000 -- npm run dev

# Run a Python HTTP server with 80-column terminal
vrc --vtty-cols 80 -- python -m http.server 8000

# Run with local terminal display visible
vrc --display -- vim notes.txt

# Run in the background as a daemon
vrc --daemon -- my-long-running-script.sh

# Run with per-command exit options
vrc --retain-on-exit --snapshot-on-exit /tmp/build.log -- cargo build

# Send initial keystrokes to the command
vrc --send-keys "ls<Enter>" -- bash
```

The command runs inside a virtual TTY with full terminal capabilities. Programs like `vim`, `htop`, `ncurses` applications, and anything that reads from `/dev/tty` will work correctly.

### Spawning Commands via the Web UI

1. Open `http://127.0.0.1:8080/admin` in your browser.
2. Use the command spawn interface to enter a command and optional arguments.
3. The command starts inside a new VTTY, and its ID appears in the running commands list.
4. Click on the command to view its terminal output.

### Spawning Commands via curl

Use `POST /api/commands` to spawn a new command:

```bash
# Start a simple command
curl -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "htop", "args": []}'

# Start a command with arguments
curl -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "python", "args": ["-m", "http.server", "8000"]}'

# Start a command bound to a certificate
curl -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "node", "args": ["server.js"], "certificate": "my-app"}'

# Start a command with exit handlers and custom timeout
curl -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "cmd": "cargo",
    "args": ["test"],
    "on_exit": "notify-send Tests Passed",
    "on_error": "notify-send Tests Failed",
    "exit_timeout": 30
  }'
```

Response:
```json
{
  "status": "ok",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000"
  },
  "error": null
}
```

Save the returned `id` — you need it to interact with the command later.

#### Common curl Spawn Patterns

```bash
# Run a shell command pipeline
curl -s -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "bash", "args": ["-c", "for i in 1 2 3 4 5; do echo $i; sleep 1; done"]}'

# Run with a custom TERM
vrc --term xterm-256color --display -- tmux
```

### Spawning Commands via the CLI

The CLI can spawn commands in three ways:

1. **At startup** with `--`: `vrc -- htop`
2. **Dynamically** via `vrc spawn` to send commands to a running instance
3. **Via API** (curl, web UI, programmatic clients)

#### `vrc spawn` — Dynamic CLI Spawning

The `spawn` subcommand discovers running vrc instances and sends a spawn request to one of them:

```bash
# If exactly one instance is running, it is used automatically
vrc spawn htop

# With arguments
vrc spawn python -m http.server 8000

# With environment variables
vrc spawn --env RUST_LOG=debug --env DATABASE_URL=postgres://localhost/mydb -- cargo run

# Ignore config environment variables (--no-env)
vrc spawn --no-env --env PATH=/usr/bin -- ./my-script.sh

# Target a specific instance by PID
vrc spawn --target 12345 -- npm run dev

# With exit handlers
vrc spawn --on-exit "notify-send Done" --on-error "notify-send Failed" -- ./build.sh

# With a custom terminal size
vrc spawn --rows 50 --cols 160 -- vim notes.txt
```

When multiple vrc instances are running and no `--target` is specified, vrc prints a list of all instances and asks you to use `--target PID` to select one.

---

## Viewing Terminal Output

### Local VTTY Display

Enable real-time terminal output on your local console with `--display`:

```bash
vrc --display -- htop
```

This mirrors the VTTY contents to stdout at the refresh interval specified by `--refresh-ms` (default: 100ms). The display shows the raw ANSI output from the child process, including colors and cursor positioning. Press `Ctrl+C` in the terminal where vrc is running to stop the instance.

### Web Admin VTTY Viewer

The web admin interface is available at `/admin` (or any unrecognised path, which redirects to the dashboard). It is split into `index.html`, `style.css`, and `app.js` — all embedded in the binary at compile time with no external dependencies. It provides a full-featured terminal management dashboard with real-time VTTY streaming, command lifecycle controls, and several productivity features.

#### Direct Command URLs

Navigate directly to a command's terminal using its name: `http://localhost:8080/<command_name>`. For example, `/htop` opens the VTTY viewer for a command named `htop`. If multiple running commands share the same name, a picker list is displayed showing each instance with its arguments so you can choose the right one. Running commands are highlighted with their elapsed uptime.

#### Top Bar Layout

The top bar is organized into three button groups:

- **Left group** — Add Panel (spawn), Pause/Run toggle, Kill All
- **Center group** — Font size controls (A-/A+), resize to fit, alternate screen buffer selector
- **Right group** — Auth token input, documentation link, keyboard shortcuts (`?`), theme toggle (sun/moon)

The layout uses a consistent button sizing system: `btn-xs` (compact), `btn-sm` (small), `btn` (default), and `btn-primary`/`btn-danger` (color variants). The center group collapses on mobile viewports.

#### Dashboard Features

**Terminal Interaction:**

- **Real-time VTTY Viewer** — Streams terminal output via the incremental diff WebSocket protocol (`GET /api/commands/{id}/ws`). Falls back to 1-second HTTP polling if WebSocket is unavailable. Automatically selects the first running command on load.
- **Click to Focus** — Click anywhere on the terminal pane to immediately capture keyboard input for sending keystrokes.
- **Mouse Event Forwarding** — Clicks, drags, and wheel events are forwarded to the child process via `POST /api/commands/{id}/mouse` when mouse tracking is enabled by the child application.
- **Mouse Wheel Scrollback** — Scroll through command history; when the child application has mouse tracking enabled, wheel events are forwarded to it, otherwise they scroll the view.
- **Terminal Search** — Press `Ctrl+F` to open a search bar inside the terminal viewer. Matches are highlighted in the output buffer.
- **Scroll-to-Bottom** — When scrolled up, a floating button appears in the bottom-right corner of the terminal. Click it to jump back to live output.
- **Selection Mode** — Toggle with `Ctrl+Shift+S` or `Alt+S` to enable native text selection on the terminal. When active, mouse events are not forwarded to the PTY, allowing you to select and copy text. The panel shows an accent border as a visual indicator.
- **Copy to Clipboard** — `Ctrl+Shift+C` copies the selected terminal text to the clipboard. If no text is selected, the full VTTY buffer content is copied instead. A "Copied!" toast confirms the action.
- **Per-Panel Font Size** — Each panel has A-/A+ buttons in its header for independent font sizing (8–28px). The size is persisted to `localStorage` and restored on page load. The global font size buttons in the top bar set the default for new panels only.
- **Persistent Scrollback** — The scrollback offset is saved to `sessionStorage` when scrolling. Re-selecting a command restores the previous scroll position. Uses session storage to avoid stale data across sessions.
- **Auto-Fit Terminal** — The terminal automatically resizes to fill the available panel space when the browser window is resized.
- **Export Output** — Download the current terminal buffer contents as a `.txt` file via the toolbar or context menu.

**Command Management:**

- **Command Sidebar** — Lists all commands with name, arguments, PID, status, and runtime. Running commands show elapsed uptime and are visually highlighted. Use the search/filter box at the top to narrow the list by command name.
- **Batch Kill All** — A button in the top bar terminates every running command on the instance in one click.
- **Pause / Run** — Toggle freeze/thaw on the currently selected command from the top bar using SIGSTOP/SIGCONT.
- **Incremental DOM Updates** — The command list is polled every second, but DOM updates are skipped when the command state fingerprint has not changed. This reduces unnecessary DOM thrashing from polling.

**Context Menu and Accessibility:**

- **Context Menu (Sidebar)** — Right-click any command in the sidebar to access quick actions: kill, freeze/thaw, copy URL, open command in a new tab. The menu is built with `createElement` and `addEventListener` (no inline `onclick`), eliminating XSS injection vectors.
- **Context Menu (Panel Headers)** — Right-click panel headers (tab bar) for Copy URL, Pause/Resume, Kill, and Remove Panel actions.
- **Keyboard-Accessible Menu** — `Shift+F10` opens the context menu, arrow keys navigate items, Enter activates the focused item, and Escape closes the menu. `role=menu` and `role=menuitem` ARIA attributes are applied.
- **Copy Command URL** — Copy the direct URL for any command to the clipboard from the context menu or a button next to the command name.

**Focus and Keyboard:**

- **Focus Management** — Modals (Add Panel, Command Picker, Shortcuts Overlay, Terminal Search Bar) trap focus within their container using Tab/Shift+Tab wrapping. When a modal opens, focus moves to the first interactive element; when it closes, focus returns to the previously focused element.
- **Escape to Dismiss** — All modals and overlays are dismissed consistently with the Escape key.
- **Keyboard Shortcuts** — Press `?` to open the shortcuts help panel showing all available keybindings and their actions.

**Connection and Theming:**

- **Connection Quality Indicator** — The bottom bar displays WebSocket round-trip latency (measured via ping/pong every 10 seconds) with color coding: green (< 100ms), yellow (100–500ms), red (> 500ms). A tooltip shows the reconnect count. Latency resets on disconnect and reconnects are tracked across the connection lifecycle.
- **Auto-Reconnect** — WebSocket connections automatically re-establish after network interruptions or server restarts, with no manual refresh needed.
- **Light Theme** — Toggle between dark and light themes via the sun/moon button in the top bar. The light theme uses a GitHub-inspired palette. When no explicit choice has been made, the theme follows the operating system's `prefers-color-scheme` media query. The active theme is persisted to `localStorage`.
- **VTTY Theme-Aware** — The terminal background adapts to the active theme (dark or light) for a seamless visual experience.
- **Responsive Layout** — The dashboard adapts to mobile and tablet screen sizes. The sidebar collapses into a toggleable drawer on narrow viewports.
- **Browser Notifications** — When enabled (via browser permission prompt), a desktop notification is sent when a command exits.

**Real-Time Log Streaming:**

- **WebSocket Log Stream** — When the log viewer is active, the client connects to `ws://host:port/api/ws/logs` and appends incoming log entries in real-time, eliminating the need for periodic polling.
- **HTTP Fallback** — The initial log load and search filtering use the HTTP endpoint (`GET /api/log`). The log toolbar shows the active transport (ws or http).
- **Auto-Scroll** — When the log viewer is already scrolled to the bottom, new entries cause automatic scrolling to keep the latest entry visible.
- **Exponential Backoff** — If the log WebSocket connection drops, the client reconnects with exponential backoff (1s, 2s, 4s, up to 30s maximum).

### VTTY API Endpoints

Three endpoints provide VTTY content at different levels of detail:

#### Full ANSI Content

```bash
curl http://127.0.0.1:8080/api/commands/550e8400-e29b-41d4-a716-446655440000/vtty
```

Response:
```json
{
  "status": "ok",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "content": "\u001b[1mhtop\u001b[0m - process viewer\r\n..."
  },
  "error": null
}
```

The `content` field contains raw ANSI escape sequences. This is useful for terminals that can interpret ANSI codes directly, or for piping into tools that render ANSI output.

#### HTML-Rendered Content

```bash
curl http://127.0.0.1:8080/api/commands/550e8400-e29b-41d4-a716-446655440000/vtty/html
```

Response:
```json
{
  "status": "ok",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "html": "<span class=\"bold\">htop</span> - process viewer\n...",
    "cursor": {
      "row": 5,
      "col": 12
    },
    "dimensions": {
      "rows": 24,
      "cols": 80
    },
    "scrollback_lines": 42
  },
  "error": null
}
```

This endpoint returns pre-rendered HTML suitable for embedding in a web page. The `cursor` field shows the current cursor position, and `scrollback_lines` indicates how many lines exist in the scrollback buffer.

#### Partial Content (Paginated)

Fetch a subset of VTTY content for efficient polling of large outputs:

```bash
# Get 50 lines starting at offset 0
curl "http://127.0.0.1:8080/api/commands/550e8400-e29b-41d4-a716-446655440000/vtty/partial?offset=0&limit=50"

# Get 20 lines starting at line 100
curl "http://127.0.0.1:8080/api/commands/550e8400-e29b-41d4-a716-446655440000/vtty/partial?offset=100&limit=20"
```

Response:
```json
{
  "status": "ok",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "offset": 0,
    "limit": 50,
    "content": "line 1 content\nline 2 content\n..."
  },
  "error": null
}
```

This is useful for implementing scrollback in a web viewer or for scripts that only need recent output without fetching the entire buffer.

### WebSocket Real-Time Streaming

vrc provides two WebSocket endpoints that push updates in real time, eliminating the need for REST polling. WebSocket connections are upgraded from standard HTTP requests and use JSON text frames for all messages.

#### VTTY WebSocket — `ws://host:port/api/commands/{id}/ws`

Connect to this endpoint to receive push-based VTTY updates for a specific command. The connection is bidirectional: the server sends terminal content updates and the client can send keystrokes and resize commands.

**Connection example (JavaScript):**
```javascript
const ws = new WebSocket('ws://127.0.0.1:8080/api/commands/550e8400.../ws');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  switch (msg.type) {
    case 'connected':
      console.log('Connected to command', msg.id);
      break;
    case 'vtty_full':
      // Initial full snapshot — render the complete terminal
      document.getElementById('terminal').innerHTML = msg.data.html;
      break;
    case 'vtty_diff':
      // Incremental diff — apply only changed cells
      applyVttyDiff(msg.data);
      break;
    case 'command_ended':
      console.log('Command', msg.id, 'has exited');
      ws.close();
      break;
    case 'error':
      console.error('Error:', msg.message);
      break;
  }
};

// Send keystrokes
ws.send(JSON.stringify({ type: 'keys', keys: 'ls -la\r' }));

// Resize terminal
ws.send(JSON.stringify({ type: 'resize', rows: 40, cols: 120 }));

// Ping/pong keepalive
ws.send(JSON.stringify({ type: 'ping' }));
```

**Incoming server messages:**

The VTTY WebSocket uses an incremental diff protocol. See [Incremental VTTY Diff Protocol](#incremental-vtty-diff-protocol) for the full protocol specification and message format details.

**Outgoing client messages:**

| Message Type | Fields | Description |
|-------------|--------|-------------|
| `keys` | `keys` (string) | Send keystrokes to the command. Supports escape sequences like `\x03` for Ctrl+C |
| `resize` | `rows` (number), `cols` (number) | Resize the virtual terminal |
| `ping` | — | Request a `pong` response for connection keepalive |

#### Log WebSocket — `ws://host:port/api/ws/logs`

Connect to this endpoint to receive real-time log entries as they are recorded. This is a read-only stream (no outgoing commands except pings).

**Connection example (JavaScript):**
```javascript
const ws = new WebSocket('ws://127.0.0.1:8080/api/ws/logs');

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.type === 'log_entry') {
    console.log('[LOG]', msg.data);
  }
};
```

**WebSocket with TLS:** When vrc is running with `--tls`, use `wss://` instead of `ws://`:
```javascript
const ws = new WebSocket('wss://127.0.0.1:8080/api/commands/.../ws');
```

**WebSocket with auth:** When authentication is enabled, pass the bearer token as a query parameter or in the initial HTTP upgrade headers:
```javascript
const ws = new WebSocket('wss://host:8080/api/commands/.../ws?token=YOUR_TOKEN');
```

### Incremental VTTY Diff Protocol

The VTTY WebSocket uses an incremental diff protocol to minimize bandwidth usage. Instead of sending the full terminal HTML on every update, the server transmits only the cells that have changed since the last broadcast. The protocol works in three phases:

**Phase 1 — Initial Full Snapshot:** Upon connection, the server sends a `vtty_full` message containing the complete terminal HTML, cursor position, dimensions, and alternate screen status. This gives the client a complete picture of the terminal state:

```json
{
  "type": "vtty_full",
  "data": {
    "id": "550e8400-...",
    "html": "<span>...</span>",
    "cursor": { "row": 5, "col": 12 },
    "dimensions": { "rows": 24, "cols": 80 },
    "alternate_screen": false
  }
}
```

**Phase 2 — Incremental Diffs:** On subsequent updates, the server sends `vtty_diff` messages containing only the cells that changed. The server maintains a copy of the last-sent buffer and computes a cell-level diff (comparing character, foreground/background colors, and text attributes) every 200ms:

```json
{
  "type": "vtty_diff",
  "data": {
    "id": "550e8400-...",
    "diff": {
      "width": 80,
      "height": 24,
      "changed_count": 3,
      "cells": [
        { "row": 5, "col": 10, "ch": "A", "fg": [255,255,255], "bg": [0,0,0], "bold": false, ... },
        { "row": 5, "col": 11, "ch": "B", "fg": [255,255,255], "bg": [0,0,0], "bold": false, ... },
        { "row": 5, "col": 12, "ch": "C", "fg": [255,255,255], "bg": [0,0,0], "bold": false, ... }
      ]
    },
    "cursor": { "row": 5, "col": 13 },
    "dimensions": { "rows": 24, "cols": 80 },
    "alternate_screen": false
  }
}
```

**Phase 3 — Resynchronization:** If the client falls behind (broadcast lag), the server automatically sends a new `vtty_full` message to resynchronize the client to the current state. This happens transparently and ensures the client always has a consistent view of the terminal.

**Updated incoming message types:**

| Message Type | Fields | Description |
|-------------|--------|-------------|
| `connected` | `id` | Sent immediately after WebSocket upgrade confirms the connection |
| `vtty_full` | `id`, `html`, `cursor`, `dimensions`, `alternate_screen` | Full terminal snapshot (sent on connect and after lag recovery) |
| `vtty_diff` | `id`, `diff`, `cursor`, `dimensions`, `alternate_screen` | Incremental diff with only changed cells |
| `command_ended` | `id` | Sent when the command exits and is removed from the manager |
| `error` | `message` | Sent when an incoming client message fails to process |
| `pong` | — | Response to a `ping` message from the client |

The admin web interface handles both `vtty_full` and `vtty_diff` messages. On `vtty_diff`, it falls back to an HTTP full-refresh to ensure correct rendering, while still benefiting from the server-side optimization of only sending diffs when the buffer actually changes. This approach avoids complex DOM diffing while keeping bandwidth usage low.

---

## Sending Keystrokes

Send keyboard input to a running command through the API:

```bash
# Send a single key
curl -X POST http://127.0.0.1:8080/api/commands/550e8400-e29b-41d4-a716-446655440000/keys \
  -H "Content-Type: application/json" \
  -d '{"keys": "q"}'

# Send text input
curl -X POST http://127.0.0.1:8080/api/commands/550e8400-e29b-41d4-a716-446655440000/keys \
  -H "Content-Type: application/json" \
  -d '{"keys": "hello world"}'

# Send special keys using escape sequences
curl -X POST http://127.0.0.1:8080/api/commands/550e8400-e29b-41d4-a716-446655440000/keys \
  -H "Content-Type: application/json" \
  -d '{"keys": "\x03"}'
```

Common escape sequences:
- `\x03` — Ctrl+C (SIGINT)
- `\x04` — Ctrl+D (EOF)
- `\x1b` — Escape key
- `\r` — Enter/Return
- `\t` — Tab
- `\x7f` — Backspace
- `\x1b[A` — Up arrow
- `\x1b[B` — Down arrow
- `\x1b[C` — Right arrow
- `\x1b[D` — Left arrow

#### Practical Keystroke Examples

```bash
ID="550e8400-e29b-41d4-a716-446655440000"

# Quit htop
curl -s -X POST "http://127.0.0.1:8080/api/commands/$ID/keys" \
  -H "Content-Type: application/json" \
  -d '{"keys": "q"}'

# Type a command in a shell and press Enter
curl -s -X POST "http://127.0.0.1:8080/api/commands/$ID/keys" \
  -H "Content-Type: application/json" \
  -d '{"keys": "ls -la\r"}'

# Send Ctrl+C to interrupt a running process
curl -s -X POST "http://127.0.0.1:8080/api/commands/$ID/keys" \
  -H "Content-Type: application/json" \
  -d '{"keys": "\x03"}'

# Quit vim by typing :q! and Enter
curl -s -X POST "http://127.0.0.1:8080/api/commands/$ID/keys" \
  -H "Content-Type: application/json" \
  -d '{"keys": "\x1b:q!\r"}'
```

---

## Managing Running Commands

### Listing Commands

#### Via CLI

```bash
vrc list
```

This queries all running vrc instances and contacts each one via HTTP to retrieve its active commands. The output shows each instance's PID, port, bind address, daemon/display status, and all running commands with their arguments and certificate bindings. If an instance is unreachable, it is marked accordingly.

#### Via curl

```bash
# List all running commands on this instance
curl http://127.0.0.1:8080/api/commands
```

Response:
```json
{
  "status": "ok",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "htop",
      "args": [],
      "pid": 12345,
      "status": "running",
      "certificate": null
    },
    {
      "id": "660e8400-e29b-41d4-a716-446655440001",
      "name": "python",
      "args": ["-m", "http.server", "8000"],
      "pid": 12346,
      "status": "running",
      "certificate": "my-app"
    }
  ],
  "error": null
}
```

#### Via the Web UI

The admin dashboard at `/admin` automatically lists all running commands. Each entry shows the command name, PID, status, and any certificate binding. The admin interface also includes a Pause/Run button in the top bar that freezes or thaws the currently selected command.

### Freezing and Thawing Commands

vrc can freeze (suspend) and thaw (resume) running commands using POSIX signals (SIGSTOP/SIGCONT). A frozen command is paused — it consumes no CPU but remains in memory and can be resumed at any time. The admin web interface includes a Pause/Run button that toggles between freeze and thaw for the currently selected command.

#### Via CLI

```bash
# Freeze a command (pause it)
vrc freeze 550e8400-e29b-41d4-a716-446655440000

# Thaw a command (resume it)
vrc thaw 550e8400-e29b-41d4-a716-446655440000

# Use --target to select which vrc instance
vrc --target 12345 freeze 550e8400-e29b-41d4-a716-446655440000
```

#### Via curl

```bash
# Freeze
ID="550e8400-e29b-41d4-a716-446655440000"
curl -X POST http://127.0.0.1:8080/api/commands/$ID/freeze

# Thaw
curl -X POST http://127.0.0.1:8080/api/commands/$ID/thaw
```

Freeze sends SIGSTOP to the child process, which causes the OS to stop scheduling it. Thaw sends SIGCONT to resume execution. This is useful for temporarily freeing CPU resources or for debugging — you can freeze a process, inspect its VTTY output, then resume it. Note that freeze/thaw is only supported on Unix-like systems.

### Killing Commands

#### Via curl

```bash
# Kill with default signal (SIGTERM)
curl -X POST http://127.0.0.1:8080/api/commands/550e8400-e29b-41d4-a716-446655440000/kill \
  -H "Content-Type: application/json" \
  -d '{}'

# Kill with a specific signal
curl -X POST http://127.0.0.1:8080/api/commands/550e8400-e29b-41d4-a716-446655440000/kill \
  -H "Content-Type: application/json" \
  -d '{"signal": "SIGKILL"}'
```

#### Via the Web UI

Click the kill button next to a command in the admin dashboard.

#### Via the CLI (stop entire instance)

```bash
# Stop a vrc instance by its PID
vrc stop 12345
```

Note: `vrc stop <pid>` first attempts to find and kill a command with that OS PID on any running instance. If no matching command is found, it falls back to shutting down the entire vrc instance.

### Kill by PID

You can kill individual commands by their OS process ID without stopping the entire vrc instance. This is useful when you know the PID of a specific child process and want to terminate it without affecting other running commands.

```bash
# Kill a command by its OS PID
curl -X POST http://127.0.0.1:8080/api/commands/kill-pid/12345

# From the CLI (queries all instances)
vrc stop 12345
```

The CLI `vrc stop` command now tries the kill-by-PID API first across all running instances. If a command with that PID is found, only that command is killed. If no command matches, it falls back to the traditional instance shutdown behavior.

### Resizing the Terminal

You can resize a running command's virtual terminal from the CLI, the API, or the web UI. The resize updates both the in-memory VTTY buffer and the underlying child PTY, causing the kernel to send a `SIGWINCH` signal to the child process. Terminal-aware applications (vim, htop, tmux, less) respond to SIGWINCH by adjusting their layout to the new dimensions.

#### Via CLI

Use the `vrc resize` subcommand. The target can be a PID (numeric) or a command name:

```bash
# Resize by command PID
vrc resize 12345 --rows 40 --cols 120

# Resize by command name
vrc resize htop --rows 50 --cols 160

# Use your current terminal's size (omit --rows/--cols)
vrc resize htop
```

When `--rows` and `--cols` are omitted (or set to 0), vrc auto-detects your terminal's current size. The command queries all running vrc instances to find the matching command. If multiple commands match the name, use the PID to disambiguate.

#### Via curl (API)

```bash
ID="550e8400-e29b-41d4-a716-446655440000"

# Resize to 40 rows by 120 columns
curl -X POST http://127.0.0.1:8080/api/commands/$ID/resize \
  -H "Content-Type: application/json" \
  -d '{"rows": 40, "cols": 120}'
```

Valid ranges: rows 1-200, cols 1-500.

#### Via WebSocket

```javascript
ws.send(JSON.stringify({ type: 'resize', rows: 40, cols: 120 }));
```

#### Spawning with a Custom Size

You can also set a per-command terminal size at spawn time, independent of the server's default VTTY dimensions. This is useful when different commands need different terminal sizes on the same vrc instance.

**Via CLI:**
```bash
# Spawn vim with a wide terminal
vrc spawn --rows 30 --cols 160 vim file.txt
```

**Via API:**
```bash
curl -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "vim", "args": ["file.txt"], "rows": 30, "cols": 160}'
```

When `rows` and `cols` are omitted from the spawn request, the server's configured default VTTY size is used. You can still resize the command later via `vrc resize` or the resize API endpoint.

---

## 8. Snapshot and Diff

vrc can store named snapshots of a command's VTTY buffer and later compute cell-level diffs against the current buffer. This is useful for automated testing (compare expected vs actual terminal output), debugging (capture a baseline and see what changed), and auditing (save terminal state at key points in a workflow). Snapshots are stored in memory and are automatically cleaned up when the command is killed or the instance shuts down.

### Storing Snapshots

Create a named snapshot of a command's current VTTY buffer:

```bash
ID="550e8400-e29b-41d4-a716-446655440000"

# Store a snapshot with a custom name
curl -X POST http://127.0.0.1:8080/api/commands/$ID/snapshot \
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
    "command_args": ["test", "--release"],
    "pid": 12345,
    "timestamp": "2025-01-15T10:30:00Z",
    "runtime_secs": 42.5
  },
  "error": null
}
```

Each snapshot records the command name, arguments, PID, timestamp, and wall-clock runtime alongside the full VTTY buffer contents.

### Listing Snapshots

List all snapshots stored for a command:

```bash
curl http://127.0.0.1:8080/api/commands/$ID/snapshots
```

Response:
```json
{
  "status": "ok",
  "data": [
    {
      "name": "initial",
      "command_id": "550e8400-...",
      "command_name": "cargo",
      "command_args": ["test", "--release"],
      "pid": 12345,
      "timestamp": "2025-01-15T10:25:00Z",
      "runtime_secs": 0.1
    },
    {
      "name": "after-build",
      "command_id": "550e8400-...",
      "command_name": "cargo",
      "command_args": ["test", "--release"],
      "pid": 12345,
      "timestamp": "2025-01-15T10:30:00Z",
      "runtime_secs": 42.5
    }
  ],
  "error": null
}
```

### Computing Diffs

Compare the current VTTY buffer against a stored snapshot. The diff is computed cell-by-cell, comparing character value, foreground/background RGB colors, and text attributes (bold, italic, underline, etc.):

```bash
curl -X POST http://127.0.0.1:8080/api/commands/$ID/diff \
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
    "changed_count": 7,
    "cells": [
      { "row": 12, "col": 0, "ch": "E", "fg": [0,255,0], "bg": [0,0,0], "bold": true, "italic": false, ... },
      { "row": 12, "col": 1, "ch": "r", "fg": [0,255,0], "bg": [0,0,0], "bold": true, "italic": false, ... },
      " "..."
    ]
  },
  "error": null
}
```

The `changed_count` field tells you how many cells differ. The `cells` array contains the details of each changed cell from the current buffer. If the buffer dimensions have changed since the snapshot was taken, all cells are considered changed.

### Deleting Snapshots

Remove a stored snapshot:

```bash
curl -X DELETE http://127.0.0.1:8080/api/commands/$ID/snapshots/after-build
```

Response:
```json
{
  "status": "ok",
  "data": { "id": "550e8400-...", "name": "after-build" },
  "error": null
}
```

---

## 9. Exit Handlers and Timeouts

vrc can automatically run external commands when a child process exits, and enforce graceful shutdown timeouts before force-killing stubborn processes.

### Per-Command Exit Handlers via API

When spawning a command via `POST /api/commands`, you can specify `on_exit`, `on_error`, and `exit_timeout` fields:

```bash
# Run tests with notifications on success or failure
curl -s -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "cmd": "cargo",
    "args": ["test", "--release"],
    "on_exit": "notify-send Build OK",
    "on_error": "notify-send Build FAILED",
    "exit_timeout": 30
  }'

# Run a build script that triggers cleanup on any exit
curl -s -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "cmd": "./build.sh",
    "args": [],
    "on_exit": "rm -rf /tmp/build-tmp",
    "on_error": "rm -rf /tmp/build-tmp",
    "exit_timeout": 15
  }'

# Run with only an error handler (no action on clean exit)
curl -s -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "cmd": "./deploy.sh",
    "args": ["--prod"],
    "on_error": "./rollback.sh && notify-send Deploy FAILED"
  }'
```

The exit handler command string is split on whitespace into a binary name and arguments. It runs as a detached (fire-and-forget) process — vrc does not wait for it to complete. Exit handlers are useful for sending notifications, cleaning up temporary files, triggering rollbacks, or alerting monitoring systems.

### Default Exit Handlers via Config and CLI

You can set default exit handlers that apply to all commands unless overridden per-command via the API:

**Via CLI flags:**
```bash
vrc --on-exit "notify-send Done" --on-error "notify-send Error" --exit-timeout 20 -- cargo test
```

**Via config file:**
```yaml
default_exit:
  exit:
    on_exit: "notify-send Done"
    on_error: "notify-send Error"
    timeout_secs: 20
```

When a command is spawned via `POST /api/commands` without `on_exit`/`on_error` fields, the defaults from the config are used. When per-command values are provided in the API request, they override the defaults entirely.

### Per-Command Exit Options

In addition to exit handlers, several options control what happens when a specific command exits. These are set per-command (via CLI or API) and do NOT modify the global defaults:

| Option | CLI Flag | API Field | Description |
|--------|----------|-----------|-------------|
| Retain buffer | `--retain-on-exit` | `retain_on_exit` | Keep VTTY in memory after exit for inspection |
| Snapshot on exit | `--snapshot-on-exit <FILE>` | `snapshot_on_exit` | Save VTTY buffer to file as plain text on exit |
| Send initial keys | `--send-keys <KEYS>` | — | Send keystrokes to the command after it starts |

**Examples:**
```bash
# Retain the buffer and save a snapshot after tests finish
vrc --retain-on-exit --snapshot-on-exit /tmp/test-output.txt -- cargo test

# Send initial commands to a shell and save output when done
vrc --send-keys "ls -la<Enter>whoami<Enter>" --snapshot-on-exit /tmp/shell.txt -- bash

# Capture htop's final screen and exit when htop quits
vrc --snapshot-on-exit /tmp/htop.txt --display -- htop
```

**Via API:**
```bash
curl -s -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "cmd": "cargo",
    "args": ["test"],
    "retain_on_exit": true,
    "snapshot_on_exit": "/tmp/test-output.txt"
  }'
```

When `--retain-on-exit` is set, the command stays in the manager after exiting (visible in the tab bar with an `[EXITED]` status). This prevents vrc from exiting even in `--display` mode — it waits until all retained commands are purged. When `--snapshot-on-exit` is set, the VTTY buffer (including scrollback) is saved to the specified file as plain text before the command is removed.

### Graceful Shutdown with Timeout

When you kill a command via the API (`POST /api/commands/{id}/kill`), vrc performs a graceful shutdown sequence:

1. **SIGINT (Ctrl+C)** is sent to the child process, giving it a chance to exit cleanly.
2. vrc waits up to `exit_timeout` seconds (default: 10) for the process to terminate.
3. If the process has not exited within the timeout, **SIGKILL** is sent to force-terminate it.

This two-phase approach prevents data corruption in processes that need to write state files, close database connections, or flush buffers before exiting. The timeout is configurable per-command or globally via `default_exit.exit.timeout_secs`.

---

## Viewing Logs

### Command Log

vrc can log all API commands it receives. Enable logging at startup:

```bash
# Log to terminal
vrc --log -- my-command

# Log to file
vrc --log-file /var/log/vrc.log -- my-command

# Log to both terminal and file
vrc --log --log-file /var/log/vrc.log -- my-command
```

### Reading Logs via the API

```bash
# Get all log entries (up to 200 by default)
curl http://127.0.0.1:8080/api/log

# Get log entries with search filter
curl "http://127.0.0.1:8080/api/log?search=spawn"

# Get log entries with pagination
curl "http://127.0.0.1:8080/api/log?offset=0&limit=50&search=kill"

# Get more results
curl "http://127.0.0.1:8080/api/log?offset=50&limit=50&search=kill"
```

Response:
```json
{
  "status": "ok",
  "data": {
    "lines": [
      "2025-01-15T10:23:01Z spawn id=550e8400 cmd=htop args=[]",
      "2025-01-15T10:23:15Z keys id=550e8400 keys=q",
      "2025-01-15T10:23:16Z kill id=550e8400 signal=SIGTERM"
    ],
    "total_lines": 42,
    "filtered_lines": 3,
    "offset": 0,
    "limit": 50,
    "search": "spawn"
  },
  "error": null
}
```

The `search` parameter filters lines case-insensitively. The response includes `total_lines` (all log entries) and `filtered_lines` (matching entries), so you can calculate the total number of pages.

### Configuring Logging in the Config File

```yaml
command_log:
  enabled: true
  file: "/var/log/vrc.log"
```

### PTY Raw Log Replay

When debugging terminal output, use `tools/ansi-replay` to replay a PTY raw log step by step:

```bash
perl tools/ansi-replay /tmp/pty-output.log           # Interactive mode
perl tools/ansi-replay /tmp/pty-output.log --dump    # Output all at once
```

Interactive controls: Space (next line), d (next 10), f (auto-play), p (peek), / (search), g (jump to line), h (help), q (quit).

---

## Environment Variables

vrc provides three layers of environment variable configuration for spawned commands. Each layer can override the previous one, giving fine-grained control over the environment each command sees.

### Layer 1: Config File (Global Defaults)

Set environment variables that apply to all spawned commands by default:

```yaml
environment:
  variables:
    RUST_LOG: "info"
    DATABASE_URL: "postgres://localhost/mydb"
    NODE_ENV: "development"
```

These variables are automatically passed to every command unless overridden or disabled.

### Layer 2: Per-Command (API or CLI)

Pass environment variables when spawning a command — these override config defaults for that specific command only.

**Via CLI --env flags:**
```bash
vrc spawn --env RUST_LOG=debug --env DATABASE_URL=postgres://prod/db -- cargo run

# At startup
vrc --env RUST_LOG=debug -- ./my-app
```

**Via API "env" field:**
```bash
curl -s -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "cmd": "cargo",
    "args": ["run"],
    "env": {
      "RUST_LOG": "debug",
      "DATABASE_URL": "postgres://prod/db"
    }
  }'
```

### Layer 3: --no-env Flag

The `--no-env` flag tells vrc to ignore all environment variables from the config file. Only variables set via `--env` CLI flags or the API `env` field will be passed to the command.

```bash
# Config has RUST_LOG=info, but we want a clean environment
vrc spawn --no-env --env PATH=/usr/bin -- ./my-script.sh

# Via API
curl -s -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "./my-script.sh", "no_env": true, "env": {"PATH": "/usr/bin"}}'
```

### Precedence Summary

```
CLI --env flags / API "env" field  (highest — always wins)
        ↓
Config environment.variables     (global defaults)
        ↓
--no-env clears config env vars   (but CLI/API env vars still apply)
```

The `TERM` environment variable is always set to the configured `vtty.term` value (default: `xterm-256color`) regardless of other settings.

---

## Configuration Profiles

Profiles let you define named sets of configuration values in your config file. When you select a profile, only the fields present in that profile override the base configuration. CLI flags always take final precedence over both the base config and the profile.

### Defining Profiles

```yaml
# vrc.yaml
profiles:
  development:
    vtty:
      rows: 40
      cols: 120
    display:
      enabled: true
      refresh_ms: 50
    environment:
      variables:
        RUST_LOG: "debug"
        NODE_ENV: "development"

  production:
    server:
      bind: "0.0.0.0"
      port: 443
    security:
      require_auth: true
    tls:
      enabled: true
    environment:
      variables:
        RUST_LOG: "warn"
        NODE_ENV: "production"

  ci:
    vtty:
      rows: 10
      cols: 80
      scrollback: 1000
    command_log:
      enabled: true
      file: "/tmp/vrc-ci.log"
```

### Using Profiles

**Via CLI:**
```bash
# Use the "development" profile
vrc --profile development -- cargo run

# Use "production" profile with TLS
vrc --profile production --tls -- ./my-server

# Use a profile and override specific values with CLI flags
vrc --profile production --port 8443 -- ./my-server
```

**Via API (spawn request):**
```bash
curl -s -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "./my-app", "profile": "development"}'
```

### How Profiles Work

1. The base configuration is loaded from config files and defaults.
2. If a profile is selected, only the fields present in the profile override the base config.
3. CLI flags are applied last and always take final precedence.

For example, with the `production` profile above:
- `server.bind` becomes `0.0.0.0` (from profile)
- `server.port` becomes `443` (from profile, unless `--port` overrides it)
- `security.require_auth` becomes `true` (from profile)
- `vtty.rows` stays at `24` (not in the profile, so base config/default is used)
- `environment.variables.RUST_LOG` becomes `warn` (from profile)

If a profile name is specified that does not exist, vrc exits with an error listing all available profile names.

### Real-Time Log Streaming via WebSocket

For push-based log streaming instead of polling, connect to the log WebSocket endpoint. See [WebSocket Real-Time Streaming](#websocket-real-time-streaming) for the `ws://host:port/api/ws/logs` protocol details.

---

## Certificate-Based Access Control

Certificates provide per-command access isolation within a vrc instance. Each certificate in the pool can be bound to running commands, ensuring only clients with the correct bearer token can interact with those commands.

### Generating a Certificate

```bash
vrc cert generate my-application
```

### Listing Certificates

```bash
# Via CLI
vrc cert list

# Via API
curl http://127.0.0.1:8080/api/certificates
```

### Using a Certificate Token

```bash
# Show certificate details including the full bearer token
vrc cert show my-application

# Use the token in API requests
TOKEN=$(vrc cert show my-application | grep -oP 'Token:\s*\K\S+')
curl -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:8080/api/commands/$ID/vtty
```

For the complete certificate management guide with advanced examples, see [docs/certificates.md](certificates.md).

---

## Remote Access and TLS

By default, vrc binds to `127.0.0.1` (localhost only) and uses plain HTTP. For remote access, you need both network binding and security.

### Quick Remote Setup

```bash
vrc --remote --tls -- my-command
```

This single flag does the following:
- Binds to `0.0.0.0` (accepts connections from any interface)
- Enables bearer token authentication (auto-generates a token if none exists)
- Enables TLS with self-signed certificates (auto-generates if none exist)

### Step-by-Step Remote Setup

1. **Start the server:**
   ```bash
   vrc --bind 0.0.0.0 --port 8080 --auth --tls -- some-command
   ```

2. **Get the authentication token:**
   ```bash
   cat ~/.config/vrc/token
   ```

3. **Get the server certificate** (for TLS verification):
   ```bash
   cat ~/.config/vrc/cert.pem
   ```

4. **Connect from a remote machine:**
   ```bash
   # Copy cert.pem to the remote machine, then:
   curl --cacert /path/to/cert.pem \
        -H "Authorization: Bearer <token-from-step-2>" \
        https://server-hostname:8080/api/commands
   ```

### Remote Access with curl — Complete Examples

```bash
TOKEN="your-token-here"
CERT="/path/to/cert.pem"
HOST="https://server.example.com:8080"

# List commands on a remote server
curl --cacert $CERT -H "Authorization: Bearer $TOKEN" "$HOST/api/commands"

# Spawn a command on a remote server
curl -s -X POST --cacert $CERT \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  "$HOST/api/commands" \
  -d '{"cmd": "bash", "args": ["-c", "uptime && df -h"]}'

# View VTTY output from a remote command
ID="550e8400-e29b-41d4-a716-446655440000"
curl --cacert $CERT -H "Authorization: Bearer $TOKEN" "$HOST/api/commands/$ID/vtty"

# Send keystrokes to a remote command
curl -s -X POST --cacert $CERT \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  "$HOST/api/commands/$ID/keys" \
  -d '{"keys": "\r"}'

# Kill a remote command
curl -s -X POST --cacert $CERT \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  "$HOST/api/commands/$ID/kill" \
  -d '{}'

# Search command logs on a remote server
curl --cacert $CERT -H "Authorization: Bearer $TOKEN" \
  "$HOST/api/log?search=spawn&limit=100"

# Gracefully shut down a remote instance
curl -s -X POST --cacert $CERT \
  -H "Authorization: Bearer $TOKEN" \
  "$HOST/api/shutdown"
```

### Using Custom TLS Certificates

Replace the auto-generated self-signed certificates with your own (e.g., from Let's Encrypt or an internal CA):

```bash
vrc --tls \
  --cert-file /etc/ssl/certs/vrc.crt \
  --key-file /etc/ssl/private/vrc.key \
  --remote -- my-command
```

Or in a config file:

```yaml
server:
  bind: "0.0.0.0"
  port: 8080

security:
  require_auth: true

tls:
  enabled: true
  cert_file: "/etc/ssl/certs/vrc.crt"
  key_file: "/etc/ssl/private/vrc.key"
```

---

## Daemon Mode

Run vrc as a background process that detaches from your terminal:

```bash
# Basic daemon mode
vrc --daemon -- my-command

# Daemon with TLS for remote access
vrc --daemon --remote --tls -- my-command

# Daemon with custom output files
vrc --daemon \
  --stdout-file /var/log/vrc/stdout \
  --stderr-file /var/log/vrc/stderr \
  -- my-command
```

In daemon mode, vrc performs a double-fork to detach from the controlling terminal. The process becomes a session leader, stdin is closed, and stdout/stderr are redirected to files (default: `/tmp/vrc.out` and `/tmp/vrc.err`). The `--display` option is automatically disabled since there is no terminal to display on.

To manage a daemon instance:

```bash
# Find the instance
vrc list

# Stop the instance
vrc stop <pid>

# Or send API commands (the HTTP server is still running)
curl http://127.0.0.1:8080/api/commands
```

---

## Interactive Display

The interactive display mode provides a terminal-based UI for monitoring and controlling running commands. Enable it with `--display`:

```bash
# View a single command's output
vrc --display -- htop

# Stay active after the command exits, switching to other commands
# (--display now includes the old --display-all behavior)
vrc --display -- htop

# Show a tab bar listing all running commands
vrc --tabs --display -- htop
```

### Tab Bar

The `--tabs` flag enables a tab bar at the top of the display that lists all running commands and allows you to switch between them. When tabs are disabled (the default), only the active command is shown. This is similar to tools like `mprocs` but the tab bar is optional.

### Keyboard Shortcuts (Keybindings)

When the interactive display is active, you can use configurable keybindings to navigate commands, toggle overlays, and spawn new processes. All keybindings use human-readable names in the config file — no raw escape sequences needed.

#### Default Keybindings

| Shortcut | Action | Notes |
|----------|--------|-------|
| `Ctrl+Right` | Switch to next command | Requires `--display` and 2+ commands; wraps around |
| `Ctrl+Left` | Switch to previous command | Requires `--display` and 2+ commands; wraps around |
| `Ctrl+L` | Toggle command log overlay | Shows recent log entries over the VTTY; press again to dismiss |
| `F12` | Spawn a new command | Opens a prompt to type a command; Enter to confirm, Ctrl+C to cancel |
| `Ctrl+H` | Show help overlay | Displays all keybindings; press any key to dismiss |
| `Ctrl+\\` | Quit display | Always active (cannot be remapped) |
| *—* | Kill active command | Disabled by default — set `keybindings.kill_command` in config to enable |
| *—* | Pause/resume active command | Disabled by default — set `keybindings.toggle_pause` in config to enable |

#### Customizing Keybindings

Keybindings are configured under `interactive.keybindings` in the config file. Set any binding to `null` to disable it:

```yaml
interactive:
  tabs: true
  keybindings:
    next_command: "ctrl+right"    # default
    prev_command: "ctrl+left"     # default
    toggle_log: "ctrl+l"          # default
    spawn_command: "f12"          # default
    show_help: "ctrl+h"           # default
    quit: "esc"                   # use Escape to quit
```

#### Supported Key Name Formats

The key parser recognizes these human-readable formats:

- **Control keys:** `ctrl+a` through `ctrl+z`, `ctrl+@`, `ctrl+[`, `ctrl+\`, `ctrl+]`, `ctrl+^`, `ctrl+_`, `ctrl+?`
- **Control + arrows:** `ctrl+left`, `ctrl+right`, `ctrl+up`, `ctrl+down`
- **Alt/Meta:** `alt+a` through `alt+z`, `alt+0` through `alt+9`, and any other single character
- **Shift + arrows:** `shift+left`, `shift+right`, `shift+up`, `shift+down`, `shift+tab`
- **Function keys:** `f1` through `f12`
- **Special keys:** `enter` (or `return`), `tab`, `backspace`, `delete`, `insert`, `home`, `end`, `pageup` (or `page_up`), `pagedown` (or `page_down`), `up`, `down`, `left`, `right`, `esc` (or `escape`), `space`
- **Single characters:** Any printable ASCII character (e.g., `a`, `1`, `@`)
- **Raw escape sequences** (backward compatible): Rust-style notation like `"\x1b[1;5C"` for Ctrl+Right

For the complete configuration reference including all keybinding fields and their defaults, see [docs/configuration.md](configuration.md#interactive).

### Command Log Overlay

Press `Ctrl+L` (or your configured `toggle_log` key) to toggle a semi-transparent log overlay on top of the VTTY display. This shows the most recent vrc command log entries (spawns, kills, send_keys events, etc.) without leaving the terminal display. Press `Ctrl+L` again to dismiss the overlay.

### Spawning Commands from the Display

Press `F12` (or your configured `spawn_command` key) to open an inline spawn prompt. The display temporarily exits raw mode so you get normal line editing. Type the command and press Enter to spawn it on the current vrc instance. Press `Ctrl+C` to cancel without spawning. After the command is spawned (or cancelled), the display returns to raw mode automatically.

### Help Overlay

Press `Ctrl+H` (or your configured `show_help` key) to display a full-screen help overlay listing all configured keybindings with their descriptions. Press any key to dismiss the overlay and return to the VTTY display.

---

## Multi-Instance Management

Multiple vrc instances can run simultaneously on different ports. This is useful for managing separate environments (development, staging, production) or for running different sets of commands independently.

### Starting Multiple Instances

```bash
# Instance 1: Development server on port 8080
vrc --port 8080 -- daemon

# Instance 2: Staging server on port 9090 with TLS
vrc --port 9090 --tls -- daemon

# Instance 3: Production server on port 443 with custom certs
vrc --port 443 --tls \
  --cert-file /etc/ssl/prod/cert.pem \
  --key-file /etc/ssl/prod/key.pem \
  --remote -- daemon
```

### Listing All Instances

```bash
vrc list
```

The enhanced `vrc list` command contacts each running instance via HTTP to retrieve its active commands. The output shows instance metadata alongside all running commands with their arguments and certificate bindings:

```
PID        PORT     BIND                 DAEMON     DISPLAY    COMMAND
12345      8080     127.0.0.1            yes        no         (idle) -> htop [80x24]
12346      9090     127.0.0.1            yes        no         (no commands)
12347      3000     127.0.0.1            no         no         (idle) -> cargo test ["--release"] [my-app]
```

If an instance is unreachable, the output indicates the connection error instead of showing commands.

### Stopping a Specific Instance or Command

```bash
# Kill a specific command by its OS PID (queries all instances first)
vrc stop 12345

# If no command with that PID is found, stops the entire instance
vrc stop 12346
```

### Using Different Configs Per Instance

Each instance can load a different configuration file:

```bash
# Dev instance
vrc -c ./configs/dev.yaml --port 8080 -- daemon

# Staging instance
vrc -c ./configs/staging.yaml --port 9090 -- daemon

# Production instance
vrc -c /etc/vrc/production.yaml --port 443 -- daemon
```

### Additional Management Subcommands

#### `list-vrc` — Compact Instance Listing

List all running vrc instances in a compact, machine-friendly format:

```bash
vrc list-vrc
```

Output includes each instance's PID, port, bind address, and daemon status. Use this when you need a quick overview without the full command details shown by `vrc list`.

#### `list-commands` — List Commands Across Instances

List all running commands on all vrc instances:

```bash
vrc list-commands
```

This contacts every running instance and displays its active commands, arguments, PIDs, and statuses in a consolidated table.

#### `stop-command` — Stop a Specific Command

Stop a specific command by its OS PID without stopping the entire vrc instance:

```bash
# Stop a command with PID 12345
vrc stop-command 12345

# With --target to select a specific instance
vrc --target 54321 stop-command 12345
```

This is equivalent to calling `POST /api/commands/kill-pid/:pid` on the target instance.

#### `kill` — Alias for stop-command

`kill` is an alias for `stop-command`. Both stop a running command by PID.

```bash
vrc kill 12345
vrc --target 54321 kill 12345
```

---

## Configuration File Reference

vrc supports three configuration file formats: YAML, TOML, and JSON. The format is detected automatically from the file extension (`.yaml`/`.yml`, `.toml`, `.json`). Configuration is loaded from multiple locations in order of increasing precedence:

```
Built-in defaults → Global config → Local config → Explicit config file → CLI flags
```

| Location | Path |
|----------|------|
| Global config | `~/.config/vrc/config.yaml` (or `.toml`) |
| Local config | `./vrc.yaml` (or `.toml`) in the current directory |
| Explicit | Any path specified with `-c <FILE>` |

### YAML Example

```yaml
# vrc.yaml
server:
  bind: "127.0.0.1"
  port: 8080

security:
  require_auth: false
  token_file: "~/.config/vrc/token"

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

command_log:
  enabled: false
  file: null

daemon:
  enabled: false
  stdout_file: "/tmp/vrc.out"
  stderr_file: "/tmp/vrc.err"

handles: []
```

### TOML Example

```toml
# vrc.toml
[server]
bind = "127.0.0.1"
port = 8080

[security]
require_auth = false
token_file = "~/.config/vrc/token"

[tls]
enabled = false

[vtty]
rows = 24
cols = 80
term = "xterm-256color"
scrollback = 5000
truecolor = true
mouse = false

[display]
enabled = false
refresh_ms = 100

[command_log]
enabled = false

[daemon]
enabled = false
stdout_file = "/tmp/vrc.out"
stderr_file = "/tmp/vrc.err"
```

### JSON Example

```json
{
  "server": {
    "bind": "127.0.0.1",
    "port": 8080
  },
  "security": {
    "require_auth": false,
    "token_file": "~/.config/vrc/token"
  },
  "tls": {
    "enabled": false
  },
  "vtty": {
    "rows": 24,
    "cols": 80,
    "term": "xterm-256color",
    "scrollback": 5000,
    "truecolor": true,
    "mouse": false
  },
  "display": {
    "enabled": false,
    "refresh_ms": 100
  },
  "command_log": {
    "enabled": false
  },
  "daemon": {
    "enabled": false,
    "stdout_file": "/tmp/vrc.out",
    "stderr_file": "/tmp/vrc.err"
  }
}
```

For the complete configuration reference with all keys, defaults, and CLI mappings, see [docs/configuration.md](configuration.md).

---

## Common Use Cases

### Development Server Orchestration

Run multiple development servers in a single vrc instance, each in its own VTTY, accessible from the web:

```bash
# Start vrc in idle mode with display disabled
vrc --daemon --log --log-file /tmp/vrc.log

# Spawn frontend dev server
curl -s -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "npm", "args": ["run", "dev:frontend"]}'

# Spawn backend API server
curl -s -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "cargo", "args": ["run"]}'

# Spawn database migration watcher
curl -s -X POST http://127.0.0.1:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "npm", "args": ["run", "watch:db"]}'

# View all running commands
curl http://127.0.0.1:8080/api/commands

# Open the web admin to monitor all three in one dashboard
# http://127.0.0.1:8080/admin
```

### CI/CD Pipeline Runner

Use vrc to run CI jobs with full terminal access for debugging failed builds:

```bash
# Start a secure vrc instance on the CI server
vrc --remote --tls --port 8080 --daemon --log-file /var/log/vrc-ci.log

# CI pipeline script spawns a build job
TOKEN=$(cat ~/.config/vrc/token)

JOB_ID=$(curl -s -X POST https://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"cmd": "./run-tests.sh", "args": ["--verbose", "--release"]}' \
  | jq -r '.data.id')

# Poll for build completion and capture output
while true; do
  STATUS=$(curl -s -H "Authorization: Bearer $TOKEN" \
    "https://localhost:8080/api/commands" \
    | jq -r ".data[] | select(.id == \"$JOB_ID\") | .status")
  [ "$STATUS" != "running" ] && break
  sleep 5
done

# Retrieve the full build log
curl -s -H "Authorization: Bearer $TOKEN" \
  "https://localhost:8080/api/commands/$JOB_ID/vtty" \
  | jq -r '.data.content'
```

### Remote Server Administration

Manage services on a remote machine through a web interface:

```bash
# On the remote server — start vrc with TLS and auth
vrc --remote --tls --port 443 --daemon

# Distribute ~/.config/vrc/token and ~/.config/vrc/cert.pem to admins

# From your local machine — run interactive commands on the remote server
TOKEN="your-token"
CERT="path/to/cert.pem"
HOST="https://remote.example.com"

# Start a monitoring session
ID=$(curl -s -X POST --cacert $CERT \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  "$HOST/api/commands" \
  -d '{"cmd": "htop"}' | jq -r '.data.id')

# Check on it later
curl --cacert $CERT -H "Authorization: Bearer $TOKEN" \
  "$HOST/api/commands/$ID/vtty/html" | jq '.data.html'

# Run a diagnostic command and capture output
DIAG_ID=$(curl -s -X POST --cacert $CERT \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  "$HOST/api/commands" \
  -d '{"cmd": "bash", "args": ["-c", "df -h && free -m && uptime"]}' \
  | jq -r '.data.id')

# Wait briefly and retrieve output
sleep 2
curl -s --cacert $CERT -H "Authorization: Bearer $TOKEN" \
  "$HOST/api/commands/$DIAG_ID/vtty" | jq -r '.data.content'
```

### Pair Programming and Collaboration

Share a terminal session between multiple developers via the web interface:

```bash
# Developer 1 starts vrc with a shared session
vrc --port 8080 --daemon

# Developer 1 starts vim in a shared VTTY
SHARED_ID=$(curl -s -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "vim", "args": ["shared-notes.txt"]}' \
  | jq -r '.data.id')

# Share the command ID and server address with Developer 2

# Developer 2 opens the web admin and views the shared session
# http://localhost:8080/admin → click on the vim command

# Developer 1 sends keystrokes remotely
curl -s -X POST http://localhost:8080/api/commands/$SHARED_ID/keys \
  -H "Content-Type: application/json" \
  -d '{"keys": "iHello from Developer 1\x1b:wq\r"}'

# Developer 2 can also send keystrokes
curl -s -X POST http://localhost:8080/api/commands/$SHARED_ID/keys \
  -H "Content-Type: application/json" \
  -d '{"keys": "iHello from Developer 2\x1b:wq\r"}'
```

### Long-Running Background Tasks

Run tasks that need to outlast your SSH session without screen or tmux:

```bash
# Start a long data processing job via vrc
vrc --port 8080 --daemon

JOB_ID=$(curl -s -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "python", "args": ["process_large_dataset.py", "--input", "data.csv"]}' \
  | jq -r '.data.id')

# Disconnect from SSH — the job keeps running because vrc is a daemon

# Reconnect later and check progress
curl -s "http://localhost:8080/api/commands/$JOB_ID/vtty/partial?offset=0&limit=20" \
  | jq -r '.data.content'
```

---

## Troubleshooting

### vrc won't start

Check that the port is not already in use:

```bash
# Check what is using port 8080
lsof -i :8080
ss -tlnp | grep 8080

# Use a different port
vrc --port 9090
```

### Connection refused

Ensure the server is running and you are using the correct address:

```bash
# Verify the instance is running
vrc list

# Check the bind address — 127.0.0.1 only accepts localhost connections
vrc --bind 0.0.0.0  # to accept remote connections
```

### TLS certificate errors

When using self-signed certificates, clients must trust the certificate explicitly:

```bash
# With curl, use --cacert
curl --cacert ~/.config/vrc/cert.pem https://localhost:8080/api/commands

# To bypass certificate verification (not recommended for production)
curl -k https://localhost:8080/api/commands
```

### Authentication failures

When auth is enabled, all API requests must include the bearer token:

```bash
# Get the token
cat ~/.config/vrc/token

# Use it in requests
TOKEN=$(cat ~/.config/vrc/token)
curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/commands
```

### Command exits immediately

Some commands require a TTY to function. vrc provides a pseudo-terminal, but the command's environment may need adjustment:

```bash
# Verify TERM is set correctly
vrc --term xterm-256color -- my-command

# Check if the command expects specific environment variables
vrc -- cmd="env TERM=xterm-256color my-command"
```

### VTTY output appears empty

The command may not have produced output yet, or the output may be in the scrollback buffer:

```bash
# Try the HTML endpoint for rendered output
curl http://localhost:8080/api/commands/$ID/vtty/html | jq '.data'

# Check the scrollback line count
curl http://localhost:8080/api/commands/$ID/vtty/html | jq '.data.scrollback_lines'

# Try fetching partial content
curl "http://localhost:8080/api/commands/$ID/vtty/partial?offset=0&limit=100"
```

### Log file not found via API

The log endpoint checks several common paths. If your log file is in a custom location, ensure the path matches what was passed to `--log-file` or configured in `command_log.file`. The API attempts to read from `/tmp/vrc.log`, `./vrc.log`, and `./vrc-commands.log` by default.

---

## Connection Types Supported

vrc supports **HTTP, HTTPS (TLS), WebSocket (ws://), and secure WebSocket (wss://)** connections. The WebSocket endpoints upgrade from HTTP and provide real-time bidirectional communication for terminal output and log streaming. The following connection modes are available:

| Mode | CLI Flag | Description |
|------|----------|-------------|
| HTTP (localhost) | *(default)* | Plain HTTP on `127.0.0.1:8080`, no auth required |
| HTTP (remote) | `--remote` | Plain HTTP on `0.0.0.0:8080`, auth required |
| HTTPS (localhost) | `--tls` | HTTPS on `127.0.0.1:8080`, auto-generated self-signed cert |
| HTTPS (remote) | `--remote --tls` | HTTPS on `0.0.0.0:8080`, auth required, self-signed cert |
| HTTPS (custom cert) | `--tls --cert-file X --key-file Y` | HTTPS with your own certificate and key |
| WebSocket | *(auto-upgrade)* | `ws://host:port/api/commands/{id}/ws` for VTTY streaming |
| Secure WebSocket | `--tls` + wss:// | `wss://host:port/api/commands/{id}/ws` for encrypted streaming |
