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

When authentication is enabled, pass the bearer token as a query parameter (WebSocket cannot set HTTP headers):

```javascript
const ws = new WebSocket('wss://host:9090/api/commands/.../ws?token=YOUR_TOKEN');
```

---

## VTTY WebSocket — `ws://host:port/api/commands/{id}/ws`

Bidirectional connection for real-time terminal output and keyboard input.

### Connection Lifecycle

1. Client opens WebSocket connection to the endpoint.
2. Server sends a `connected` message confirming the connection.
3. Server sends a `vtty_full` message with the complete terminal HTML state.
4. Server sends `vtty_dirty` messages when the buffer changes (lightweight notification — no cell data).
5. Client fetches fresh HTML via `GET /api/commands/{id}/vtty/html` upon receiving `vtty_dirty`.
6. If the client falls behind, server may send a new `vtty_full` to resynchronize.
7. When the command exits, server sends `command_ended` and closes the connection.

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

Full terminal HTML snapshot. Sent on connect and after broadcast lag recovery.

```json
{
  "type": "vtty_full",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "html": "<span class=\"bold\">htop</span> - process viewer\n...",
    "cursor": { "row": 5, "col": 12 },
    "dimensions": { "rows": 24, "cols": 80 },
    "alternate_screen": false,
    "cursor_visible": true
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
| `data.cursor_visible` | boolean | Whether the cursor should be displayed |

#### `vtty_dirty`

Lightweight dirty-change notification. Sent when the VTTY buffer has been modified. Contains no cell data — the client must fetch fresh HTML via HTTP.

```json
{
  "type": "vtty_dirty",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `data.id` | string | Command UUID whose buffer changed |

The server broadcasts `vtty_dirty` at a configurable interval (default: 200ms, controlled by `web.dirty_check_ms`). Rate limiting is applied per command (default: 30 updates/sec, controlled by `web.rate_limit.max_updates_per_sec`).

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

The `keys` string supports raw escape sequences and special key notation:

| Notation | Result |
|----------|--------|
| `\r` or `\n` | Carriage return |
| `\x1b` | Escape (`<Esc>`) |
| `\x09` or `\t` | Tab |
| `\x7f` | Backspace |
| `\x1b[3~` | Delete |
| `\x1b[A/B/C/D` | Arrow keys (Up/Down/Left/Right) |
| `\x1bOP` through `\x1b[19~` | Function keys (F1–F8) |
| `\x01` through `\x1c` | Ctrl+A through Ctrl+\ |
| Any other character | Sent as-is |

#### `paste`

Paste a block of text into the command's PTY. Uses bracketed-paste mode if the terminal supports it.

```json
{
  "type": "paste",
  "text": "pasted content here"
}
```

#### `resize`

Resize the command's virtual terminal. The child process receives SIGWINCH.

```json
{
  "type": "resize",
  "rows": 40,
  "cols": 120
}
```

Valid ranges: rows 1–200, cols 1–500. Defaults to 24x80 if not specified.

#### `ping`

Request a `pong` response for connection keepalive.

```json
{
  "type": "ping"
}
```

### Client Implementation Strategy

1. On `vtty_full`, render the HTML directly into the terminal container.
2. On `vtty_dirty`, debounce (e.g., 50ms) and fetch fresh HTML via `GET /api/commands/{id}/vtty/html`, then replace the rendered output. This avoids sending cell data over WebSocket while keeping the client simple.
3. On `command_ended`, display the exit status and close the WebSocket.
4. Send `ping` every 30 seconds to keep the connection alive.
5. On `error`, log the error and consider reconnecting with exponential backoff.

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
