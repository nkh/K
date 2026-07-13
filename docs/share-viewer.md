# Terminal Sharing & Viewer

The share and viewer feature provides two ways to view a terminal outside the main admin panel: **Share Terminal** for giving teammates a live read-only (or interactive) link, and **Open in New Tab** for a clean, distraction-free view in your own browser.

---

## Share Terminal

Share a running terminal with anyone by generating a link. The recipient opens the link in their browser and sees a real-time, standalone terminal viewer — no login required.

### Creating a Share Link

1. Right-click a panel header or a command in the sidebar.
2. Select **Share Terminal…**.
3. Configure the share options in the modal:
   - **Label** — optional description shown in the viewer header
   - **Keyboard access** — allow the viewer to send keystrokes (off by default)
   - **Expiration** — how long the link remains valid

4. Click **Create Link**. The modal shows the URL — copy it and send it to your teammate.

### Expiration Options

| Duration | Value |
|----------|-------|
| 1 hour | `1` |
| 4 hours | `4` |
| 24 hours | `24` |
| 3 days | `72` |
| 1 week | `168` |
| Never | `0` |

### Viewer Experience

The share link opens `viewer.html` — a standalone page with no sidebar, top bar, or other admin UI. The page displays the terminal output in real time via WebSocket. If keyboard access was granted, the viewer can type into the terminal; otherwise, it is read-only.

When the share link expires or the token is invalidated, the page shows an error and stops updating.

---

## Open in New Tab

Opens the same standalone viewer in a new browser tab, but authenticated as you. No share link or expiration to manage.

### Usage

1. Right-click a panel header.
2. Select **Open in New Tab**.
3. A new tab opens with the viewer, authenticated via a 1-hour token.

Full keyboard access is always enabled (you are the owner). The token expires after one hour — simply repeat the action to get a fresh tab.

---

## API Reference

### Authenticated Endpoints

These require a Bearer token in the `Authorization` header.

#### `POST /api/commands/:id/share`

Create a share token for a command.

**Request body:**

```json
{
  "keyboard": false,
  "expires_hours": 24,
  "label": "deploy logs"
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `keyboard` | boolean | `false` | Allow the viewer to send keystrokes |
| `expires_hours` | number | `24` | Hours until the token expires (`0` = never) |
| `label` | string | `null` | Optional description for the viewer header |

**Response** (`200`):

```json
{
  "status": "ok",
  "data": {
    "token": "a1b2c3d4-...",
    "url": "/share/a1b2c3d4-...",
    "expires_at": "2026-06-15T18:00:00Z",
    "keyboard": false
  }
}
```

#### `GET /api/viewer/:id`

Create a 1-hour viewer token for the **Open in New Tab** flow. Full keyboard access is always enabled.

**Response** (`200`):

```json
{
  "status": "ok",
  "data": {
    "cmd_id": "550e8400-e29b-41d4-a716-446655440000",
    "keyboard": true,
    "token": "e5f6g7h8-..."
  }
}
```

### Public Endpoints

These are authenticated by the share/viewer token in the URL path — no Bearer token required.

#### `GET /api/share/:token`

Validate a share token and retrieve the initial terminal state.

**Response** (`200`):

```json
{
  "status": "ok",
  "data": {
    "cmd_id": "550e8400-...",
    "keyboard": false,
    "html": "<span class=\"bold\">htop</span> - process viewer\n...",
    "dimensions": { "rows": 24, "cols": 80 },
    "label": "deploy logs"
  }
}
```

**Error responses:**

| Status | Meaning |
|--------|---------|
| `404` | Token not found |
| `410` | Token has expired |

### Pages

#### `GET /share/:token`

Serves the `viewer.html` page for share links. Validates the token before serving — returns `404` or `410` if the token is invalid or expired.

#### `GET /viewer/:token`

Serves the `viewer.html` page for **Open in New Tab** tokens. The token is created by the authenticated `/api/viewer/:id` endpoint.

---

## Share WebSocket — `ws://host:port/api/share/:token/ws`

Real-time streaming for shared and viewer terminals. The protocol mirrors the main VTTY WebSocket (see [WebSocket Protocol](./websocket.md)) with additional access-control enforcement.

### Connection Lifecycle

1. Client opens the WebSocket connection.
2. Server validates the share token. If invalid or expired, the upgrade is rejected.
3. Server sends a `connected` message.
4. Server sends `vtty_dirty` messages when the terminal buffer changes.
5. Client fetches incremental HTML via `GET /api/commands/{cmd_id}/vtty/diff?baseline=...` on dirty.
6. On command exit, server sends `command_ended`.

### Server-to-Client Messages

| Type | Description |
|------|-------------|
| `connected` | Connection confirmed |
| `vtty_dirty` | Terminal buffer changed — client should fetch updated HTML |
| `vtty_close` | Terminal session closed |
| `command_ended` | The underlying command has exited |
| `pong` | Response to a `ping` |
| `error` | Processing error (e.g., rejected keystroke) |

### Client-to-Server Messages

| Type | Allowed When Read-Only | Description |
|------|------------------------|-------------|
| `keys` | No | Send keystrokes to the terminal |
| `paste` | No | Paste text into the terminal |
| `resize` | No | Resize the virtual terminal |
| `ping` | Yes | Keepalive request |

If the share token was created with `keyboard: false`, the server rejects `keys`, `paste`, and `resize` messages with an `error` response.

---

## Architecture

### Share Flow

```
Admin panel → POST /api/commands/:id/share → receives token
Admin copies /share/{token} URL → sends to teammate
Teammate opens URL → viewer.html loads
viewer.html → GET /api/share/{token} → initial HTML + metadata
viewer.html → WS /api/share/{token}/ws → real-time dirty signals
viewer.html → GET /api/commands/{cmd_id}/vtty/diff?baseline=... → incremental updates
```

### Open in New Tab Flow

```
Admin panel → GET /api/viewer/{cmd_id} → receives 1-hour token
Admin panel → window.open('/viewer/{token}') → new tab
viewer.html → same WS + diff flow as share
```

Both flows use the same `viewer.html` page and the same WebSocket protocol. The only differences are token creation and keyboard access defaults.

---

## Security Model

| Property | Detail |
|----------|--------|
| **Token format** | UUID v4 |
| **Storage** | In-memory (`DashMap`), never persisted to disk |
| **Expiration** | Configured per share; enforced on access and by periodic cleanup |
| **Keyboard access** | Explicitly opt-in per share link (`keyboard: false` by default) |
| **Token creation** | Share tokens require Bearer auth; viewer tokens require Bearer auth |
| **Viewer pages** | Public (authenticated by the token in the URL) |
| **WS validation** | Token is validated before the WebSocket upgrade is accepted |

Share tokens are ephemeral. Restarting the server invalidates all outstanding share links. Expired tokens are cleaned up lazily on access and periodically by a background task.

---

## Files

| File | Purpose |
|------|---------|
| `src/web/handlers/share.rs` | `ws_share_stream`, `create_viewer_token`, `validate_share_token`, `handle_share_vtty_socket` |
| `src/web/state.rs` | `label` field on `ShareToken` |
| `src/web/router.rs` | Routes: `/api/share/:token/ws`, `/api/viewer/:id`, `/viewer/:token` |
| `src/web/handlers/admin.rs` | Viewer page serve, replaced inline share page with `viewer.html` |
| `static/admin/viewer.html` | Standalone terminal viewer page |
| `static/admin/modules/api.js` | `createShareToken()`, `createViewerToken()` |
| `static/admin/modules/panels-context-menu.js` | Context menu items: "Share Terminal…" and "Open in New Tab" |
| `static/admin/test/test_share_viewer.js` | Test coverage for share/viewer flows |
| `static/admin/test/test_api.js` | Assertions for new API methods |