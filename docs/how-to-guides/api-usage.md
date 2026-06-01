# UDS IPC Usage

Learn how to programmatically control vrc using the UDS IPC interface via the `vrc` CLI subcommands — from listing commands and sending keystrokes to capturing output and managing processes.

> **This guide covers the vrc UDS IPC interface.** For the vrw HTTP API (REST endpoints, web dashboard, WebSocket), see [REST API Reference](../api.md). The two interfaces provide equivalent functionality — vrc uses Unix Domain Sockets and CLI subcommands, while vrw uses HTTP endpoints and a web UI.

## UDS IPC Overview

All inter-instance communication in vrc uses Unix Domain Sockets (UDS). Each vrc instance creates a control socket at:

```
~/.local/share/vrc/control-{pid}.sock
```

The socket uses `0600` permissions (owner read/write only), providing security through filesystem permissions. The wire protocol uses length-prefixed JSON framing.

## Listing Instances and Commands

```bash
# List all running instances and their commands
vrc list

# Show a specific instance
vrc list --target 12345
```

## Sending Keystrokes

Send keystrokes to a running command via UDS:

```bash
# Send text input
vrc keys 12345 "ls -la<Enter>"

# Send special keys
vrc keys 12345 "<C-c>"     # Ctrl+C
vrc keys 12345 "<Esc>:q!<Enter>"  # Quit vim
vrc keys 12345 "q"        # Quit htop
```

### Common Escape Sequences

| Sequence | Key |
|----------|-----|
| `\x03` | Ctrl+C (SIGINT) |
| `\x04` | Ctrl+D (EOF) |
| `\x1b` | Escape |
| `\r` | Enter |
| `\t` | Tab |
| `\x7f` | Backspace |
| `\x1b[A` | Up arrow |
| `\x1b[B` | Down arrow |

## Viewing Terminal Output

Use `vrc cat` to read a command's VTTY buffer:

```bash
# Auto-select if only one command
vrc cat

# Target a specific command by PID
vrc cat 12345

# With ANSI colors preserved
vrc cat --color-always htop
```

## Spawning Commands

Spawn a new command in a running instance:

```bash
vrc spawn-in 12345 -- htop
vrc spawn-in 12345 -- python -m http.server 8000
```

With options:

```bash
vrc spawn-in 12345 --rows 50 --cols 160 -- vim notes.txt
```

## Freezing and Thawing

Pause a command's output processing (freeze) and resume it later (thaw):

```bash
vrc freeze 5678    # SIGSTOP
vrc thaw 5678      # SIGCONT
```

## Resizing

Change the terminal dimensions of a running command:

```bash
vrc resize htop --rows 50 --cols 160
```

When `--rows` and `--cols` are omitted, vrc auto-detects your terminal's current size.

## Scripting Example

Here is a complete script that spawns commands in a daemon instance, waits, and retrieves output:

```bash
#!/usr/bin/env bash
set -euo pipefail

PID=12345

# Spawn commands
vrc spawn-in $PID -- npm run build
vrc spawn-in $PID -- npm test

# Poll until build finishes
sleep 10

# Capture output
vrc cat $PID --color-always > /tmp/build-output.txt
echo "Output retrieved."
```

For the complete CLI reference, see [`../reference/cli.md`](../reference/cli.md).

## vrw Equivalents

The following table maps common vrc UDS IPC commands to their vrw HTTP API equivalents:

| vrc (UDS IPC) | vrw (HTTP API) | Description |
|---------------|-----------------|-------------|
| `vrc list` | `GET /api/commands` | List running commands |
| `vrc keys <pid> "text"` | `POST /api/commands/{id}/input` | Send keystrokes |
| `vrc cat` | `GET /api/commands/{id}/vtty/html` | View terminal output |
| `vrc spawn-in <pid> -- cmd` | `POST /api/commands` | Spawn a new command |
| `vrc freeze <pid>` | `POST /api/commands/{id}/freeze` | Freeze a command |
| `vrc thaw <pid>` | `POST /api/commands/{id}/thaw` | Thaw a command |
| `vrc resize cmd --rows N --cols M` | `POST /api/commands/{id}/resize` | Resize terminal |

For full vrw API documentation, see [`../api.md`](../api.md).
