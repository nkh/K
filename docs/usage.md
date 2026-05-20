# vrunner User Guide

A practical guide to using vrunner for common tasks. This document covers the web administrative interface, CLI controller, and direct HTTP API access via curl and other tools.

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
5. [Sending Keystrokes](#sending-keystrokes)
6. [Managing Running Commands](#managing-running-commands)
   - [Listing Commands](#listing-commands)
   - [Killing Commands](#killing-commands)
   - [Resizing the Terminal](#resizing-the-terminal)
7. [Viewing Logs](#viewing-logs)
8. [Certificate-Based Access Control](#certificate-based-access-control)
9. [Remote Access and TLS](#remote-access-and-tls)
10. [Daemon Mode](#daemon-mode)
11. [Multi-Instance Management](#multi-instance-management)
12. [Configuration File Reference](#configuration-file-reference)
13. [Common Use Cases](#common-use-cases)
    - [Development Server Orchestration](#development-server-orchestration)
    - [CI/CD Pipeline Runner](#cicd-pipeline-runner)
    - [Remote Server Administration](#remote-server-administration)
    - [Pair Programming and Collaboration](#pair-programming-and-collaboration)
    - [Long-Running Background Tasks](#long-running-background-tasks)
14. [Troubleshooting](#troubleshooting)

---

## Concepts Overview

vrunner is a process manager that runs commands inside virtual TTYs (VTTYs) and exposes them through a web API. Rather than wrapping processes directly, vrunner creates pseudo-terminals, giving child processes full terminal capabilities including ANSI colors, cursor movement, and interactive keyboard input.

The key architectural concept is the separation between **starting a command** and **interacting with it**. A command can be started from the CLI, the web UI, or the API. Once running, it can be monitored and controlled from any of those interfaces interchangeably. This makes vrunner suitable for scenarios where a command needs to be started from one place (like a CI script) and monitored from another (like a web dashboard).

vrunner supports three controllers:
- **CLI** — direct command-line invocation for starting, listing, and stopping instances
- **Web Admin** — a browser-based dashboard at `/admin` for managing commands visually
- **HTTP API** — a RESTful API for programmatic access from scripts, curl, or custom clients

All three controllers communicate with the same vrunner instance. The CLI subcommands (`list`, `stop`, `cert`) connect to running instances over HTTP to perform management operations.

---

## Getting Started

### Installation

Build from source using Cargo:

```bash
git clone https://github.com/yourusername/vrunner.git
cd vrunner
cargo build --release
# Binary is at target/release/vrunner
```

Or install system-wide:

```bash
cargo install --path .
```

### First Run

Start vrunner in its simplest form — idle mode on localhost with no command:

```bash
vrunner
```

This starts an HTTP server on `http://127.0.0.1:8080`. No commands are running yet; the instance is ready to receive API requests or web UI connections. You can verify it is working by listing commands:

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
vrunner -- htop
```

vrunner spawns `htop` inside a virtual TTY, starts the HTTP server, and waits. You can open the web admin at `http://127.0.0.1:8080/admin` to see htop's terminal output, or use curl to interact with it programmatically.

### Getting Help

vrunner includes built-in help via clap:

```bash
vrunner --help
vrunner -h
vrunner cert --help
vrunner cert generate --help
```

These display all available options, subcommands, and their descriptions. The help text is the authoritative reference for CLI flags.

---

## Running Commands

### Running a Command on Startup

Use the `--` separator to pass a command to vrunner at launch. Everything after `--` is treated as the child command and its arguments:

```bash
# Run a development server
vrunner --port 3000 -- npm run dev

# Run a Python HTTP server with 80-column terminal
vrunner --vtty-cols 80 -- python -m http.server 8000

# Run with local terminal display visible
vrunner --display -- vim notes.txt

# Run in the background as a daemon
vrunner --daemon -- my-long-running-script.sh
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
vrunner --term xterm-256color --display -- tmux
```

### Spawning Commands via the CLI

The CLI can spawn commands at startup (via `--`), but cannot dynamically spawn additional commands after the server is running. For dynamic spawning, use the web UI or API. The CLI is primarily used for:

- Starting an instance with an initial command
- Listing running instances across the system
- Stopping instances by PID
- Managing certificates

---

## Viewing Terminal Output

### Local VTTY Display

Enable real-time terminal output on your local console with `--display`:

```bash
vrunner --display -- htop
```

This mirrors the VTTY contents to stdout at the refresh interval specified by `--refresh-ms` (default: 100ms). The display shows the raw ANSI output from the child process, including colors and cursor positioning. Press `Ctrl+C` in the terminal where vrunner is running to stop the instance.

### Web Admin VTTY Viewer

The web admin interface at `/admin` provides a VTTY viewer that fetches terminal content from the API. The viewer renders HTML output from `GET /api/commands/:id/vtty/html`, which includes cursor position, terminal dimensions, and scrollback information. Navigate to the admin page, click on a running command, and the VTTY viewer displays its output.

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
vrunner list
```

This queries all running vrunner instances on the system and displays their PID, port, bind address, daemon status, display status, and current command.

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
      "pid": 12345,
      "status": "running",
      "certificate": null
    },
    {
      "id": "660e8400-e29b-41d4-a716-446655440001",
      "name": "python",
      "pid": 12346,
      "status": "running",
      "certificate": "my-app"
    }
  ],
  "error": null
}
```

#### Via the Web UI

The admin dashboard at `/admin` automatically lists all running commands. Each entry shows the command name, PID, status, and any certificate binding.

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
# Stop a vrunner instance by its PID
vrunner stop 12345
```

Note: `vrunner stop <pid>` shuts down the entire vrunner instance (including all commands it manages), not an individual command. To kill individual commands, use the API or web UI.

### Resizing the Terminal

Change the virtual terminal dimensions for a running command:

```bash
curl -X POST http://127.0.0.1:8080/api/commands/550e8400-e29b-41d4-a716-446655440000/resize \
  -H "Content-Type: application/json" \
  -d '{"rows": 40, "cols": 120}'
```

Valid ranges: rows 1-200, cols 1-500. The child process receives a `SIGWINCH` signal when the terminal is resized, which allows terminal-aware applications (vim, htop, tmux) to adjust their layouts.

---

## Viewing Logs

### Command Log

vrunner can log all API commands it receives. Enable logging at startup:

```bash
# Log to terminal
vrunner --log -- my-command

# Log to file
vrunner --log-file /var/log/vrunner.log -- my-command

# Log to both terminal and file
vrunner --log --log-file /var/log/vrunner.log -- my-command
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
  file: "/var/log/vrunner.log"
```

---

## Certificate-Based Access Control

Certificates provide per-command access isolation within a vrunner instance. Each certificate in the pool can be bound to running commands, ensuring only clients with the correct bearer token can interact with those commands.

### Generating a Certificate

```bash
vrunner cert generate my-application
```

### Listing Certificates

```bash
# Via CLI
vrunner cert list

# Via API
curl http://127.0.0.1:8080/api/certificates
```

### Using a Certificate Token

```bash
# Show certificate details including the full bearer token
vrunner cert show my-application

# Use the token in API requests
TOKEN=$(vrunner cert show my-application | grep -oP 'Token:\s*\K\S+')
curl -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:8080/api/commands/$ID/vtty
```

For the complete certificate management guide with advanced examples, see [docs/certificates.md](certificates.md).

---

## Remote Access and TLS

By default, vrunner binds to `127.0.0.1` (localhost only) and uses plain HTTP. For remote access, you need both network binding and security.

### Quick Remote Setup

```bash
vrunner --remote --tls -- my-command
```

This single flag does the following:
- Binds to `0.0.0.0` (accepts connections from any interface)
- Enables bearer token authentication (auto-generates a token if none exists)
- Enables TLS with self-signed certificates (auto-generates if none exist)

### Step-by-Step Remote Setup

1. **Start the server:**
   ```bash
   vrunner --bind 0.0.0.0 --port 8080 --auth --tls -- some-command
   ```

2. **Get the authentication token:**
   ```bash
   cat ~/.config/vrunner/token
   ```

3. **Get the server certificate** (for TLS verification):
   ```bash
   cat ~/.config/vrunner/cert.pem
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
vrunner --tls \
  --cert-file /etc/ssl/certs/vrunner.crt \
  --key-file /etc/ssl/private/vrunner.key \
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
  cert_file: "/etc/ssl/certs/vrunner.crt"
  key_file: "/etc/ssl/private/vrunner.key"
```

---

## Daemon Mode

Run vrunner as a background process that detaches from your terminal:

```bash
# Basic daemon mode
vrunner --daemon -- my-command

# Daemon with TLS for remote access
vrunner --daemon --remote --tls -- my-command

# Daemon with custom output files
vrunner --daemon \
  --stdout-file /var/log/vrunner/stdout \
  --stderr-file /var/log/vrunner/stderr \
  -- my-command
```

In daemon mode, vrunner performs a double-fork to detach from the controlling terminal. The process becomes a session leader, stdin is closed, and stdout/stderr are redirected to files (default: `/tmp/vrunner.out` and `/tmp/vrunner.err`). The `--display` option is automatically disabled since there is no terminal to display on.

To manage a daemon instance:

```bash
# Find the instance
vrunner list

# Stop the instance
vrunner stop <pid>

# Or send API commands (the HTTP server is still running)
curl http://127.0.0.1:8080/api/commands
```

---

## Multi-Instance Management

Multiple vrunner instances can run simultaneously on different ports. This is useful for managing separate environments (development, staging, production) or for running different sets of commands independently.

### Starting Multiple Instances

```bash
# Instance 1: Development server on port 8080
vrunner --port 8080 -- daemon

# Instance 2: Staging server on port 9090 with TLS
vrunner --port 9090 --tls -- daemon

# Instance 3: Production server on port 443 with custom certs
vrunner --port 443 --tls \
  --cert-file /etc/ssl/prod/cert.pem \
  --key-file /etc/ssl/prod/key.pem \
  --remote -- daemon
```

### Listing All Instances

```bash
vrunner list
```

Output format:
```
PID        PORT     BIND                 DAEMON     DISPLAY    COMMAND
12345      8080     127.0.0.1            yes        no         (idle)
12346      9090     127.0.0.1            yes        no         (idle)
12347      443      0.0.0.0              yes        no         (idle)
```

### Stopping a Specific Instance

```bash
vrunner stop 12345
```

### Using Different Configs Per Instance

Each instance can load a different configuration file:

```bash
# Dev instance
vrunner -c ./configs/dev.yaml --port 8080 -- daemon

# Staging instance
vrunner -c ./configs/staging.yaml --port 9090 -- daemon

# Production instance
vrunner -c /etc/vrunner/production.yaml --port 443 -- daemon
```

---

## Configuration File Reference

vrunner supports three configuration file formats: YAML, TOML, and JSON. The format is detected automatically from the file extension (`.yaml`/`.yml`, `.toml`, `.json`). Configuration is loaded from multiple locations in order of increasing precedence:

```
Built-in defaults → Global config → Local config → Explicit config file → CLI flags
```

| Location | Path |
|----------|------|
| Global config | `~/.config/vrunner/config.yaml` (or `.toml`) |
| Local config | `./vrunner.yaml` (or `.toml`) in the current directory |
| Explicit | Any path specified with `-c <FILE>` |

### YAML Example

```yaml
# vrunner.yaml
server:
  bind: "127.0.0.1"
  port: 8080

security:
  require_auth: false
  token_file: "~/.config/vrunner/token"

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
  stdout_file: "/tmp/vrunner.out"
  stderr_file: "/tmp/vrunner.err"

handles: []
```

### TOML Example

```toml
# vrunner.toml
[server]
bind = "127.0.0.1"
port = 8080

[security]
require_auth = false
token_file = "~/.config/vrunner/token"

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
stdout_file = "/tmp/vrunner.out"
stderr_file = "/tmp/vrunner.err"
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
    "token_file": "~/.config/vrunner/token"
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
    "stdout_file": "/tmp/vrunner.out",
    "stderr_file": "/tmp/vrunner.err"
  }
}
```

For the complete configuration reference with all keys, defaults, and CLI mappings, see [docs/configuration.md](configuration.md).

---

## Common Use Cases

### Development Server Orchestration

Run multiple development servers in a single vrunner instance, each in its own VTTY, accessible from the web:

```bash
# Start vrunner in idle mode with display disabled
vrunner --daemon --log --log-file /tmp/vrunner.log

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

Use vrunner to run CI jobs with full terminal access for debugging failed builds:

```bash
# Start a secure vrunner instance on the CI server
vrunner --remote --tls --port 8080 --daemon --log-file /var/log/vrunner-ci.log

# CI pipeline script spawns a build job
TOKEN=$(cat ~/.config/vrunner/token)

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
# On the remote server — start vrunner with TLS and auth
vrunner --remote --tls --port 443 --daemon

# Distribute ~/.config/vrunner/token and ~/.config/vrunner/cert.pem to admins

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
# Developer 1 starts vrunner with a shared session
vrunner --port 8080 --daemon

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
# Start a long data processing job via vrunner
vrunner --port 8080 --daemon

JOB_ID=$(curl -s -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "python", "args": ["process_large_dataset.py", "--input", "data.csv"]}' \
  | jq -r '.data.id')

# Disconnect from SSH — the job keeps running because vrunner is a daemon

# Reconnect later and check progress
curl -s "http://localhost:8080/api/commands/$JOB_ID/vtty/partial?offset=0&limit=20" \
  | jq -r '.data.content'
```

---

## Troubleshooting

### vrunner won't start

Check that the port is not already in use:

```bash
# Check what is using port 8080
lsof -i :8080
ss -tlnp | grep 8080

# Use a different port
vrunner --port 9090
```

### Connection refused

Ensure the server is running and you are using the correct address:

```bash
# Verify the instance is running
vrunner list

# Check the bind address — 127.0.0.1 only accepts localhost connections
vrunner --bind 0.0.0.0  # to accept remote connections
```

### TLS certificate errors

When using self-signed certificates, clients must trust the certificate explicitly:

```bash
# With curl, use --cacert
curl --cacert ~/.config/vrunner/cert.pem https://localhost:8080/api/commands

# To bypass certificate verification (not recommended for production)
curl -k https://localhost:8080/api/commands
```

### Authentication failures

When auth is enabled, all API requests must include the bearer token:

```bash
# Get the token
cat ~/.config/vrunner/token

# Use it in requests
TOKEN=$(cat ~/.config/vrunner/token)
curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/commands
```

### Command exits immediately

Some commands require a TTY to function. vrunner provides a pseudo-terminal, but the command's environment may need adjustment:

```bash
# Verify TERM is set correctly
vrunner --term xterm-256color -- my-command

# Check if the command expects specific environment variables
vrunner -- cmd="env TERM=xterm-256color my-command"
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

The log endpoint checks several common paths. If your log file is in a custom location, ensure the path matches what was passed to `--log-file` or configured in `command_log.file`. The API attempts to read from `/tmp/vrunner.log`, `./vrunner.log`, and `./vrunner-commands.log` by default.

---

## Connection Types Supported

vrunner supports **HTTP and HTTPS (TLS)** connections. **WebSocket is not currently supported** — terminal output is retrieved via REST polling. The following connection modes are available:

| Mode | CLI Flag | Description |
|------|----------|-------------|
| HTTP (localhost) | *(default)* | Plain HTTP on `127.0.0.1:8080`, no auth required |
| HTTP (remote) | `--remote` | Plain HTTP on `0.0.0.0:8080`, auth required |
| HTTPS (localhost) | `--tls` | HTTPS on `127.0.0.1:8080`, auto-generated self-signed cert |
| HTTPS (remote) | `--remote --tls` | HTTPS on `0.0.0.0:8080`, auth required, self-signed cert |
| HTTPS (custom cert) | `--tls --cert-file X --key-file Y` | HTTPS with your own certificate and key |

WebSocket support is a planned feature. To add it, the `ws` feature would need to be enabled in axum, and WebSocket upgrade handlers would need to be added to the router for streaming VTTY output in real time.
