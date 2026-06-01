# Web UI Introduction

The vrw web UI is a browser-based control plane for managing terminal commands. It provides a real-time view of command output, interactive keyboard input, multi-instance panel support, and comprehensive command lifecycle management — all accessible from any web browser.

## Accessing the Web UI

By default, vrw serves the web UI at `http://127.0.0.1:9090/admin`. The bind address and port can be configured via CLI flags (`--bind`, `--port`) or the configuration file. If the `--remote` flag is used, the server binds to `0.0.0.0` and requires authentication.

## URL Routing

The web UI supports path-based command routing. Appending a command name to the URL (e.g., `/admin/htop` or `/admin/bash`) will auto-select that command when the page loads. If multiple commands share the same name, a picker overlay appears to let you choose which one to view. This uses basename matching, so `/admin/usr/bin/htop` also works.

## Layout Overview

The web UI is divided into four main regions:

![Overview with numbered regions](screenshots/25-overview-numbered.png)

| # | Region | Description |
|---|--------|-------------|
| 1 | **Sidebar** | Command list (with filter, kill all, pinning), spawn form, templates, and certificate management |
| 2 | **Top Bar** | Global controls: sidebar toggle, command navigation, panel management, search, theme, sound, logs, status, auth token, docs, shortcuts |
| 3 | **Panel Header** | Per-panel controls: command info, restart, resources, font size, terminal resize, buffer select, refresh throttle, send keys, copy, export, screenshot, panel theme |
| 4 | **Terminal (VTTY)** | Real-time terminal output rendered in the browser with search, scrollback, and copy support |
| 5 | **Bottom Bar** | Status information: command label, cursor position, dimensions, scrollback indicator, connection status, update mode, poll interval, refresh throttle, WebSocket quality |

## Responsive Design

The sidebar can be collapsed or resized by dragging its right edge. On screens narrower than 768px, the sidebar becomes a floating overlay. The terminal automatically adjusts its column/row count to fill available space. The top bar layout is responsive and hides less essential controls on narrow screens.

## Multi-Instance Support

The web UI can connect to multiple vrw instances simultaneously. Use the **+ Panel** button in the top bar to add additional instance panels. Peer instances are automatically discovered via the server's peer registration API. When a peer registers or unregisters, panels are added or removed dynamically without requiring a page reload.

## Technology

The web UI is a single-page application (SPA) embedded into the vrw binary via `rust_embed`. It uses vanilla HTML, CSS, and JavaScript with no external dependencies. Terminal rendering uses a `<pre>` element updated via WebSocket push or HTTP polling. An incremental diff protocol (Level 3) minimizes bandwidth by sending only changed cells rather than full HTML replacements.
