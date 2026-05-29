# vrunner CLI Reference

Complete reference for the `vrunner` command-line interface. Options are
organized by functional category. Every flag corresponds to a configuration key
described in [`../configuration.md`](../configuration.md); CLI flags take
precedence over config-file values.

---

## Synopsis

```
vrunner [GENERAL OPTIONS] [CATEGORY OPTIONS] -- <command> [args...]
vrunner <subcommand> [SUBCOMMAND OPTIONS]
```

---

## General Options

These top-level flags control which configuration file is loaded and provide
standard help/version information.

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--config <path>` | `-c` | `vrunner.yaml` in the current directory | Path to the configuration file. |
| `--help` | `-h` | — | Print usage summary and exit. |
| `--version` | `-V` | — | Print version string and exit. |

The config file is resolved in this order:

1. Path given via `--config`.
2. `./vrunner.yaml` (or `./vrunner.yml`).

---

## Server Options

Control how vrunner binds its HTTP/WebSocket server.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--bind <addr>` | `127.0.0.1` | `server.bind` | Host address to listen on. Use `0.0.0.0` to accept connections from any interface. |
| `--port <n>` | `9090` | `server.port` | TCP port for the HTTP server. |
| `--remote` | `false` | `server.remote` | Enable remote access mode. Binds to `0.0.0.0` and enables authentication (`--auth`). |

**Examples**

```bash
# Listen on all interfaces, port 9090
vrunner --bind 0.0.0.0 --port 9090 -- python -m http.server

# Remote access with TLS
vrunner --remote --tls -- python app.py
```

---

## Security Options

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--auth` | `false` | `security.require_auth` | Require authentication for API requests. When enabled, every HTTP request must include an `Authorization: Bearer <token>` header or supply the token via the `token` query parameter. |
| `--token-file <path>` | — | `security.token_file` | Path to a file containing a shared secret token. The file is read once at startup; the trailing newline is stripped. Default: `~/.config/vrunner/token`. |

When `--auth` is active, every HTTP request must include an
`Authorization: Bearer <token>` header or supply the token via the `token`
query parameter. WebSocket connections pass the token through the
`token` query parameter during the handshake.

See [`../configuration.md`](../configuration.md) for additional security
settings such as rate limiting and allowed origins.

---

## TLS Options

Enable HTTPS and secure WebSocket (`wss://`) transport.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--tls` | `false` | `tls.enabled` | Enable TLS for the server. Requires `--cert-file` and `--key-file`. |
| `--cert-file <path>` | — | `tls.cert_file` | Path to the PEM-encoded TLS certificate (or full certificate chain). |
| `--key-file <path>` | — | `tls.key_file` | Path to the PEM-encoded TLS private key. |
| `--certificate <NAME:CERT:KEY>` | — | — | Define a named certificate (repeatable). Format: `NAME:CERT_FILE:KEY_FILE`. Used for per-command access control. |

TLS is automatically enabled when `--remote` is combined with certificate
paths. See [`../certificates.md`](../certificates.md) and the
`cert` subcommand below for managed certificate workflows.

**Examples**

```bash
vrunner --tls --cert-file /etc/ssl/certs/vrunner.pem \
        --key-file /etc/ssl/private/vrunner.key -- node server.js
```

---

## VTTY Options

Control the virtual terminal (PTY) that vrunner creates for the child command.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--vtty-rows <n>` | `24` | `vtty.rows` | Initial number of rows in the virtual terminal. |
| `--vtty-cols <n>` | `80` | `vtty.cols` | Initial number of columns in the virtual terminal. |
| `--term <type>` | `xterm-256color` | `vtty.term` | Value of the `TERM` environment variable inside the child process. |
| `--scrollback <n>` | `5000` | `vtty.scrollback` | Maximum number of scrollback lines retained in the virtual terminal ring buffer. |
| `--truecolor` / `--no-truecolor` | `true` | `vtty.truecolor` | Enable or disable 24-bit true-color support. When disabled, the terminal reports `xterm-256color` regardless of the `--term` setting. |
| `--mouse` / `--no-mouse` | `false` | `vtty.mouse` | Enable or disable mouse event forwarding from connected WebSocket clients to the child PTY. |

The virtual terminal dimensions can be changed at runtime via the
[resize subcommand](#resize) or the WebSocket `resize` message
(see [`../websocket.md`](../websocket.md)).

---

## Display Options

Configure the built-in terminal multiplexer dashboard rendered in the
terminal where vrunner itself is running.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--display` / `--no-display` | `false` | `display.enabled` | Enable the interactive display. Must be explicitly set to render command output locally. |
| `--display-all` | `false` | `display.all` | Show *all* command outputs simultaneously instead of the active pane only. Each pane is given a proportional viewport. Implies `--display`. |
| `--refresh-ms <n>` | `100` | `display.refresh_ms` | Milliseconds between display redraw cycles. Lower values produce smoother output at the cost of higher CPU usage. |

When `--no-display` is set (the default), vrunner runs in headless mode:
command output is not rendered locally, but the HTTP/WebSocket API remains
fully functional.

---

## Interactive Options

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--tabs` | `false` | `interactive.tabs` | Show a tab bar listing all running commands. Tabs can be switched with configurable keybindings (see [`keybindings.md`](keybindings.md)). |

Interactive keybindings are documented in full in
[`keybindings.md`](keybindings.md).

---

## Logging Options

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--log` | `false` | `command_log.enabled` | Enable API command logging to the terminal. |
| `--log-file <path>` | — | `command_log.file` | Enable API command logging and write output to the given file. |
| `--log-pty-raw <path>` | — | `command_log.pty_raw_log` | Log raw bytes received from the child PTY to the given file before any ANSI processing. Useful for debugging terminal escape sequences. Produces very verbose output. |

---

## Daemon Options

Run vrunner as a background daemon process.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--daemon` | `false` | `daemon.enabled` | Fork into the background after initialization. The parent process exits once the server is ready. Conflicts with `--display`, `--display-all`, and `--tabs`. |
| `--stdout-file <path>` | `vrunner.out` | `daemon.stdout_file` | File to which the daemon's stdout is redirected. |
| `--stderr-file <path>` | `vrunner.err` | `daemon.stderr_file` | File to which the daemon's stderr is redirected. |

**Example**

```bash
vrunner --daemon -- python worker.py
```

---

## Exit Handler Options

These options control what happens when the child command exits or encounters
an error, and allow automated keystroke injection.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--on-exit <cmd>` | — | `exit_handler.on_exit` | Shell command to execute (as a detached process) when the child exits normally (exit code 0). |
| `--on-error <cmd>` | — | `exit_handler.on_error` | Shell command to execute (as a detached process) when the child exits with a non-zero code. |
| `--exit-timeout <secs>` | `10` | `exit_handler.timeout_secs` | Seconds to wait for the child process to terminate gracefully before sending `SIGKILL`. |
| `--retain-on-exit` | `false` | — | Keep the virtual terminal session alive after the child process exits. The terminal remains accessible via the WebSocket API for inspection. Per-command option only (not applied to global defaults). |
| `--snapshot-on-exit <file>` | — | — | Save the VTTY buffer to the specified file when the command exits. Per-command option only (not applied to global defaults). |
| `--send-keys <keys>` | — | `exit_handler.send_keys` | Keystrokes to send to the child PTY after it starts. Special keys use `<...>` notation, e.g. `<Enter>`, `<C-c>`, `<Esc>`. See [Special Key Notation](#special-key-notation). Common use case: send `<C-c>` before shutdown to allow graceful termination of interactive programs. |

**Example**

```bash
# Send Ctrl+C on start and wait up to 30 seconds for graceful exit
vrunner --send-keys "<C-c>" --exit-timeout 30 -- app
```

---

## Subcommands

Subcommands operate on *already running* vrunner instances via the HTTP API or
local IPC.

### `list`

List all running vrunner instances.

```bash
vrunner list
```

Output includes instance PID, status, and other details.

---

### `stop`

Stop a vrunner instance by PID. Auto-selects if only one instance is running.

```bash
vrunner stop [pid]
```

| Argument | Description |
|----------|-------------|
| `pid` | PID of the instance to stop. Omit to auto-select the single running instance. |

---

### `spawn`

Dynamically create a new command in a running vrunner instance.

```bash
vrunner spawn <cmd> [args...] [--rows <n>] [--cols <n>]
```

| Argument / Flag | Description |
|-----------------|-------------|
| `cmd` | Command to run. |
| `args` | Arguments for the command (trailing, all remaining tokens after `cmd`). |
| `--rows <n>` | VTTY rows for the spawned command. |
| `--cols <n>` | VTTY columns for the spawned command. |

---

### `freeze`

Pause a running command by sending `SIGSTOP`. The process is frozen in place;
its terminal buffer is preserved.

```bash
vrunner freeze <pid>
```

| Argument | Description |
|----------|-------------|
| `pid` | PID of the command to freeze. |

---

### `thaw`

Resume a previously frozen command by sending `SIGCONT`.

```bash
vrunner thaw <pid>
```

| Argument | Description |
|----------|-------------|
| `pid` | PID of the command to thaw. |

---

### `resize`

Change the virtual terminal dimensions of a running command.

```bash
vrunner resize <target> --rows <n> --cols <n>
```

| Argument / Flag | Default | Description |
|-----------------|---------|-------------|
| `target` | — | PID or name of the command to resize. |
| `--rows <n>` | terminal height | New terminal row count (`0` = use terminal height). |
| `--cols <n>` | terminal width | New terminal column count (`0` = use terminal width). |

---

### `purge`

Remove an exited command from vrunner's internal state, discarding its
scrollback buffer and snapshot data.

```bash
vrunner purge [target]
```

| Argument | Description |
|----------|-------------|
| `target` | Command ID or name of the exited command to purge. |

---

### `cert`

Managed TLS certificate operations. See [`../certificates.md`](../certificates.md)
for detailed workflows.

#### `cert generate`

Generate a new named certificate.

```bash
vrunner cert generate <name>
```

| Argument | Description |
|----------|-------------|
| `name` | Name for the certificate (e.g., `webapp-frontend`). |

#### `cert list`

List all certificates in the pool.

```bash
vrunner cert list
```

#### `cert show`

Display the details of a specific certificate.

```bash
vrunner cert show <name>
```

| Argument | Description |
|----------|-------------|
| `name` | Name of the certificate to display. |

#### `cert remove`

Remove a certificate from the pool by name.

```bash
vrunner cert remove <name>
```

| Argument | Description |
|----------|-------------|
| `name` | Name of the certificate to remove. |

---

### `list-vrunner`

Discover running vrunner instances on the local machine.

```bash
vrunner list-vrunner
```

Machine-readable, tab-separated output. Includes PID, listen address,
port, config path, and uptime.

---

### `list-commands`

List running commands (machine-readable, tab-separated).

```bash
vrunner list-commands
```

---

### `stop-command`

Stop a specific command by PID or name (not the whole instance).

```bash
vrunner stop-command [target]
```

| Argument | Description |
|----------|-------------|
| `target` | PID or name of the command to stop. |

---

### `config-check`

Validate configuration files without starting the server. Reports errors,
warnings, and deprecation notices.

```bash
vrunner config-check
```

Exit codes: `0` = valid, `1` = errors found.

---

### `cat`

Print the VTTY buffer of a running command to stdout. This is useful for
inspecting what a command is currently displaying, capturing its output into
a file, or piping it into other tools. By default the output is plain text
with all ANSI formatting stripped, making it safe for grep, awk, and other
text-processing utilities.

```bash
# Print plain text (no colors)
vrunner cat

# Print with ANSI colors preserved
vrunner cat --color-always

# Target a specific command by PID or name
vrunner cat 12345
vrunner cat --color-always htop
```

| Flag | Default | Description |
|------|---------|-------------|
| `--color-always` | `false` | Preserve ANSI color escape sequences in the output. When set, the buffer is rendered with its original colors, bold, underline, and other formatting exactly as the child process displayed it. Without this flag, all ANSI sequences are stripped and only plain text is printed. |

When no target is given and exactly one command is running, that command
is used automatically. If multiple commands are running, you must specify
a target by PID or by name. The output includes both the visible screen
content and the scrollback history.

Common use cases:

```bash
# Capture command output to a file
vrunner cat my-server > output.log

# Search for a pattern in a running command's output
vrunner cat db-console | grep "ERROR"

# View colored output (useful for commands that use syntax highlighting)
vrunner cat --color-always vim-session | less -R
```

---

### `screenshot`

Capture the VTTY buffer of a running command as a PNG screenshot.

```bash
# Screenshot with defaults
vrunner screenshot

# Screenshot a specific command with custom settings
vrunner screenshot --output capture.png --font-size 16 htop
```

| Argument / Flag | Default | Description |
|-----------------|---------|-------------|
| `target` | auto-select | PID or name of the command to screenshot. |
| `--output <path>` | `screenshot.png` | Output file path for the PNG image. |
| `--font-size <px>` | `14` | Font size in pixels per character cell (range: 6–48). |
| `--font-name <path>` | system default | Path to a TTF/OTF font file. When omitted, vrunner searches common system paths for a monospace font. |

---

## Special Key Notation

The `--send-keys` flag and the `keys` field in the API accept key
sequences using `<...>` notation for special keys. Literal characters can
be typed directly.

### Modifier Prefixes

| Prefix | Meaning | Example |
|--------|---------|---------|
| `C-` | Control modifier | `<C-c>`, `<C-d>` |
| `M-` | Alt / Meta modifier | `<M-Enter>`, `<M-f>` |
| `S-` | Shift modifier | `<S-Tab>` |

Modifiers may be combined: `<C-S-Esc>`.

### Special Keys

| Notation | Meaning |
|----------|---------|
| `<Enter>` | Enter / Return |
| `<Tab>` | Tab |
| `<Esc>` | Escape |
| `<Space>` | Space bar |
| `<BS>` | Backspace |
| `<Del>` | Delete |
| `<Ins>` | Insert |
| `<Home>` | Home |
| `<End>` | End |
| `<PgUp>` | Page Up |
| `<PgDn>` | Page Down |
| `<Up>` | Arrow Up |
| `<Down>` | Arrow Down |
| `<Left>` | Arrow Left |
| `<Right>` | Arrow Right |
| `F1` through `F20` | Function keys |

### Literal Characters

Any single printable character can be used directly: `a`, `Z`, `0`, `!`, `/`.

### Examples

```bash
# Send Ctrl+C followed by "y" and Enter
vrunner --send-keys "<C-c>y<Enter>" -- interactive-app

# Send Escape, then :q! followed by Enter (Vim quit)
vrunner --send-keys "<Esc>:q!<Enter>" -- vim file.txt

# Send Alt+F to jump forward one word in readline
vrunner --send-keys "<M-f>" -- bash
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success (normal exit, config-check passed). |
| `1` | General error (invalid arguments, file not found). |
| `2` | Child process exited with an error. |
| `130` | vrunner was interrupted by `SIGINT` (Ctrl+C). |

---

## See Also

- [`../configuration.md`](../configuration.md) — Full configuration file reference
- [`keybindings.md`](keybindings.md) — Interactive keyboard shortcuts
- [`../api.md`](../api.md) — HTTP API reference
- [`../websocket.md`](../websocket.md) — WebSocket protocol reference
- [`../hooks.md`](../hooks.md) — Event hooks reference
