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
| `--port` | `<PORT>` | `8080` | Server TCP port |
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
| `--refresh-ms` | `<MS>` | `100` | Display refresh interval (ms) |

### Logging Options

| Flag | Argument | Default | Description |
|------|----------|---------|-------------|
| `--log` | — | — | Log API commands to terminal |
| `--log-file` | `<FILE>` | — | Log API commands to file |

### Daemon Options

| Flag | Argument | Default | Description |
|------|----------|---------|-------------|
| `--daemon` | — | — | Run as a background daemon (Unix) |
| `--stdout-file` | `<FILE>` | `/tmp/vrunner.out` | Daemon stdout redirect |
| `--stderr-file` | `<FILE>` | `/tmp/vrunner.err` | Daemon stderr redirect |

### Subcommands

| Command | Argument | Description |
|---------|----------|-------------|
| `list` | — | List all running vrunner instances |
| `stop` | `<PID>` | Shut down a vrunner instance by PID |
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
  port: 8080

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
```

For the complete configuration reference with all entries, types, defaults, and CLI flag mappings, see [docs/configuration.md](docs/configuration.md).

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

Output:
```
PID        PORT     BIND                 DAEMON     DISPLAY    COMMAND
12345      8080     127.0.0.1            no         yes        vim
12346      9090     127.0.0.1            yes        no         (idle)
```

### Stop an instance gracefully

```bash
vrunner stop 12345
```

This sends an HTTP shutdown request to the instance. If the instance is unresponsive, you can fall back to `kill 12345`.

---

## Web API

### Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `GET /api/commands` | GET | List all running commands |
| `POST /api/commands` | POST | Start a new command |
| `POST /api/commands/:id/keys` | POST | Send keystrokes to a command |
| `POST /api/commands/:id/kill` | POST | Kill a running command |
| `GET /api/commands/:id/vtty` | GET | Get full VTTY contents (raw ANSI) |
| `GET /api/commands/:id/vtty/html` | GET | Get VTTY contents as rendered HTML |
| `GET /api/commands/:id/vtty/partial` | GET | Get partial VTTY contents (paginated) |
| `POST /api/commands/:id/resize` | POST | Resize a command's virtual terminal |
| `GET /api/commands/:id/handles` | GET | List output handles for a command |
| `POST /api/commands/:id/handles` | POST | Add an output handle to a command |
| `GET /api/info` | GET | Get instance info (counts, auth status) |
| `GET /api/log` | GET | Get command log entries (search, pagination) |
| `POST /api/shutdown` | POST | Shut down the vrunner instance |
| `GET /api/certificates` | GET | List all certificates in the pool |

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
