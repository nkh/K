# vrunner

A virtual terminal runner and process orchestrator with a web-first control plane.

**vrunner** executes commands inside virtual TTYs, exposes their output through a web API, and provides a built-in administrative interface for remote control. By default, it runs silently with no terminal output, making it ideal for background development servers, CI pipelines, or headless environments.

---

## Features

- **Silent by Default** — No terminal clutter unless you explicitly enable local display.
- **Virtual TTY Execution** — Run terminal-aware programs (`vim`, `htop`, `ncurses` apps) inside a pseudo-terminal.
- **Web API** — Start, kill, send keystrokes, and retrieve terminal contents via HTTP(S).
- **Admin Dashboard** — Built-in web interface at `/admin`.
- **TLS Encryption** — Optional HTTPS with auto-generated self-signed certificates.
- **Bearer Token Auth** — Optional authentication for remote access scenarios.
- **Daemon Mode** — Run as a background process with no terminal attachment.
- **Instance Management** — Run multiple vrunner instances on different ports; list and shut them down by PID.
- **Command Logging** — Audit all incoming API commands to the terminal or a file.
- **Local VTTY Display** — Mirror a command's virtual terminal to your local screen on demand.
- **Declarative Config** — Configure via YAML, TOML, or JSON, overridden by CLI flags.
- **Extensible Handles** — Route extra file descriptors to the VTTY or to managed log files.
- **Certificate Pool** — Manage named certificates for per-command access control. Each running application can be bound to a specific certificate.
- **WebSocket Streaming** — Real-time VTTY output and log streaming via WebSocket, with bidirectional keyboard input support and an incremental diff protocol that transmits only changed cells.
- **Exit Handlers** — Run commands on child exit (clean or error), with configurable grace period before force-kill.
- **Environment Variables** — Three-layer env var control: config defaults, per-command overrides, and --no-env flag.
- **Configuration Profiles** — Named configuration presets for different environments (dev, prod, CI).
- **CLI Spawn** — Dynamically spawn commands on running instances with `vrunner spawn`.
- **Freeze/Thaw** — Suspend and resume running commands via SIGSTOP/SIGCONT.
- **Snapshot & Diff** — Store named snapshots of VTTY buffers and compute cell-level diffs against the current buffer for debugging and testing.
- **Kill by PID** — Stop individual commands by their OS process ID from the CLI or API, without stopping the entire vrunner instance.
- **VTTY Resize** — Resize a running command's virtual terminal from the CLI (`vrunner resize`) or the API. The child process receives SIGWINCH so terminal-aware apps adjust their layout.
- **Per-Command Size** — Spawn commands with a custom terminal size via `vrunner spawn --rows N --cols M` or the API `rows`/`cols` fields, independent of the server's default VTTY dimensions.
- **Enhanced Instance Listing** — `vrunner list` queries running instances to show their active commands, arguments, PIDs, and certificate bindings in a unified table.

---

## Installation

### From Source (Cargo)

```bash
git clone https://github.com/yourusername/vrunner.git
cd vrunner
cargo build --release
```

The binary will be at `target/release/vrunner`.

### Prebuilt Binaries

Download from the [Releases](https://github.com/yourusername/vrunner/releases) page.

---

## Quick Start

### Run silently in idle mode (waits for web commands)

```bash
vrunner
```

### Run a command silently in the background

```bash
vrunner --daemon -- htop
```

### Run with local VTTY display

```bash
vrunner --display -- vim file.txt
```

### Run on a custom port

```bash
vrunner --port 9090 -- npm run dev
```

### Generate a certificate for an application

```bash
vrunner cert generate my-app
```

### Run with TLS (HTTPS)

```bash
vrunner --tls -- python -m http.server 8000
```

### Accept remote connections securely

```bash
vrunner --remote --tls -- some-server
```

This binds to `0.0.0.0`, enables TLS, and requires a bearer token for authentication.

### List all running instances

```bash
vrunner list
```

### Stop an instance

```bash
vrunner stop 12345
```

---

## CLI Reference

```
vrunner [OPTIONS] [-- <COMMAND> [ARGS...]]
```

### General Options

| Flag | Short | Argument | Description |
|------|-------|----------|-------------|
| `--config` | `-c` | `<FILE>` | Path to configuration file |
| `--help` | `-h` | — | Print help |
| `--version` | `-V` | — | Print version |

### Server Options

| Flag | Argument | Default | Description |
|------|----------|---------|-------------|
| `--bind` | `<ADDR>` | `127.0.0.1` | Server bind address |
| `--port` | `<PORT>` | `9090` | Server TCP port |
| `--remote` | — | — | Bind to `0.0.0.0` and enable auth |

### Security Options

| Flag | Argument | Default | Description |
|------|----------|---------|-------------|
| `--auth` | — | — | Require bearer token authentication |
| `--token-file` | `<FILE>` | `~/.config/vrunner/token` | Path to bearer token file |

### TLS Options

| Flag | Argument | Default | Description |
|------|----------|---------|-------------|
| `--tls` | — | — | Enable HTTPS with self-signed certificates |
| `--cert-file` | `<FILE>` | auto | Path to PEM certificate |
| `--key-file` | `<FILE>` | auto | Path to PEM private key |

### Certificate Options

| Flag | Argument | Description |
|------|----------|-------------|
| `--certificate` | `NAME:CERT:KEY` | Define a named certificate for the pool (repeatable) |

### VTTY Options

| Flag | Argument | Default | Description |
|------|----------|---------|-------------|
| `--vtty-rows` | `<N>` | `24` | Terminal height in rows |
| `--vtty-cols` | `<N>` | `80` | Terminal width in columns |
| `--term` | `<TERM>` | `xterm-256color` | TERM value for child processes |
| `--scrollback` | `<N>` | `5000` | Scrollback buffer size (lines) |
| `--truecolor` / `--no-truecolor` | — | on | Enable/disable 24-bit color |
| `--mouse` / `--no-mouse` | — | off | Enable/disable mouse forwarding |

### Display Options

| Flag | Argument | Default | Description |
|------|----------|---------|-------------|
| `--display` / `--no-display` | — | off | Show/hide VTTY on local terminal |
| `--display-all` | — | off | Keep displaying after initial command exits (switch to next) |
| `--refresh-ms` | `<MS>` | `100` | Display refresh interval (ms) |

### Logging Options

| Flag | Argument | Default | Description |
|------|----------|---------|-------------|
| `--log` | — | — | Log API commands to terminal |
| `--log-file` | `<FILE>` | — | Log API commands to file |
| `--log-pty-raw` | `<FILE>` | — | Log raw PTY output to file for debugging |

### Daemon Options

| Flag | Argument | Default | Description |
|------|----------|---------|-------------|
| `--daemon` | — | — | Run as a background daemon (Unix) |
| `--stdout-file` | `<FILE>` | `/tmp/vrunner.out` | Daemon stdout redirect |
| `--stderr-file` | `<FILE>` | `/tmp/vrunner.err` | Daemon stderr redirect |

### Exit Handler Options

| Flag | Argument | Default | Description |
|------|----------|---------|-------------|
| `--on-exit` | `<CMD>` | — | Run command on clean exit (exit code 0) |
| `--on-error` | `<CMD>` | — | Run command on error exit (non-zero) |
| `--exit-timeout` | `<SECS>` | `10` | Grace period before SIGKILL |

### Environment Variable Options

| Flag | Argument | Description |
|------|----------|-------------|
| `--env` | `<KEY=VALUE>` | Set environment variable (repeatable) |
| `--no-env` | — | Ignore config file environment variables |

### Profile Options

| Flag | Argument | Description |
|------|----------|-------------|
| `--profile` | `<NAME>` | Apply a named configuration profile from config |
| `--target` | `<PID>` | Target a specific vrunner instance by PID (for spawn/freeze/thaw/resize) |

### Interactive Options

| Flag | Argument | Default | Description |
|------|----------|---------|-------------|
| `--tabs` | — | off | Show tab bar for command switching in display |

### Subcommands

| Command | Argument | Description |
|---------|----------|-------------|
| `list` | — | List all running vrunner instances |
| `stop` | `<PID>` | Shut down a vrunner instance by PID |
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

### The `--` Separator

The `--` separator is required to distinguish vrunner's own flags from the command you want to run. Everything after `--` is passed as the child command and its arguments:

```bash
vrunner --port 3000 --display -- python -m http.server 8000
```

Here `--port 3000` and `--display` are vrunner options, while `python -m http.server 8000` is the command to execute.

---

## Configuration

`vrunner` reads configuration from multiple sources, resolved in order of precedence. YAML, TOML, and JSON formats are supported (detected by file extension):

| Priority | Source | Path |
|----------|--------|------|
| Highest | CLI flags | Command-line arguments |
| High | Explicit config | `-c <FILE>` (any supported format) |
| Medium | Local config | `./vrunner.yaml` or `./vrunner.toml` |
| Low | Global config | `~/.config/vrunner/config.yaml` or `~/.config/vrunner/config.toml` |
| Lowest | Built-in defaults | Compiled into the binary |

### Example `vrunner.yaml`

```yaml
server:
  bind: "127.0.0.1"
  port: 9090

security:
  require_auth: false
  token_file: "~/.config/vrunner/token"

tls:
  enabled: false
  cert_file: null
  key_file: null

vtty:
  rows: 24
  cols: 80
  term: "xterm-256color"
  scrollback: 5000
  truecolor: true
  mouse: false

display:
  enabled: false
  refresh_ms: 100

command_log:
  enabled: false
  file: null

daemon:
  enabled: false
  stdout_file: "/tmp/vrunner.out"
  stderr_file: "/tmp/vrunner.err"

handles: []

web:
  update_mode: "push"
  dirty_check_ms: 200
  default_poll_ms: 500

interactive:
  tabs: false

default_exit:
  exit:
    on_exit: null
    on_error: null
    timeout_secs: 10

environment:
  variables: {}

profiles: {}

For the complete configuration reference with all entries, types, defaults, and CLI flag mappings, see [docs/configuration.md](docs/configuration.md). For environment variables and profiles, see [docs/usage.md](docs/usage.md).

---

## Security

### Default: Localhost (No Auth)

By default, vrunner binds to `127.0.0.1` and requires no authentication. This is safe for local use because any process that can reach localhost already has shell access to the machine.

### Remote Access

To accept connections from other machines, use the `--remote` flag (or set `server.bind: "0.0.0.0"` and `security.require_auth: true` in the config). This enables bearer token authentication automatically.

When auth is enabled, a 256-bit random token is generated and saved to `~/.config/vrunner/token` on first use. Include it in API requests:

```bash
curl -H "Authorization: Bearer <token>" http://<host>:8080/api/commands
```

### TLS Encryption

Enable HTTPS with `--tls`. Self-signed certificates are automatically generated on first use and saved to `~/.config/vrunner/`. Distribute the certificate to authorized clients:

```bash
# Server (first run — certs auto-generated)
vrunner --remote --tls -- some-command

# Client
curl --cacert ~/.config/vrunner/cert.pem \
     -H "Authorization: Bearer <token>" \
     https://<host>:8080/api/commands
```

For custom certificates (e.g., from Let's Encrypt), use `--cert-file` and `--key-file`.

See [docs/configuration.md](docs/configuration.md) for full TLS documentation.

---

## Certificate Management

vrunner supports a pool of named certificates that can be bound to individual commands. When a command is bound to a certificate, only clients presenting that certificate's derived bearer token can interact with its endpoints.

See [docs/certificates.md](docs/certificates.md) for the full certificate management guide.

---

## Instance Management

Multiple vrunner instances can run simultaneously on different ports. Each instance registers itself in a shared registry so you can discover and manage them.

### List all instances

```bash
vrunner list
```

Queries all running vrunner instances and their active commands. Each instance is contacted via HTTP to retrieve its current command list, showing command names, arguments, PIDs, and certificate bindings:

```
PID        PORT     BIND                 DAEMON     DISPLAY    COMMAND
12345      8080     127.0.0.1            yes        no         (idle) -> htop [80x24]
12346      9090     127.0.0.1            yes        no         (no commands)
12347      3000     127.0.0.1            no         no         (idle) -> cargo test ["--release"] [my-app]
```

### Stop an instance or command

```bash
# Stop a specific command by its OS PID (queries all instances)
vrunner stop 12345

# If no command with that PID is found, falls back to stopping the entire instance
vrunner stop 12346
```

This first attempts to kill a command with the given PID on any running instance via the API. If no matching command is found, it falls back to sending an HTTP shutdown request to stop the entire instance. If the instance is unresponsive, you can also use `kill 12346` directly.

---

## Web API

### Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `GET /api/commands` | GET | List all running commands |
| `POST /api/commands` | POST | Start a new command |
| `POST /api/commands/:id/keys` | POST | Send keystrokes to a command |
| `POST /api/commands/:id/kill` | POST | Kill a running command |
| `POST /api/commands/:id/freeze` | POST | Freeze (suspend) a running command |
| `POST /api/commands/:id/thaw` | POST | Thaw (resume) a frozen command |
| `GET /api/commands/:id/vtty` | GET | Get full VTTY contents (raw ANSI) |
| `GET /api/commands/:id/vtty/html` | GET | Get VTTY contents as rendered HTML |
| `GET /api/commands/:id/vtty/partial` | GET | Get partial VTTY contents (paginated) |
| `POST /api/commands/:id/resize` | POST | Resize a command's virtual terminal (sends SIGWINCH) |
| `GET /api/commands/:id/handles` | GET | List output handles for a command |
| `POST /api/commands/:id/handles` | POST | Add an output handle to a command |
| `GET /api/info` | GET | Get instance info (counts, auth status) |
| `GET /api/log` | GET | Get command log entries (search, pagination) |
| `POST /api/shutdown` | POST | Shut down the vrunner instance |
| `GET /api/certificates` | GET | List all certificates in the pool |
| `POST /api/commands/kill-pid/:pid` | POST | Kill a command by its OS PID |
| `GET /api/commands/:id/ws` | GET (WS) | WebSocket: real-time VTTY streaming (incremental diff protocol) |
| `POST /api/commands/:id/snapshot` | POST | Store a named snapshot of the VTTY buffer |
| `GET /api/commands/:id/snapshots` | GET | List all snapshots for a command |
| `POST /api/commands/:id/diff` | POST | Compute diff against a stored snapshot |
| `DELETE /api/commands/:id/snapshots/:name` | DELETE | Delete a stored snapshot |
| `GET /api/ws/logs` | GET (WS) | WebSocket: real-time log streaming |

### Response Format

All responses use a standard JSON envelope:

```json
{
  "status": "ok",
  "data": { ... },
  "error": null
}
```

Error responses:
```json
{
  "status": "error",
  "data": null,
  "error": "Description of the error"
}
```

### Examples

```bash
# Start a command
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "htop", "args": []}'

# Start a command with a custom terminal size
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "vim", "args": ["file.txt"], "rows": 40, "cols": 120}'

# Resize a running command (sends SIGWINCH)
curl -X POST http://localhost:8080/api/commands/<id>/resize \
  -H "Content-Type: application/json" \
  -d '{"rows": 50, "cols": 160}'

# Send keystrokes
curl -X POST http://localhost:8080/api/commands/<id>/keys \
  -H "Content-Type: application/json" \
  -d '{"keys": "q"}'

# Get VTTY output
curl http://localhost:8080/api/commands/<id>/vtty

# Graceful shutdown
curl -X POST http://localhost:8080/api/shutdown
```

---

## Admin Interface

Open `http://localhost:8080/admin` in your browser for a built-in web dashboard.

When TLS is enabled, use `https://localhost:8080/admin` instead.

The admin interface features a real-time VTTY viewer powered by the incremental diff WebSocket protocol, a Pause/Run button to freeze and thaw commands, 1-second polling for HTTP fallback, and auto-selection of the first available command.

---

## Documentation

| Document | Description |
|----------|-------------|
| [docs/usage.md](docs/usage.md) | Practical user guide with curl and web UI examples |
| [docs/configuration.md](docs/configuration.md) | Complete configuration reference |
| [docs/certificates.md](docs/certificates.md) | Certificate management guide |
| [docs/architecture.md](docs/architecture.md) | Technical architecture details |
| [docs/requirements.md](docs/requirements.md) | Formal requirements specification |
| [man/vrunner.1](man/vrunner.1) | Unix manpage |
| [man/vrunnerctrl.1](man/vrunnerctrl.1) | CLI controller and API reference manpage |

Install the manpage:
```bash
cp man/vrunner.1 /usr/local/share/man/man1/
man vrunner
```

---

## Architecture

See [docs/architecture.md](docs/architecture.md) for technical details.
See [docs/requirements.md](docs/requirements.md) for formal requirements.

---

## License

MIT OR Apache-2.0
