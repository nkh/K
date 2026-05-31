# Getting Started with vrl

A progressive, hands-on tutorial. Each lesson builds on the previous one — follow them in order.

**Prerequisites:** A Linux, macOS, or Windows system with Rust installed (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh`).

---

## Lesson 1: Your First Command

vrl runs a command in a virtual terminal (PTY). Start simple:

```bash
vrl -- echo "Hello from vrl"
```

The command runs, prints output, and exits. Let's keep it alive:

```bash
vrl -- top
```

Now `top` is running inside vrl. Open another terminal:

```bash
vrl list
```

You should see one instance with its PID, daemon/display status, uptime, and commands.

**Exercise 1.1**: Run `vrl -- sleep 60` in the background (`&`).
Use `vrl list` to see it. Stop it with `Ctrl+C` or `vrl stop`.

**Exercise 1.2**: What happens if you run `vrl -- sleep 60 --sleep 60`?
Why? (Hint: check the `--` separator behavior — everything after `--` is the child command.)

---

## Lesson 2: Local Terminal Display

vrl has a built-in interactive display:

```bash
vrl --display -- htop
```

The VTTY contents are mirrored to your terminal at the refresh interval (default: 100ms).

- `--display`: Show terminal output in your current terminal
- `--display-all`: Stay running after the command exits (monitor mode)
- `--tabs`: Show a tab bar listing all commands

**Exercise 2.1**: Run `vrl --display --display-all --tabs -- sleep 100`.
While it's running, use `vrl spawn-in <pid> -- htop` in another terminal to add `htop`.
Use `Ctrl+Right` to switch between them in the display.

**Exercise 2.2**: Enable `kill_command` and `toggle_pause` keybindings in your config.
Test them: kill a command with `Ctrl+K`, then freeze/thaw with `Ctrl+Z`.

---

## Lesson 3: Configuration File

vrl reads config from (in order of precedence):

1. `~/.config/vrl/config.yaml` (global)
2. `./vrl.yaml` (project-local)
3. `--config <FILE>` (explicit)

Copy the example config:

```bash
cp examples/vrl.example.yaml ./vrl.yaml
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
vrl --profile dev -- cargo run
vrl --profile prod -- ./my-server
```

**Exercise 4.1**: Create a "small" profile with a 20x60 terminal and a "wide" profile
with 50x200. Run the same command under both.

---

## Lesson 5: Environment Variables

### Via CLI

```bash
vrl --env RUST_LOG=debug --env DATABASE_URL=postgres://localhost/db -- ./my-app
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
vrl --no-env -- ./my-app
```

**Exercise 5.1**: Set `RUST_LOG=debug` in config, then spawn a command with
`RUST_LOG=error`. Verify the CLI value wins.

---

## Lesson 6: Command Lifecycle

### Exit handlers

Run a command when a child exits:

```bash
vrl --on-exit "notify-send Done" -- on-success-script.sh
vrl --on-error "notify-send FAILED" -- flaky-test.sh
```

### Freeze and thaw

Suspend a command without killing it:

```bash
vrl freeze 5678
vrl thaw 5678
```

### Timeout

vrl sends `SIGTERM`, waits `timeout_secs` (default 10), then `SIGKILL`:

```bash
vrl --exit-timeout 5 -- ./my-server
```

**Exercise 6.1**: Run `vrl --on-exit "echo CALLBACK RAN" -- sleep 1`.
Check the vrl log output. Does the callback run?

---

## Lesson 7: Daemon Mode

Run vrl in the background:

```bash
vrl --daemon -- ./my-long-running-server
```

The process forks and returns immediately. Check status:

```bash
vrl list
```

Stop the instance:

```bash
vrl stop
```

Redirect output:

```bash
vrl --daemon --stdout-file /tmp/vrl.out --stderr-file /tmp/vrl.err -- ./server
```

**Exercise 7.1**: Start a daemon, verify it's running with `vrl list`,
then stop it with `vrl stop`.

---

## Lesson 8: UDS IPC Commands

vrl uses Unix Domain Sockets for all inter-instance communication.
The control socket is at `~/.local/share/vrl/control-{pid}.sock`.

### Send keystrokes

```bash
vrl keys 12345 "ls -la<Enter>"
vrl keys 12345 "<C-c>"  # Ctrl+C
```

### View terminal output

```bash
vrl cat
vrl cat --color-always htop
vrl cat 12345
```

### Spawn in a running instance

```bash
vrl spawn-in 12345 -- htop
vrl spawn-in 12345 -- python -m http.server 8000
```

### Freeze/thaw

```bash
vrl freeze 5678
vrl thaw 5678
```

### Resize

```bash
vrl resize htop --rows 50 --cols 160
```

---

## Lesson 9: Advanced Patterns

### Resize a running command

```bash
vrl resize htop --rows 50 --cols 160
```

### Send initial keystrokes

```bash
vrl --send-keys "ls<Enter>" -- bash
```

### Retain buffer after exit

```bash
vrl --retain-on-exit -- cargo test
```

### Save output on exit

```bash
vrl --snapshot-on-exit /tmp/build.log -- cargo build
```

---

## What's Next?

Now that you have completed the tutorial, explore:

- **[How-To Guides](../how-to-guides/)** — Task-specific recipes for common workflows
- **[Reference](../reference/)** — Look up any config key or CLI flag
- **[Explanation](../explanation/)** — Understand the architecture and design decisions
- **[FAQ](../faq.md)** — Answers to frequently asked questions
