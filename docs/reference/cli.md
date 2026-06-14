# vrc / vrw CLI Reference

Complete reference for the `vrc` and `vrw` command-line interfaces. Both
binaries share the same core CLI options (VTTY, display, daemon, exit handlers)
and differ in their transport layer and binary-specific subcommands. Every flag
corresponds to a configuration key described in [`../configuration.md`](../configuration.md);
CLI flags take precedence over config-file values.

| | vrc | vrw |
|--|-----|---------|
| **Transport** | UDS IPC | HTTP + WebSocket |
| **Default feature** | Yes | No (`--features vrw`) |
| **Server** | UDS socket (`control-{pid}.sock`) | HTTP server (`:9090`) |

Options marked **Shared** work identically for both binaries.

---

## Synopsis

### vrc

```
vrc [GENERAL OPTIONS] [CATEGORY OPTIONS] -- <command> [args...]
vrc <subcommand> [SUBCOMMAND OPTIONS]
```

### vrw

```
vrw [GENERAL OPTIONS] [SERVER OPTIONS] [CATEGORY OPTIONS] -- <command> [args...]
vrw <subcommand> [SUBCOMMAND OPTIONS]
```

When no subcommand is given and no flags are present, trailing arguments are treated as an implicit spawn. `vrw btop` is equivalent to `vrw spawn btop` — both send the command to an already-running vrw instance. If no instance is running, the command fails. To start a **new** instance with a command, use `vrw --display btop` (or any other flag combination, e.g., `vrw --daemon -- python server.py`).

---

## General Options (Shared)

These top-level flags control which configuration file is loaded and provide
standard help/version information.

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--config <path>` | `-c` | `vrc.yaml` in the current directory | Path to the configuration file. |
| `--help` | `-h` | — | Print usage summary and exit. |
| `--version` | `-V` | — | Print version string and exit. |
| `--profile <name>` | `-P` | — | Apply a named configuration profile from the config file. |
| `--working-directory <dir>` | `-w` | — | Set the working directory for spawned commands. |
| `--pid <pid>` | `-t` | — | Target a specific instance by PID. Alias for `--target`. |

The config file is resolved in this order:

1. Path given via `--config`.
2. `./vrc.yaml` (or `./vrc.yml`).

---

## Server Options (vrw only)

These options control the HTTP server that vrw starts.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--bind <addr>` | `127.0.0.1` | `server.bind` | Bind address for the HTTP server. |
| `--port <n>` | `9090` | `server.port` | Port for the HTTP server. |
| `--auth` | `false` | `security.require_auth` | Enable bearer token authentication. |
| `--tls` | `false` | `tls.enabled` | Enable TLS with auto-generated self-signed certificates. |
| `--remote` | `false` | — | Shorthand for `--bind 0.0.0.0 --auth --tls`. Accept connections from any interface with authentication and encryption. |
| `--cert-file <path>` | — | `tls.cert_file` | Path to a custom TLS certificate file. |
| `--key-file <path>` | — | `tls.key_file` | Path to a custom TLS private key file. |
| `--server-name <name>` | — | `server.name` | Assign a human-readable name to this server instance. Displayed in `vrw list`, `vrw cat`, and the web UI panel titlebar instead of host:port. |
| `--register-with <port>` | — | — | Register this instance with another vrw server at the specified port. |
| `--token-file <path>` | `~/.config/vrw/token` | `security.token_file` | Path to the bearer token file. If the file does not exist when auth is required, a cryptographically random 256-bit token is generated and saved. |
| `--certificate <name:cert:key>` | — | `certificates` | Define a named certificate for the certificate pool. Repeatable. Format: `NAME:CERT:KEY`. |

---

## VTTY Options (Shared)

Control the virtual terminal (PTY) that the binary creates for the child command.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--vtty-rows <n>` | `24` | `vtty.rows` | Initial number of rows in the virtual terminal. |
| `--vtty-cols <n>` | `80` | `vtty.cols` | Initial number of columns in the virtual terminal. |
| `--term <type>` | `xterm-256color` | `vtty.term` | Value of the `TERM` environment variable inside the child process. |
| `--scrollback <n>` | `5000` | `vtty.scrollback` | Maximum number of scrollback lines retained in the virtual terminal ring buffer. |
| `--truecolor` / `--no-truecolor` | `true` | `vtty.truecolor` | Enable or disable 24-bit true-color support. |
| `--mouse` / `--no-mouse` | `false` | `vtty.mouse` | Enable or disable mouse event forwarding to the child PTY. |

The virtual terminal dimensions can be changed at runtime via the
[resize subcommand](#resize).

---

## Display Options (Shared)

Configure the built-in terminal multiplexer rendered in the
terminal where the binary itself is running.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--display` / `--no-display` | `false` | `display.enabled` | Enable the interactive display and keep showing output after the initial command exits. Equivalent to the old --display-all. |
| `--refresh-ms <n>` | `100` | `display.refresh_ms` | Milliseconds between display redraw cycles. Lower values produce smoother output at the cost of higher CPU usage. |

When `--no-display` is set (the default), the binary runs in headless mode:
command output is not rendered locally.

---

## Interactive Options (Shared)

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--tabs` | `false` | `interactive.tabs` | Show a tab bar listing all running commands. |

**Per-subcommand `-i, --interactive`**: Several subcommands (`list`, `stop`, `kill`, `stop-command`, `cat`, `freeze`, `thaw`, `resize`, `screenshot`) accept `-i` / `--interactive` as a subcommand flag (not a top-level flag). When given, the subcommand presents a numbered list for interactive selection instead of requiring a PID or target argument.

Interactive keybindings are documented in full in
[`keybindings.md`](keybindings.md).

---

## Logging Options (Shared)

| Flag | Short | Default | Config Key | Description |
|------|-------|---------|------------|-------------|
| `--log` | `-l` | `false` | `command_log.enabled` | Enable command logging to the terminal. |
| `--log-file <path>` | `-L` | — | `command_log.file` | Enable command logging and write output to the given file. |
| `--log-pty-raw <path>` | — | — | `command_log.pty_raw_log` | Log raw bytes received from the child PTY to the given file before any ANSI processing. |
| `--no-log` | — | `false` | `command_log.enabled` | Suppress activity logging entirely (no buffer, no broadcast, no file, no stdout). Overrides `--log`. |
| `--no-terminal-log` | — | `false` | — | Suppress terminal event output when not in `--display` mode. Events are still buffered, broadcast, and logged to the log file if `--log` is active. |
| `--quiet` | `-q` | `false` | — | Hidden alias for `--no-terminal-log`. Only suppresses terminal output, not file logging. |
| `--color-terminal-log` | `-F` | `false` | — | Use ANSI color codes in terminal log output. Each field (timestamp, id, command name, event details) gets a distinct color for readability. |

### Log Format

Each log line contains the following fields:

| Field | Terminal (spaces) | File (tabs) | Description |
|-------|-------------------|-------------|-------------|
| Timestamp | `HH:MM:SS.cc` | `HH:MM:SS.cc` | Local time, hundredths of a second |
| ID | 8 chars (truncated UUID) | 8 chars | First 8 characters of the command UUID |
| Command | 20 chars (padded) | command name | Command name (from `cmd=` or `name=`) |
| Event + details | `event: details...` | `event: details...` | Event type and arguments (id/cmd omitted in terminal to avoid repetition) |

**Terminal output example** (space-separated, columns aligned, id and cmd stripped from details):

```
17:25:03.12 a1b2c3d4 htop                 spawn: args=[] cert=None env=[] size=24x80 dir=None
17:25:10.45 a1b2c3d4 htop                 resize: rows=40 cols=120
17:26:05.78 a1b2c3d4 htop                 exited: name=htop code=Some(0)
17:26:05.79 a1b2c3d4 htop                 exit: retained=false code=Some(0)
```

**File output** (tab-separated, no color, no padding, full details):

```
17:25:03.12     a1b2c3d4        htop    spawn: id=a1b2c3d4-... cmd=htop args=[] cert=None env=[] size=24x80 dir=None
17:26:05.78     a1b2c3d4        htop    exited: id=a1b2c3d4-... name=htop code=Some(0)
```

Use `-F` / `--color-terminal-log` to enable ANSI colors in the terminal (each field gets a distinct color):

```
[dark-grey]17:25:03.12[reset] [green]a1b2c3d4[reset] [bright-white]htop               [reset] [bright-white]spawn[reset]: [white]args=[][reset] [blue]cert=None[reset] [green]env=[][reset] [bright-yellow]size=24x80[reset] [blue]dir=None[reset]
```

Color assignments for detail fields:

| Detail field | Color |
|-------------|-------|
| `args=` | Bright white |
| `cert=` | Blue |
| `env=` | Green |
| `size=` | Bright yellow |
| `dir=` | Blue |

### Logging Decision Table

The following table shows what happens for each combination of command mode and logging flags:

| Mode | No flags | `--log` | `--log-file` | `--no-log` | `-q` / `--no-terminal-log` | `-F` / `--color-terminal-log` |
|------|----------|---------|--------------|-------------|---------------------------|-------------------------------|
| **`vrw`** (no display, no daemon) | Event loop prints to terminal from broadcast | Event loop + stdout + file | File only | Nothing buffered, broadcast, or printed | Event loop suppressed; buffer + broadcast + file still work | Colors in terminal output |
| **`vrw --daemon`** | Buffer + broadcast only (no terminal) | Buffer + broadcast + file | File only | Nothing buffered, broadcast, or printed | N/A (no terminal in daemon mode) | N/A |
| **`vrw --display`** | Log overlay in display (from memory buffer) | Log overlay + stdout + file | Log overlay + file | Nothing buffered, broadcast, or printed | N/A (display replaces terminal) | Colors in log overlay |
| **`vrw -F`** | Colors in event loop output | Colors in event loop + stdout | Colors in event loop; file is plain | Nothing | Colors in event loop if shown | Colors in terminal output |

### Default behavior without `--display`

When vrc/vrw runs **without** `--display` (the default), event log entries are automatically printed to the terminal in real time via the event loop (subscribes to the CommandLogger broadcast). This includes:

- **spawn** — a new command was spawned
- **exited** — a command process terminated (with exit code)
- **exit** — a command was removed from or retained in the manager
- **kill** — a command was killed via the API or CLI
- **resize** — a command's VTTY was resized
- **freeze** / **thaw** — a command was suspended or resumed
- **send_keys** — keystrokes were injected into a command
- **purge** — a retained command was discarded

Use `--no-terminal-log` (or `-q`) to suppress this output:

```bash
vrw --no-terminal-log -- python server.py        # silent — no event log
vrw -q --daemon -- worker.sh                     # silent daemon
```

---

## Signal Options (Shared)

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--handle-sigwinch` | `false` | — | Resize the VTTY when the terminal is resized via SIGWINCH. By default VTTY dimensions are fixed at spawn time. |

---

## Daemon Options (Shared)

Run as a background daemon process.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--daemon` | `false` | `daemon.enabled` | Daemonize into the background after initialization (via the `daemonize` crate). Conflicts with `--display` and `--tabs`. |
| `--stdout-file <path>` | `$XDG_STATE_HOME/vrc.out` | `daemon.stdout_file` | File to which the daemon's stdout is redirected. |
| `--stderr-file <path>` | `$XDG_STATE_HOME/vrc.err` | `daemon.stderr_file` | File to which the daemon's stderr is redirected. |

**Example**

```bash
vrc --daemon -- python worker.py
vrw --daemon -- python worker.py
```

---

## Exit Handler Options (Shared)

These options control what happens when the child command exits or encounters
an error, and allow automated keystroke injection.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--on-exit <cmd>` | — | `exit_handler.on_exit` | Shell command to execute (as a detached process) when the child exits normally. |
| `--on-error <cmd>` | — | `exit_handler.on_error` | Shell command to execute (as a detached process) when the child exits with a non-zero code. |
| `--exit-timeout <secs>` | `10` | `exit_handler.timeout_secs` | Seconds to wait for the child process to terminate gracefully before sending `SIGKILL`. |
| `--retain-on-exit` | `false` | — | Keep the virtual terminal session alive after the child process exits. Per-command option only. |
| `--snapshot-on-exit <file>` | — | — | Save the VTTY buffer to the specified file when the command exits. Per-command option only. |
| `--send-keys <keys>` | — | `exit_handler.send_keys` | Keystrokes to send to the child PTY after it starts. See [Special Key Notation](#special-key-notation). |

**Example**

```bash
vrc --send-keys "<C-c>" --exit-timeout 30 -- app
```

---

## Subcommands — vrc

vrc subcommands operate on *already running* vrc instances via UDS IPC.

### `list`

List all running vrc instances and their commands.

```bash
vrc list
```

Output includes instance PID, status, daemon/display flags, uptime, and commands with their PIDs, names, and running status.

> **Note:** `--target` is a top-level flag, not a subcommand flag. Use `vrc --target 12345 list`.

---

### `stop`

Stop a vrc instance by PID. Sends `SIGTERM` to the instance process.

```bash
vrc stop [pid]
```

| Argument | Description |
|----------|-------------|
| `pid` | PID of the instance to stop. Omit to auto-select the single running instance. |

---

### `kill`

Stop (kill) a command inside a running vrc instance.

```bash
vrc kill <pid> [-c <command>] [-i] [-a]
```

| Flag | Description |
|------|-------------|
| `-c, --command <id>` | ID of the target command (omit for first). |
| `-i, --interactive` | Interactively select commands to kill. |
| `-a, --all` | Stop all commands and exit. |

---

### `stop-command`

Alias for `vrc kill`. Stop a running command inside a running vrc instance.

---

### `spawn-in`

Dynamically create a new command in a running vrc instance via UDS IPC.

```bash
vrc spawn-in <pid> -- <cmd> [args...] [--rows <n>] [--cols <n>]
```

| Argument / Flag | Description |
|-----------------|-------------|
| `pid` | PID of the target vrc instance. |
| `cmd` | Command to run. |
| `args` | Arguments for the command (everything after `--`). |
| `--rows <n>` | VTTY rows for the spawned command. |
| `--cols <n>` | VTTY columns for the spawned command. |

---

### `keys`

Send keystrokes to a running command via UDS IPC.

```bash
vrc keys <pid> <keys>
```

---

### `cat`

Print the VTTY buffer of a running command to stdout.

```bash
vrc cat <pid>
vrc cat 12345
```

> **Note:** `--color-always` is available on `vrw cat`, not `vrc cat`. The `pid` argument is required for `vrc cat`.

| Flag | Default | Description |
|------|---------|-------------|
| *(none for vrc cat)* | — | `pid` is a required positional argument. |

### `freeze`

Pause a running command by sending `SIGSTOP`.

```bash
vrc freeze <pid>
```

---

### `thaw`

Resume a previously frozen command by sending `SIGCONT`.

```bash
vrc thaw <pid>
```

---

### `resize`

Change the virtual terminal dimensions of a running command.

```bash
vrc resize <target> --rows <n> --cols <n>
```

| Argument / Flag | Default | Description |
|-----------------|---------|-------------|
| `target` | — | PID or name of the command to resize. |
| `--rows <n>` | terminal height | New terminal row count. |
| `--cols <n>` | terminal width | New terminal column count. |

---

### `config-check`

Validate configuration files without starting anything.

```bash
vrc config-check
```

Exit codes: `0` = valid, `1` = errors found.

---

### `completions`

Generate shell completion scripts.

```bash
vrc completions bash > /etc/bash_completion.d/vrc
vrc completions zsh > ~/.zsh/completions/_vrc
vrc completions fish > ~/.config/fish/completions/vrc.fish
```

---

## Subcommands — vrw

vrw subcommands communicate with running instances via the HTTP API.

### `list`

List all running vrw instances and their commands.

```bash
vrw list
```

---

### `stop`

Stop a vrw instance by PID. Sends a shutdown request via the HTTP API.

```bash
vrw stop [pid]
```

---

### `spawn`

Spawn a new command in a running vrw instance via the HTTP API.

```bash
vrw spawn [OPTIONS] CMD [ARGS...]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--rows <n>` | config default | VTTY rows for the spawned command. |
| `--cols <n>` | config default | VTTY columns for the spawned command. |

```bash
vrw spawn htop
vrw --target 12345 spawn npm run dev
```

---

### `stop-command`

Stop a specific running command by ID or name.

```bash
vrw stop-command <target> [-i] [-a]
```

| Flag | Description |
|------|-------------|
| `-i, --interactive` | Interactively select commands to stop. |
| `-a, --all` | Stop all commands and exit. |

---

### `kill`

Alias for `vrw stop-command`. Stop a running command.

```bash
vrw kill [target] [-i] [-a]
```

---

### `list-vrw`

List all running vrw server instances.

```bash
vrw list-vrw
```

---

### `list-commands`

List all commands across all running vrw instances.

```bash
vrw list-commands
```

---

### `freeze`

Pause a running command by sending `SIGSTOP`.

```bash
vrw freeze <pid>
```

---

### `thaw`

Resume a previously frozen command by sending `SIGCONT`.

```bash
vrw thaw <pid>
```

---

### `resize`

Change the virtual terminal dimensions of a running command.

```bash
vrw resize <target> --rows <n> --cols <n>
```

---

### `purge`

Remove a retained (exited) command from memory.

```bash
vrw purge [target] [-i, --interactive]
```

---

### `keep`

Tag a running command to retain its VTTY buffer after exit. Equivalent to setting `--retain-on-exit` on a command that has already started.

```bash
vrw keep [target] [-i, --interactive]
```

---

### `unkeep`

Remove the retain tag from a command so it will be cleaned up on exit. If the command has already exited, it is removed immediately.

```bash
vrw unkeep [target] [-i, --interactive]
```

---

### `screenshot`

Capture a PNG screenshot of a running command's VTTY output.

```bash
vrw screenshot [name] [--output <path>]
```

---

### `cat`

Print the VTTY buffer of a running command to stdout.

```bash
vrw cat [name]
```

---

### `config-check`

Validate configuration files without starting anything.

```bash
vrw config-check
```

---

### `completions`

Generate shell completion scripts.

```bash
vrw completions bash > /etc/bash_completion.d/vrw
vrw completions zsh > ~/.zsh/completions/_vrw
vrw completions fish > ~/.config/fish/completions/vrw.fish
```

---

### `cert`

Manage per-command client certificates (vrw only).

```bash
vrw cert generate <name>
vrw cert list
vrw cert show <name>
vrw cert remove <name>
```

---

## Special Key Notation (Shared)

The `--send-keys` flag accepts key sequences using `<...>` notation for special keys.

### Modifier Prefixes

| Prefix | Meaning | Example |
|--------|---------|---------|
| `C-` | Control modifier | `<C-c>`, `<C-d>` |
| `M-` | Alt / Meta modifier | `<M-Enter>`, `<M-f>` |

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

### Examples

```bash
# Send Ctrl+C followed by "y" and Enter
vrc --send-keys "<C-c>y<Enter>" -- interactive-app

# Send Escape, then :q! followed by Enter (Vim quit)
vrc --send-keys "<Esc>:q!<Enter>" -- vim file.txt
```

---

## Exit Codes (Shared)

| Code | Meaning |
|------|---------|
| `0` | Success. |
| `1` | General error. |
| `2` | Child process exited with an error. |
| `130` | Interrupted by `SIGINT`. |

---

## See Also

- [`../configuration.md`](../configuration.md) — Full configuration file reference
- [`keybindings.md`](keybindings.md) — Interactive keyboard shortcuts
- [`../hooks.md`](../hooks.md) — Event hooks reference
- [`../api.md`](../api.md) — vrw REST API reference
- [`../explanation/architecture.md`](../explanation/architecture.md) — System architecture
