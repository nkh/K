# Keybindings Reference

Complete reference for all keyboard shortcuts in the interactive terminal display. These keybindings are **shared by both vrc and vrw** when using `--display`, `--display-all`, or `--tabs` mode. Keybindings are divided into default bindings, user-configurable bindings, and hardcoded system shortcuts.

---

## Default Keybindings

These keybindings are active when the interactive display is attached (in both `vrc --display` and `vrw --display`). The
**Mode** column indicates when each shortcut is available.

| Key | Action | Mode |
|-----|--------|------|
| `Tab` | Switch to the next command pane / tab. | `display-all`, `always` |
| `Shift+Tab` | Switch to the previous command pane / tab. | `display-all`, `always` |
| `1` – `9` | Jump to tab by index (1-based). `0` jumps to tab 10. | `display-all`, `always` |
| `Ctrl+B` | Toggle the log panel visibility. | `always` |
| `Ctrl+N` | Spawn a new command prompt. | `always` |
| `Ctrl+H` | Show the help overlay with all available keybindings. | `always` |
| `Ctrl+K` | Kill the currently focused command (send `SIGKILL`). | `active` |
| `Ctrl+P` | Pause / resume the currently focused command (`SIGSTOP` / `SIGCONT`). | `active` |
| `Ctrl+W` | Toggle wide mode for the active pane (expand to full width). | `display-all` |
| `Ctrl+L` | Force a full redraw of the display. | `always` |
| `Ctrl+R` | Toggle raw output mode (show ANSI escapes as text). | `active` |
| `?` | Show the help overlay (same as `Ctrl+H`). | `always` |
| `q` | Quit the vrw interactive display (detaches; vrw continues). | `always` |
| `Q` | Quit vrw entirely (shuts down the server). | `always` |

### Mode Descriptions

| Mode | Description |
|------|-------------|
| `always` | Available regardless of which command is focused or how many commands exist. |
| `active` | Only available when a running command pane is focused. |
| `display-all` | Only available when `--display-all` mode is enabled (multi-pane view). |

---

## Configurable Keybindings

All keybindings listed above (except the hardcoded shortcuts) can be
remapped in the configuration file under the `keybindings` section.
See [`../configuration.md`](../configuration.md) for the full syntax.

| Config Key | Default | Description |
|------------|---------|-------------|
| `keybindings.next_command` | `Tab` | Advance focus to the next command tab or pane. |
| `keybindings.prev_command` | `Shift+Tab` | Move focus to the previous command tab or pane. |
| `keybindings.toggle_log` | `Ctrl+B` | Show or hide the scrolling log panel overlay. |
| `keybindings.spawn_command` | `Ctrl+N` | Open the spawn dialog to create a new command. |
| `keybindings.show_help` | `Ctrl+H` | Toggle the keybinding help overlay. |
| `keybindings.quit` | `q` | Detach from the interactive display without stopping vrw. |
| `keybindings.quit_all` | `Q` | Shut down the entire vrw instance. |
| `keybindings.kill_command` | `Ctrl+K` | Send `SIGKILL` to the focused command. |
| `keybindings.toggle_pause` | `Ctrl+P` | Pause or resume the focused command. |

### Configuration Example

```yaml
keybindings:
  next_command: "ctrl+j"
  prev_command: "ctrl+k"
  toggle_log: "ctrl+l"
  spawn_command: "ctrl+n"
  show_help: "f1"
  quit: "ctrl+q"
  quit_all: "ctrl+shift+q"
  kill_command: "ctrl+x"
  toggle_pause: "ctrl+z"
```

---

## Supported Key Name Formats

When defining keybindings in the configuration file or the `--send-keys` CLI
flag (see [`cli.md`](cli.md)), the following formats are accepted.

### Modifier Prefixes

Modifiers are combined with a key name using `+`. Multiple modifiers can be
stacked.

| Prefix | Meaning |
|--------|---------|
| `ctrl+` | Control key |
| `alt+` | Alt / Meta key |
| `shift+` | Shift key |

**Examples:** `ctrl+a`, `alt+enter`, `ctrl+shift+tab`, `ctrl+alt+delete`.

### Function Keys

Function keys are written as `f1` through `f20`. They can be combined with
modifiers.

| Format | Key |
|--------|-----|
| `f1` | F1 |
| `f2` | F2 |
| `f3` | F3 |
| `f4` | F4 |
| `f5` | F5 |
| `f6` | F6 |
| `f7` | F7 |
| `f8` | F8 |
| `f9` | F9 |
| `f10` | F10 |
| `f11` | F11 |
| `f12` | F12 |
| `f13`–`f20` | Extended function keys (terminal-dependent). |

**Example:** `shift+f5`, `ctrl+f1`.

### Special Keys

These names refer to non-alphanumeric keys.

| Name | Key |
|------|-----|
| `enter` | Enter / Return |
| `tab` | Tab |
| `esc` / `escape` | Escape |
| `space` | Space bar |
| `backspace` | Backspace |
| `delete` / `del` | Delete |
| `insert` / `ins` | Insert |
| `home` | Home |
| `end` | End |
| `pageup` / `pgup` | Page Up |
| `pagedown` / `pgdn` | Page Down |
| `up` | Arrow Up |
| `down` | Arrow Down |
| `left` | Arrow Left |
| `right` | Arrow Right |

### Single Characters

Any single printable ASCII character can be used as a keybinding by itself,
without any prefix: `a`, `Z`, `0`, `!`, `/`, `.`.

### Quoting in Config

When a key contains `+` or a modifier prefix in YAML, wrap it in quotes to
prevent YAML parsing issues.

```yaml
# Correct: quoted key names
keybindings:
  next_command: "ctrl+j"
  quit: "q"

# Also correct: quoted numeric keys
keybindings:
  jump_tab_1: "1"
```

---

## Hardcoded Shortcuts

The following shortcuts are **not configurable** and cannot be remapped. They
are intercepted by the terminal driver or vrw's signal handler before any
keybinding processing occurs.

| Shortcut | Action | Details |
|----------|--------|---------|
| `Ctrl+\` | Quit (core dump) | Sends `SIGQUIT` to vrw. Produces a stack trace on supported platforms. Useful for debugging. |
| `Ctrl+C` | Shutdown | Sends `SIGINT` to vrw. The first press initiates a graceful shutdown (equivalent to `Q`). A second press within 2 seconds forces immediate termination. |

These shortcuts bypass the keybinding system entirely and are handled by the
process signal layer. They are active at all times, including when the help
overlay or spawn dialog is open.

> **Note:** `Ctrl+C` is *not* forwarded to the child command. To send
> `Ctrl+C` to the child process, use the `toggle_pause` / `kill_command`
> keybindings, or send keystrokes via the WebSocket API
> (see [`../websocket.md`](../websocket.md)).

---

## Interaction with the Child Process

When a command pane is focused and vrw is in normal (non-overlay) mode,
keyboard input is forwarded to the child PTY. Keybindings take priority: if a
pressed key matches a binding, the binding action is executed and the key is
**not** forwarded to the child.

The following keybindings consume input that would otherwise be sent to the
child:

| Key | Reason |
|-----|--------|
| `Tab` / `Shift+Tab` | Used for tab switching. |
| `Ctrl+H` | Used for the help overlay (conflicts with backspace in some terminals). |
| `Ctrl+P` | Used for pause toggle. |
| `Ctrl+N` | Used for spawn dialog. |
| `q` | Used for quit. |
| `Q` | Used for quit-all. |

To send these keys to the child command, use the WebSocket API `write`
message (see [`../websocket.md`](../websocket.md)) or configure an alternative
keybinding and free the original key.

---

## Mouse Support

When `--mouse` is enabled (the default), vrw captures and interprets mouse
events in the terminal:

| Event | Action |
|-------|--------|
| Click on tab | Switch focus to that command tab. |
| Click on log panel | Scroll the log panel. |
| Scroll wheel | Scroll the focused command's terminal viewport. |
| Right-click | Context menu (spawn / kill / rename). |

Mouse events are **not** forwarded to the child PTY while the interactive
display is active. Use `--no-mouse` or configure `vtty.mouse: false` to pass
all mouse events through to the child process.

---

## See Also

- [`cli.md`](cli.md) — CLI reference including `--send-keys` notation
- [`../configuration.md`](../configuration.md) — Full configuration reference with `keybindings` section details
- [`../api.md`](../api.md) — HTTP API reference for programmatic interaction
- [`../websocket.md`](../websocket.md) — WebSocket protocol for sending input to the child PTY
