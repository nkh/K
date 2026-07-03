# Frequently Asked Questions

## General

### What is vrc?

vrc is a virtual terminal runner and process orchestrator. It runs commands inside pseudo-terminals (PTYs) and exposes them through a UDS (Unix Domain Socket) IPC interface with a built-in local terminal display. Unlike tools like tmux that are designed for terminal multiplexing, vrc is designed for programmatic control and fast CLI-based monitoring of terminal processes.

### Who is vrc for?

vrc is for anyone who needs to run, monitor, or control terminal processes: developers orchestrating services, DevOps engineers managing processes on servers, CI/CD engineers running terminal-aware tests, and system administrators managing headless machines.

### What language is vrc written in?

vrc is written in Rust and compiles to a single statically-linked binary with no runtime dependencies. This makes deployment trivial — just copy the binary to any Linux, macOS, or Windows machine.

### How is vrc different from tmux or screen?

tmux and screen are terminal multiplexers designed for interactive use within a single terminal session. vrc is a process orchestrator with UDS IPC and a local terminal display designed for programmatic control and fast startup (~5ms). While both provide PTY support, vrc adds daemon mode, configuration profiles, per-command exit handlers, and a multi-instance registry with no network overhead.

### What are the system requirements?

vrc requires Rust 1.75+ to build from source. The binary runs on Linux, macOS, and Windows. There are no runtime dependencies — the binary is statically linked. Disk space for the binary is approximately 4-6 MB (release build).

### Is vrc production-ready?

Yes. vrc is designed for production use with daemon mode (via the `daemonize` crate), graceful shutdown with configurable timeouts, per-command exit handlers, and UDS IPC secured by filesystem permissions (0600).

### What license does vrc use?

vrc is dual-licensed under `GPL-3.0-or-later OR Artistic-2.0`.

---

## vrw

### What is vrw?

vrw is the web-enabled variant of vrc. It shares all the core VTTY, process management, and display features of vrc but adds an HTTP server, WebSocket streaming, a built-in web admin dashboard, REST API, TLS encryption, and certificate-based access control. Build with `cargo build --features vrw`. Both binaries share the same configuration format and most CLI flags.

### How do I access the web admin dashboard?

Start vrw and navigate to `http://localhost:9090/admin` in your browser. The dashboard shows all running commands, renders their VTTY output in real-time via WebSocket, and provides controls for spawning, stopping, sending keystrokes, taking screenshots, freezing/thawing, and purging commands.

### What is the difference between vrc and vrw?

vrc uses Unix Domain Socket (UDS) IPC for inter-process communication and is designed for local terminal use. vrw uses HTTP/WebSocket for communication and adds a web admin dashboard, REST API, TLS support, certificate management, and remote access capabilities. Choose vrc for lightweight local process management, or vrw when you need web-based control or remote access.

---

## Installation and Setup

### How do I install vrc?

The recommended method is building from source:

```bash
git clone https://github.com/nkh/K.git
cd K
cargo build --release
# Binary at target/release/vrc
```

For a system-wide install:

```bash
cargo install --path .
```

### How do I install the man pages?

```bash
cp man/vrc.1 /usr/local/share/man/man1/
man vrc
```

### How do I verify vrc is working?

Start vrc with a command:

```bash
vrc -- echo "Hello from vrc"
vrc -- htop
```

### How do I update vrc?

Pull the latest source and rebuild:

```bash
git pull
cargo build --release
```

### Does vrc work on Windows?

vrc compiles on Windows but daemon mode and some POSIX-specific features (SIGSTOP/SIGCONT for freeze/thaw) are not available. The VTTY and display functionality work on all platforms.

### Does vrc work on macOS?

Yes. vrc builds and runs on macOS including ARM (Apple Silicon). All features work on macOS.

### Can I run vrc without Docker or any runtime?

Yes. vrc is a single statically-linked binary with zero runtime dependencies.

---

## Running Commands

### How do I run a command with vrc?

Use the `--` separator to pass a command:

```bash
vrc -- htop
vrc --display -- vim notes.txt
vrc --daemon -- my-long-running-script.sh
```

### How do I run multiple commands at once?

Start vrc in idle mode, then spawn commands:

```bash
vrc &
vrc spawn-in 12345 -- htop
vrc spawn-in 12345 -- python -m http.server 8000
```

### How do I spawn a command in a running instance?

```bash
vrc spawn-in <pid> -- <cmd> [args...]
```

### How do I pass arguments to a command?

Arguments go after the command name, separated by `--`:

```bash
vrc -- python -m http.server 8000
```

### Can I run vrc without any command?

Yes. Start vrc in idle mode:

```bash
vrc
```

### How do I send initial keystrokes to a command?

Use `--send-keys`:

```bash
vrc --send-keys "ls<Enter>" -- bash
```

### How do I keep a command's output after it exits?

Use `--retain-on-exit`:

```bash
vrc --retain-on-exit -- cargo test
```

### How do I save output to a file when a command exits?

Use `--snapshot-on-exit`:

```bash
vrc --snapshot-on-exit /tmp/test-output.txt -- cargo test
```

---

## UDS IPC

### How does vrc communicate between instances?

All inter-instance communication uses Unix Domain Sockets (UDS). Each vrc instance creates a control socket at `~/.local/share/vrc/control-{pid}.sock` with `0600` permissions. The CLI subcommands (`keys`, `cat`, `spawn-in`, `freeze`, `thaw`, `resize`) connect to this socket and send length-prefixed JSON messages.

### What is the UDS wire protocol?

Messages use length-prefixed JSON framing: `[4 bytes big-endian u32][JSON payload]`. The client sends `ControlCommand` messages and receives `ControlResponse` messages.

### Is UDS IPC secure?

Yes. The UDS socket is created with `0600` permissions (owner read/write only). Only processes running as the same user can connect. There is no network exposure — UDS is local-only by definition.

### How do I view a command's terminal output?

Use `vrc cat`:

```bash
vrc cat <pid>       # by PID (required)
```

> **Note:** `pid` is a required argument. `--color-always` is available on `vrw cat`, not `vrc cat`.

### How do I send keystrokes to a command?

```bash
vrc keys 12345 "ls -la<Enter>"
vrc keys 12345 "<C-c>"  # Ctrl+C
```

---

## Interactive Display / TUI

### How do I use the local terminal display?

Add `--display` to mirror VTTY output to your terminal:

```bash
vrc --display -- htop
```

Add `--display` to stay in display mode after the command exits:

```bash
vrc --display --tabs -- htop
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

### How do I run vrc as a daemon?

```bash
vrc --daemon -- my-command
```

The process daemonizes into the background using the `daemonize` crate.

### How do I redirect daemon output?

```bash
vrc --daemon --stdout-file /var/log/vrc/stdout \
  --stderr-file /var/log/vrc/stderr -- my-command
```

### How do I find running vrc instances?

```bash
vrc list
```

### How do I stop a daemon?

```bash
vrc stop <PID>
```

---

## Multi-Instance

### Can I run multiple vrc instances?

Yes. Each instance has its own PID and UDS control socket:

```bash
vrc -- htop &
vrc -- tail -f /var/log/app.log &
```

### How do I list all instances?

```bash
vrc list
```

### Can instances share configuration?

No. Each instance loads its own config file independently. However, you can use the same config file for multiple instances by specifying it with `--config`.

---

## Troubleshooting

### vrc won't start. What should I check?

1. Check your config file for syntax errors: `vrc config-check`
2. Check if another vrc instance is running: `vrc list`
3. Check the logs if running as a daemon: `cat $XDG_STATE_HOME/vrc.err`

### The display exits immediately after my command finishes. Why?

By default, when the CLI command exits, the display closes. Use `--display` to stay in display mode and switch to other running commands.

### How do I debug terminal output issues?

Use `--log-pty-raw` to capture raw PTY bytes:

```bash
vrc --log-pty-raw /tmp/pty.log -- my-command
perl tools/ansi-replay /tmp/pty.log
```

### How do I clean up stale instances?

Stale pidfiles (from crashed instances) are auto-cleaned by `vrc list`. If cleanup fails, manually remove files in `~/.local/share/vrc/instances/`.

### Memory usage is high. What can I do?

- Reduce scrollback: `--scrollback 1000` (default is 5000)
- Use smaller terminal sizes: `--vtty-rows 24 --vtty-cols 80`

### How do I report a bug?

Open an issue on [GitHub](https://github.com/nkh/K/issues) with:
1. vrc version (`vrc --version`)
2. Operating system
3. Steps to reproduce
4. Expected vs actual behavior
5. Relevant logs (`--log` or `--log-pty-raw` output)

### Shell completions are not working after installation

Ensure the completion script was installed in the correct directory for your shell. For bash, completions must be in a directory listed in `BASH_COMPLETION_USER_DIR` or `/etc/bash_completion.d/`. For zsh, the file must be in a directory in your `fpath` (commonly `~/.zsh/completions/`). For fish, completions go in `~/.config/fish/completions/`. After installing, restart your shell or run `source ~/.bashrc` (or equivalent) to reload.

### My completions show vrw options when I run `vrc`

This can happen if you generated completions for the wrong binary. Make sure to generate completions using the correct binary name: `vrc completions bash > ...` for vrc completions, or `vrw completions bash > ...` for vrw completions. When building with both features, the binary detects its name from argv[0] and generates the appropriate completion set.
