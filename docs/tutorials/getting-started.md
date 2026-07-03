# Getting Started with vrc

A progressive, hands-on tutorial. Each lesson builds on the previous one — follow them in order.

> **Two binaries:** This tutorial uses `vrc` (local UDS IPC) for its examples. `vrw` (HTTP server + web dashboard) supports all the same commands — just replace `vrc` with `vrw` in any example. Where the binaries differ (e.g., `vrc spawn-in` vs `vrw spawn`, or vrw-only features like the web dashboard), callouts are provided.

**Prerequisites:** A Linux, macOS, or Windows system with Rust installed (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh`).

---

## Lesson 1: Your First Command

vrc runs a command in a virtual terminal (PTY). Start simple:

```bash
vrc -- echo "Hello from vrc"
```

> **With vrw:** `vrw -- echo "Hello from vrw"`

The command runs, prints output, and exits. Let's keep it alive:

```bash
vrc -- top
```

> **With vrw:** `vrw -- top`

Now `top` is running inside vrc. Open another terminal:

```bash
vrc list
```

> **With vrw:** `vrw list`

You should see one instance with its PID, daemon/display status, uptime, and commands.

**Exercise 1.1**: Run `vrc -- sleep 60` in the background (`&`).
Use `vrc list` to see it. Stop it with `Ctrl+C` or `vrc stop`.

> **With vrw:** Replace `vrc` with `vrw` in each command.

**Exercise 1.2**: What happens if you run `vrc -- sleep 60 --sleep 60`?
Why? (Hint: check the `--` separator behavior — everything after `--` is the child command.)

---

## Lesson 2: Local Terminal Display

vrc has a built-in interactive display:

```bash
vrc --display -- htop
```

> **With vrw:** `vrw --display -- htop` — vrw supports the same `--display` and `--tabs` flags.

The VTTY contents are mirrored to your terminal at the refresh interval (default: 100ms).

- `--display`: Show terminal output in your current terminal and stay running after the command exits (monitor mode)
- `--tabs`: Show a tab bar listing all commands

**Exercise 2.1**: Run `vrc --display --tabs -- sleep 100`.
While it's running, use `vrc spawn-in <pid> -- htop` in another terminal to add `htop`.
Use `Ctrl+Right` to switch between them in the display.

> **With vrw:** Use `vrw spawn -- htop` to add commands to a running instance, and open the web dashboard at `http://localhost:9090/admin` for a browser-based view.

**Exercise 2.2**: Enable `kill_command` and `toggle_pause` keybindings in your config.
Test them: kill a command with `Ctrl+K`, then freeze/thaw with `Ctrl+Z`.

---

## Lesson 3: Configuration File

vrc reads config from (in order of precedence):

1. `~/.config/vrc/config.yaml` (global)
2. `./vrc.yaml` (project-local)
3. `--config <FILE>` (explicit)

> **With vrw:** Config paths are `~/.config/vrw/config.yaml` (global) and `./vrw.yaml` (project-local). The config format is identical; only vrw adds HTTP/TLS/web-specific keys (`port`, `tls`, `web`, etc.).

Copy the example config:

```bash
cp examples/vrc.example.yaml ./vrc.yaml
```

### Change the terminal size

```yaml
vtty:
  rows: 40
  cols: 120
```

**Exercise 3.1**: Create a config file with 20x60 terminal. Run the same command under both default and custom sizes.

---

## Lesson 4: Configuration Profiles

Profiles let you define named presets for different environments.

```yaml
profiles:
  dev:
    vtty:
      rows: 40
      cols: 120
    environment:
      variables:
        RUST_LOG: "debug"
  prod:
    display:
      enabled: false
```

Select a profile:

```bash
vrc --profile dev -- cargo run
vrc --profile prod -- ./my-server
```

**Exercise 4.1**: Create a "small" profile with a 20x60 terminal and a "wide" profile
with 50x200. Run the same command under both.

---

## Lesson 5: Environment Variables

### Via CLI

```bash
vrc --env RUST_LOG=debug --env DATABASE_URL=postgres://localhost/db -- ./my-app
```

### Via config

```yaml
environment:
  variables:
    RUST_LOG: "info"
    DATABASE_URL: "postgres://localhost/db"
```

### Isolate from parent environment

```bash
vrc --no-env -- ./my-app
```

**Exercise 5.1**: Set `RUST_LOG=debug` in config, then spawn a command with
`RUST_LOG=error`. Verify the CLI value wins.

---

## Lesson 6: Command Lifecycle

### Exit handlers

Run a command when a child exits:

```bash
vrc --on-exit "notify-send Done" -- on-success-script.sh
vrc --on-error "notify-send FAILED" -- flaky-test.sh
```

### Freeze and thaw

Suspend a command without killing it:

```bash
vrc freeze 5678
vrc thaw 5678
```

### Timeout

vrc sends `SIGTERM`, waits `timeout_secs` (default 10), then `SIGKILL`:

```bash
vrc --exit-timeout 5 -- ./my-server
```

**Exercise 6.1**: Run `vrc --on-exit "echo CALLBACK RAN" -- sleep 1`.
Check the vrc log output. Does the callback run?

---

## Lesson 7: Daemon Mode

Run vrc in the background:

```bash
vrc --daemon -- ./my-long-running-server
```

> **With vrw:** `vrw --daemon -- ./my-long-running-server` — works the same way. vrw also supports `--web --port 9090` to start the web dashboard alongside the daemon.

The process forks and returns immediately. Check status:

```bash
vrc list
```

Stop the instance:

```bash
vrc stop
```

Redirect output:

```bash
vrc --daemon --stdout-file /tmp/vrc.out --stderr-file /tmp/vrc.err -- ./server
```

**Exercise 7.1**: Start a daemon, verify it's running with `vrc list`,
then stop it with `vrc stop`.

---

## Lesson 8: UDS IPC Commands

vrc uses Unix Domain Sockets for all inter-instance communication.
The control socket is at `~/.local/share/vrc/control-{pid}.sock`.

> **With vrw:** vrw provides an HTTP REST API instead of UDS IPC. See [REST API Reference](../api.md) for the equivalent endpoints. Common mappings:
> - `vrc list` → `GET /api/commands`
> - `vrc keys` → `POST /api/commands/{id}/input`
> - `vrc cat` → `GET /api/commands/{id}/vtty/html`
> - `vrc spawn-in` → `POST /api/commands`

### Send keystrokes

```bash
vrc keys 12345 "ls -la<Enter>"
vrc keys 12345 "<C-c>"  # Ctrl+C
```

### View terminal output

```bash
vrc cat
vrc cat --color-always htop
vrc cat 12345
```

### Spawn in a running instance

```bash
vrc spawn-in 12345 -- htop
vrc spawn-in 12345 -- python -m http.server 8000
```

### Freeze/thaw

```bash
vrc freeze 5678
vrc thaw 5678
```

### Resize

```bash
vrc resize htop --rows 50 --cols 160
```

---

## Lesson 9: Advanced Patterns

### Resize a running command

```bash
vrc resize htop --rows 50 --cols 160
```

### Send initial keystrokes

```bash
vrc --send-keys "ls<Enter>" -- bash
```

### Retain buffer after exit

```bash
vrc --retain-on-exit -- cargo test
```

### Save output on exit

```bash
vrc --snapshot-on-exit /tmp/build.log -- cargo build
```

---

## What's Next?

Now that you have completed the tutorial, explore:

- **[How-To Guides](../how-to-guides/)** — Task-specific recipes for common workflows
- **[Reference](../reference/)** — Look up any config key or CLI flag
- **[Explanation](../explanation/)** — Understand the architecture and design decisions
- **[FAQ](../faq.md)** — Answers to frequently asked questions
- **[vrw Web Dashboard](../how-to-guides/web-dashboard.md)** — If you need remote access or a browser-based UI, try vrw's web dashboard
