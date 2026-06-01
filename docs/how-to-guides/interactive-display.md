# Interactive Display (TUI)

Learn how to use the built-in terminal user interface (TUI) for local monitoring and interaction with spawned commands directly in your terminal.

> **Both vrc and vrw** support `--display`, `--display-all`, and `--tabs` flags. The examples below use `vrc` but apply equally to `vrw` — just replace `vrc` with `vrw` in any command. vrw also provides a [web dashboard](web-dashboard.md) for browser-based monitoring.

## Enabling the Display

Use the `--display` flag to open the interactive TUI when vrc starts:

```bash
vrc --display -- htop
```

By default, `--display` shows only the first spawned command's terminal. Use `--display-all` to show all commands as split panes:

```bash
vrc --display-all \
  -- htop \
  -- tail -f /var/log/syslog
```

Use `--tabs` to show commands as tabbed views within a single pane:

```bash
vrc --tabs \
  -- htop \
  -- tail -f /var/log/syslog \
  -- npm run dev
```

> **With vrw:** `vrw --tabs -- htop -- tail -f /var/log/syslog -- npm run dev` — works identically. You can also open `http://localhost:8080/admin` for a browser-based view.

## Layout Modes

| Flag | Layout |
|------|--------|
| `--display` | Single pane showing the first command |
| `--display-all` | Split panes showing all commands simultaneously |
| `--tabs` | Tabbed single pane with one command per tab |

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle focus between panes or tabs |
| `Ctrl+S` | Toggle split-pane mode (single ↔ split) |
| `Ctrl+F` | Open search bar |
| `Ctrl+K` | Kill the focused command |
| `Ctrl+W` | Close the focused pane/tab |
| `Ctrl+L` | Clear the focused terminal |
| `Ctrl+/` | Toggle sidebar visibility |
| `Enter` | Confirm in search bar |
| `Escape` | Close search bar / exit context menu |
| `Ctrl+Q` | Quit vrc TUI |
| `F1` | Show help panel |
| `F12` | Open spawn prompt |
| `Up` / `Down` | Scroll terminal buffer |
| `PgUp` / `PgDn` | Scroll one page up/down |

## Mouse Support

The TUI supports mouse interaction when your terminal emulator reports mouse events:

- **Click** on a pane or tab to focus it.
- **Right-click** to open the context menu.
- **Scroll** with the mouse wheel to scroll the terminal buffer.
- **Drag** pane borders to resize split panes.

Mouse support is enabled automatically. If your terminal does not support mouse events, all functionality remains accessible via keyboard.

## Search (Ctrl+F)

Press `Ctrl+F` to open the search bar at the bottom of the focused pane:

1. Type your search query.
2. Press `Enter` to find the next match (highlighted in reverse video).
3. Press `Enter` again or `Down` to jump to the next match.
4. Press `Shift+Enter` or `Up` to jump to the previous match.
5. Press `Escape` to close the search bar.

Search operates on the terminal's scrollback buffer, so it finds text that has scrolled off-screen.

## Retain on Exit

By default, when a command exits, its pane remains visible with the final output (and an `[EXITED]` indicator). This lets you review the output after the process finishes.

## Tabs with Status Indicators

In `--tabs` mode, each tab shows:

- The command name.
- A status indicator (running, exited, paused, focused).

Switch between tabs with `Tab` / `Shift+Tab` or click with the mouse.

## Resizing

When you resize your terminal window, the TUI and all child PTYs are automatically resized to fit. All running commands receive a `SIGWINCH` signal and adjust their output accordingly.

For display details, see the architecture documentation in [`../explanation/architecture.md`](../explanation/architecture.md).
