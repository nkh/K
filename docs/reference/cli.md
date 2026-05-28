# vrunner CLI Reference

Complete reference for the `vrunner` command-line interface. Options are
organized by functional category. Every flag corresponds to a configuration key
described in [`../configuration.md`](../configuration.md); CLI flags take
precedence over config-file and environment-variable values.

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
2. `VRUNNER_CONFIG` environment variable.
3. `./vrunner.yaml` (or `./vrunner.yml`).

---

## Server Options

Control how vrunner binds its HTTP/WebSocket server.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--bind <addr>` | `127.0.0.1` | `server.bind` | Host address to listen on. Use `0.0.0.0` to accept connections from any interface. |
| `--port <n>` | `8080` | `server.port` | TCP port for the HTTP server. |
| `--remote` | `false` | `server.remote` | Enable remote access mode. When set, vrunner relaxes loopback-only checks and applies remote-access defaults (e.g. `--bind 0.0.0.0`). |

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
| `--auth <method>` | `none` | `security.auth` | Authentication method to enforce. Accepted values: `none`, `token`, `basic`. |
| `--token-file <path>` | — | `security.token_file` | Path to a file containing a shared secret token (used when `--auth token` is set). The file is read once at startup; the trailing newline is stripped. |

When `--auth token` is active, every HTTP request must include an
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

TLS is automatically enabled when `--remote` is combined with certificate
paths. See [`certificates.md`](../certificates.md) and the
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
| `--scrollback <n>` | `10000` | `vtty.scrollback` | Maximum number of scrollback lines retained in the virtual terminal ring buffer. |
| `--truecolor` / `--no-truecolor` | `true` | `vtty.truecolor` | Enable or disable 24-bit true-color support. When disabled, the terminal reports `xterm-256color` regardless of the `--term` setting. |
| `--mouse` / `--no-mouse` | `true` | `vtty.mouse` | Enable or disable mouse event forwarding from connected WebSocket clients to the child PTY. |

The virtual terminal dimensions can be changed at runtime via the
[resize subcommand](#resize) or the WebSocket `resize` message
(see [`../websocket.md`](../websocket.md)).

---

## Display Options

Configure the built-in terminal multiplexer dashboard rendered in the
terminal where vrunner itself is running.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--display` / `--no-display` | `true` (auto) | `display.enabled` | Enable the interactive display. Automatically disabled when stdout is not a TTY (e.g. piped output, CI). |
| `--display-all` | `false` | `display.all` | Show *all* command outputs simultaneously instead of the active pane only. Each pane is given a proportional viewport. |
| `--refresh-ms <n>` | `100` | `display.refresh_ms` | Milliseconds between display redraw cycles. Lower values produce smoother output at the cost of higher CPU usage. |
| `--tabs` | `true` | `display.tabs` | Show a tab bar listing all running commands. Tabs can be switched with configurable keybindings (see [`keybindings.md`](keybindings.md)). |

When `--no-display` is set, vrunner runs in headless mode: command output is
not rendered locally, but the HTTP/WebSocket API remains fully functional.

---

## Logging Options

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--log <level>` | `info` | `logging.level` | Minimum log severity. Accepted values (lowest to highest): `trace`, `debug`, `info`, `warn`, `error`. |
| `--log-file <path>` | — | `logging.file` | Write log output to the given file in addition to stderr. The file is created if it does not exist; it is appended to if it does. |
| `--log-pty-raw` | `false` | `logging.pty_raw` | Log raw bytes received from the child PTY before any ANSI processing. Useful for debugging terminal escape sequences. Produces very verbose output. |

---

## Daemon Options

Run vrunner as a background daemon process.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--daemon` | `false` | `daemon.enabled` | Fork into the background after initialization. The parent process exits once the server is ready. |
| `--stdout-file <path>` | `vrunner.out` | `daemon.stdout_file` | File to which the daemon's stdout is redirected. |
| `--stderr-file <path>` | `vrunner.err` | `daemon.stderr_file` | File to which the daemon's stderr is redirected. |

On startup in daemon mode, vrunner writes its PID to `vrunner.pid` in the
working directory (configurable via `daemon.pid_file`).

**Example**

```bash
vrunner --daemon --log warn -- python worker.py
```

---

## Exit Handler Options

These options control what happens when the child command exits or encounters
an error, and allow automated keystroke injection.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--on-exit <action>` | `stop` | `exit_handler.on_exit` | Action when the child exits normally. Values: `stop` (shut down vrunner), `restart` (restart the command), `hold` (keep the terminal available). |
| `--on-error <action>` | `stop` | `exit_handler.on_error` | Action when the child exits with a non-zero code. Same values as `--on-exit`. |
| `--exit-timeout <duration>` | `10s` | `exit_handler.timeout` | Maximum time to wait for the child process to terminate gracefully before sending `SIGKILL`. Accepts human-friendly durations: `30s`, `1m`, `500ms`. |
| `--retain-on-exit` | `false` | `exit_handler.retain` | Keep the virtual terminal session alive after the child process exits. The terminal remains accessible via the WebSocket API for inspection. |
| `--snapshot-on-exit` | `false` | `exit_handler.snapshot` | Automatically capture a terminal snapshot when the child process exits. Snapshots are stored in the configured snapshot directory. |
| `--send-keys <keys>` | — | `exit_handler.send_keys` | Comma-separated list of keys to send to the child PTY on exit. See [Special Key Notation](#special-key-notation) for the full syntax. Common use case: send `ctrl+c` before shutdown to allow graceful termination of interactive programs. |

**Example**

```bash
# Send Ctrl+C and wait up to 30 seconds on exit
vrunner --on-exit stop --exit-timeout 30s --send-keys "ctrl+c" -- app
```

---

## Interactive Options

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--tabs` | `true` | `display.tabs` | Enable the tab bar in the interactive display. See [Display Options](#display-options) above. |

Interactive keybindings are documented in full in
[`keybindings.md`](keybindings.md).

---

## Subcommands

Subcommands operate on *already running* vrunner instances via the HTTP API or
local IPC.

### `list`

List all commands managed by the running vrunner instance.

```bash
vrunner list [--format <table|json|plain>] [--config <path>]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--format` | `table` | Output format. `json` produces machine-readable output suitable for piping. |

Output columns: ID, name, PID, status, uptime, exit code (if exited).

---

### `stop`

Gracefully stop a running command or the entire vrunner instance.

```bash
vrunner stop [--id <command-id>] [--signal <sig>] [--config <path>]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--id` | *primary* | ID of the command to stop. Omit to stop all commands and shut down vrunner. |
| `--signal` | `SIGTERM` | Signal to send to the child process. |

---

### `spawn`

Dynamically create a new command in a running vrunner instance.

```bash
vrunner spawn --name <name> -- <command> [args...]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--name` | auto-generated | Human-readable name for the new command. |
| `--env <k=v>` | — | Environment variable to set. May be specified multiple times. |
| `--dir <path>` | vrunner's working directory | Working directory for the new command. |

---

### `freeze`

Pause a running command by sending `SIGSTOP`. The process is frozen in place;
its terminal buffer is preserved.

```bash
vrunner freeze --id <command-id> [--config <path>]
```

---

### `thaw`

Resume a previously frozen command by sending `SIGCONT`.

```bash
vrunner thaw --id <command-id> [--config <path>]
```

---

### `resize`

Change the virtual terminal dimensions of a running command.

```bash
vrunner resize --id <command-id> --rows <n> --cols <n> [--config <path>]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--rows` | — | New terminal row count. |
| `--cols` | — | New terminal column count. |

---

### `purge`

Remove a stopped command from vrunner's internal state, freeing its
scrollback buffer and snapshot data.

```bash
vrunner purge --id <command-id> [--config <path>]
```

---

### `cert`

Managed TLS certificate operations. See [`../certificates.md`](../certificates.md)
for detailed workflows.

#### `cert generate`

Generate a self-signed TLS certificate and private key.

```bash
vrunner cert generate [--days <n>] [--host <hostname>] \
    [--cert-out <path>] [--key-out <path>]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--days` | `365` | Certificate validity period in days. |
| `--host` | `localhost` | Subject Alternative Name (SAN) for the certificate. May include IP addresses. |
| `--cert-out` | `vrunner.pem` | Output path for the certificate. |
| `--key-out` | `vrunner-key.pem` | Output path for the private key. |

#### `cert list`

List all certificates stored in vrunner's certificate store.

```bash
vrunner cert list [--format <table|json>]
```

#### `cert show`

Display the details of a specific certificate.

```bash
vrunner cert show <fingerprint> [--format <text|pem|json>]
```

#### `cert remove`

Remove a certificate from the store by its fingerprint.

```bash
vrunner cert remove <fingerprint>
```

---

### `list-vrunner`

Discover running vrunner instances on the local machine.

```bash
vrunner list-vrunner [--format <table|json|plain>]
```

Scans for PID files and lock sockets. Output includes PID, listen address,
port, config path, and uptime.

---

### `list-commands`

Alias for `list`, retained for backward compatibility.

```bash
vrunner list-commands [--format <table|json|plain>]
```

---

### `stop-command`

Alias for `stop --id <id>`, retained for backward compatibility.

```bash
vrunner stop-command <command-id> [--signal <sig>]
```

---

### `config-check`

Validate a configuration file without starting vrunner. Reports errors,
warnings, and deprecation notices.

```bash
vrunner config-check [--config <path>] [--strict]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--strict` | `false` | Treat warnings as errors and exit with non-zero status. |

Exit codes: `0` = valid, `1` = errors found, `2` = warnings (unless `--strict`).

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

## Special Key Notation

The `--send-keys` flag and the `keys` field in the API both accept a
comma-separated list of key identifiers. The following notation is supported.

### Modifier Prefixes

| Prefix | Meaning | Example |
|--------|---------|---------|
| `ctrl+` | Control modifier | `ctrl+c`, `ctrl+d` |
| `alt+` | Alt / Meta modifier | `alt+enter`, `alt+f` |
| `shift+` | Shift modifier | `shift+tab` |

Modifiers may be combined: `ctrl+shift+esc`.

### Function Keys

| Notation | Key |
|----------|-----|
| `f1` through `f20` | Function keys |
| `f1` | F1 |
| `f12` | F12 |

### Special Keys

| Notation | Meaning |
|----------|---------|
| `enter` | Enter / Return |
| `tab` | Tab |
| `esc` | Escape |
| `space` | Space bar |
| `backspace` | Backspace |
| `delete` | Delete |
| `insert` | Insert |
| `home` | Home |
| `end` | End |
| `pageup` | Page Up |
| `pagedown` | Page Down |
| `up` | Arrow Up |
| `down` | Arrow Down |
| `left` | Arrow Left |
| `right` | Arrow Right |

### Literal Characters

Any single printable character can be used directly: `a`, `Z`, `0`, `!`, `/`.

### Examples

```bash
# Send Ctrl+C followed by "y" and Enter
vrunner --send-keys "ctrl+c,y,enter" -- interactive-app

# Send Escape, then :q! followed by Enter (Vim quit)
vrunner --send-keys "esc,:,q,!,enter" -- vim file.txt

# Send Alt+F to jump forward one word in readline
vrunner --send-keys "alt+f" -- bash
```

---

## Environment Variables

All CLI flags can be set via environment variables using the `VRUNNER_`
prefix with uppercased, underscore-separated names.

| Variable | Equivalent Flag |
|----------|----------------|
| `VRUNNER_CONFIG` | `--config` |
| `VRUNNER_PORT` | `--port` |
| `VRUNNER_BIND` | `--bind` |
| `VRUNNER_REMOTE` | `--remote` |
| `VRUNNER_AUTH` | `--auth` |
| `VRUNNER_TOKEN_FILE` | `--token-file` |
| `VRUNNER_TLS` | `--tls` |
| `VRUNNER_CERT_FILE` | `--cert-file` |
| `VRUNNER_KEY_FILE` | `--key-file` |
| `VRUNNER_TERM` | `--term` |
| `VRUNNER_LOG` | `--log` |
| `VRUNNER_LOG_FILE` | `--log-file` |
| `VRUNNER_DAEMON` | `--daemon` |
| `VRUNNER_DISPLAY` | `--display` / `--no-display` |
| `VRUNNER_DISPLAY_ALL` | `--display-all` |

Environment variables are overridden by CLI flags, which are in turn
overridden by configuration file values for keys that support runtime
reconfiguration.

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
