# Web Dashboard

Learn how to use the vrunner admin interface to monitor, interact with, and manage all spawned commands from a single browser window.

## Accessing the Dashboard

Start vrunner with web UI enabled and open the admin interface in your browser:

```bash
vrunner --web --port 8080
# Open http://localhost:8080/admin
```

## Layout Overview

![Dashboard overview](../web-ui/screenshots/01-overview.png)

The dashboard has four main areas:

- **Top Bar** — Font size, resize, buffer select, theme, auth token, and view toggles.
- **Sidebar** — Tabs for Commands, Spawn, Templates, and Certs.
- **Panel Header** — Per-panel controls: send keys, copy, export, pause, theme.
- **Terminal Pane** — The main VTTY viewer that renders the selected command's output in real time.

## Real-Time VTTY Viewer

The central terminal pane renders a full ANSI/xterm-compatible terminal. It supports:

- **Color and formatting** — 256-color and true-color support with all standard ANSI sequences.
- **Cursor tracking** — Cursor position, visibility, and style are preserved.
- **Alternate screen** — Full-screen applications like `htop`, `vim`, and `less` render correctly.

Click anywhere inside the terminal pane to **focus** it for keyboard input. The focused terminal has a highlighted border.

## Command Sidebar

![Sidebar commands](../web-ui/screenshots/03-sidebar-commands.png)

The sidebar lists every spawned command with:

- **Kill button** — Close (×) button at the far left of each command entry.
- **Pin** — Star button to pin a command to the top of the list.
- **Name** or command string as the label.
- **Status** — Row background indicates running, frozen, or exited state.
- **Badges** — Exit code, runtime, resource usage, certificate.

When multiple vrunner instances are connected, a sort bar at the top lets you group commands by instance or view all sorted alphabetically.

### Search

Type in the search box at the top of the sidebar to filter commands by name or command string. The list updates as you type.

### Batch Operations

- **Kill All** — Terminates every running command. Use with care — there is no undo.
- **Pause / Run** — Freezes all command output or resumes it. Useful when you need to inspect a moment in time.

## Terminal Interactions

### Keyboard Input (Click-to-Focus)

Click a terminal pane to give it keyboard focus. All keystrokes are forwarded to the underlying command. Only one terminal is focused at a time.

### Terminal Search (Ctrl+F)

Press **Ctrl+F** inside a focused terminal to open the search bar. Type your query and press Enter to highlight matches. Use arrow keys or Shift+Enter to cycle through results.

### Scroll to Bottom

When you scroll up in the terminal buffer, a **scroll-to-bottom** button appears. Click it or press **End** to jump back to live output.

### Drag-to-Resize Panels

Drag the borders between the sidebar and the terminal pane to resize them. Your preferred width is persisted in `localStorage` and restored on reload.

## Exporting Output

Right-click inside a terminal pane to open the context menu and select **Export Output**. Choose from:

- **Plain text** — Raw output without ANSI codes.
- **HTML** — Styled HTML with preserved formatting.
- **ANSI** — Full output with escape sequences intact.

## Right-Click Context Menu

Right-click on a terminal or sidebar item for additional options:

- **Copy** — Copy selected text to clipboard.
- **Paste** — Paste clipboard contents.
- **Clear** — Clear the terminal buffer.
- **Export Output** — Save terminal contents.
- **Kill** — Terminate the command.
- **Restart** — Kill and re-spawn the command with the same parameters.

## Browser Notifications

Enable browser notifications to receive alerts when:

- A command exits (success or failure).
- A command produces error output.
- All commands have finished.

Grant permission when prompted by the browser. Toggle notifications in the top bar settings menu.

## Keyboard Shortcuts

Press **?** or click the keyboard icon in the top bar to open the shortcuts panel:

| Shortcut | Action |
|----------|--------|
| `Ctrl+F` | Search in terminal |
| `Ctrl+K` | Kill focused command |
| `Ctrl+Shift+K` | Kill all commands |
| `Ctrl+Space` | Pause / Resume all |
| `Ctrl+/` | Toggle sidebar |
| `?` | Show shortcuts panel |
| `Escape` | Close overlays |

## Auto-Reconnect

If the connection to vrunner drops, the dashboard automatically attempts to reconnect every 3 seconds. A connection status indicator in the top bar shows the current state:

- **Green** — Connected
- **Yellow** — Reconnecting
- **Red** — Disconnected

## Responsive Layout

The dashboard adapts to different screen sizes:

- **Desktop** — Full sidebar + terminal pane.
- **Tablet** — Collapsible sidebar with toggle.
- **Mobile** — Sidebar becomes a slide-out drawer; terminal fills the screen.

## Command-Name URLs

Each command has a shareable URL based on its name:

```
http://localhost:8080/admin/htop
http://localhost:8080/admin/syslog
```

Bookmark or share these links to jump directly to a specific command's terminal view.

## Multi-Instance View

When running multiple vrunner instances, you can configure the dashboard to show commands from several instances in a single view. Set the `VRUNNER_PEERS` environment variable:

```bash
VRUNNER_PEERS="http://host1:8080,http://host2:8080" vrunner --web
```

Each command in the sidebar shows which instance it belongs to, with a colored indicator.

For details on the underlying API endpoints, see [`../reference/api.md`](../reference/api.md).
