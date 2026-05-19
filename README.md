# vrunner

A virtual terminal runner and process orchestrator with a web-first control plane.

**vrunner** executes commands inside virtual TTYs, exposes their output through a web API, and provides a built-in administrative interface for remote control. By default, it runs silently with no terminal output, making it ideal for background development servers, CI pipelines, or headless environments.

---

## Features

- **Silent by Default** — No terminal clutter unless you explicitly enable local display.
- **Virtual TTY Execution** — Run terminal-aware programs (`vim`, `htop`, `ncurses` apps) inside a pseudo-terminal.
- **Web API** — Start, kill, send keystrokes, and retrieve terminal contents via HTTP.
- **Admin Dashboard** — Built-in web interface at `/admin`.
- **Daemon Mode** — Run as a background process with no terminal attachment.
- **Instance Management** — Run multiple vrunner instances on different ports; list and shut them down by PID.
- **Command Logging** — Audit all incoming API commands to the terminal or a file.
- **Local VTTY Display** — Mirror a command's virtual terminal to your local screen on demand.
- **Declarative Config** — Configure via YAML, overridden by CLI flags.
- **Extensible Handles** — Route extra file descriptors to the VTTY or to managed log files.

---

## Installation

### From Source (Cargo)

```bash
git clone https://github.com/yourusername/vrunner.git
cd vrunner
cargo build --release
```

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

Options:
  -c, --config <FILE>     Path to configuration file
      --bind <ADDR>         Server bind address [default: 127.0.0.1]
      --port <PORT>         Server port [default: 8080]
      --daemon              Run as a background daemon
      --display             Show VTTY on local terminal
      --no-display          Disable local terminal display
      --log                 Log API commands to terminal
      --log-file <FILE>     Log API commands to file
      --vtty-rows <N>       VTTY height
      --vtty-cols <N>       VTTY width
  -h, --help                Print help
  -V, --version             Print version

Commands:
  list                      List running vrunner instances
  stop <PID>                Shut down a vrunner instance by PID
```

The `--` separator is required to distinguish vrunner's own flags from the command you want to run. For example:

```bash
vrunner --port 3000 --display -- python -m http.server 8000
```

Here `--port 3000` and `--display` are vrunner options, while `python -m http.server 8000` is the command to execute.

---

## Configuration

`vrunner` reads configuration from:

1. `~/.config/vrunner/config.yaml` (global)
2. `./vrunner.yaml` (local, overrides global)
3. CLI flags (override both)

### Example `vrunner.yaml`

```yaml
server:
  bind: "127.0.0.1"
  port: 8080

vtty:
  rows: 24
  cols: 80
  term: "xterm-256color"
  scrollback: 5000
  truecolor: true

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
```

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

| Endpoint | Method | Description |
|----------|--------|-------------|
| `GET /api/commands` | List all commands | |
| `POST /api/commands` | Start a new command | |
| `POST /api/commands/:id/keys` | Send keystrokes | |
| `POST /api/commands/:id/kill` | Kill a command | |
| `GET /api/commands/:id/vtty` | Get full VTTY contents | |
| `GET /api/commands/:id/vtty/partial` | Get partial VTTY contents | |
| `POST /api/shutdown` | Shut down the vrunner instance | |

---

## Admin Interface

Open `http://localhost:8080/admin` in your browser.

---

## Architecture

See [docs/architecture.md](docs/architecture.md) for technical details.
See [docs/requirements.md](docs/requirements.md) for formal requirements.

---

## License

MIT OR Apache-2.0
