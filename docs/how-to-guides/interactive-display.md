# Interactive Display (TUI)

Learn how to use vrunner's built-in terminal user interface (TUI) for local monitoring and interaction with spawned commands directly in your terminal.

## Enabling the Display

Use the `--display` flag to open the interactive TUI when vrunner starts:

```bash
vrunner --display --cmd "htop" --name "monitor"
```

By default, `--display` shows only the first spawned command's terminal. Use `--display-all` to show all commands as split panes:

```bash
vrunner --display-all \
  --cmd "htop" --name "monitor" \
  --cmd "tail -f /var/log/syslog" --name "logs"
```

Use `--tabs` to show commands as tabbed views within a single pane:

```bash
vrunner --tabs \
  --cmd "htop" --name "monitor" \
  --cmd "tail -f /var/log/syslog" --name "logs" \
  --cmd "npm run dev" --name "frontend"
```

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
| `Ctrl+Q` | Quit vrunner TUI |
| `F1` | Show help panel |
| `F5` | Refresh terminal display |
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

## Split-Pane Mode (Ctrl+S)

Toggle between single-pane and split-pane layouts at runtime:

1. Start with `--display` (single pane) or `--display-all` (split panes).
2. Press `Ctrl+S` at any time to switch the layout.
3. In split mode, each command gets its own pane.
4. Use `Tab` to cycle focus between panes.

Pane sizes are initially equal. Drag borders with the mouse to adjust.

## Retain on Exit

By default, when a command exits, its pane remains visible with the final output (and an `[EXITED]` indicator). This lets you review the output after the process finishes.

To auto-close exited panes, use the `--no-retain` flag:

```bash
vrunner --display-all --no-retain \
  --cmd "npm run build" --name "build" \
  --cmd "npm test" --name "test"
```

## Tabs with Status Indicators

In `--tabs` mode, each tab shows:

- The command name (or command string if no name is set).
- A status indicator:
  - **`●` Green** — Running
  - **`●` Gray** — Exited with code 0
  - **`●` Red** — Exited with non-zero code
  - **`●` Yellow** — Paused
  - **`●` Cyan** — Focused (current tab)

Switch between tabs with `Tab` / `Shift+Tab` or click with the mouse.

## Context Menu (Right-Click)

Right-click inside a focused pane to open a context menu with options:

- **Kill** — Terminate the command.
- **Restart** — Kill and re-spawn with the same parameters.
- **Clear** — Clear the terminal buffer.
- **Copy Selection** — Copy selected text to the system clipboard.
- **Paste** — Paste from the system clipboard.
- **Freeze / Thaw** — Pause or resume output processing.
- **Export** — Save terminal output to a file.

Use arrow keys to navigate the menu and `Enter` to select. Press `Escape` to cancel.

## Resizing

When you resize your terminal window, the TUI and all child PTYs are automatically resized to fit. All running commands receive a `SIGWINCH` signal and adjust their output accordingly.

If a command does not handle `SIGWINCH` correctly (rare), you can manually resize a specific pane by pressing `F5` after resizing the terminal window.

## TUI + Web Simultaneously

You can run the TUI and web UI at the same time. The web UI provides additional features like browser notifications, export options, and remote access:

```bash
vrunner --display-all --web --port 8080 \
  --cmd "htop" --name "monitor" \
  --cmd "tail -f /var/log/syslog" --name "logs"
```

The TUI renders in your terminal while the web UI is available at `http://localhost:8080/admin`. Both show the same commands with synchronized output.

For web dashboard details, see [`web-dashboard.md`](web-dashboard.md). For the API, see [`../reference/api.md`](../reference/api.md).
