# vrunner HTTP API Reference

Complete reference for the vrunner REST API and WebSocket endpoints.

## Base URL

```
http://<host>:<port>
```

Default port: **9090** (configurable via `--port` or `web.port` in config).

## Authentication

When a bearer token is configured (via `--token <token>` or `web.token` in config),
all API endpoints require an `Authorization` header:

```
Authorization: Bearer <token>
```

WebSocket connections pass the token as a query parameter:

```
ws://<host>:<port>/api/commands/<id>/ws?token=<token>
```

Without a configured token, all endpoints are open (no auth required).

## Response Format

All JSON responses follow this structure:

```json
{
  "status": "ok",
  "data": { ... },
  "error": null
}
```

On error:

```json
{
  "status": "error",
  "data": null,
  "error": "Error description"
}
```

---

## Endpoints

### Commands

#### `GET /api/commands`

List all running commands.

**Response:**

```json
{
  "status": "ok",
  "data": [
    {
      "id": "uuid-string",
      "name": "/usr/bin/htop",
      "args": [],
      "pid": 12345,
      "status": "running",
      "certificate": null,
      "exit": {
        "on_exit": "",
        "on_error": "",
        "exit_timeout": 10
      }
    }
  ]
}
```

---

#### `POST /api/commands`

Spawn a new command.

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cmd` | string | Yes | Command path to execute |
| `args` | string[] | No | Arguments to pass to the command |
| `certificate` | string | No | Name of a certificate from the certificate store to bind |
| `on_exit` | string | No | Command to run when the process exits |
| `on_error` | string | No | Command to run when the process exits with non-zero status |
| `exit_timeout` | number | No | Timeout in seconds for exit handlers (default: 10) |
| `rows` | number | No | VTTY rows (1-200, overrides config default) |
| `cols` | number | No | VTTY columns (1-500, overrides config default) |
| `env` | object | No | Per-command environment variables (merged with config-level env) |
| `no_env` | boolean | No | When true, skip all config-level environment variables |

**Response:**

```json
{
  "status": "ok",
  "data": { "id": "uuid-string", "pid": 12345 }
}
```

---

#### `POST /api/commands/:id/keys`

Send keystrokes to a running command's PTY stdin.

**Request body:**

```json
{ "keys": "q" }
```

The `keys` string supports special key notation:

| Notation | Result |
|----------|--------|
| `<Enter>` or `<Return>` | Carriage return (`\r`) |
| `<Esc>` | Escape (`\x1b`) |
| `<Tab>` | Tab (`\x09`) |
| `<Backspace>` | Backspace (`\x7f`) |
| `<Delete>` | Delete (`\x1b[3~`) |
| `<Up>`, `<Down>`, `<Left>`, `<Right>` | Arrow keys |
| `<F1>` through `<F12>` | Function keys |
| `<C-c>`, `<C-d>`, `<C-z>` | Ctrl+letter |
| `<A-x>` | Alt+letter |
| Any other character | Sent as-is |

**Response:**

```json
{
  "status": "ok",
  "data": { "id": "uuid-string", "keys_sent": "q" }
}
```

---

#### `POST /api/commands/:id/kill`

Kill a running command (sends SIGINT by default, equivalent to Ctrl+C).

**Request body (optional):**

```json
{ "signal": "SIGTERM" }
```

| Field | Type | Description |
|-------|------|-------------|
| `signal` | string | Signal name (e.g. `SIGTERM`, `SIGKILL`). Default: sends Ctrl+C byte (`\x03`) |

**Response:**

```json
{ "status": "ok", "data": { "id": "uuid-string" } }
```

---

#### `POST /api/commands/kill-pid/{pid}`

Kill a command by its OS PID (as opposed to command UUID).

**Response:**

```json
{ "status": "ok", "data": { "pid": 12345 } }
```

---

#### `POST /api/commands/:id/freeze`

Suspend a running command via `SIGSTOP`.

**Response:**

```json
{ "status": "ok", "data": { "id": "uuid-string", "frozen": true } }
```

---

#### `POST /api/commands/:id/thaw`

Resume a frozen command via `SIGCONT`.

**Response:**

```json
{ "status": "ok", "data": { "id": "uuid-string", "frozen": false } }
```

---

### VTTY (Virtual Terminal)

#### `GET /api/commands/:id/vtty`

Get the VTTY output as raw ANSI text.

**Response:**

```json
{ "status": "ok", "data": { "id": "...", "content": "raw ANSI text..." } }
```

---

#### `GET /api/commands/:id/vtty/html`

Get the VTTY output as rendered HTML with inline styles. This is the primary endpoint
used by the web UI to display terminal content. Each cell is a `<span>` with per-cell
color and style attributes.

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `scrollback_offset` | number | 0 | Number of lines to scroll back into the scrollback buffer |

**Response:**

```json
{
  "status": "ok",
  "data": {
    "id": "...",
    "html": "<span>...</span>",
    "cursor": { "row": 0, "col": 0 },
    "dimensions": { "rows": 24, "cols": 80 },
    "scrollback_lines": 0,
    "alternate_screen": false,
    "mouse_tracking": false
  }
}
```

---

#### `GET /api/commands/:id/vtty/buffer?screen=current`

Fetch a specific screen buffer as HTML.

**Query parameters:**

| Parameter | Values | Description |
|-----------|--------|-------------|
| `screen` | `current` (default), `main`, `alt` | Which buffer to return |

**Response:**

```json
{
  "status": "ok",
  "data": {
    "id": "...",
    "screen": "main",
    "html": "<span>...</span>",
    "alternate_screen": false,
    "dimensions": { "rows": 24, "cols": 80 }
  }
}
```

---

#### `GET /api/commands/:id/vtty/changed`

Lightweight dirty check for poll mode. Returns whether the buffer has changed
since the last call, without returning any HTML.

**Response:**

```json
{ "status": "ok", "data": { "id": "...", "changed": true } }
```

---

#### `GET /api/commands/:id/vtty/partial?offset=0&limit=50`

Get a paginated portion of the VTTY output as ANSI text.

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `offset` | number | 0 | Line offset to start from |
| `limit` | number | 50 | Maximum number of lines to return |

**Response:**

```json
{
  "status": "ok",
  "data": { "id": "...", "offset": 0, "limit": 50, "content": "..." }
}
```

---

#### `POST /api/commands/:id/resize`

Resize a command's virtual terminal. This resizes both the PTY master (sending
`SIGWINCH` to the child process) and the in-memory VTTY buffer.

**Request body:**

```json
{ "rows": 40, "cols": 120 }
```

| Field | Type | Range | Description |
|-------|------|-------|-------------|
| `rows` | number | 1-200 | Terminal rows |
| `cols` | number | 1-500 | Terminal columns |

**Response:**

```json
{ "status": "ok", "data": { "id": "...", "rows": 40, "cols": 120 } }
```

---

### Snapshots

#### `POST /api/commands/:id/snapshot`

Store a named snapshot of the command's current VTTY buffer. Snapshots are kept
in memory and can be used for diff comparisons.

**Request body:**

```json
{ "name": "before-test" }
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | `"default"` | Snapshot name |

**Response:**

```json
{
  "status": "ok",
  "data": {
    "id": "...",
    "name": "before-test",
    "command_name": "/usr/bin/htop",
    "command_args": [],
    "pid": 12345,
    "timestamp": "2026-05-24T12:00:00+00:00",
    "runtime_secs": 30
  }
}
```

---

#### `GET /api/commands/:id/snapshots`

List all stored snapshots for a command.

**Response:**

```json
{
  "status": "ok",
  "data": [ { "name": "before-test", "timestamp": "...", ... } ]
}
```

---

#### `POST /api/commands/:id/diff`

Compute a cell-by-cell diff of the current VTTY buffer against a stored snapshot.

**Request body:**

```json
{ "name": "before-test" }
```

**Response:**

```json
{
  "status": "ok",
  "data": {
    "id": "...",
    "name": "before-test",
    "width": 80,
    "height": 24,
    "changed_count": 5,
    "cells": [ { "row": 0, "col": 0, "old": "a", "new": "b" } ]
  }
}
```

---

#### `DELETE /api/commands/:id/snapshots/{name}`

Delete a stored snapshot.

**Response:**

```json
{ "status": "ok", "data": { "id": "...", "name": "before-test" } }
```

---

### Handles

#### `GET /api/commands/:id/handles`

List output handles attached to a command.

**Response:**

```json
{
  "status": "ok",
  "data": { "id": "...", "handles": [ { "name": "stdout", "type": "file" } ] }
}
```

---

#### `POST /api/commands/:id/handles`

Attach a new output handle to a command.

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Logical name for the handle (must be unique per command) |
| `sink` | string | Yes | Sink type: `"file"`, `"vtty"`, or `"null"` |
| `path` | string | No | File path for `"file"` sinks. Supports `{id}` and `{name}` placeholders. Ignored for other types. |

**Response:**

```json
{
  "status": "ok",
  "data": {
    "id": "uuid-string",
    "name": "stdout",
    "sink": "file",
    "message": "Handle attached successfully"
  },
  "error": null
}
```

---

### Server

#### `GET /api/info`

Get instance information and server configuration.

**Response:**

```json
{
  "status": "ok",
  "data": {
    "command_count": 3,
    "certificate_count": 1,
    "certificates": ["client-cert"],
    "auth_enabled": false,
    "web": {
      "update_mode": "push",
      "dirty_check_ms": 200,
      "default_poll_ms": 500
    }
  }
}
```

---

#### `GET /api/certificates`

List all certificates in the certificate store.

**Response:**

```json
{
  "status": "ok",
  "data": [
    {
      "name": "client-cert",
      "cert_file": "/path/to/cert.pem",
      "key_file": "/path/to/key.pem",
      "token_preview": "abcdef1234567890"
    }
  ]
}
```

---

#### `GET /api/log?search=&limit=200&offset=0`

Read command log entries. When a log file is configured (`--log-file` or
`command_log.file` in config), entries are read from the file. Otherwise,
entries are served from an in-memory ring buffer (last 2048 entries).

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `search` | string | (none) | Case-insensitive search filter |
| `limit` | number | 200 | Maximum entries to return |
| `offset` | number | 0 | Number of entries to skip |

**Response:**

```json
{
  "status": "ok",
  "data": {
    "lines": ["[2026-05-24T12:00:00Z] spawn: id=... cmd=..."],
    "total_lines": 42,
    "filtered_lines": 42,
    "offset": 0,
    "limit": 200,
    "search": "",
    "source": "memory"
  }
}
```

The optional `source` field indicates whether data came from `"file"` or `"memory"`.

---

#### `POST /api/shutdown`

Initiate a graceful server shutdown.

**Response:**

```json
{ "status": "ok", "data": { "message": "shutdown initiated" } }
```

---

## WebSocket Endpoints

### `GET /api/commands/:id/ws?token=<token>`

Real-time VTTY streaming. This is the primary mechanism for push-mode terminal updates.

**Connection lifecycle:**

1. Server sends `{"type":"connected","id":"<command-id>"}`
2. Server sends `{"type":"vtty_full","data":{"html":"...","cursor":{...},"dimensions":{...},"alternate_screen":false}}`
3. Server sends `{"type":"vtty_dirty","data":{"id":"..."}}` whenever the buffer changes
4. Client should fetch full HTML via `GET /api/commands/:id/vtty/html` after receiving `vtty_dirty`
5. Server sends `{"type":"command_ended","id":"..."}` when the command exits

**Client-to-server messages:**

| Type | Body | Description |
|------|------|-------------|
| `keys` | `{"type":"keys","keys":"q"}` | Send keystrokes (same syntax as HTTP endpoint) |
| `resize` | `{"type":"resize","rows":40,"cols":120}` | Resize the terminal |
| `ping` | `{"type":"ping"}` | Keep-alive; server responds with `{"type":"pong"}` |

**Server-to-client messages:**

| Type | Description |
|------|-------------|
| `connected` | Connection established |
| `vtty_full` | Full HTML snapshot (sent on connect and after broadcast lag) |
| `vtty_dirty` | Buffer has changed; client should fetch HTML via HTTP |
| `command_ended` | The command has exited |
| `error` | An error occurred (e.g. failed to send keys) |
| `pong` | Response to `ping` |

---

### `GET /api/ws/logs?token=<token>`

Real-time log streaming. Subscribes to the command logger's broadcast channel
and forwards log entries as they are written.

**Server-to-client messages:**

| Type | Body | Description |
|------|------|-------------|
| `connected` | `{"type":"connected","stream":"logs"}` | Connection established |
| `log_entry` | `{"type":"log_entry","data":"[timestamp] command: details"}` | New log entry |
| `pong` | `{"type":"pong"}` | Response to `ping` |

**Client-to-server messages:**

| Type | Description |
|------|-------------|
| `ping` | Keep-alive; server responds with `pong` |

---

## Web UI

### Admin Panel

The admin web interface is served at the root URL and `/admin`:

```
GET /
GET /admin
```

All static assets (HTML, CSS, JS, favicon) are embedded in the binary. No external
dependencies or CDN resources are required.

### Command-Name URL Routing

Any path that doesn't match an API endpoint is treated as a command name.
Navigate to `/<command-name>` to auto-select and view that command's VTTY:

```
/htop           → shows the htop command's terminal
/btop           → shows the btop command's terminal
/my-script.sh   → shows the my-script.sh command's terminal
```

If multiple commands share the same name, a picker overlay is displayed
letting you choose which one to view (showing arguments, PID, and status).

Paths that don't match any command name fall back to the admin panel.

### Multi-Instance View

Multi-instance view is supported via query parameters:

```
/?instance=http://host1:9090&label=Instance1&instance=http://host2:9091&label=Instance2
/admin?instance=http://host1:9090&label=Instance1&instance=http://host2:9091&label=Instance2
```

### Command Lookup API

```
GET /api/commands/lookup/:name
```

Returns all commands matching the given name (supports basename matching,
e.g. `/usr/bin/htop` matches `htop`).  Each result includes alive status
and runtime:

```json
{
  "status": "ok",
  "data": [
    {
      "id": "abc123",
      "name": "htop",
      "args": [],
      "pid": 12345,
      "alive": true,
      "runtime_secs": 342.5,
      "certificate": null
    }
  ],
  "error": null
}
```

### Command List Enhancements

The `GET /api/commands` response now includes per-command `alive` status and
`runtime_secs` fields, and the `status` field reflects the actual process
state (`"running"` or `"exited"`) instead of always returning `"running"`.

---

### Purge (Delete Retained Commands)

#### `DELETE /api/commands/:id`

Permanently remove a retained (exited) command from the manager. This discards the VTTY buffer, scrollback, and all associated state. Only commands that have exited and were spawned with `retain_on_exit: true` can be purged.

**Response:**

```json
{ "status": "ok", "data": { "id": "...", "purged": true } }
```

Error (command not found or not retained):

```json
{ "status": "error", "data": null, "error": "Command not found or not retained" }
```

---

### Mouse Events

#### `POST /api/commands/:id/mouse`

Forward a mouse event to a command's PTY. Used by the web UI to send mouse clicks, drags, and wheel events to the child process.

**Request body:**

```json
{
  "kind": "press",
  "button": "left",
  "row": 10,
  "col": 20,
  "modifiers": []
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `kind` | string | Yes | Event kind: `press`, `release`, `motion`, `wheel_up`, `wheel_down`, `drag` |
| `button` | string | Yes | Button name: `left`, `right`, `middle`, `none` |
| `row` | number | Yes | Row position (0-based) |
| `col` | number | Yes | Column position (0-based) |
| `modifiers` | string[] | No | Active modifier keys: `shift`, `ctrl`, `alt` |

**Response:**

```json
{ "status": "ok", "data": { "id": "...", "kind": "press", "button": "left", "row": 10, "col": 20 } }
```
