# vrunner WebSocket Protocol Specification

This document specifies the WebSocket message formats used by vrunner's two WebSocket endpoints.

---

## Connections

WebSocket connections upgrade from standard HTTP requests. Both endpoints use JSON text frames for all messages.

### With TLS

When vrunner runs with `--tls`, use `wss://` instead of `ws://`:

```javascript
const ws = new WebSocket('wss://host:9090/api/commands/.../ws');
```

### With Authentication

When authentication is enabled, pass the bearer token as a query parameter:

```javascript
const ws = new WebSocket('wss://host:9090/api/commands/.../ws?token=YOUR_TOKEN');
```

---

## VTTY WebSocket — `ws://host:port/api/commands/:id/ws`

Bidirectional connection for real-time terminal output and keyboard input.

### Connection Lifecycle

1. Client opens WebSocket connection to the endpoint.
2. Server sends a `connected` message confirming the connection.
3. Server sends a `vtty_full` message with the complete terminal state.
4. Server periodically sends `vtty_diff` messages with only changed cells.
5. If the client falls behind, server sends a new `vtty_full` to resynchronize.
6. When the command exits, server sends `command_ended` and closes the connection.

### Server-to-Client Messages

#### `connected`

Sent immediately after WebSocket upgrade confirms the connection.

```json
{
  "type": "connected",
  "id": "550e8400-e29b-41d4-a716-446655440000"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `type` | `"connected"` | Message type identifier |
| `id` | string | Command UUID |

#### `vtty_full`

Full terminal snapshot. Sent on connect and after broadcast lag recovery.

```json
{
  "type": "vtty_full",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "html": "<span class=\"bold\">htop</span> - process viewer\n...",
    "cursor": { "row": 5, "col": 12 },
    "dimensions": { "rows": 24, "cols": 80 },
    "alternate_screen": false
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `data.id` | string | Command UUID |
| `data.html` | string | Complete terminal rendered as inline HTML |
| `data.cursor` | object | Current cursor position (`row`, `col`) |
| `data.dimensions` | object | Terminal dimensions (`rows`, `cols`) |
| `data.alternate_screen` | boolean | Whether the alternate screen buffer is active |

#### `vtty_diff`

Incremental diff containing only changed cells. Sent every ~200ms when the buffer changes.

```json
{
  "type": "vtty_diff",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "diff": {
      "width": 80,
      "height": 24,
      "changed_count": 3,
      "cells": [
        {
          "row": 5, "col": 10, "ch": "A",
          "fg": [255, 255, 255], "bg": [0, 0, 0],
          "bold": false, "italic": false, "underline": false,
          "strikethrough": false, "dim": false, "reverse": false,
          "hidden": false, "wide": false
        }
      ]
    },
    "cursor": { "row": 5, "col": 13 },
    "dimensions": { "rows": 24, "cols": 80 },
    "alternate_screen": false
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `data.diff.width` | number | Buffer width in columns |
| `data.diff.height` | number | Buffer height in rows |
| `data.diff.changed_count` | number | Number of changed cells in this diff |
| `data.diff.cells` | array | Array of changed cell objects |
| `data.diff.cells[].row` | number | Cell row position (0-based) |
| `data.diff.cells[].col` | number | Cell column position (0-based) |
| `data.diff.cells[].ch` | string | Character (single Unicode codepoint) |
| `data.diff.cells[].fg` | array | Foreground RGB [R, G, B] (0–255) |
| `data.diff.cells[].bg` | array | Background RGB [R, G, B] (0–255) |
| `data.diff.cells[].bold` | boolean | Bold text attribute |
| `data.diff.cells[].italic` | boolean | Italic text attribute |
| `data.diff.cells[].underline` | boolean | Underline text attribute |
| `data.diff.cells[].strikethrough` | boolean | Strikethrough text attribute |
| `data.diff.cells[].dim` | boolean | Dim/faint text attribute |
| `data.diff.cells[].reverse` | boolean | Reverse video (swap fg/bg) |
| `data.diff.cells[].hidden` | boolean | Hidden text attribute |
| `data.diff.cells[].wide` | boolean | Wide character (occupies two columns) |

#### `command_ended`

Sent when the command exits. The connection is not closed by the server; the client should close it.

```json
{
  "type": "command_ended",
  "id": "550e8400-e29b-41d4-a716-446655440000"
}
```

#### `error`

Sent when an incoming client message fails to process.

```json
{
  "type": "error",
  "message": "Failed to send keys: command not found"
}
```

#### `pong`

Response to a `ping` message from the client.

```json
{
  "type": "pong"
}
```

### Client-to-Server Messages

#### `keys`

Send keystrokes to the command's PTY stdin.

```json
{
  "type": "keys",
  "keys": "ls -la\r"
}
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

#### `resize`

Resize the command's virtual terminal. The child process receives SIGWINCH.

```json
{
  "type": "resize",
  "rows": 40,
  "cols": 120
}
```

Valid ranges: rows 1–200, cols 1–500.

#### `ping`

Request a `pong` response for connection keepalive.

```json
{
  "type": "ping"
}
```

### Client Implementation Strategy

1. On `vtty_full`, render the HTML directly.
2. On `vtty_diff`, either:
   - Apply cell-level DOM updates for optimal performance, or
   - Fetch full HTML via `GET /api/commands/:id/vtty/html` for correctness.
3. On `command_ended`, close the WebSocket.
4. Send `ping` every 30 seconds to keep the connection alive.
5. On `error`, log the error and consider reconnecting.

---

## Log WebSocket — `ws://host:port/api/ws/logs`

Read-only stream of command log entries.

### Server-to-Client Messages

#### `connected`

```json
{
  "type": "connected",
  "stream": "logs"
}
```

#### `log_entry`

```json
{
  "type": "log_entry",
  "data": "2026-05-24T12:00:00Z spawn id=550e8400 cmd=htop args=[]"
}
```

#### `pong`

```json
{
  "type": "pong"
}
```

### Client-to-Server Messages

| Type | Body | Description |
|------|------|-------------|
| `ping` | `{"type":"ping"}` | Keepalive request |

---

## Error Handling

Both WebSocket endpoints use the same error conventions:

- **Connection errors**: The HTTP upgrade response (e.g., 404 for unknown commands, 401 for auth failure) indicates the error before the WebSocket is established.
- **Protocol errors**: After upgrade, `error` messages describe processing failures.
- **Connection drops**: Clients should implement reconnection with exponential backoff.
- **Idle timeout**: There is no server-side idle timeout. Clients should send periodic `ping` messages.
