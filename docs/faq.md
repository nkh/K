# Frequently Asked Questions

## General

### What is vrl?

vrl is a virtual terminal runner and process orchestrator. It runs commands inside pseudo-terminals (PTYs) and exposes them through a UDS (Unix Domain Socket) IPC interface with a built-in local terminal display. Unlike tools like tmux that are designed for terminal multiplexing, vrl is designed for programmatic control and fast CLI-based monitoring of terminal processes.

### Who is vrl for?

vrl is for anyone who needs to run, monitor, or control terminal processes: developers orchestrating services, DevOps engineers managing processes on servers, CI/CD engineers running terminal-aware tests, and system administrators managing headless machines.

### What language is vrl written in?

vrl is written in Rust and compiles to a single statically-linked binary with no runtime dependencies. This makes deployment trivial — just copy the binary to any Linux, macOS, or Windows machine.

### How is vrl different from tmux or screen?

tmux and screen are terminal multiplexers designed for interactive use within a single terminal session. vrl is a process orchestrator with UDS IPC and a local terminal display designed for programmatic control and fast startup (~5ms). While both provide PTY support, vrl adds daemon mode, configuration profiles, per-command exit handlers, and a multi-instance registry with no network overhead.

### What are the system requirements?

vrl requires Rust 1.75+ to build from source. The binary runs on Linux, macOS, and Windows. There are no runtime dependencies — the binary is statically linked. Disk space for the binary is approximately 4-6 MB (release build).

### Is vrl production-ready?

Yes. vrl is designed for production use with daemon mode (double-fork), graceful shutdown with configurable timeouts, per-command exit handlers, and UDS IPC secured by filesystem permissions (0600).

### What license does vrl use?

vrl is dual-licensed under `GPL-3.0-or-later OR Artistic-2.0`.

---

## Installation and Setup

### How do I install vrl?

The recommended method is building from source:

```bash
git clone https://github.com/nkh/K.git
cd K
cargo build --release
# Binary at target/release/vrl
```

For a system-wide install:

```bash
cargo install --path .
```

### How do I install the man pages?

```bash
cp man/vrl.1 /usr/local/share/man/man1/
man vrl
```

### How do I verify vrl is working?

Start vrl with a command:

```bash
vrl -- echo "Hello from vrl"
vrl -- htop
```

### How do I update vrl?

Pull the latest source and rebuild:

```bash
git pull
cargo build --release
```

### Does vrl work on Windows?

vrl compiles on Windows but daemon mode and some POSIX-specific features (SIGSTOP/SIGCONT for freeze/thaw) are not available. The VTTY and display functionality work on all platforms.

### Does vrl work on macOS?

Yes. vrl builds and runs on macOS including ARM (Apple Silicon). All features work on macOS.

### Can I run vrl without Docker or any runtime?

Yes. vrl is a single statically-linked binary with zero runtime dependencies.

---

## Running Commands

### How do I run a command with vrl?

Use the `--` separator to pass a command:

```bash
vrl -- htop
vrl --display -- vim notes.txt
vrl --daemon -- my-long-running-script.sh
```

### How do I run multiple commands at once?

Start vrl in idle mode, then spawn commands:

```bash
vrl &
vrl spawn-in 12345 -- htop
vrl spawn-in 12345 -- python -m http.server 8000
```

### How do I spawn a command in a running instance?

```bash
vrl spawn-in <pid> -- <cmd> [args...]
```

### How do I pass arguments to a command?

Arguments go after the command name, separated by `--`:

```bash
vrl -- python -m http.server 8000
```

### Can I run vrl without any command?

Yes. Start vrl in idle mode:

```bash
vrl
```

### How do I send initial keystrokes to a command?

Use `--send-keys`:

```bash
vrl --send-keys "ls<Enter>" -- bash
```

### How do I keep a command's output after it exits?

Use `--retain-on-exit`:

```bash
vrl --retain-on-exit -- cargo test
```

### How do I save output to a file when a command exits?

Use `--snapshot-on-exit`:

```bash
vrl --snapshot-on-exit /tmp/test-output.txt -- cargo test
```

---

## UDS IPC

### How does vrl communicate between instances?

All inter-instance communication uses Unix Domain Sockets (UDS). Each vrl instance creates a control socket at `~/.local/share/vrl/control-{pid}.sock` with `0600` permissions. The CLI subcommands (`keys`, `cat`, `spawn-in`, `freeze`, `thaw`, `resize`) connect to this socket and send length-prefixed JSON messages.

### What is the UDS wire protocol?

Messages use length-prefixed JSON framing: `[4 bytes big-endian u32][JSON payload]`. The client sends `ControlCommand` messages and receives `ControlResponse` messages.

### Is UDS IPC secure?

Yes. The UDS socket is created with `0600` permissions (owner read/write only). Only processes running as the same user can connect. There is no network exposure — UDS is local-only by definition.

### How do I view a command's terminal output?

Use `vrl cat`:

```bash
vrl cat           # auto-select if only one command
vrl cat 12345      # by PID
vrl cat --color-always htop  # with ANSI colors
```

### How do I send keystrokes to a command?

```bash
vrl keys 12345 "ls -la<Enter>"
vrl keys 12345 "<C-c>"  # Ctrl+C
```

---

## Interactive Display / TUI

### How do I use the local terminal display?

Add `--display` to mirror VTTY output to your terminal:

```bash
vrl --display -- htop
```

Add `--display-all` to stay in display mode after the command exits:

```bash
vrl --display-all --tabs -- htop
```

### How do I switch between commands in the display?

Use `Ctrl+Left` / `Ctrl+Right` to navigate between commands. Enable tabs with `--tabs` to see a tab bar at the top.

### How do I search in the terminal display?

Press `Ctrl+F` to open a search overlay.

### How do I quit the interactive display?

Press `Ctrl+\` to quit.

### How do I spawn a new command from within the display?

Press `F12` to open a spawn prompt.

---

## Daemon Mode

### How do I run vrl as a daemon?

```bash
vrl --daemon -- my-command
```

The process double-forks and detaches from the terminal.

### How do I redirect daemon output?

```bash
vrl --daemon --stdout-file /var/log/vrl/stdout \
  --stderr-file /var/log/vrl/stderr -- my-command
```

### How do I find running vrl instances?

```bash
vrl list
```

### How do I stop a daemon?

```bash
vrl stop <PID>
```

---

## Multi-Instance

### Can I run multiple vrl instances?

Yes. Each instance has its own PID and UDS control socket:

```bash
vrl -- htop &
vrl -- tail -f /var/log/app.log &
```

### How do I list all instances?

```bash
vrl list
```

### Can instances share configuration?

No. Each instance loads its own config file independently. However, you can use the same config file for multiple instances by specifying it with `--config`.

---

## Troubleshooting

### vrl won't start. What should I check?

1. Check your config file for syntax errors: `vrl config-check`
2. Check if another vrl instance is running: `vrl list`
3. Check the logs if running as a daemon: `cat /tmp/vrl.err`

### The display exits immediately after my command finishes. Why?

By default, when the CLI command exits, the display closes. Use `--display-all` to stay in display mode and switch to other running commands.

### How do I debug terminal output issues?

Use `--log-pty-raw` to capture raw PTY bytes:

```bash
vrl --log-pty-raw /tmp/pty.log -- my-command
perl tools/ansi-replay /tmp/pty.log
```

### How do I clean up stale instances?

Stale pidfiles (from crashed instances) are auto-cleaned by `vrl list`. If cleanup fails, manually remove files in `~/.local/share/vrl/instances/`.

### Memory usage is high. What can I do?

- Reduce scrollback: `--scrollback 1000` (default is 5000)
- Use smaller terminal sizes: `--vtty-rows 24 --vtty-cols 80`

### How do I report a bug?

Open an issue on [GitHub](https://github.com/nkh/K/issues) with:
1. vrl version (`vrl --version`)
2. Operating system
3. Steps to reproduce
4. Expected vs actual behavior
5. Relevant logs (`--log` or `--log-pty-raw` output)
