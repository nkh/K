# vrunner Hands-On Tutorial

A progressive, exercise-driven introduction to vrunner.
Each lesson builds on the previous one — follow them in order.

---

## Lesson 1: Your First Command

vrunner runs a command in a virtual terminal (PTY) and serves it over HTTP.
Start simple:

```bash
vrunner -- echo "Hello from vrunner"
```

The command runs, prints output, and exits. Not very exciting yet.
Let's keep it alive:

```bash
vrunner -- top
```

Now `top` is running inside vrunner. The server is up on port 9090.
Open another terminal:

```bash
curl -s http://127.0.0.1:9090/api/commands | python3 -m json.tool
```

You should see one command with its UUID, name, PID, and status.

**Exercise 1.1**: Run `vrunner -- sleep 60` in the background (`&`).
Use `curl` to list commands. Kill it with `Ctrl+C` or `vrunner stop`.

**Exercise 1.2**: What happens if you run `vrunner -- sleep 60 --sleep 60`?
Why? (Hint: check the `--` separator behavior in the README.)

---

## Lesson 2: The Web Interface

vrunner has a built-in web admin panel:

```bash
vrunner --display -- htop
```

Open your browser: **http://127.0.0.1:9090/** (or `/admin`)

You can also navigate directly to a command by name: **http://127.0.0.1:9090/htop**

The web UI shows:
- A sidebar with all running commands (with alive status and runtime)
- A terminal view with live output
- A spawn form to launch new commands
- A status bar at the bottom with the full command name

**Exercise 2.1**: Spawn a second command (`ls -la`) from the web UI spawn form.
Switch between the two commands using the sidebar.

**Exercise 2.2**: Try the status bar toggle. Hide it, then show it again.
(Your preference is saved in the browser's localStorage.)

**Exercise 2.3**: In the spawn form, click "Auto-fit" to match the terminal size
to your browser window, then spawn a command. What TERM value does the child see?

---

## Lesson 3: Web API Basics

The web UI is just a frontend for the REST API. Let's use it directly.

### Spawn a command

```bash
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "echo hello world"}'
```

Save the returned `id` — you'll need it.

### View terminal output

```bash
# Full HTML rendering
curl -s "http://127.0.0.1:9090/api/commands/<id>/vtty/html"

# Check if output changed since last poll
curl -s "http://127.0.0.1:9090/api/commands/<id>/vtty/changed"
```

### Send keystrokes

```bash
curl -X POST "http://127.0.0.1:9090/api/commands/<id>/keys" \
  -H "Content-Type: application/json" \
  -d '{"keys": "ls -la\n"}'
```

### Kill a command

```bash
curl -X POST "http://127.0.0.1:9090/api/commands/<id>/kill"
```

**Exercise 3.1**: Spawn `cat` (it waits for input), then send it text via the API.
Verify the output appears in the VTTY HTML endpoint.

**Exercise 3.2**: Use the `changed` endpoint to implement a simple poll loop
in a shell script that prints "output changed!" when new data arrives.

---

## Lesson 4: Configuration File

vrunner reads config from (in order of precedence):

1. `~/.config/vrunner/config.yaml` (global)
2. `./vrunner.yaml` (project-local)
3. `--config <FILE>` (explicit)

Copy the example config:

```bash
cp examples/vrunner.example.yaml ./vrunner.yaml
```

### Change the port

```yaml
server:
  bind: "127.0.0.1"
  port: 8080
```

### Enable authentication

```yaml
security:
  require_auth: true
```

Restart vrunner. Now every API request needs a bearer token:

```bash
TOKEN=$(cat ~/.config/vrunner/token)
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:9090/api/commands
```

**Exercise 4.1**: Create a config file that binds to port 7777 with auth enabled.
Verify you can't access the API without the token.

**Exercise 4.2**: Use `--port 9999` on the command line to override the config's port 7777.
Which port does vrunner actually use? (CLI > config.)

---

## Lesson 5: Configuration Profiles

Profiles let you define named presets for different environments.

```yaml
profiles:
  entries:
    dev:
      server:
        port: 9090
      vtty:
        rows: 40
        cols: 120
      environment:
        variables:
          RUST_LOG: "debug"
    prod:
      server:
        bind: "0.0.0.0"
        port: 80
      security:
        require_auth: true
      environment:
        variables:
          RUST_LOG: "warn"
```

Select a profile:

```bash
vrunner --profile dev -- cargo run
vrunner --profile prod -- ./my-server
```

**Exercise 5.1**: Create a "small" profile with a 20x60 terminal and a "wide" profile
with 50x200. Run the same command under both and observe the difference via the web UI.

---

## Lesson 6: Local Terminal Display

For a tmux/mprocs-like experience, use `--display`:

```bash
vrunner --display --display-all --tabs -- htop
```

- `--display`: Show terminal output in your current terminal
- `--display-all`: Stay running after the command exits (monitor mode)
- `--tabs`: Show a tab bar listing all commands at the top

When `--display-all` is active and your initial command exits, vrunner
switches to the next available command (if any were spawned via the API).

### Interactive keybindings

| Key | Action |
|-----|--------|
| `Ctrl+H` | Show help overlay |
| `Ctrl+Left` | Switch to previous command |
| `Ctrl+Right` | Switch to next command |
| `Ctrl+L` | Toggle command log overlay |
| `F12` | Open spawn prompt |
| `Ctrl+\` | Quit display |

Customize in config:

```yaml
interactive:
  tabs: true
  keybindings:
    kill_command: "ctrl+k"
    toggle_pause: "ctrl+z"
```

**Exercise 6.1**: Run `vrunner --display --display-all --tabs -- sleep 100`.
While it's running, use `vrunner spawn` in another terminal to add `htop`.
Use `Ctrl+Right` to switch between them in the display.

**Exercise 6.2**: Enable `kill_command` and `toggle_pause` keybindings in your config.
Test them: kill a command with `Ctrl+K`, then freeze/thaw with `Ctrl+Z`.

---

## Lesson 7: Environment Variables

### Via CLI

```bash
vrunner --env RUST_LOG=debug --env DATABASE_URL=postgres://localhost/db -- ./my-app
```

### Via config

```yaml
environment:
  variables:
    RUST_LOG: "info"
    DATABASE_URL: "postgres://localhost/db"
```

### Per-command (API)

```bash
curl -X POST http://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "cmd": "env",
    "env": {"MY_VAR": "from_api", "RUST_LOG": "override"}
  }'
```

### Isolate from parent environment

```bash
vrunner --no-env -- ./my-app
```

This skips all config-level env vars. Only `--env` CLI flags and per-command
API env vars are passed through. `TERM` is always set.

**Exercise 7.1**: Set `RUST_LOG=debug` in config, then spawn a command with
`RUST_LOG=error` via the API. Verify the API value wins.

---

## Lesson 8: Command Lifecycle

### Exit handlers

Run a command when a child exits:

```bash
vrunner --on-exit "notify-send Done" -- on-success-script.sh
vrunner --on-error "notify-send FAILED" -- flaky-test.sh
```

Via config:

```yaml
default_exit:
  exit:
    on_exit: "notify-send 'Command finished'"
    on_error: "notify-send 'Command failed'"
    timeout_secs: 10
```

### Freeze and thaw

Suspend a command without killing it:

```bash
vrunner freeze <PID>
vrunner thaw <PID>
```

### Timeout

vrunner sends `SIGTERM`, waits `timeout_secs` (default 10), then `SIGKILL`:

```bash
vrunner --exit-timeout 5 -- ./my-server
```

**Exercise 8.1**: Run `vrunner --on-exit "echo CALLBACK RAN" -- sleep 1`.
Check the vrunner log output. Does the callback run?

**Exercise 8.2**: Run a command, freeze it, observe via the web UI (the VTTY
stops updating), then thaw it.

---

## Lesson 9: Snapshots and Diffs

Take a snapshot of the current terminal state:

```bash
curl -X POST "http://127.0.0.1:9090/api/commands/<id>/snapshot" \
  -d '{"name": "before-test"}'
```

Run your test, then diff:

```bash
curl -X POST "http://127.0.0.1:9090/api/commands/<id>/diff" \
  -d '{"name": "before-test"}'
```

List and delete snapshots:

```bash
curl "http://127.0.0.1:9090/api/commands/<id>/snapshots"
curl -X DELETE "http://127.0.0.1:9090/api/commands/<id>/snapshots/before-test"
```

**Exercise 9.1**: Run `top`, take a snapshot, wait 5 seconds, take another snapshot,
then diff them. How many cells changed?

---

## Lesson 10: Daemon Mode

Run vrunner in the background:

```bash
vrunner --daemon -- ./my-long-running-server
```

The process forks and returns immediately. Check status:

```bash
vrunner list
```

Stop the instance:

```bash
vrunner stop
```

Redirect output:

```bash
vrunner --daemon --stdout-file /tmp/vrunner.out --stderr-file /tmp/vrunner.err -- ./server
```

**Exercise 10.1**: Start a daemon, verify it's running with `vrunner list`,
spawn a command via the API, then stop it with `vrunner stop`.

---

## Lesson 11: TLS and Certificates

### Self-signed TLS

```bash
vrunner --tls -- ./my-app
```

Certificates are auto-generated in `~/.config/vrunner/`.
Access via HTTPS: `https://127.0.0.1:9090/admin`

### Per-command certificates

```bash
# Generate a named certificate
vrunner cert generate myapp

# Spawn a command bound to the certificate
curl -k -X POST https://127.0.0.1:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "bash", "certificate": "myapp"}'
```

Only clients presenting the `myapp` certificate (or its derived token)
can interact with that command.

**Exercise 11.1**: Start vrunner with `--tls`, open the admin panel in your browser
(accept the self-signed cert), and verify everything works over HTTPS.

---

## Lesson 12: Multiple Instances

vrunner supports multiple instances running simultaneously:

```bash
# Terminal 1
vrunner --port 9090 -- ./server-a

# Terminal 2
vrunner --port 9091 -- ./server-b
```

List all instances:

```bash
vrunner list          # Human-readable
vrunner list-vrunner # Machine-readable (tab-separated)
```

### Cross-instance spawn

```bash
# Spawn on a specific instance by PID
vrunner --target <PID> -- spawn -- vim notes.txt

# Or interactively (prompts if multiple instances)
vrunner spawn -- htop
```

**Exercise 12.1**: Start two vrunner instances on different ports.
Spawn commands on each. Use `vrunner stop` to stop one while the other keeps running.

---

## Lesson 13: PTY Raw Logging and Replay

Debug what a command is actually sending to the terminal:

```bash
vrunner --log-pty-raw /tmp/pty.log -- ls --color=always
```

The log file contains one line per `read()` call:

```
00000 ls --color=always\r\n
00003 \x1b[0m\x1b[01;34mCargo.toml\x1b[0m\r\n
00005 \x1b[0m\x1b[01;34mCargo.lock\x1b[0m\r\n
```

- Left column: elapsed time in milliseconds
- Right column: printable ASCII as-is, non-printable bytes as `\xHH`

### Replay step-by-step

```bash
perl tools/ansi-replay /tmp/pty.log           # Interactive
perl tools/ansi-replay /tmp/pty.log --dump    # All at once
```

| Key | Action |
|-----|--------|
| `Space` / `Enter` | Replay 1 line |
| `d` / `Right` | Replay 10 lines |
| `f` | Auto-play all remaining |
| `p` / `Left` | Peek at next line (no output) |
| `/pattern` | Search forward |
| `g N` | Jump to line N |
| `s` | Toggle elapsed time / line number |
| `h` | Help |
| `q` / `Esc` | Quit |

**Exercise 13.1**: Run a colorful command (e.g., `ls --color`, `htop`, or `vim`)
with `--log-pty-raw`. Open the log file and inspect the ANSI codes.
Replay it step-by-step with `ansi-replay`.

**Exercise 13.2**: Run `vim` under vrunner with PTY logging. Open vim, do some
editing, then quit. Replay the log. Can you follow what happened?

---

## Lesson 14: WebSocket Streaming

The web UI uses WebSockets for real-time updates. Connect directly:

```bash
# VTTY stream
wscat -c "ws://127.0.0.1:9090/api/commands/<id>/ws"

# Log stream
wscat -c "ws://127.0.0.1:9090/api/ws/logs"
```

Messages received on the VTTY WebSocket include:
- `vtty_full`: Complete terminal state (HTML + cursor + dimensions)
- `vtty_dirty`: Terminal content has changed, fetch fresh HTML via HTTP

The web client receives `vtty_dirty`, then polls:
```
GET /api/commands/<id>/vtty/html
```

**Exercise 14.1**: Use `wscat` to connect to the VTTY WebSocket.
Watch the messages flow in. Type text into the web UI and observe.

---

## Lesson 15: Advanced Patterns

### Resize a running command

```bash
vrunner resize <PID_or_name> --rows 50 --cols 160
```

Or via API:

```bash
curl -X POST "http://127.0.0.1:9090/api/commands/<id>/resize" \
  -H "Content-Type: application/json" \
  -d '{"rows": 50, "cols": 160}'
```

### Spawn with custom terminal size

```bash
vrunner spawn --rows 50 --cols 160 -- vim large_file.txt
```

### Multi-command orchestration

```bash
# Start vrunner headless
vrunner --port 9090 &

# Spawn multiple workers
curl -X POST http://127.0.0.1:9090/api/commands -d '{"cmd": "worker-1"}'
curl -X POST http://127.0.0.1:9090/api/commands -d '{"cmd": "worker-2"}'
curl -X POST http://127.0.0.1:9090/api/commands -d '{"cmd": "worker-3"}'

# Monitor all of them
vrunner --target $(vrunner list-vrunner | cut -f1) --display --display-all --tabs
```

### VTTY configuration

```yaml
vtty:
  rows: 30
  cols: 100
  term: "xterm-256color"
  scrollback: 10000
  truecolor: true
  mouse: true
```

**Exercise 15.1**: Start a command with default 24x80 size.
Resize it to 50x160 via the API. Verify the resize propagated (check `$LINES` and `$COLUMNS`
inside the child with `vrunner spawn -- env \| grep COLUMNS`).

---

## Quick Reference

### CLI Flags

| Flag | Description |
|------|-------------|
| `--bind ADDR` | Bind address (default: 127.0.0.1) |
| `--port PORT` | TCP port (default: 9090) |
| `--remote` | Bind 0.0.0.0 + enable auth |
| `--auth` | Require bearer token auth |
| `--tls` | Enable HTTPS |
| `--display` | Show terminal in local console |
| `--display-all` | Stay after command exits |
| `--tabs` | Show tab bar for commands |
| `--daemon` | Run in background |
| `--config FILE` | Config file path |
| `--profile NAME` | Configuration profile |
| `--log` | Enable API command logging |
| `--log-file FILE` | Log commands to file |
| `--log-pty-raw FILE` | Log raw PTY output to file |
| `--env K=V` | Set environment variable |
| `--no-env` | Skip config environment vars |
| `--term TERM` | TERM value for child |
| `--vtty-rows N` | Terminal rows |
| `--vtty-cols N` | Terminal columns |
| `--scrollback N` | Scrollback buffer lines |
| `--truecolor` | Enable 24-bit color |
| `--mouse` | Enable mouse forwarding |
| `--on-exit CMD` | Run on clean exit |
| `--on-error CMD` | Run on error exit |
| `--exit-timeout SECS` | Grace period before SIGKILL |
| `--certificate NAME:CERT:KEY` | Add named certificate |

### Subcommands

| Command | Description |
|---------|-------------|
| `vrunner list` | List running instances |
| `vrunner list-vrunner` | Machine-readable instance list |
| `vrunner list-commands` | Machine-readable command list |
| `vrunner stop [PID]` | Stop an instance |
| `vrunner stop-command [target]` | Stop a specific command |
| `vrunner spawn <cmd> [args]` | Spawn on a running instance |
| `vrunner freeze <PID>` | SIGSTOP a command |
| `vrunner thaw <PID>` | SIGCONT a command |
| `vrunner resize <target>` | Resize command's terminal |
| `vrunner cert generate <name>` | Create named certificate |
| `vrunner cert list` | List certificates |
| `vrunner cert show <name>` | Show certificate details |
| `vrunner cert remove <name>` | Delete certificate |
