# Frequently Asked Questions

## General

### What is vrunner?

vrunner is a virtual terminal runner and process orchestrator with a web-first control plane. It runs commands inside pseudo-terminals (PTYs), exposes them through a REST API and built-in web dashboard, and provides WebSocket-based real-time streaming. Unlike tools like tmux that are designed for terminal multiplexing, vrunner is designed for programmatic control and remote monitoring of terminal processes via HTTP.

### Who is vrunner for?

vrunner is for anyone who needs to run, monitor, or control terminal processes programmatically: web developers orchestrating frontend and backend services, DevOps engineers managing remote servers, CI/CD engineers running terminal-aware tests, system administrators managing headless machines, and pair programmers sharing terminal sessions through a browser.

### What language is vrunner written in?

vrunner is written in Rust and compiles to a single statically-linked binary with no runtime dependencies. This makes deployment trivial — just copy the binary to any Linux, macOS, or Windows machine.

### How is vrunner different from tmux or screen?

tmux and screen are terminal multiplexers designed for interactive use within a single terminal session. vrunner is a process orchestrator with an HTTP API and web dashboard designed for programmatic control. While both provide PTY support, vrunner adds a REST API (30+ endpoints), WebSocket streaming with an incremental diff protocol, TLS encryption, bearer token authentication, per-command certificate isolation, daemon mode, and a multi-instance registry. See the [comparison page](explanation/comparison.md) for a full feature matrix.

### How is vrunner different from gotty or wetty?

gotty and wetty expose a single terminal in a browser. vrunner manages multiple commands simultaneously with a full REST API for programmatic spawning, killing, resizing, and monitoring. vrunner also supports TLS, authentication, daemon mode, configuration profiles, and per-command access control — features that gotty and wetty lack.

### How is vrunner different from mprocs?

mprocs is a Go-based multi-process runner with a terminal UI, similar to vrunner's `--display` mode. However, mprocs has no web API, no remote access, no authentication, no TLS, no configuration profiles, and no per-command isolation. vrunner provides all of these, making it suitable for production use cases beyond local development.

### What are the system requirements?

vrunner requires Rust 1.75+ to build from source. The binary runs on Linux, macOS, and Windows. There are no runtime dependencies — the binary is statically linked and includes the embedded admin web UI. Disk space for the binary is approximately 5 MB.

### Is vrunner production-ready?

Yes. vrunner is designed for production use with built-in TLS encryption, bearer token authentication, daemon mode (double-fork), graceful shutdown with configurable timeouts, and a certificate pool for per-command access control. The incremental diff WebSocket protocol minimizes bandwidth for remote monitoring. See the [security model](explanation/security-model.md) for production hardening guidance.

### What license does vrunner use?

vrunner is dual-licensed under `GPL-3.0-or-later OR Artistic-2.0`.

---

## Installation and Setup

### How do I install vrunner?

The recommended method is building from source:

```bash
git clone https://github.com/nkh/K.git
cd K
cargo build --release
# Binary at target/release/vrunner
```

For a system-wide install:

```bash
cargo install --path .
```

### How do I install the man pages?

```bash
cp man/vrunner.1 /usr/local/share/man/man1/
cp man/vrunnerctrl.1 /usr/local/share/man/man1/
man vrunner
```

### How do I verify vrunner is working?

Start vrunner in idle mode and check the API:

```bash
vrunner &
curl http://127.0.0.1:9090/api/commands
# Expected: {"status":"ok","data":[],"error":null}
```

Open `http://127.0.0.1:9090/admin` in your browser to see the dashboard.

### How do I update vrunner?

Pull the latest source and rebuild:

```bash
git pull
cargo build --release
```

If installed system-wide: `cargo install --path .`

### Does vrunner work on Windows?

vrunner compiles on Windows but daemon mode and some POSIX-specific features (SIGSTOP/SIGCONT for freeze/thaw) are not available. The web API, dashboard, and VTTY functionality work on all platforms.

### Does vrunner work on macOS?

Yes. vrunner builds and runs on macOS including ARM (Apple Silicon) via Rosetta or native Rust. All features work on macOS.

### Can I run vrunner without Docker or any runtime?

Yes. vrunner is a single statically-linked binary with zero runtime dependencies. No Docker, Node.js, Go, or shared libraries are needed.

---

## Running Commands

### How do I run a command with vrunner?

Use the `--` separator to pass a command:

```bash
vrunner -- htop
vrunner --display -- vim notes.txt
vrunner --daemon -- my-long-running-script.sh
```

### How do I run multiple commands at once?

Start vrunner in idle mode, then spawn commands:

```bash
vrunner --port 9090 &
vrunner spawn htop
vrunner spawn -- python -m http.server 8000
vrunner spawn -- cargo test
```

### How do I spawn a command from the web UI?

Open the admin dashboard at `http://localhost:9090/admin`, enter a command in the spawn form, and click Spawn. The command appears in the sidebar with its name, PID, and status.

### How do I spawn a command via the API?

```bash
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "htop", "args": []}'
```

### How do I pass arguments to a command?

Arguments go after the command name, separated by `--`:

```bash
vrunner -- python -m http.server 8000
```

Via API, use the `args` array:

```json
{"cmd": "python", "args": ["-m", "http.server", "8000"]}
```

### Can I run vrunner without any command?

Yes. Start vrunner in idle mode:

```bash
vrunner
```

The HTTP server starts and waits for commands to be spawned via the API, web UI, or `vrunner spawn`.

### How do I send initial keystrokes to a command?

Use `--send-keys`:

```bash
vrunner --send-keys "ls<Enter>" -- bash
```

This sends `ls` followed by Enter after the command starts.

### How do I keep a command's output after it exits?

Use `--retain-on-exit`:

```bash
vrunner --retain-on-exit -- cargo test
```

The VTTY buffer stays in memory and the command remains visible in the tab bar and web UI with an `[EXITED]` status. Purge it later with `vrunner purge` or `DELETE /api/commands/:id`.

### How do I save output to a file when a command exits?

Use `--snapshot-on-exit`:

```bash
vrunner --snapshot-on-exit /tmp/test-output.txt -- cargo test
```

The VTTY buffer (including scrollback) is saved as plain text when the command finishes.

---

## Web UI and Dashboard

### How do I access the web dashboard?

Open `http://localhost:9090/admin` (or just `http://localhost:9090/`). If TLS is enabled, use `https://` instead.

### How do I navigate directly to a specific command?

Navigate to `http://localhost:9090/<command_name>`. For example, `/htop` opens the VTTY viewer for a command named `htop`. If multiple commands share the same name, a picker list is shown.

### How do I search in the terminal viewer?

Press `Ctrl+F` inside the terminal pane. A search bar appears. Type your query and press Enter to navigate between matches. Press Escape to close.

### How do I switch between commands in the sidebar?

Click a command in the sidebar list. The terminal pane updates to show that command's output. Use the search/filter box at the top of the sidebar to narrow the list.

### How do I resize the terminal in the web UI?

The terminal auto-resizes to fill the available space when you resize the browser window. You can also drag the divider between the sidebar and terminal pane.

### How do I export terminal output?

Right-click a command in the sidebar and select "Export output" to download the current terminal buffer as a `.txt` file.

### How do I send keyboard input?

Click anywhere on the terminal pane to capture focus (shown with a blue outline), then type normally. Keystrokes are forwarded to the child process in real time.

### What does the scrollback indicator mean?

When you scroll up from the live output, a yellow "SCROLLBACK" label appears in the bottom bar. A floating "Back to bottom" button appears — click it to return to the live output.

### How do I kill all running commands?

Click the "Kill All" button in the top bar. This sends SIGINT to every running command with the configured timeout.

### How do I freeze/thaw a command from the web UI?

Click the Pause/Run button in the top bar. This toggles between SIGSTOP (freeze) and SIGCONT (thaw) for the currently selected command.

### What keyboard shortcuts does the web UI support?

Press `?` to open the shortcuts help panel. The web UI also supports `Ctrl+F` for terminal search and standard browser keyboard shortcuts.

### Why do exited commands have a different background?

Commands that have exited (with `--retain-on-exit` enabled) are shown with a subtle red tint in the sidebar and a reduced opacity. This makes it easy to distinguish running from exited commands at a glance.

---

## API and WebSocket

### How do I authenticate API requests?

When auth is enabled (`--auth` or `--remote`), include a bearer token in the `Authorization` header:

```bash
curl -H "Authorization: Bearer $(cat ~/.config/vrunner/token)" \
  http://localhost:9090/api/commands
```

### Where is the bearer token stored?

By default at `~/.config/vrunner/token`. The path is configurable with `--token-file` or `security.token_file` in the config. The file has `0600` permissions (owner read/write only).

### How do I send special keys via the API?

Use escape sequences in the `keys` field:

```bash
# Ctrl+C
curl -X POST http://localhost:9090/api/commands/$ID/keys \
  -d '{"keys": "\x03"}'

# Arrow up
curl -X POST http://localhost:9090/api/commands/$ID/keys \
  -d '{"keys": "\x1b[A"}'
```

Common sequences: `\x03` (Ctrl+C), `\x04` (Ctrl+D), `\x1b` (Escape), `\r` (Enter), `\t` (Tab), `\x7f` (Backspace).

### How do I use the WebSocket endpoint?

```javascript
const ws = new WebSocket('ws://localhost:9090/api/commands/<id>/ws');
ws.onmessage = (e) => {
  const msg = JSON.parse(e.data);
  if (msg.type === 'vtty_diff') applyDiff(msg.data);
  if (msg.type === 'command_ended') ws.close();
};
ws.send(JSON.stringify({ type: 'keys', keys: 'ls\r' }));
```

With TLS: `wss://` instead of `ws://`. With auth: `?token=YOUR_TOKEN` query parameter.

### What is the incremental diff protocol?

Instead of sending full terminal HTML on every update, the server computes a cell-level diff (comparing character, RGB colors, and attributes) and sends only changed cells. This reduces bandwidth significantly for terminals with static content. See the [incremental diff explanation](explanation/incremental-diff.md) for details.

### How do I get raw ANSI output?

```bash
curl http://localhost:9090/api/commands/$ID/vtty
```

The response `content` field contains raw ANSI escape sequences.

### How do I get HTML-rendered output?

```bash
curl http://localhost:9090/api/commands/$ID/vtty/html
```

Returns pre-rendered HTML with inline styles, cursor position, and dimensions.

### How do I paginate through terminal output?

```bash
curl "http://localhost:9090/api/commands/$ID/vtty/partial?offset=0&limit=50"
```

Use `offset` and `limit` query parameters to page through large outputs.

---

## Configuration

### What config file formats does vrunner support?

YAML (`.yaml`), TOML (`.toml`), and JSON (`.json`). The format is auto-detected from the file extension.

### Where does vrunner look for config files?

In order of precedence: `~/.config/vrunner/config.yaml` (global) → `./vrunner.yaml` (local) → explicit path via `--config <FILE>`. CLI flags override all config files.

### How do I change the port?

Via config:

```yaml
server:
  port: 8080
```

Or via CLI: `vrunner --port 8080 -- my-command`. CLI flags always win.

### How do I enable authentication?

Via config:

```yaml
security:
  require_auth: true
```

Or via CLI: `vrunner --auth -- my-command`. Or use `--remote` which sets both `bind: 0.0.0.0` and `require_auth: true`.

### How do I use configuration profiles?

Define named presets in your config:

```yaml
profiles:
  development:
    vtty:
      rows: 40
      cols: 120
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

Select with `--profile`:

```bash
vrunner --profile development -- cargo run
vrunner --profile production -- ./my-server
```

### How do I set environment variables for commands?

Three layers: config file (`environment.variables`), CLI flags (`--env KEY=VALUE`), and per-command API field (`env`). API and CLI values override config values. Use `--no-env` to skip config-level env vars entirely. The `TERM` variable is always set from `vtty.term`.

### How do I customize keyboard shortcuts?

```yaml
interactive:
  keybindings:
    kill_command: "ctrl+k"
    toggle_pause: "ctrl+p"
    quit: "esc"
```

See the [keybindings reference](reference/keybindings.md) for all supported keys.

### Can I use TOML or JSON instead of YAML?

Yes. Name your file `vrunner.toml` or `vrunner.json` instead of `vrunner.yaml`. vrunner auto-detects the format from the extension.

---

## Security and TLS

### How do I enable TLS?

```bash
vrunner --tls -- my-command
```

Self-signed certificates are auto-generated in `~/.config/vrunner/`. For production, use custom certs:

```bash
vrunner --tls --cert-file /etc/ssl/certs/vrunner.crt \
  --key-file /etc/ssl/private/vrunner.key -- my-command
```

### How do I access a TLS-protected server from curl?

```bash
curl --cacert ~/.config/vrunner/cert.pem \
  -H "Authorization: Bearer $TOKEN" \
  https://server:9090/api/commands
```

### What is the quickest way to enable secure remote access?

```bash
vrunner --remote --tls -- my-command
```

This single command binds to `0.0.0.0`, generates a bearer token, and generates self-signed TLS certificates.

### How do certificates provide per-command access control?

Each certificate in the pool has a derived bearer token (SHA-256 of the cert PEM). When you bind a certificate to a command at spawn time, only clients presenting that certificate's token can interact with the command. See the [certificates guide](how-to-guides/certificates.md).

### How do I generate a certificate?

```bash
vrunner cert generate my-app
vrunner cert list
vrunner cert show my-app
vrunner cert remove my-app
```

### Is vrunner secure for production use?

vrunner follows a secure-by-default model: it binds to localhost only, requires no auth for local access, and only enables network features when explicitly requested. For production, enable `--remote --tls` with custom certificates, use a reverse proxy (nginx/Caddy), configure firewall rules, and use per-command certificates for multi-tenant isolation. See the [security model](explanation/security-model.md) for details.

### What CORS policy does vrunner use?

vrunner uses `tower-http` CORS middleware. By default, CORS is permissive for localhost. For production, configure the allowed origins in the server configuration to restrict which domains can access the API.

---

## Interactive Display / TUI

### How do I use the local terminal display?

Add `--display` to mirror VTTY output to your terminal:

```bash
vrunner --display -- htop
```

Add `--display-all` to stay in display mode after the command exits:

```bash
vrunner --display-all --tabs -- htop
```

### How do I switch between commands in the display?

Use `Ctrl+Left` / `Ctrl+Right` to navigate between commands. Enable tabs with `--tabs` to see a tab bar at the top of the display.

### How do I search in the terminal display?

Press `Ctrl+F` to open a search overlay. Type your query and press Enter to navigate between matches. Press Escape to close.

### How do I enable split-pane view?

Press `Ctrl+S` to toggle split-pane mode, showing two commands side by side. Use `Ctrl+Left`/`Ctrl+Right` to switch which pane is active.

### How do I enable mouse support?

Add `--mouse` to forward mouse events to the child process:

```bash
vrunner --display --mouse -- htop
```

Mouse-aware applications (htop, vim, mc) will receive mouse input as if running directly in the terminal.

### How do I quit the interactive display?

Press `Ctrl+\` to quit. You can also set a custom quit key in the config:

```yaml
interactive:
  keybindings:
    quit: "esc"
```

### How do I spawn a new command from within the display?

Press `F12` to open a spawn prompt. Type the command and press Enter. The new command appears in the tab bar and you can switch to it with `Ctrl+Right`.

---

## Daemon Mode

### How do I run vrunner as a daemon?

```bash
vrunner --daemon -- my-command
```

The process double-forks and detaches from the terminal. stdin is closed; stdout/stderr redirect to files (default: `/tmp/vrunner.out`, `/tmp/vrunner.err`).

### How do I redirect daemon output?

```bash
vrunner --daemon --stdout-file /var/log/vrunner/stdout \
  --stderr-file /var/log/vrunner/stderr -- my-command
```

### How do I find running vrunner instances?

```bash
vrunner list
```

This queries all registered instances and shows their PID, port, bind address, and commands.

### How do I stop a daemon?

```bash
vrunner stop <PID>
```

Or: `curl -X POST http://localhost:9090/api/shutdown`

### Can vrunner run as a systemd service?

Yes. Create a systemd unit file that runs `vrunner` with your config:

```ini
[Unit]
Description=vrunner
After=network.target

[Service]
ExecStart=/usr/local/bin/vrunner -c /etc/vrunner/vrunner.yaml
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

---

## Multi-Instance

### Can I run multiple vrunner instances?

Yes. Each instance runs on a different port:

```bash
vrunner --port 8080 -- daemon
vrunner --port 9090 -- daemon
```

### How do I spawn on a specific instance?

```bash
vrunner --target <PID> spawn -- npm run dev
```

### How do I list all instances?

```bash
vrunner list          # Human-readable
vrunner list-vrunner # Machine-readable (tab-separated)
```

### Can instances share configuration?

No. Each instance loads its own config file independently. However, you can use the same config file for multiple instances by specifying it with `--config`.

---

## Troubleshooting

### vrunner won't start. What should I check?

1. Check if port 9090 (or your configured port) is already in use: `lsof -i :9090`
2. Check if another vrunner instance is running: `vrunner list`
3. Check your config file for syntax errors: `vrunner config-check`
4. Check the logs if running as a daemon: `cat /tmp/vrunner.err`

### The web dashboard shows a blank terminal. Why?

This happens when no command is running. Spawn a command via the web UI or API first.

### I can't connect from a remote machine. Why?

By default, vrunner binds to `127.0.0.1` (localhost only). Use `--remote` or `--bind 0.0.0.0` to accept remote connections. Also ensure your firewall allows the port.

### My browser shows a certificate warning. What do I do?

vrunner uses self-signed certificates by default. Click "Advanced" then "Proceed" to accept. For production, use CA-signed certificates.

### The token file doesn't exist. What happened?

When auth is enabled and the token file doesn't exist, vrunner auto-generates a 256-bit random token and saves it. Check `~/.config/vrunner/token`.

### Commands aren't being retained after exit. Why?

`--retain-on-exit` is a per-command flag, not a global setting. You must specify it for each command you want to retain. The global config `default_exit.exit.retain_on_exit` is for API-spawned commands that don't specify their own value.

### The display exits immediately after my command finishes. Why?

By default, when the CLI command exits, the display closes. Use `--display-all` to stay in display mode and switch to other running commands. Use `--retain-on-exit` to keep exited commands visible.

### The web UI doesn't show real-time updates. Why?

The web UI uses WebSockets by default. If WebSocket connections fail, it falls back to 1-second HTTP polling. Check your browser's developer console for WebSocket errors. Ensure no reverse proxy is buffering WebSocket connections.

### How do I debug terminal output issues?

Use `--log-pty-raw` to capture raw PTY bytes:

```bash
vrunner --log-pty-raw /tmp/pty.log -- my-command
perl tools/ansi-replay /tmp/pty.log
```

### How do I clean up stale instances?

Stale pidfiles (from crashed instances) are auto-cleaned by `vrunner list`. If cleanup fails, manually remove files in `~/.local/share/vrunner/instances/`.

### Memory usage is high. What can I do?

- Reduce scrollback: `--scrollback 1000` (default is 5000)
- Purge retained exited commands: `vrunner purge` or web UI purge button
- Use smaller terminal sizes: `--vtty-rows 24 --vtty-cols 80`

### How do I report a bug?

Open an issue on [GitHub](https://github.com/nkh/K/issues) with:
1. vrunner version (`vrunner --version`)
2. Operating system
3. Steps to reproduce
4. Expected vs actual behavior
5. Relevant logs (`--log` or `--log-pty-raw` output)
