# vrl CLI Reference

Complete reference for the `vrl` command-line interface. Options are
organized by functional category. Every flag corresponds to a configuration key
described in [`../configuration.md`](../configuration.md); CLI flags take
precedence over config-file values.

---

## Synopsis

```
vrl [GENERAL OPTIONS] [CATEGORY OPTIONS] -- <command> [args...]
vrl <subcommand> [SUBCOMMAND OPTIONS]
```

---

## General Options

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

## VTTY Options

Control the virtual terminal (PTY) that vrl creates for the child command.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--vtty-rows <n>` | `24` | `vtty.rows` | Initial number of rows in the virtual terminal. |
| `--vtty-cols <n>` | `80` | `vtty.cols` | Initial number of columns in the virtual terminal. |
| `--term <type>` | `xterm-256color` | `vtty.term` | Value of the `TERM` environment variable inside the child process. |
| `--scrollback <n>` | `5000` | `vtty.scrollback` | Maximum number of scrollback lines retained in the virtual terminal ring buffer. |
| `--truecolor` / `--no-truecolor` | `true` | `vtty.truecolor` | Enable or disable 24-bit true-color support. |
| `--mouse` / `--no-mouse` | `false` | `vtty.mouse` | Enable or disable mouse event forwarding to the child PTY. |

The virtual terminal dimensions can be changed at runtime via the
[resize subcommand](#resize) or via the `vrl resize` command.

---

## Display Options

Configure the built-in terminal multiplexer rendered in the
terminal where vrl itself is running.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--display` / `--no-display` | `false` | `display.enabled` | Enable the interactive display. Must be explicitly set to render command output locally. |
| `--display-all` | `false` | `display.all` | Show *all* command outputs simultaneously instead of the active pane only. Implies `--display`. |
| `--refresh-ms <n>` | `100` | `display.refresh_ms` | Milliseconds between display redraw cycles. Lower values produce smoother output at the cost of higher CPU usage. |

When `--no-display` is set (the default), vrl runs in headless mode:
command output is not rendered locally.

---

## Interactive Options

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--tabs` | `false` | `interactive.tabs` | Show a tab bar listing all running commands. |

Interactive keybindings are documented in full in
[`keybindings.md`](keybindings.md).

---

## Logging Options

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--log` | `false` | `command_log.enabled` | Enable command logging to the terminal. |
| `--log-file <path>` | — | `command_log.file` | Enable command logging and write output to the given file. |
| `--log-pty-raw <path>` | — | `command_log.pty_raw_log` | Log raw bytes received from the child PTY to the given file before any ANSI processing. |

---

## Daemon Options

Run vrl as a background daemon process.

| Flag | Default | Config Key | Description |
|------|---------|------------|-------------|
| `--daemon` | `false` | `daemon.enabled` | Fork into the background after initialization. Conflicts with `--display`, `--display-all`, and `--tabs`. |
| `--stdout-file <path>` | `vrl.out` | `daemon.stdout_file` | File to which the daemon's stdout is redirected. |
| `--stderr-file <path>` | `vrl.err` | `daemon.stderr_file` | File to which the daemon's stderr is redirected. |

**Example**

```bash
vrl --daemon -- python worker.py
```

---

## Exit Handler Options

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
# Send Ctrl+C on start and wait up to 30 seconds for graceful exit
vrl --send-keys "<C-c>" --exit-timeout 30 -- app
```

---

## Subcommands

Subcommands operate on *already running* vrl instances via UDS IPC.

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

Stop a vrl instance by PID. Auto-selects if only one instance is running.

```bash
vrl stop [pid]
```

| Argument | Description |
|----------|-------------|
| `pid` | PID of the instance to stop. Omit to auto-select the single running instance. |

---

### `spawn-in`

Dynamically create a new command in a running vrl instance.

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

Send keystrokes to a running command.

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

Generate shell completion scripts for vrl.

```bash
vrl completions bash > /etc/bash_completion.d/vrl
vrl completions zsh > ~/.zsh/completions/_vrl
vrl completions fish > ~/.config/fish/completions/vrl.fish
```

---

## Special Key Notation

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

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success. |
| `1` | General error. |
| `2` | Child process exited with an error. |
| `130` | vrl was interrupted by `SIGINT`. |

---

## See Also

- [`../configuration.md`](../configuration.md) — Full configuration file reference
- [`keybindings.md`](keybindings.md) — Interactive keyboard shortcuts
- [`../hooks.md`](../hooks.md) — Event hooks reference
