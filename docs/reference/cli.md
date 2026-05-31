# vrl / vrunner CLI Reference

Complete reference for the `vrl` and `vrunner` command-line interfaces. Both
binaries share the same core CLI options (VTTY, display, daemon, exit handlers)
and differ in their transport layer and binary-specific subcommands. Every flag
corresponds to a configuration key described in [`../configuration.md`](../configuration.md);
CLI flags take precedence over config-file values.

| | vrl | vrunner |
|--|-----|---------|
| **Transport** | UDS IPC | HTTP + WebSocket |
| **Default feature** | Yes | No (`--features vrunner`) |
| **Server** | UDS socket (`control-{pid}.sock`) | HTTP server (`:9090`) |

Options marked **Shared** work identically for both binaries.

---

## Synopsis

### vrl

```
vrl [GENERAL OPTIONS] [CATEGORY OPTIONS] -- <command> [args...]
vrl <subcommand> [SUBCOMMAND OPTIONS]
```

### vrunner

```
vrunner [GENERAL OPTIONS] [SERVER OPTIONS] [CATEGORY OPTIONS] -- <command> [args...]
vrunner <subcommand> [SUBCOMMAND OPTIONS]
```

---

## General Options (Shared)

These top-level flags control which configuration file is loaded and provide
standard help/version information.

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--config <path>` | `-c` | `vrl.yaml` in the current directory | Path to the configuration file. |
| `--help` | `-h` | — | Print usage summary and exit. |
| `--version` | `-V` | — | Print version string and exit. |

The config file is resolved in this order:

1. Path given via `--config`.
2. `./vrl.yaml` (or `./vrl.yml`).

---

## Server Options (vrunner only)

These options control the HTTP server that vrunner starts.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--bind <addr>` | `127.0.0.1` | `server.bind` | Bind address for the HTTP server. |
| `--port <n>` | `9090` | `server.port` | Port for the HTTP server. |
| `--auth` | `false` | `security.require_auth` | Enable bearer token authentication. |
| `--tls` | `false` | `tls.enabled` | Enable TLS with auto-generated self-signed certificates. |
| `--remote` | `false` | — | Shorthand for `--bind 0.0.0.0 --auth --tls`. Accept connections from any interface with authentication and encryption. |
| `--cert-file <path>` | — | `tls.cert_file` | Path to a custom TLS certificate file. |
| `--key-file <path>` | — | `tls.key_file` | Path to a custom TLS private key file. |

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
| `--display` / `--no-display` | `false` | `display.enabled` | Enable the interactive display. Must be explicitly set to render command output locally. |
| `--display-all` | `false` | `display.all` | Show *all* command outputs simultaneously instead of the active pane only. Implies `--display`. |
| `--refresh-ms <n>` | `100` | `display.refresh_ms` | Milliseconds between display redraw cycles. Lower values produce smoother output at the cost of higher CPU usage. |

When `--no-display` is set (the default), the binary runs in headless mode:
command output is not rendered locally.

---

## Interactive Options (Shared)

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--tabs` | `false` | `interactive.tabs` | Show a tab bar listing all running commands. |

Interactive keybindings are documented in full in
[`keybindings.md`](keybindings.md).

---

## Logging Options (Shared)

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--log` | `false` | `command_log.enabled` | Enable command logging to the terminal. |
| `--log-file <path>` | — | `command_log.file` | Enable command logging and write output to the given file. |
| `--log-pty-raw <path>` | — | `command_log.pty_raw_log` | Log raw bytes received from the child PTY to the given file before any ANSI processing. |

---

## Daemon Options (Shared)

Run as a background daemon process.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--daemon` | `false` | `daemon.enabled` | Fork into the background after initialization. Conflicts with `--display`, `--display-all`, and `--tabs`. |
| `--stdout-file <path>` | `vrl.out` | `daemon.stdout_file` | File to which the daemon's stdout is redirected. |
| `--stderr-file <path>` | `vrl.err` | `daemon.stderr_file` | File to which the daemon's stderr is redirected. |

**Example**

```bash
vrl --daemon -- python worker.py
vrunner --daemon -- python worker.py
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
vrl --send-keys "<C-c>" --exit-timeout 30 -- app
```

---

## Subcommands — vrl

vrl subcommands operate on *already running* vrl instances via UDS IPC.

### `list`

List all running vrl instances and their commands.

```bash
vrl list
```

Output includes instance PID, status, daemon/display flags, uptime, and commands with their PIDs, names, and running status.

```bash
vrl list --target 12345
```

---

### `stop`

Stop a vrl instance by PID. Sends `SIGTERM` to the instance process.

```bash
vrl stop [pid]
```

| Argument | Description |
|----------|-------------|
| `pid` | PID of the instance to stop. Omit to auto-select the single running instance. |

---

### `spawn-in`

Dynamically create a new command in a running vrl instance via UDS IPC.

```bash
vrl spawn-in <pid> -- <cmd> [args...] [--rows <n>] [--cols <n>]
```

| Argument / Flag | Description |
|-----------------|-------------|
| `pid` | PID of the target vrl instance. |
| `cmd` | Command to run. |
| `args` | Arguments for the command (everything after `--`). |
| `--rows <n>` | VTTY rows for the spawned command. |
| `--cols <n>` | VTTY columns for the spawned command. |

---

### `keys`

Send keystrokes to a running command via UDS IPC.

```bash
vrl keys <pid> <keys>
```

---

### `cat`

Print the VTTY buffer of a running command to stdout.

```bash
vrl cat
vrl cat --color-always
vrl cat 12345
vrl cat --color-always htop
```

| Flag | Default | Description |
|------|---------|-------------|
| `--color-always` | `false` | Preserve ANSI color escape sequences in the output. |

---

### `freeze`

Pause a running command by sending `SIGSTOP`.

```bash
vrl freeze <pid>
```

---

### `thaw`

Resume a previously frozen command by sending `SIGCONT`.

```bash
vrl thaw <pid>
```

---

### `resize`

Change the virtual terminal dimensions of a running command.

```bash
vrl resize <target> --rows <n> --cols <n>
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
vrl config-check
```

Exit codes: `0` = valid, `1` = errors found.

---

### `completions`

Generate shell completion scripts.

```bash
vrl completions bash > /etc/bash_completion.d/vrl
vrl completions zsh > ~/.zsh/completions/_vrl
vrl completions fish > ~/.config/fish/completions/vrl.fish
```

---

## Subcommands — vrunner

vrunner subcommands communicate with running instances via the HTTP API.

### `list`

List all running vrunner instances and their commands.

```bash
vrunner list
```

---

### `stop`

Stop a vrunner instance by PID. Sends a shutdown request via the HTTP API.

```bash
vrunner stop [pid]
```

---

### `spawn`

Spawn a new command in a running vrunner instance via the HTTP API.

```bash
vrunner spawn [OPTIONS] CMD [ARGS...]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--env <k=v>` | — | Environment variables for the command. |
| `--rows <n>` | config default | VTTY rows for the spawned command. |
| `--cols <n>` | config default | VTTY columns for the spawned command. |
| `--dir <path>` | — | Working directory for the command. |

```bash
vrunner spawn htop
vrunner spawn --env RUST_LOG=debug -- cargo run
vrunner --target 12345 spawn npm run dev
```

---

### `stop-command`

Stop a specific running command by ID or name.

```bash
vrunner stop-command <target>
```

---

### `list-vrunner`

List all running vrunner server instances.

```bash
vrunner list-vrunner
```

---

### `list-commands`

List all commands across all running vrunner instances.

```bash
vrunner list-commands
```

---

### `freeze`

Pause a running command by sending `SIGSTOP`.

```bash
vrunner freeze <pid>
```

---

### `thaw`

Resume a previously frozen command by sending `SIGCONT`.

```bash
vrunner thaw <pid>
```

---

### `resize`

Change the virtual terminal dimensions of a running command.

```bash
vrunner resize <target> --rows <n> --cols <n>
```

---

### `purge`

Remove a retained (exited) command from memory.

```bash
vrunner purge [target]
```

---

### `screenshot`

Capture a PNG screenshot of a running command's VTTY output.

```bash
vrunner screenshot [name] [--output <path>]
```

---

### `cat`

Print the VTTY buffer of a running command to stdout.

```bash
vrunner cat [name]
```

---

### `config-check`

Validate configuration files without starting anything.

```bash
vrunner config-check
```

---

### `completions`

Generate shell completion scripts.

```bash
vrunner completions bash > /etc/bash_completion.d/vrunner
vrunner completions zsh > ~/.zsh/completions/_vrunner
vrunner completions fish > ~/.config/fish/completions/vrunner.fish
```

---

### `cert`

Manage per-command client certificates (vrunner only).

```bash
vrunner cert generate <name>
vrunner cert list
vrunner cert show <name>
vrunner cert remove <name>
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
vrl --send-keys "<C-c>y<Enter>" -- interactive-app

# Send Escape, then :q! followed by Enter (Vim quit)
vrl --send-keys "<Esc>:q!<Enter>" -- vim file.txt
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
- [`../api.md`](../api.md) — vrunner REST API reference
- [`../explanation/architecture.md`](../explanation/architecture.md) — System architecture
