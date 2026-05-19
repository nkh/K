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
   - [vtty](#vtty)
   - [display](#display)
   - [command_log](#command_log)
   - [daemon](#daemon)
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
| `port` | `u16` | `8080` | `--port <PORT>` | TCP port for the HTTP server. |

**Example:**
```yaml
server:
  bind: "127.0.0.1"
  port: 8080
```

### `security`

Controls bearer token authentication for API requests.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `require_auth` | `bool` | `false` | `--auth` | When `false` (default), no authentication is required for API requests. This is safe for localhost since the sender already has machine access. When `true`, every API request must include a valid `Authorization: Bearer <token>` header. |
| `token_file` | `string` | `~/.config/vrunner/token` | `--token-file <FILE>` | Path to a file containing the bearer token. If this file does not exist when auth is required, a cryptographically random 256-bit token (64 hex characters) is generated and saved to this path. The file is created with restrictive permissions (`0600`) so only the owner can read it. |

**Example:**
```yaml
security:
  require_auth: false
  token_file: "~/.config/vrunner/token"
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
| `refresh_ms` | `u64` | `100` | `--refresh-ms <MS>` | Refresh interval in milliseconds when local display is enabled. Lower values provide smoother rendering at the cost of higher CPU usage. Recommended range: 50–200ms. |

**Example:**
```yaml
display:
  enabled: false
  refresh_ms: 100
```

### `command_log`

Controls logging of API commands received by the server.

| Key | Type | Default | CLI Flag | Description |
|-----|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `--log` | When `true`, all incoming API commands (spawn, kill, send_keys, etc.) are logged. Each log entry includes a timestamp, the command type, and relevant parameters. |
| `file` | `string?` | `null` | `--log-file <FILE>` | Path to a log file for command entries. When set, logs are written to this file in addition to the terminal (if enabled). When `null`, logs are only written to the terminal. |

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
| `--port` | `<PORT>` | `server.port` | Server TCP port. Default: `8080`. |
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
| `--no-display` | — | `display.enabled` → `false` | Disable local terminal display. |
| `--refresh-ms` | `<MS>` | `display.refresh_ms` | Display refresh interval in milliseconds. |

### Logging Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--log` | — | `command_log.enabled` → `true` | Log API commands to the terminal. |
| `--log-file` | `<FILE>` | `command_log.file` + `command_log.enabled` → `true` | Log API commands to a file (also enables logging). |

### Daemon Options

| Flag | Argument | Config Key | Description |
|------|----------|------------|-------------|
| `--daemon` | — | `daemon.enabled` → `true` | Run as a background daemon. Implicitly disables local display. |
| `--stdout-file` | `<FILE>` | `daemon.stdout_file` | Redirect daemon stdout to file. |
| `--stderr-file` | `<FILE>` | `daemon.stderr_file` | Redirect daemon stderr to file. |

### Subcommands

| Command | Argument | Description |
|---------|----------|-------------|
| `list` | — | List all running vrunner instances. |
| `stop` | `<PID>` | Gracefully shut down a vrunner instance by PID. |

---

## Config-to-CLI Mapping

Every configuration file entry has a corresponding CLI flag. This table summarizes the complete mapping:

| Config Section | Config Key | CLI Flag(s) |
|---------------|------------|-------------|
| `server` | `bind` | `--bind <ADDR>` |
| `server` | `port` | `--port <PORT>` |
| `security` | `require_auth` | `--auth` |
| `security` | `token_file` | `--token-file <FILE>` |
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
| `display` | `refresh_ms` | `--refresh-ms <MS>` |
| `command_log` | `enabled` | `--log` |
| `command_log` | `file` | `--log-file <FILE>` |
| `daemon` | `enabled` | `--daemon` |
| `daemon` | `stdout_file` | `--stdout-file <FILE>` |
| `daemon` | `stderr_file` | `--stderr-file <FILE>` |
| `handles` | *(array)* | Config only — no CLI equivalent |

**CLI-only flags** (no config key, by design):

| Flag | Effect |
|------|--------|
| `--remote` | Sets `server.bind = "0.0.0.0"` and `security.require_auth = true`. A convenience shorthand for enabling secure remote access. |
| `--config <FILE>` | Specifies which config file to load. |

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
  port: 8080               # TCP port

security:
  require_auth: false      # no auth for localhost
  token_file: "~/.config/vrunner/token"

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
  refresh_ms: 100          # 10 FPS when display is on

command_log:
  enabled: false           # no command logging by default
  file: null               # null = terminal only, set path for file logging

daemon:
  enabled: false           # run in foreground by default
  stdout_file: "/tmp/vrunner.out"
  stderr_file: "/tmp/vrunner.err"

# Additional output sinks for spawned commands
handles: []
  # - name: "debug"       # sink identifier
  #   sink: "file"         # "file", "vtty", or "null"
  #   path: "./logs/{name}-{id}.log"  # with placeholder expansion
```
