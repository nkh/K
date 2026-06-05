# Terminal View (VTTY)

The terminal view is the main content area that renders command output in real time. It emulates a virtual terminal (VTTY) in the browser, supporting ANSI escape sequences, colors, and cursor positioning.

![Terminal view](screenshots/08-terminal-view.png)

## Per-Panel WebSocket Architecture

Each panel in the UI maintains its own dedicated WebSocket connection to stream VTTY updates independently. When a command is selected for a panel, `connectPanelWs(panelId)` opens a WebSocket to `/api/commands/{id}/ws` scoped to that panel. When the command is changed or the panel is removed, `disconnectPanelWs(panelId)` closes it. This per-panel model allows multiple panels to display output from different commands (and different server instances) simultaneously, each with its own independent update stream, latency measurement, and reconnection logic.

The per-panel WebSocket handles five message types from the server: `vtty_full` (complete HTML snapshot), `vtty_diff` (incremental cell-level diff), `vtty_dirty` (buffer-changed notification), `command_ended` (process exit notification), and `pong` (latency measurement response). All VTTY updates are routed exclusively to the panel's own DOM elements via the panel's state object (`state.panels[panelId]`).

## Rendering

Terminal output is rendered inside a `<pre>` element using monospace fonts. The content is updated via two transport modes:

- **Push (WebSocket)**: The server sends lightweight `vtty_dirty` or `vtty_diff` notifications when the buffer changes. The client then fetches fresh HTML via `GET /api/commands/{id}/vtty/html`. This is the default and most efficient mode. The incremental diff protocol (Level 3) sends only changed cells, reducing bandwidth significantly.
- **Poll (HTTP)**: The client periodically calls `GET /api/commands/{id}/vtty/changed` to check if the buffer has changed, then fetches updated HTML. The poll interval is configurable in the bottom bar.

## Terminal Switching

When switching between commands (clicking in the sidebar, using prev/next navigation, or spawning a new command), the terminal display is cleared immediately. Only the active command's output is written to the terminal. Stale cell grids and generation trackers from the previous command are discarded, ensuring a clean state for the new command.

## Buffer Support

The terminal supports both main and alternate screen buffers:

- **Main buffer**: The standard scrollback buffer used by most command-line applications. Content scrolls up as new output appears.
- **Alternate buffer**: Used by full-screen applications (htop, vim, less, top). This buffer replaces the main buffer and typically has no scrollback.

Switch between buffers using the **Buffer Select** dropdown in the panel header. When viewing the alternate buffer, an **ALT SCREEN** badge appears.

## Navigation

- **Scrolling**: Use the mouse wheel or scrollbar to navigate through the terminal scrollback. When scrolled up from the bottom, a **SCROLLBACK** indicator appears in the bottom bar and a **↓** button appears in the lower-right corner of the terminal.
- **Scroll to bottom**: Click the **↓** button or use the `End` key to jump to the latest output.

## Search

Each terminal panel has a built-in search bar (toggled via `Ctrl+F` or the panel header). The search highlights all matching occurrences in the terminal buffer and allows navigation between matches with **↓** (next) and **↑** (previous) buttons.

## Selection Mode

Toggle selection mode via **Ctrl+Shift+S**, **Alt+S**, or the panel header context menu. When active:

- Mouse events are not forwarded to the PTY, enabling native browser text selection
- A blue outline appears around the terminal to indicate selection mode is active
- Selected text can be copied to the clipboard via the **Copy** button or `Ctrl+Shift+C`

## Per-Panel Customization

Each panel supports individual customization:

| Feature | Control |
|---------|---------|
| Font size | `A-` / `A+` buttons in panel header |
| Theme | Panel theme toggle button (`◯` / `☾` / `☀`) in panel header |
| Terminal dimensions | `Rows` / `Cols` / `Resize` in panel header |
| Refresh throttle | `-` / `off` / `+` in panel header |
| Selection mode | `Ctrl+Shift+S` or context menu |
| Screenshot | `📷` button in panel header |
| Export | `⤋` button in panel header |
