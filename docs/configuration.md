# vrc Configuration Reference

Complete reference for all configuration entries, CLI flags, and their relationships.

---

## Table of Contents

1. [Configuration File Locations](#configuration-file-locations)
2. [Precedence Order](#precedence-order)
3. [Configuration Sections](#configuration-sections)
   - [vtty](#vtty)
   - [display](#display)
   - [interactive](#interactive)
   - [default_exit](#default_exit)
   - [command_log](#command_log)
   - [daemon](#daemon)
   - [hooks](#hooks)
   - [handles](#handles)
   - [templates](#templates)
   - [environment](#environment)
   - [profiles](#profiles)
4. [CLI Flag Reference](#cli-flag-reference)
5. [Config-to-CLI Mapping](#config-to-cli-mapping)
6. [Full Example](#full-example)

---

## Configuration File Locations

vrc loads configuration from multiple YAML sources. All locations are optional — if no config file exists, built-in defaults are used.

| Location | Scope | Path |
|----------|-------|------|
| Global | System-wide defaults | `~/.config/vrc/config.yaml` |
| Local | Project-specific overrides | `./vrc.yaml` (current working directory) |
| Explicit | User-specified file | Any path via `-c <FILE>` or `--config <FILE>` |

## Precedence Order

Configuration values are resolved in the following order, where **later sources override earlier ones**:

```
Built-in defaults → Global config → Local config → CLI flags
```

CLI flags always take the highest precedence.

---

## Configuration Sections

### `vtty`

Controls the virtual terminal (VTTY) properties reported to child processes.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `rows` | `u16` | `24` | `--vtty-rows <N>` | Number of rows in the virtual terminal. |
| `cols` | `u16` | `80` | `--vtty-cols <N>` | Number of columns in the virtual terminal. |
| `term` | `string` | `"xterm-256color"` | `--term <TERM>` | The `TERM` environment variable value reported to child processes. |
| `scrollback` | `usize` | `5000` | `--scrollback <N>` | Maximum number of scrollback lines retained in the buffer. |
| `truecolor` | `bool` | `true` | `--truecolor` / `--no-truecolor` | Enable 24-bit truecolor support. |
| `mouse` | `bool` | `false` | `--mouse` / `--no-mouse` | Enable mouse event forwarding to child processes. |

**Example:**
```yaml
vtty:
  rows: 24
  cols: 80
  term: "xterm-256color"
  scrollback: 5000
  truecolor: true
  mouse: false
```

### `display`

Controls local terminal display of the VTTY output.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `--display` / `--no-display` | Mirror VTTY output to the local terminal. |
| `display_all` | `bool` | `false` | `--display-all` *(deprecated)* | Stay active after the initial CLI command exits, switching to the next available command. Now implicitly set when `display.enabled = true` via `--display`. The `--display-all` flag is no longer needed. |
| `refresh_ms` | `u64` | `100` | `--refresh-ms <MS>` | Display refresh interval in milliseconds. |

**Example:**
```yaml
display:
  enabled: false
  display_all: false
  refresh_ms: 100
```

### `command_log`

Controls logging of commands received by the server.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `--log` | Log all incoming commands. |
| `file` | `string?` | `null` | `--log-file <FILE>` | Path to a log file. |
| `pty_raw_log` | `string?` | `null` | `--log-pty-raw <FILE>` | Path to log raw PTY output for ANSI debugging. |

**Example:**
```yaml
command_log:
  enabled: false
  file: null
```

### `hooks`

Global lifecycle event hooks. When a hook is set, vrc executes the specified shell command when the corresponding event occurs.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `on_spawn` | `string?` | `null` | Shell command to run when a new command is spawned. |
| `on_exit` | `string?` | `null` | Shell command to run when a command exits cleanly. |
| `on_error` | `string?` | `null` | Shell command to run when a command exits with a non-zero code. |
| `on_kill` | `string?` | `null` | Shell command to run when a command is killed. |

**Available placeholders:**

| Placeholder | Expanded to |
|------------|-------------|
| `{name}` | Command name |
| `{id}` | Command UUID |
| `{pid}` | OS process ID |
| `{exit_code}` | Exit code (only in `on_exit` and `on_error`) |

**Example:**

```yaml
hooks:
  on_spawn: "echo 'Started {name} (pid={pid})' >> /var/log/vrc.log"
  on_error: "notify-send 'vrc' '{name} exited with code {exit_code}'"
```

### `daemon`

Controls Unix daemon (background process) behavior.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `--daemon` | Run as a background daemon. |
| `stdout_file` | `string` | `"/tmp/vrc.out"` | `--stdout-file <FILE>` | Redirect stdout to file. |
| `stderr_file` | `string` | `"/tmp/vrc.err"` | `--stderr-file <FILE>` | Redirect stderr to file. |

**Example:**
```yaml
daemon:
  enabled: false
  stdout_file: "/tmp/vrc.out"
  stderr_file: "/tmp/vrc.err"
```

### `interactive`

Controls the interactive terminal display behavior when `--display` is enabled.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `tabs` | `bool` | `false` | `--tabs` | Show a tab bar listing all running commands. |
| `keybindings.next_command` | `string?` | `"ctrl+right"` | — | Key to switch to next command. |
| `keybindings.prev_command` | `string?` | `"ctrl+left"` | — | Key to switch to previous command. |
| `keybindings.toggle_log` | `string?` | `"ctrl+l"` | — | Key to show/hide command log overlay. |
| `keybindings.spawn_command` | `string?` | `"f12"` | — | Key to open spawn prompt. |
| `keybindings.show_help` | `string?` | `"ctrl+h"` | — | Key to show help overlay. |
| `keybindings.quit` | `string?` | `null` | — | Key to quit display. |
| `keybindings.kill_command` | `string?` | `null` | — | Key to kill active command. |
| `keybindings.toggle_pause` | `string?` | `null` | — | Key to pause/resume active command. |

**Example:**
```yaml
interactive:
  tabs: true
  keybindings:
    next_command: "ctrl+right"
    prev_command: "ctrl+left"
    toggle_log: "ctrl+l"
    spawn_command: "f12"
    show_help: "ctrl+h"
    quit: "esc"
```

### `default_exit`

Default exit configuration applied to all commands unless overridden per-command.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `exit.on_exit` | `string?` | `null` | `--on-exit <CMD>` | Command to run on clean exit (exit 0). |
| `exit.on_error` | `string?` | `null` | `--on-error <CMD>` | Command to run on non-zero exit. |
| `exit.timeout_secs` | `u64` | `10` | `--exit-timeout <SECS>` | Seconds before SIGKILL after SIGTERM. |
| `exit.retain_on_exit` | `bool` | `false` | — | Keep VTTY buffer in memory after exit. |
| `exit.snapshot_on_exit` | `string?` | `null` | — | Save VTTY buffer to file on exit. |

**Example:**
```yaml
default_exit:
  exit:
    on_exit: "notify-send Done"
    on_error: "notify-send Error"
    timeout_secs: 15
```

### `handles`

Defines additional output sinks attached to every spawned command.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `name` | `string` | — | Identifier for the handle. |
| `sink` | `string` | — | Sink type: `"file"`, `"vtty"`, or `"null"`. |
| `path` | `string?` | `null` | File path for `"file"` sinks. Supports `{id}` and `{name}` placeholders. |

**Example:**
```yaml
handles:
  - name: "debug"
    sink: "file"
    path: "./logs/debug-{id}.log"
  - name: "aux"
    sink: "vtty"
  - name: "discard"
    sink: "null"
```

### `templates`

Pre-defined command templates for quick spawning.

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `name` | `string` | Yes | Display name. |
| `cmd` | `string` | Yes | The command executable. |
| `args` | `string?` | No | Arguments for the command. |
| `env` | `array<string>?` | No | Extra environment variables. |
| `workdir` | `string?` | No | Working directory. |
| `rows` | `u16?` | No | VTTY rows. |
| `cols` | `u16?` | No | VTTY columns. |

**Example:**
```yaml
templates:
  - name: "Dev Server"
    cmd: "npm"
    args: "run dev"
    workdir: "/home/user/myproject"
    env:
      - "NODE_ENV=development"
    rows: 40
    cols: 120
```

### `environment`

Default environment variables passed to all spawned commands.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `variables` | `map<string, string>` | `{}` | `--env <KEY=VALUE>` | Key-value pairs. |

**Example:**
```yaml
environment:
  variables:
    RUST_LOG: "info"
    DATABASE_URL: "postgres://localhost/mydb"
```

### `profiles`

Named configuration presets selected via `--profile <name>`.

**Example:**
```yaml
profiles:
  development:
    vtty:
      rows: 40
      cols: 120
    display:
      enabled: true
    environment:
      variables:
        RUST_LOG: "debug"
```

---

## CLI Flag Reference

```
vrc [OPTIONS] [-- <COMMAND> [ARGS...]]
```

### General Options

| Flag | Short | Argument | Description |
|------|-------|----------|-------------|
| `--config` | `-c` | `<FILE>` | Path to a YAML configuration file. |
| `--help` | `-h` | — | Print help information. |
| `--version` | `-V` | — | Print version information. |

### VTTY Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--vtty-rows` | `<N>` | `vtty.rows` | Virtual terminal height in rows. |
| `--vtty-cols` | `<N>` | `vtty.cols` | Virtual terminal width in columns. |
| `--term` | `<TERM>` | `vtty.term` | TERM value for child processes. |
| `--scrollback` | `<N>` | `vtty.scrollback` | Scrollback buffer size in lines. |
| `--truecolor` | — | `vtty.truecolor` → `true` | Enable 24-bit truecolor. |
| `--no-truecolor` | — | `vtty.truecolor` → `false` | Disable 24-bit truecolor. |
| `--mouse` | — | `vtty.mouse` → `true` | Enable mouse event forwarding. |
| `--no-mouse` | — | `vtty.mouse` → `false` | Disable mouse event forwarding. |

### Display Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--display` | — | `display.enabled` → `true` | Show VTTY output on the local terminal. |
| `--display-all` | — | *(deprecated)* | Equivalent to `--display`. Kept for backward compatibility; `--display` now includes this behavior. |
| `--no-display` | — | `display.enabled` → `false` | Disable local terminal display. |
| `--refresh-ms` | `<MS>` | `display.refresh_ms` | Display refresh interval in milliseconds. |

### Logging Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--log` | — | `command_log.enabled` → `true` | Log commands to the terminal. |
| `--no-log` | — | `command_log.enabled` → `false` | Suppress activity logging. Overrides `--log`. |
| `--log-file` | `<FILE>` | `command_log.file` + `command_log.enabled` → `true` | Log commands to a file. |
| `--log-pty-raw` | `<FILE>` | `command_log.pty_raw_log` | Log raw PTY output. |

### Daemon Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--daemon` | — | `daemon.enabled` → `true` | Run as a background daemon. |
| `--stdout-file` | `<FILE>` | `daemon.stdout_file` | Redirect daemon stdout to file. |
| `--stderr-file` | `<FILE>` | `daemon.stderr_file` | Redirect daemon stderr to file. |

### Exit Handler Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--on-exit` | `<CMD>` | `default_exit.exit.on_exit` | Run on exit code 0. |
| `--on-error` | `<CMD>` | `default_exit.exit.on_error` | Run on non-zero exit. |
| `--exit-timeout` | `<SECS>` | `default_exit.exit.timeout_secs` | Seconds before SIGKILL (default: 10). |
| `--retain-on-exit` | — | — | Keep VTTY buffer after exit. Per-command only. |
| `--snapshot-on-exit` | `<FILE>` | — | Save VTTY buffer to file on exit. Per-command only. |
| `--send-keys` | `<KEYS>` | — | Send keystrokes after spawn. |
| `--env` | `<KEY=VALUE>` | — | Set environment variable for the command. |
| `--no-env` | — | — | Skip config-level env vars. |

### Interactive Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--tabs` | — | `interactive.tabs` → `true` | Show tab bar in display. |

### Subcommands

| Command | Argument | Description |
|---------|----------|-------------|
| `list` | `--target <PID>` | List all running vrc instances and their commands |
| `stop` | `[PID]` | Gracefully shut down a vrc instance |
| `spawn-in` | `<pid> -- <cmd> [args...]` | Spawn a command in a running instance |
| `keys` | `<pid> <keys>` | Send keystrokes to a command |
| `cat` | `[pid] [--color-always]` | Print VTTY buffer |
| `freeze` | `<pid>` | Freeze a command (SIGSTOP) |
| `thaw` | `<pid>` | Thaw a command (SIGCONT) |
| `resize` | `<target> --rows N --cols M` | Resize a command's VTTY |
| `config-check` | — | Validate configuration files |
| `completions` | `<shell>` | Generate shell completions |

---

## Config-to-CLI Mapping

| Config Section | Config Key | CLI Flag(s) |
|---------------|------------|-------------|
| `vtty` | `rows` | `--vtty-rows <N>` |
| `vtty` | `cols` | `--vtty-cols <N>` |
| `vtty` | `term` | `--term <TERM>` |
| `vtty` | `scrollback` | `--scrollback <N>` |
| `vtty` | `truecolor` | `--truecolor` / `--no-truecolor` |
| `vtty` | `mouse` | `--mouse` / `--no-mouse` |
| `display` | `enabled` | `--display` / `--no-display` |
| `display` | `display_all` | `--display-all` *(deprecated; use `--display`)* |
| `display` | `refresh_ms` | `--refresh-ms <MS>` |
| `command_log` | `enabled` | `--log` |
| `command_log` | `file` | `--log-file <FILE>` |
| `daemon` | `enabled` | `--daemon` |
| `daemon` | `stdout_file` | `--stdout-file <FILE>` |
| `daemon` | `stderr_file` | `--stderr-file <FILE>` |
| `interactive` | `tabs` | `--tabs` |
| `default_exit.exit` | `on_exit` | `--on-exit <CMD>` |
| `default_exit.exit` | `on_error` | `--on-error <CMD>` |
| `default_exit.exit` | `timeout_secs` | `--exit-timeout <SECS>` |
| `environment` | `variables` | `--env <KEY=VALUE>` |

---

## Full Example

A complete `vrc.yaml`:

```yaml
# vrc configuration — all entries with their defaults shown

vtty:
  rows: 24
  cols: 80
  term: "xterm-256color"
  scrollback: 5000
  truecolor: true
  mouse: false

display:
  enabled: false
  display_all: false
  refresh_ms: 100

command_log:
  enabled: false
  file: null

daemon:
  enabled: false
  stdout_file: "/tmp/vrc.out"
  stderr_file: "/tmp/vrc.err"

# Additional output sinks for spawned commands
handles: []

# Pre-defined command templates
templates: []

interactive:
  tabs: false
  keybindings:
    next_command: "ctrl+right"
    prev_command: "ctrl+left"
    toggle_log: "ctrl+l"
    spawn_command: "f12"
    show_help: "ctrl+h"
    quit: null
    kill_command: null
    toggle_pause: null

default_exit:
  exit:
    on_exit: null
    on_error: null
    timeout_secs: 10
    retain_on_exit: false
    snapshot_on_exit: null

environment:
  variables: {}

hooks:
  on_spawn: null
  on_exit: null
  on_error: null
  on_kill: null

profiles: {}
```
