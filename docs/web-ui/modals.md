# Modals and Overlays

The web UI uses modals and overlays for focused interactions that require user attention.

## Keyboard Shortcuts Overlay

![Keyboard shortcuts](screenshots/12-keyboard-shortcuts.png)

Displays a reference table of all available keyboard shortcuts in the web UI.

| Shortcut | Action |
|----------|--------|
| `?` | Show this help |
| `Ctrl+F` | Search in terminal |
| `Ctrl+Shift+C` | Copy terminal selection |
| `Ctrl+Shift+S` / `Alt+S` | Toggle selection mode |
| `Escape` | Close search / menu |
| `Alt+Left` / `Alt+Right` | Navigate prev/next command |
| `Any key` | Focus key input (when not in a field) |
| `Enter` | Send keystrokes to terminal |

**Open**: Click the **?** button in the top bar.

## Add Panel Modal

![Add panel modal](screenshots/20-add-panel-modal.png)

Used to connect to an additional vrw instance in a new panel.

### Instance URL
The base URL of the vrw instance (e.g., `http://localhost:9090`).

### Label (optional)
A friendly name for this instance, displayed in the panel header and instance URL field.

### Auth Token (optional)
A bearer token for authenticating with this instance. If omitted, the global token (set in the top bar) is used.

### Actions
- **Cancel**: Close the modal without adding a panel.
- **Add Panel**: Create the new panel and connect to the specified instance.

**Open**: Click the **+ Panel** button in the top bar.

## Command Picker Modal

When the URL contains a command name that matches multiple running commands (e.g., `/admin/bash` when multiple bash instances are running), a picker overlay appears. Each matching command is listed with its name, PID, alive/exited status, and runtime. Click a command to view its terminal, or click **Cancel** to dismiss.

The picker supports keyboard navigation: Tab and Shift+Tab cycle through the command items, and Escape closes the picker.

## Special Keys Help Modal

![Special keys help](screenshots/13-special-keys-help.png)

Displays a comprehensive reference for typing special keys in the send keys input field. See [Special Keys Reference](./special-keys.md) for the full listing.

**Open**: Click the **?** button next to the send keys input in any panel header.

## Global Search Overlay

See [Global Search](./global-search.md) for details.

**Open**: Click the **🔍** button in the top bar.

## Focus Management

All modals implement focus trapping: while a modal is open, the Tab key cycles through the modal's interactive elements and cannot escape to the page behind it. Pressing Escape closes the modal and restores focus to the previously active element.
