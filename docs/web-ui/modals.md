# Modals and Overlays

The web UI uses modals and overlays for focused interactions that require user attention.

## Keyboard Shortcuts Overlay

![Keyboard shortcuts](screenshots/12-keyboard-shortcuts.png)

Displays a reference table of all available keyboard shortcuts in the web UI.

| Shortcut | Action |
|----------|--------|
| `Ctrl+F` | Open terminal search within the selected panel |
| `Ctrl+Shift+F` | Open global search across all commands |
| `Escape` | Close any open overlay/modal |
| `L` | Toggle log viewer |
| `+` / `-` | Increase / decrease font size |
| `Enter` (in send keys) | Send keystrokes to command |

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

## Special Keys Help Modal

![Special keys help](screenshots/13-special-keys-help.png)

Displays a comprehensive reference for typing special keys in the send keys input field. See [Special Keys Reference](./special-keys.md) for the full listing.

**Open**: Click the **?** button next to the send keys input in any panel header.

## Focus Management

All modals implement focus trapping: while a modal is open, the Tab key cycles through the modal's interactive elements and cannot escape to the page behind it. Pressing Escape closes the modal and restores focus to the previously active element.
