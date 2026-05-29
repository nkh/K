# API Usage

Learn how to programmatically control vrunner using `curl` and shell scripts — from listing and killing commands to sending keystrokes, capturing snapshots, and reading logs.

All examples use `http://localhost:8080`. Replace with your server address as needed. For the full API reference, see [`../api.md`](../api.md).

## Universal Response Envelope

Every JSON endpoint uses the same response structure:

```json
{ "status": "ok", "data": { ... }, "error": null }
```

On error:

```json
{ "status": "error", "data": null, "error": "Missing 'cmd' field" }
```

## Authentication

When auth is enabled (via `--auth <token>` or config), include the bearer token in requests:

```bash
# HTTP endpoints
curl -H "Authorization: Bearer <token>" http://localhost:8080/api/commands

# WebSocket endpoints (query param — WebSocket cannot set headers)
ws://localhost:8080/api/commands/<id>/ws?token=<token>
```

## Listing Commands

Retrieve all spawned commands with their status, PID, and metadata:

```bash
curl -s http://localhost:8080/api/commands | jq .
```

Response:

```json
{
  "status": "ok",
  "data": [
    {
      "id": "a1b2c3d4-...",
      "name": "/usr/bin/htop",
      "args": [],
      "pid": 12345,
      "alive": true,
      "frozen": false,
      "runtime_secs": 42.5,
      "exit_code": null,
      "status": "running",
      "certificate": null,
      "exit": {
        "on_exit": "",
        "on_error": "",
        "exit_timeout": 10,
        "retain_on_exit": false
      }
    }
  ]
}
```

Look up a command by name:

```bash
curl -s "http://localhost:8080/api/commands/lookup/htop" | jq .
```

## Spawning Commands

Spawn a new command via the API:

```bash
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "/usr/bin/htop"}'
```

Response:

```json
{ "status": "ok", "data": { "id": "a1b2c3d4-...", "pid": 12345 } }
```

### Full Spawn Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cmd` | string | *(required)* | Command executable to run |
| `args` | string[] | `[]` | Arguments to pass to the command |
| `dir` | string | null | Working directory override |
| `env` | object | `{}` | Per-command environment variables |
| `no_env` | boolean | `false` | Skip config-level env vars entirely |
| `certificate` | string | null | Certificate name for access control |
| `retain_on_exit` | boolean | `false` | Keep VTTY buffer after process exits |
| `exit_timeout` | integer | `10` | Seconds before SIGKILL after SIGTERM |
| `on_exit` | string | null | Shell command to run on clean exit (exit 0) |
| `on_error` | string | null | Shell command to run on non-zero exit |
| `snapshot_on_exit` | string | null | File path to save VTTY buffer on exit |
| `rows` | integer | from config | VTTY rows (1-200) |
| `cols` | integer | from config | VTTY cols (1-500) |

Example with options:

```bash
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "cmd": "npm",
    "args": ["run", "build"],
    "dir": "/home/user/project",
    "env": { "NODE_ENV": "production" },
    "retain_on_exit": true,
    "exit_timeout": 30,
    "on_error": "echo \"Build failed with exit code {exit_code}\""
  }'
```

## Killing Commands

Terminate a specific command by its ID:

```bash
curl -X POST http://localhost:8080/api/commands/a1b2c3d4/kill
```

Optionally specify a signal:

```bash
curl -X POST http://localhost:8080/api/commands/a1b2c3d4/kill \
  -H "Content-Type: application/json" \
  -d '{"signal": "SIGTERM"}'
```

Kill a command by its OS PID:

```bash
curl -X POST http://localhost:8080/api/commands/kill-pid/12345
```

Purge a command's VTTY data after it has exited (removes it from the command list):

```bash
curl -X DELETE http://localhost:8080/api/commands/a1b2c3d4
```

## Sending Keystrokes

Send keystrokes or control sequences to a running command:

```bash
curl -X POST http://localhost:8080/api/commands/a1b2c3d4/keys \
  -H "Content-Type: application/json" \
  -d '{"keys": "q"}'
```

### Common Escape Sequences

Use these special character sequences in the `keys` field:

| Sequence | Key | Example |
|----------|-----|---------|
| `\n` | Enter | `{"keys": "\n"}` |
| `\r` | Carriage return | `{"keys": "\r"}` |
| `\x03` | Ctrl+C | `{"keys": "\x03"}` |
| `\x04` | Ctrl+D | `{"keys": "\x04"}` |
| `\x1a` | Ctrl+Z | `{"keys": "\x1a"}` |
| `\x17` | Ctrl+W | `{"keys": "\x17"}` |
| `\x15` | Ctrl+U | `{"keys": "\x15"}` |
| `\x0c` | Ctrl+L | `{"keys": "\x0c"}` |
| `\x7f` | Backspace | `{"keys": "\x7f"}` |
| `\t` | Tab | `{"keys": "\t"}` |
| `\x1b[A` | Up arrow | `{"keys": "\x1b[A"}` |
| `\x1b[B` | Down arrow | `{"keys": "\x1b[B"}` |
| `\x1b[C` | Right arrow | `{"keys": "\x1b[C"}` |
| `\x1b[D` | Left arrow | `{"keys": "\x1b[D"}` |
| `\x1b` | Escape | `{"keys": "\x1b"}` |

Send a multi-character string:

```bash
# Type "ls -la" and press Enter
curl -X POST http://localhost:8080/api/commands/a1b2c3d4/keys \
  -H "Content-Type: application/json" \
  -d '{"keys": "ls -la\r"}'
```

### Sending Mouse Events

```bash
curl -X POST http://localhost:8080/api/commands/a1b2c3d4/mouse \
  -H "Content-Type: application/json" \
  -d '{"event": "down", "button": 0, "x": 10, "y": 5}'
```

| Field | Values | Description |
|-------|--------|-------------|
| `event` | `"down"`, `"up"`, `"move"`, `"wheel_up"`, `"wheel_down"` | Mouse event type |
| `button` | `0` (left), `1` (middle), `2` (right) | Mouse button |
| `x` | integer (1-based) | Column position |
| `y` | integer (1-based) | Row position |

## Freeze and Thaw

Pause a command's output processing (freeze) and resume it later (thaw):

```bash
# Freeze — sends SIGSTOP to the process
curl -X POST http://localhost:8080/api/commands/a1b2c3d4/freeze

# Thaw — sends SIGCONT to the process
curl -X POST http://localhost:8080/api/commands/a1b2c3d4/thaw
```

This is useful for inspecting terminal output without it scrolling away.

## Resizing

Change the terminal dimensions of a running command:

```bash
curl -X POST http://localhost:8080/api/commands/a1b2c3d4/resize \
  -H "Content-Type: application/json" \
  -d '{"rows": 40, "cols": 120}'
```

## VTTY Endpoints

vrunner provides multiple ways to retrieve terminal output, each suited to different use cases.

### Full ANSI Output

Returns the complete terminal buffer as raw ANSI text:

```bash
curl -s http://localhost:8080/api/commands/a1b2c3d4/vtty | jq .
```

Response:

```json
{ "status": "ok", "data": { "id": "a1b2c3d4-...", "content": "<ANSI text>" } }
```

### Plain Text Output

Returns the terminal content stripped of ANSI formatting:

```bash
curl -s http://localhost:8080/api/commands/a1b2c3d4/vtty/text | jq .
```

Response:

```json
{ "status": "ok", "data": { "id": "a1b2c3d4-...", "text": "plain text content" } }
```

### HTML Output

Returns the terminal rendered as styled HTML, suitable for embedding in web pages:

```bash
curl -s http://localhost:8080/api/commands/a1b2c3d4/vtty/html | jq .
```

Response includes cursor position, dimensions, and screen state:

```json
{
  "status": "ok",
  "data": {
    "id": "a1b2c3d4-...",
    "html": "<pre>...</pre>",
    "cursor": { "row": 12, "col": 40 },
    "dimensions": { "rows": 24, "cols": 80 },
    "scrollback_lines": 500,
    "alternate_screen": false,
    "cursor_visible": true
  }
}
```

### Buffer Endpoint

Retrieve a specific screen buffer (main or alternate):

```bash
# Current screen (auto-selects main or alt)
curl -s "http://localhost:8080/api/commands/a1b2c3d4/vtty/buffer"

# Force the main buffer
curl -s "http://localhost:8080/api/commands/a1b2c3d4/vtty/buffer?screen=main"
```

### Partial Output

Retrieve a range of rows from the terminal buffer:

```bash
# Get 50 lines starting from offset 0 (default)
curl -s "http://localhost:8080/api/commands/a1b2c3d4/vtty/partial"

# Get 30 lines starting from line 100
curl -s "http://localhost:8080/api/commands/a1b2c3d4/vtty/partial?offset=100&limit=30"
```

### Dirty Check

Check if the buffer has changed since the last fetch (for poll-mode clients):

```bash
curl -s "http://localhost:8080/api/commands/a1b2c3d4/vtty/changed" | jq .
```

```json
{ "status": "ok", "data": { "id": "a1b2c3d4-...", "changed": true } }
```

## Resource Monitoring

Retrieve CPU and memory usage for a running command (Linux only, reads from `/proc`):

```bash
curl -s http://localhost:8080/api/commands/a1b2c3d4/resources | jq .
```

Response:

```json
{
  "status": "ok",
  "data": {
    "pid": 12345,
    "cpu_percent": 2.5,
    "memory_mb": 14.3,
    "threads": 4,
    "alive": true
  }
}
```

## Sharing a Terminal

Create a time-limited share link for read-only (or read-write) terminal access:

```bash
curl -X POST http://localhost:8080/api/commands/a1b2c3d4/share \
  -H "Content-Type: application/json" \
  -d '{"keyboard": false, "expires_hours": 24}'
```

Response:

```json
{
  "status": "ok",
  "data": {
    "token": "e5f6a7b8-...",
    "url": "/share/e5f6a7b8-...",
    "expires_at": "24h from now",
    "keyboard": false
  }
}
```

The share URL is publicly accessible at `/share/<token>` without authentication.

## Snapshots and Diffs

Capture terminal state at a point in time and compare against the current state:

```bash
# Create a snapshot named "after-build"
curl -X POST http://localhost:8080/api/commands/a1b2c3d4/snapshot \
  -H "Content-Type: application/json" \
  -d '{"name": "after-build"}'

# List all snapshots for a command
curl -s http://localhost:8080/api/commands/a1b2c3d4/snapshots | jq .

# Compute diff against a named snapshot
curl -X POST http://localhost:8080/api/commands/a1b2c3d4/diff \
  -H "Content-Type: application/json" \
  -d '{"name": "after-build"}'

# Delete a snapshot
curl -X DELETE http://localhost:8080/api/commands/a1b2c3d4/snapshots/after-build
```

See [`snapshots-diffs.md`](snapshots-diffs.md) for full details.

## Log Reading

Read the global event log (not per-command — logs are server-wide):

```bash
# Read the full log
curl -s http://localhost:8080/api/log | jq .

# Search for a pattern
curl -s "http://localhost:8080/api/log?search=error" | jq .

# Limit the number of entries
curl -s "http://localhost:8080/api/log?limit=20" | jq .
```

## Scripting Example

Here is a complete script that spawns a command, waits for it to finish, and retrieves its output:

```bash
#!/usr/bin/env bash
set -euo pipefail

SERVER="http://localhost:8080"

# Spawn the command
RESPONSE=$(curl -s -X POST "$SERVER/api/commands" \
  -H "Content-Type: application/json" \
  -d '{"cmd": "npm", "args": ["run", "build"], "retain_on_exit": true}')

CMD_ID=$(echo "$RESPONSE" | jq -r '.data.id')
echo "Spawned command: $CMD_ID"

# Poll until finished
while true; do
  STATUS=$(curl -s "$SERVER/api/commands" | \
    jq -r ".data[] | select(.id == \"$CMD_ID\") | .status")
  if [ "$STATUS" != "running" ] && [ "$STATUS" != "frozen" ]; then
    echo "Command finished with status: $STATUS"
    break
  fi
  sleep 2
done

# Retrieve final output
curl -s "$SERVER/api/commands/$CMD_ID/vtty/text" | jq -r '.data.text'
echo "Output retrieved."
```

## Server Info

Get runtime information about the vrunner instance:

```bash
curl -s http://localhost:8080/api/info | jq .
```

```json
{
  "status": "ok",
  "data": {
    "command_count": 3,
    "certificate_count": 1,
    "certificates": ["webapp-frontend"],
    "auth_enabled": true,
    "web": {
      "update_mode": "push",
      "dirty_check_ms": 200,
      "default_poll_ms": 500
    }
  }
}
```

## Graceful Shutdown

Shut down the vrunner server:

```bash
curl -X POST http://localhost:8080/api/shutdown
```

For full endpoint documentation, see [`../api.md`](../api.md).
