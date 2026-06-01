# Keyboard Shortcuts

The web UI provides several keyboard shortcuts for common actions.

## Global Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+C` | Copy terminal text selection to clipboard |
| `Ctrl+Shift+S` / `Alt+S` | Toggle selection mode in the terminal panel |
| `Alt+Left` / `Alt+Right` | Navigate to previous/next command in the list |
| `?` | Show keyboard shortcuts overlay |
| `Shift+F10` / Context Menu key | Open context menu for the focused element (command item or panel header) |
| `Escape` | Close any open modal, overlay, search bar, or context menu |
| `Any key` | Focus the key input field (when not already in an input/textarea/select) |

## Terminal Shortcuts (when terminal is focused)

When the terminal panel is focused (after clicking on it), keystrokes are sent directly to the PTY process. The following shortcuts are intercepted:

| Shortcut | Action |
|----------|--------|
| `Ctrl+F` | Open terminal search within the selected panel |
| `Escape` | Close terminal search bar |

## Terminal Search

| Shortcut | Action |
|----------|--------|
| `Ctrl+F` | Open terminal search bar (when terminal is focused or in vtty view) |
| `Escape` | Close terminal search bar |
| `Enter` (in search) | Jump to next search match |
| `Shift+Enter` (in search) | Jump to previous search match |

## Send Keys Input

| Shortcut | Action |
|----------|--------|
| `Enter` | Send the typed keystrokes to the command |

## Spawn Form

| Shortcut | Action |
|----------|--------|
| `Enter` (in Command/Args/Dir fields) | Submit the spawn form |

## Modal Navigation

| Shortcut | Action |
|----------|--------|
| `Tab` | Move to next focusable element within modal |
| `Shift+Tab` | Move to previous focusable element within modal |
| `Escape` | Close the modal and restore focus |

## Shortcuts Reference Overlay

![Shortcuts overlay](screenshots/12-keyboard-shortcuts.png)

Click the **?** button in the top bar to display a quick reference overlay with all available shortcuts. The overlay traps focus for accessibility — Tab and Shift+Tab cycle through the focusable elements within the panel.
