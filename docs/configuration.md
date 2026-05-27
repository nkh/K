# vrunner Configuration Reference

Complete reference for all configuration entries, CLI flags, and their relationships.

---

## Table of Contents

1. [Configuration File Locations](#configuration-file-locations)
2. [Precedence Order](#precedence-order)
3. [Configuration Sections](#configuration-sections)
   - [server](#server)
   - [security](#security)
   - [tls](#tls)
   - [certificates](#certificates)
   - [vtty](#vtty)
   - [display](#display)
   - [interactive](#interactive)
   - [default_exit](#default_exit)
   - [command_log](#command_log)
   - [daemon](#daemon)
   - [web](#web)
   - [handles](#handles)
4. [CLI Flag Reference](#cli-flag-reference)
5. [Config-to-CLI Mapping](#config-to-cli-mapping)
6. [Security Model](#security-model)
7. [TLS Setup](#tls-setup)
8. [Full Example](#full-example)

---

## Configuration File Locations

vrunner loads configuration from multiple YAML sources. All locations are optional — if no config file exists, built-in defaults are used.

| Location | Scope | Path |
|----------|-------|------|
| Global | System-wide defaults | `~/.config/vrunner/config.yaml` |
| Local | Project-specific overrides | `./vrunner.yaml` (current working directory) |
| Explicit | User-specified file | Any path via `-c <FILE>` or `--config <FILE>` |

## Precedence Order

Configuration values are resolved in the following order, where **later sources override earlier ones**:

```
Built-in defaults → Global config → Local config → CLI flags
```

CLI flags always take the highest precedence. Boolean flags like `--auth` or `--tls` set the corresponding config value to `true`, overriding whatever was set in any config file. Complementary flag pairs (e.g., `--truecolor` / `--no-truecolor`) allow explicit override in either direction.

---

## Configuration Sections

### `server`

Controls the HTTP server binding and port.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `bind` | `string` | `"127.0.0.1"` | `--bind <ADDR>` | Network interface to bind to. `"127.0.0.1"` restricts access to localhost only (safe default). Set to `"0.0.0.0"` to listen on all interfaces and accept remote connections. |
| `port` | `u16` | `9090` | `--port <PORT>` | TCP port for the HTTP server. |

**Example:**
```yaml
server:
  bind: "127.0.0.1"
  port: 9090
```

### `security`

Controls bearer token authentication and CORS policy for API requests.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `require_auth` | `bool` | `false` | `--auth` | When `false` (default), no authentication is required for API requests. This is safe for localhost since the sender already has machine access. When `true`, every API request must include a valid `Authorization: Bearer <token>` header. |
| `token_file` | `string` | `~/.config/vrunner/token` | `--token-file <FILE>` | Path to a file containing the bearer token. If this file does not exist when auth is required, a cryptographically random 256-bit token (64 hex characters) is generated and saved to this path. The file is created with restrictive permissions (`0600`) so only the owner can read it. |
| `cors.policy` | `string` | `"any"` | — | CORS policy for cross-origin requests. `"any"` allows all origins (default). `"none"` blocks all cross-origin requests. A comma-separated list of origins (e.g., `"https://app.example.com,https://admin.example.com"`) allows only those specific origins. Each origin must include the scheme (`http` or `https`). Config-file-only; no CLI flag. |

**Example:**
```yaml
security:
  require_auth: false
  token_file: "~/.config/vrunner/token"
  cors:
    policy: "any"
```

**CORS policy examples:**
```yaml
# Allow all origins (default, backward compatible)
security:
  cors:
    policy: "any"

# Block all cross-origin requests
security:
  cors:
    policy: "none"

# Allow specific origins only
security:
  cors:
    policy: "https://dashboard.example.com,https://ci.example.com"
```

### `tls`

Controls TLS (HTTPS) encryption for the server.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `--tls` | When `false` (default), the server uses plain HTTP. When `true`, the server uses HTTPS with TLS encryption. Self-signed certificates are automatically generated on first use if no custom certificate paths are provided. |
| `cert_file` | `string?` | `null` | `--cert-file <FILE>` | Path to a PEM-encoded X.509 certificate file. If `null` and TLS is enabled, the default path `~/.config/vrunner/cert.pem` is used. If neither exists, a new self-signed certificate is generated. |
| `key_file` | `string?` | `null` | `--key-file <FILE>` | Path to a PEM-encoded private key file. If `null` and TLS is enabled, the default path `~/.config/vrunner/key.pem` is used. If neither exists, a new key pair is generated. |

**Example:**
```yaml
tls:
  enabled: false
  cert_file: null
  key_file: null
```

### `certificates`

Manages a pool of named certificates for per-command access control. Each certificate in the pool is a named cert/key pair with a derived bearer token (SHA-256 of the certificate PEM). Commands can be bound to a specific certificate, restricting access to only clients presenting that certificate's token.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `directory` | `string?` | `null` | — | Base directory for storing generated certificate files. When set, `vrunner cert generate` will save certs as subdirectories under this path. If `null`, generated certificates are stored in a temporary location. |
| `entries` | `array<CertificateEntryConfig>` | `[]` | `--certificate NAME:CERT:KEY` | Pre-defined certificate entries to load into the pool at startup. Each entry specifies a name, certificate file, and key file. |

**CertificateEntryConfig fields:**

| Key | Type | Description |
|-----|------|-------------|
| `name` | `string` | Unique identifier for the certificate in the pool. |
| `cert_file` | `string` | Path to the PEM-encoded certificate file. |
| `key_file` | `string` | Path to the PEM-encoded private key file. |

**Example:**
```yaml
certificates:
  directory: "~/.config/vrunner/certs"
  entries:
    - name: "my-app"
      cert_file: "my-app/cert.pem"
      key_file: "my-app/key.pem"
    - name: "staging"
      cert_file: "/etc/ssl/staging/cert.pem"
      key_file: "/etc/ssl/staging/key.pem"
```

### `vtty`

Controls the virtual terminal (VTTY) properties reported to child processes.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `rows` | `u16` | `24` | `--vtty-rows <N>` | Number of rows (lines) in the virtual terminal. Corresponds to the `LINES` environment variable reported to the child process. |
| `cols` | `u16` | `80` | `--vtty-cols <N>` | Number of columns (characters per line) in the virtual terminal. Corresponds to the `COLUMNS` environment variable reported to the child process. |
| `term` | `string` | `"xterm-256color"` | `--term <TERM>` | The `TERM` environment variable value reported to child processes. Determines the terminal capabilities that programs expect. Common values: `"xterm-256color"`, `"xterm"`, `"vt100"`, `"screen"`. |
| `scrollback` | `usize` | `5000` | `--scrollback <N>` | Maximum number of scrollback lines retained in the buffer. When the terminal scrolls past this limit, the oldest lines are discarded. Higher values use more memory. |
| `truecolor` | `bool` | `true` | `--truecolor` / `--no-truecolor` | Enable 24-bit truecolor (16.7 million colors) support. When enabled, programs can use RGB color values for precise color control. Disable for terminals that only support 256-color or 16-color palettes. |
| `mouse` | `bool` | `false` | `--mouse` / `--no-mouse` | Enable mouse event forwarding to child processes. When enabled, mouse movements, clicks, and scroll events are passed through to the child process, enabling mouse-driven interfaces in programs like `vim`, `htop`, or `mc`. |

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
| `enabled` | `bool` | `false` | `--display` / `--no-display` | When `true`, the VTTY output is mirrored to the local terminal screen in real time. This is useful for interactive programs when you want to see the output directly. When `false` (default), vrunner operates silently with no terminal output. Automatically disabled in daemon mode. |
| `display_all` | `bool` | `false` | `--display-all` | When `true`, the local display stays active after the initial CLI command exits, switching to show the next available command (monitor mode). When `false` (default), the display is dismissed when the direct CLI command finishes but the server continues running. Note: when all commands have exited (no retained commands remain), vrunner exits even in `display_all` mode. |
| `refresh_ms` | `u64` | `100` | `--refresh-ms <MS>` | Refresh interval in milliseconds when local display is enabled. Lower values provide smoother rendering at the cost of higher CPU usage. Recommended range: 50–200ms. |

**Example:**
```yaml
display:
  enabled: false
  display_all: false
  refresh_ms: 100
```

### `command_log`

Controls logging of API commands received by the server.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `--log` | When `true`, all incoming API commands (spawn, kill, send_keys, etc.) are logged. Each log entry includes a timestamp, the command type, and relevant parameters. |
| `file` | `string?` | `null` | `--log-file <FILE>` | Path to a log file for command entries. When set, logs are written to this file in addition to the terminal (if enabled). When `null`, logs are only written to the terminal. |
| `pty_raw_log` | `string?` | `null` | `--log-pty-raw <FILE>` | Path to log raw PTY output for ANSI debugging. When set, all bytes written to the child PTY are recorded to this file. |

**Example:**
```yaml
command_log:
  enabled: false
  file: null
```

### `daemon`

Controls Unix daemon (background process) behavior.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `--daemon` | When `true`, vrunner detaches from the controlling terminal and runs as a background daemon using a double-fork pattern. Stdin is closed, stdout and stderr are redirected to files, and the process becomes a session leader. Only available on Unix-like systems. |
| `stdout_file` | `string` | `"/tmp/vrunner.out"` | `--stdout-file <FILE>` | Path to redirect stdout to when running as a daemon. |
| `stderr_file` | `string` | `"/tmp/vrunner.err"` | `--stderr-file <FILE>` | Path to redirect stderr to when running as a daemon. |

**Example:**
```yaml
daemon:
  enabled: false
  stdout_file: "/tmp/vrunner.out"
  stderr_file: "/tmp/vrunner.err"
```

### `interactive`

Controls the interactive terminal display behavior when `--display` is enabled, including keyboard shortcuts for navigating commands, toggling overlays, and spawning new commands.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `tabs` | `bool` | `false` | `--tabs` | When `true`, shows a tab bar listing all running commands at the top of the interactive display. When `false` (default), only the active command is shown. This is similar to `mprocs`-style display but the tab bar is optional. |
| `keybindings.next_command` | `string?` | `"ctrl+right"` | — | Key sequence to switch to the next running command. Only active when `display.display_all` is enabled and multiple commands are running. Set to `null` to disable. |
| `keybindings.prev_command` | `string?` | `"ctrl+left"` | — | Key sequence to switch to the previous running command. Wraps around to the last command. Only active when `display.display_all` is enabled. Set to `null` to disable. |
| `keybindings.toggle_log` | `string?` | `"ctrl+l"` | — | Key sequence to show or hide the command log overlay. When the log is visible, recent log entries are displayed over the VTTY output. Press the same key again to dismiss. Set to `null` to disable. |
| `keybindings.spawn_command` | `string?` | `"f12"` | — | Key sequence to open a spawn prompt. Temporarily exits raw mode so you can type a command to spawn. Press Enter to confirm or Ctrl+C to cancel. Set to `null` to disable. |
| `keybindings.show_help` | `string?` | `"ctrl+h"` | — | Key sequence to show the help overlay. Displays all configured keybindings with their descriptions. Press any key to dismiss. Set to `null` to disable. |
| `keybindings.quit` | `string?` | `null` | — | Key sequence to quit the interactive display loop. When not set, use `Ctrl+\\` to quit. Set to `"esc"` for Escape-key quit. |
| `keybindings.kill_command` | `string?` | `null` | — | Key sequence to kill the active command (SIGTERM). Disabled by default — uncomment in config to enable. |
| `keybindings.toggle_pause` | `string?` | `null` | — | Key sequence to pause/resume (SIGSTOP/SIGCONT) the active command. Disabled by default — uncomment in config to enable. |

**Hardcoded shortcuts** (always active, cannot be remapped):

| Shortcut | Action |
|----------|--------|
| `Ctrl+\\` | Quit the interactive display (always works, even if `keybindings.quit` is unset) |
| `Ctrl+C` | Shut down the entire vrunner instance (when the display is dismissed) |

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

#### Supported Key Names

Key names use a human-readable format. The following formats are recognized:

**Control keys:** `ctrl+a` through `ctrl+z`, `ctrl+@`, `ctrl+[`, `ctrl+\`, `ctrl+]`, `ctrl+^`, `ctrl+_`, `ctrl+?`

**Control + arrow keys:** `ctrl+left`, `ctrl+right`, `ctrl+up`, `ctrl+down`

**Alt/Meta keys:** `alt+a` through `alt+z`, `alt+0` through `alt+9`, and any other single character

**Shift + arrow keys:** `shift+left`, `shift+right`, `shift+up`, `shift+down`, `shift+tab`

**Function keys:** `f1` through `f12`

**Special keys:** `enter` (or `return`), `tab`, `backspace`, `delete`, `insert`, `home`, `end`, `pageup` (or `page_up`), `pagedown` (or `page_down`), `up`, `down`, `left`, `right`, `esc` (or `escape`), `space`

**Single characters:** Any printable ASCII character (e.g., `a`, `1`, `@`)

**Raw escape sequences** (backward compatible): You can still use Rust-style escape notation like `"\x1b[1;5C"` for Ctrl+Right, but the human-readable names are strongly preferred for clarity and maintainability.

### `default_exit`

Default exit configuration applied to all commands unless overridden per-command via the spawn API. Controls what happens when a command exits and how long vrunner waits before force-killing.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `exit.on_exit` | `string?` | `null` | `--on-exit <CMD>` | Command to run when the child exits cleanly (exit code 0). The string is split on whitespace into a binary and arguments. Set to `null` to disable. |
| `exit.on_error` | `string?` | `null` | `--on-error <CMD>` | Command to run when the child exits with a non-zero code. Same parsing rules as `on_exit`. Set to `null` to disable. |
| `exit.timeout_secs` | `u64` | `10` | `--exit-timeout <SECS>` | Maximum seconds to wait for a child process to exit after SIGTERM before sending SIGKILL. Applies when kill is called or when the server shuts down. |
| `exit.retain_on_exit` | `bool` | `false` | — | When `true`, the command's VTTY buffer is kept in memory after the child exits. The command appears in the tab bar and web UI with an "exited" status. Default: `false` (commands are removed on exit). This can also be set per-command via the CLI `--retain-on-exit` flag or the API `retain_on_exit` field. |
| `exit.snapshot_on_exit` | `string?` | `null` | — | When set to a file path, the VTTY buffer (including scrollback) is saved as plain text to that file when the child exits. This is a per-command option set via the CLI `--snapshot-on-exit` flag or the API `snapshot_on_exit` field; it cannot be set in the config file's `default_exit` section. |

**Example:**
```yaml
default_exit:
  exit:
    on_exit: "notify-send Done"
    on_error: "notify-send Error"
    timeout_secs: 15
```

Exit handler commands are spawned as detached (fire-and-forget) processes. vrunner does not wait for them to complete. When a command is spawned via `POST /api/commands` with `on_exit` or `on_error` fields, those per-command values override these defaults entirely.

### `web`

Controls how the web UI discovers terminal buffer changes.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `update_mode` | string | `"push"` | How the web UI detects changes: `"push"` (server notifies via WebSocket) or `"poll"` (client polls) |
| `dirty_check_ms` | number | `200` | Server-side dirty-check interval in ms (push mode) |
| `default_poll_ms` | number | `500` | Client-side polling interval in ms (poll mode) |

**Note:** These fields are config-file-only; there are no corresponding CLI flags.

**Example:**
```yaml
web:
  update_mode: "push"
  dirty_check_ms: 200
  default_poll_ms: 500
```

### `handles`

Defines additional output sinks attached to every spawned command. Each handle routes command output to a named destination.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `name` | `string` | — | — | Identifier for the handle, used in API requests to reference the sink. Must be unique within the handles list. |
| `sink` | `string` | — | — | Sink type. Supported values: `"file"` (writes to a file path), `"vtty"` (merges into the VTTY stream), `"null"` (discards output). |
| `path` | `string?` | `null` | — | File path for `"file"` sinks. Supports `{id}` (command UUID) and `{name}` (command name) placeholders that are expanded at spawn time. Required for file sinks, ignored for others. |

**Note:** Handles are configured only via the config file. There are no CLI flags for individual handle definitions.

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

### `environment`

Default environment variables passed to all spawned commands unless overridden per-command or disabled with `--no-env`.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `variables` | `map<string, string>` | `{}` | `--env <KEY=VALUE>` | Key-value pairs of environment variables to pass to every spawned command. Per-command `--env` flags or API `env` field override these values. The `TERM` variable is always set from `vtty.term` regardless of this section. |

**Example:**
```yaml
environment:
  variables:
    RUST_LOG: "info"
    DATABASE_URL: "postgres://localhost/mydb"
    NODE_ENV: "development"
```

### `profiles`

Named configuration presets that can be selected via `--profile <name>`. Only fields present in the profile override the base configuration; CLI flags always take final precedence.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| (dynamic) | `map<string, ConfigSection>` | `{}` | `--profile <name>` | Map of profile names to partial configuration overrides. Each value uses the same schema as the top-level config but only the specified fields override. |

**Example:**
```yaml
profiles:
  development:
    vtty:
      rows: 40
      cols: 120
    display:
      enabled: true
      refresh_ms: 50
    environment:
      variables:
        RUST_LOG: "debug"

  production:
    server:
      bind: "0.0.0.0"
      port: 443
    security:
      require_auth: true
    tls:
      enabled: true
```

---

## CLI Flag Reference

```
vrunner [OPTIONS] [-- <COMMAND> [ARGS...]]
```

### General Options

| Flag | Short | Argument | Description |
|------|-------|----------|-------------|
| `--config` | `-c` | `<FILE>` | Path to a YAML configuration file. Overrides global and local configs. |
| `--help` | `-h` | — | Print help information. |
| `--version` | `-V` | — | Print version information. |

### Server Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--bind` | `<ADDR>` | `server.bind` | Server bind address. Default: `127.0.0.1`. |
| `--port` | `<PORT>` | `server.port` | Server TCP port. Default: `9090`. |
| `--remote` | — | `server.bind` + `security.require_auth` | Convenience flag that sets bind to `0.0.0.0` and enables authentication. Use this to accept remote connections securely. |

### Security Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--auth` | — | `security.require_auth` | Require bearer token authentication for all API requests. |
| `--token-file` | `<FILE>` | `security.token_file` | Path to the bearer token file. |

### TLS Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--tls` | — | `tls.enabled` | Enable TLS (HTTPS) with self-signed certificates. |
| `--cert-file` | `<FILE>` | `tls.cert_file` | Path to a custom PEM certificate file. |
| `--key-file` | `<FILE>` | `tls.key_file` | Path to a custom PEM private key file. |

### VTTY Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--vtty-rows` | `<N>` | `vtty.rows` | Virtual terminal height in rows. |
| `--vtty-cols` | `<N>` | `vtty.cols` | Virtual terminal width in columns. |
| `--term` | `<TERM>` | `vtty.term` | TERM value reported to child processes. |
| `--scrollback` | `<N>` | `vtty.scrollback` | Scrollback buffer size in lines. |
| `--truecolor` | — | `vtty.truecolor` → `true` | Enable 24-bit truecolor. |
| `--no-truecolor` | — | `vtty.truecolor` → `false` | Disable 24-bit truecolor. |
| `--mouse` | — | `vtty.mouse` → `true` | Enable mouse event forwarding. |
| `--no-mouse` | — | `vtty.mouse` → `false` | Disable mouse event forwarding. |

### Display Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--display` | — | `display.enabled` → `true` | Show VTTY output on the local terminal. |
| `--display-all` | — | `display.display_all` → `true` | Keep displaying after the initial CLI command exits, switching to the next available command. |
| `--no-display` | — | `display.enabled` → `false` | Disable local terminal display. |
| `--refresh-ms` | `<MS>` | `display.refresh_ms` | Display refresh interval in milliseconds. |

### Logging Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--log` | — | `command_log.enabled` → `true` | Log API commands to the terminal. |
| `--log-file` | `<FILE>` | `command_log.file` + `command_log.enabled` → `true` | Log API commands to a file (also enables logging). |
| `--log-pty-raw` | `<FILE>` | `command_log.pty_raw_log` | Log raw PTY output from child processes to a file for debugging. See the PTY Raw Logging section below. |

### Daemon Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--daemon` | — | `daemon.enabled` → `true` | Run as a background daemon. Implicitly disables local display. |
| `--stdout-file` | `<FILE>` | `daemon.stdout_file` | Redirect daemon stdout to file. |
| `--stderr-file` | `<FILE>` | `daemon.stderr_file` | Redirect daemon stderr to file. |

### Exit Handler Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--on-exit` | `<CMD>` | `default_exit.exit.on_exit` | Run this command when a child exits cleanly (exit code 0) |
| `--on-error` | `<CMD>` | `default_exit.exit.on_error` | Run this command when a child exits with an error (non-zero) |
| `--exit-timeout` | `<SECS>` | `default_exit.exit.timeout_secs` | Seconds to wait for graceful exit before SIGKILL (default: 10) |
| `--retain-on-exit` | — | — | Keep the VTTY buffer in memory after the CLI command exits. **Per-command only** — applies to the command specified on the CLI, not to future API-spawned commands. |
| `--snapshot-on-exit` | `<FILE>` | — | Save the VTTY buffer to a file as plain text when the CLI command exits. **Per-command only.** |
| `--send-keys` | `<KEYS>` | — | Send keystrokes to the command after it starts. Supports the same notation as the API's `send_keys` endpoint (e.g., `"ls<Enter>"`, `"<C-c>quit<Enter>"`). |

### Interactive Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--tabs` | — | `interactive.tabs` → `true` | Show tab bar for command switching in interactive display |

### Subcommands

| Command | Argument | Description |
|---------|----------|-------------|
| `list` | — | List all running vrunner instances |
| `stop` | `[PID]` | Gracefully shut down a vrunner instance. When no PID is given and exactly one instance is running, it is stopped automatically. When multiple instances are running, a list is shown. |
| `spawn` | `<cmd> [args...]` | Spawn a command on a running instance |
| `freeze` | `<pid>` | Freeze (suspend) a running command via SIGSTOP |
| `thaw` | `<pid>` | Thaw (resume) a frozen command via SIGCONT |
| `list-vrunner` | — | List running instances (compact format) |
| `list-commands` | — | List commands on all running instances |
| `stop-command` | `<pid>` | Stop a specific command by PID |
| `resize` | `<target> --rows N --cols M` | Resize a running command's VTTY (sends SIGWINCH) |
| `cert generate` | `<name>` | Generate a named certificate |
| `cert list` | — | List all certificates in the pool |
| `cert show` | `<name>` | Show certificate details and token |
| `cert remove` | `<name>` | Remove a certificate from the pool |

---

## Config-to-CLI Mapping

Every configuration file entry has a corresponding CLI flag. This table summarizes the complete mapping:

| Config Section | Config Key | CLI Flag(s) |
|---------------|------------|-------------|
| `server` | `bind` | `--bind <ADDR>` |
| `server` | `port` | `--port <PORT>` |
| `security` | `require_auth` | `--auth` |
| `security` | `token_file` | `--token-file <FILE>` |
| `security` | `cors.policy` | Config only — no CLI equivalent |
| `tls` | `enabled` | `--tls` |
| `tls` | `cert_file` | `--cert-file <FILE>` |
| `tls` | `key_file` | `--key-file <FILE>` |
| `vtty` | `rows` | `--vtty-rows <N>` |
| `vtty` | `cols` | `--vtty-cols <N>` |
| `vtty` | `term` | `--term <TERM>` |
| `vtty` | `scrollback` | `--scrollback <N>` |
| `vtty` | `truecolor` | `--truecolor` / `--no-truecolor` |
| `vtty` | `mouse` | `--mouse` / `--no-mouse` |
| `display` | `enabled` | `--display` / `--no-display` |
| `display` | `display_all` | `--display-all` |
| `display` | `refresh_ms` | `--refresh-ms <MS>` |
| `command_log` | `enabled` | `--log` |
| `command_log` | `file` | `--log-file <FILE>` |
| `daemon` | `enabled` | `--daemon` |
| `daemon` | `stdout_file` | `--stdout-file <FILE>` |
| `daemon` | `stderr_file` | `--stderr-file <FILE>` |
| `handles` | *(array)* | Config only — no CLI equivalent |
| `certificates` | `directory` | Config only — no CLI equivalent |
| `certificates` | `entries` | `--certificate NAME:CERT:KEY` |
| `web` | *(all fields)* | Config only — no CLI equivalent |
| `interactive` | `tabs` | `--tabs` |
| `command_log` | `pty_raw_log` | `--log-pty-raw <FILE>` |
| `default_exit.exit` | `on_exit` | `--on-exit <CMD>` |
| `default_exit.exit` | `on_error` | `--on-error <CMD>` |
| `default_exit.exit` | `timeout_secs` | `--exit-timeout <SECS>` |

**CLI-only flags** (no config key, by design):

| Flag | Effect |
|------|--------|
| `--remote` | Sets `server.bind = "0.0.0.0"` and `security.require_auth = true`. A convenience shorthand for enabling secure remote access. |
| `--config <FILE>` | Specifies which config file to load. |
| `--retain-on-exit` | Keep the VTTY buffer in memory after the CLI command exits (per-command). |
| `--snapshot-on-exit <FILE>` | Save VTTY buffer to file on exit (per-command). |
| `--send-keys <KEYS>` | Send initial keystrokes to the command after it starts. |

---

## Security Model

vrunner follows a **secure-by-default, opt-in-for-network** security model.

### Default: No Auth (Localhost)

When bound to `127.0.0.1` (the default), no authentication is required. This is safe because any process that can reach localhost already has shell access to the machine. Adding a bearer token in this scenario provides no meaningful security improvement.

### Enabling Auth for Remote Access

When you expose vrunner to the network (by setting `bind` to `0.0.0.0`), you should enable authentication. There are two ways:

1. **Explicit:** Use `--auth` or set `security.require_auth: true` in the config.
2. **Automatic:** Use `--remote`, which sets both `bind: 0.0.0.0` and `require_auth: true`.

When auth is enabled, every API request must include:

```
Authorization: Bearer <token>
```

The token is loaded from `~/.config/vrunner/token` (or a custom path). If the file does not exist, a 256-bit random token is generated automatically and saved with restrictive file permissions (`0600`).

### Accessing an Auth-Protected Server

```bash
# Using curl
curl -H "Authorization: Bearer <token>" http://localhost:8080/api/commands

# Using the vrunner admin interface
# The admin page will prompt for the token.
```

---

## TLS Setup

### Automatic Self-Signed Certificates

When TLS is enabled with `--tls` or `tls.enabled: true`, vrunner automatically handles certificate management:

1. If custom `cert_file` and `key_file` paths are provided and both files exist, they are loaded.
2. Otherwise, defaults are used (`~/.config/vrunner/cert.pem` and `~/.config/vrunner/key.pem`).
3. If the default files do not exist, a new self-signed X.509 certificate is generated with:
   - **Subject:** CN=vrunner, O=vrunner
   - **SANs:** DNS:localhost, IP:127.0.0.1, IP:::1
   - **Key usage:** Digital Signature, Key Encipherment
   - **Extended key usage:** Server Authentication
   - **Validity:** 2025-01-01 to 2030-01-01
   - **Key size:** 256-bit EC (via `rcgen` defaults)
4. The private key file is created with `0600` permissions (owner read/write only).

### Distributing Certificates to Clients

Since the certificate is self-signed (not signed by a public CA), clients must explicitly trust it. Copy the `cert.pem` file to each authorized client machine and use it with:

```bash
# curl
curl --cacert /path/to/cert.pem https://localhost:8080/api/commands

# With authentication
curl --cacert /path/to/cert.pem \
     -H "Authorization: Bearer <token>" \
     https://localhost:8080/api/commands

# Python requests
import requests
r = requests.get("https://localhost:8080/api/commands", verify="/path/to/cert.pem")

# wget
wget --ca-certificate=/path/to/cert.pem https://localhost:8080/api/commands
```

### Using Custom Certificates

You can provide your own certificates (e.g., from Let's Encrypt or an internal CA):

```bash
vrunner --tls --cert-file /etc/ssl/certs/vrunner.crt --key-file /etc/ssl/private/vrunner.key -- some-command
```

Or in the config file:

```yaml
tls:
  enabled: true
  cert_file: "/etc/ssl/certs/vrunner.crt"
  key_file: "/etc/ssl/private/vrunner.key"
```

---

## Full Example

A complete `vrunner.yaml` with all sections documented:

```yaml
# vrunner configuration — all entries with their defaults shown

server:
  bind: "127.0.0.1"       # localhost only (safe default)
  port: 9090               # TCP port

security:
  require_auth: false      # no auth for localhost
  token_file: "~/.config/vrunner/token"
  cors:
    policy: "any"          # allow all origins (default)

tls:
  enabled: false           # plain HTTP by default
  cert_file: null          # auto-generate if null and TLS enabled
  key_file: null           # auto-generate if null and TLS enabled

vtty:
  rows: 24                 # terminal height
  cols: 80                 # terminal width
  term: "xterm-256color"   # TERM value for child processes
  scrollback: 5000         # scrollback buffer size
  truecolor: true          # 24-bit color support
  mouse: false             # no mouse forwarding by default

display:
  enabled: false           # silent by default
  display_all: false       # dismiss display when CLI command exits
  refresh_ms: 100          # 10 FPS when display is on

command_log:
  enabled: false           # no command logging by default
  file: null               # null = terminal only, set path for file logging
  pty_raw_log: null         # set path to log raw PTY output for ANSI debugging

daemon:
  enabled: false           # run in foreground by default
  stdout_file: "/tmp/vrunner.out"
  stderr_file: "/tmp/vrunner.err"

# Additional output sinks for spawned commands
handles: []
  # - name: "debug"       # sink identifier
  #   sink: "file"         # "file", "vtty", or "null"
  #   path: "./logs/{name}-{id}.log"  # with placeholder expansion

# Web admin panel options
web:
  update_mode: "push"       # "push" (server notifies via WebSocket) or "poll" (client polls)
  dirty_check_ms: 200       # server-side dirty-check interval in ms (push mode)
  default_poll_ms: 500      # client-side polling interval in ms (poll mode)

# Interactive display options
interactive:
  tabs: false               # show tab bar when --display is active
  keybindings:
    next_command: "ctrl+right"   # switch to next command (display-all mode)
    prev_command: "ctrl+left"    # switch to previous command (display-all mode)
    toggle_log: "ctrl+l"         # show/hide command log overlay
    spawn_command: "f12"         # open prompt to spawn new command
    show_help: "ctrl+h"          # show keybinding help overlay
    # quit: "esc"                # (disabled by default; use Ctrl+\\ to quit)
    # kill_command: "ctrl+k"     # (disabled by default; kill active command with SIGTERM)
    # toggle_pause: "ctrl+p"     # (disabled by default; pause/resume with SIGSTOP/SIGCONT)

# Default exit configuration for all commands
default_exit:
  exit:
    on_exit: null             # command to run on clean exit (exit code 0)
    on_error: null            # command to run on error exit (non-zero)
    timeout_secs: 10          # grace period before SIGKILL
    retain_on_exit: false     # keep buffer after exit (per-command via --retain-on-exit)
    # snapshot_on_exit: null  # save buffer to file on exit (per-command via --snapshot-on-exit)
```
