# Keyboard Shortcuts

The web UI provides keyboard shortcuts for common actions. Windowing and pane-splitting shortcuts use a **prefix key** system modelled after screen(1) and tmux(1): press `Ctrl+A` then a second key.

## Prefix Key (`Ctrl+A`)

Press `Ctrl+A` to enter prefix mode (a "PREFIX" indicator appears in the bottom-right corner). Within 1 second, press the second key. If no matching key is pressed, prefix mode cancels automatically.

| Shortcut | Action |
|----------|--------|
| `Ctrl+A` then `\|` | Split pane vertically (side by side) |
| `Ctrl+A` then `-` | Split pane horizontally (top/bottom) |
| `Ctrl+A` then `Ctrl+D` | Close pane (remove split) |
| `Ctrl+A` then `c` | Create new panel |
| `Ctrl+A` then `w` | Create new window |
| `Ctrl+A` then `W` | Close current window |
| `Ctrl+A` then `t` | Toggle panel theme (inherit / light / grey / dark) |
| `Ctrl+A` then `1`–`9` | Switch to window N |
| `Escape` (while in prefix) | Cancel prefix mode |

## Global Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+A` | Prefix key — enter command mode for window/pane shortcuts |
| `Ctrl+Shift+C` | Copy terminal text selection to clipboard |
| `Ctrl+Shift+E` | Export terminal as text |
| `Ctrl+Shift+R` | Restart the command in the active panel |
| `Ctrl+Shift+S` / `Alt+S` | Toggle selection mode in the terminal panel |
| `Alt+T` | Toggle panel theme (inherit / light / grey / dark) |
| `Alt+N` | Add a new panel |
| `Alt+Left` / `Alt+Right` | Navigate to previous/next command in the list |
| `?` | Show keyboard shortcuts overlay |
| `Shift+F10` / Context Menu key | Open context menu for the focused element (command item or panel header) |
| `Escape` | Close any open modal, overlay, search bar, or context menu |
| `Any key` | Focus the key input field (when not already in an input/textarea/select) |

## Legacy Alt+ Shortcuts (still available)

The Alt+ shortcuts for windowing/pane operations remain available as alternatives to the prefix system:

| Shortcut | Action |
|----------|--------|
| `Alt+\|` | Split pane vertically (side by side) |
| `Alt+-` | Split pane horizontally (top/bottom) |
| `Alt+Ctrl+D` | Close pane (remove split) |
| `Alt+N` | New panel |
| `Alt+W` | New window |
| `Alt+Shift+W` | Close window |
| `Alt+1`–`Alt+9` | Switch to window N |

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

## Window Bar Toolbar

The window bar at the top of the panel area always contains action buttons:

| Button | Action |
|--------|--------|
| `+ Win` | Create a new window |
| `\| Split V` | Split pane vertically (side by side) |
| `— Split H` | Split pane horizontally (top/bottom) |
| `✕ Close` | Close the focused split pane |

## Customizing Shortcuts

Shortcuts are stored in the browser's `localStorage` under the key `vrw_custom_shortcuts`. To customize, open the browser DevTools console and run:

```js
// Change the prefix key from Ctrl+A to Ctrl+B:
localStorage.setItem('vrw_custom_shortcuts', JSON.stringify({
    "prefix": { key: "b", ctrl: true, isPrefix: true },
    // Re-point prefix shortcuts to the new prefix:
    "p-split-vertical": { key: "|", prefix: true },
    "p-split-horizontal": { key: "-", prefix: true },
    "p-unsplit": { key: "d", ctrl: true, prefix: true }
}));
```

Then reload the page. Only the `key` and modifier fields are customizable; the action always comes from the built-in default.

## Shortcuts Reference Overlay

![Shortcuts overlay](screenshots/12-keyboard-shortcuts.png)

Click the **?** button in the top bar to display a quick reference overlay with all available shortcuts. The overlay traps focus for accessibility — Tab and Shift+Tab cycle through the focusable elements within the panel.
