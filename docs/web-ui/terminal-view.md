# Terminal View (VTTY)

The terminal view is the main content area that renders command output in real time. It emulates a virtual terminal (VTTY) in the browser, supporting ANSI escape sequences, colors, and cursor positioning.

![Terminal view](screenshots/08-terminal-view.png)

## Rendering

Terminal output is rendered inside a `<pre>` element using monospace fonts. The content is updated via two transport modes:

- **Push (WebSocket)**: The server sends lightweight `vtty_dirty` notifications when the buffer changes. The client then fetches fresh HTML via `GET /api/commands/{id}/vtty/html`. This is the default and most efficient mode.
- **Poll (HTTP)**: The client periodically calls `GET /api/commands/{id}/vtty/changed` to check if the buffer has changed, then fetches updated HTML. The poll interval is configurable in the bottom bar.

## Buffer Support

The terminal supports both main and alternate screen buffers:

- **Main buffer**: The standard scrollback buffer used by most command-line applications. Content scrolls up as new output appears.
- **Alternate buffer**: Used by full-screen applications (htop, vim, less, top). This buffer replaces the main buffer and typically has no scrollback.

Switch between buffers using the **Buffer Select** dropdown in the top bar. When viewing the alternate buffer, an **ALT SCREEN** badge appears.

## Navigation

- **Scrolling**: Use the mouse wheel or scrollbar to navigate through the terminal scrollback. When scrolled up from the bottom, a **SCROLLBACK** indicator appears in the bottom bar and a **↓** button appears in the lower-right corner of the terminal.
- **Scroll to bottom**: Click the **↓** button or use the `End` key to jump to the latest output.

## Search

Each terminal panel has a built-in search bar (toggled via `Ctrl+F` or the right-click context menu). The search highlights all matching occurrences in the terminal buffer and allows navigation between matches with **↓** (next) and **↑** (previous) buttons.

## Selection Mode

Toggle selection mode via the **Select** button in the panel header or the right-click context menu. When active:

- Mouse events are not forwarded to the PTY, enabling native browser text selection
- A blue outline appears around the terminal to indicate selection mode is active
- Selected text can be copied to the clipboard via the **Copy** button or `Ctrl+C`

## Per-Panel Customization

Each panel supports individual customization:

| Feature | Control |
|---------|---------|
| Font size | `A-` / `A+` buttons in panel header |
| Theme | Panel theme toggle button (`◯` / `☾` / `☀`) |
| Selection mode | Select button or context menu |
