# UDS IPC Usage

Learn how to programmatically control vrl using the UDS IPC interface via the `vrl` CLI subcommands — from listing commands and sending keystrokes to capturing output and managing processes.

## UDS IPC Overview

All inter-instance communication in vrl uses Unix Domain Sockets (UDS). Each vrl instance creates a control socket at:

```
~/.local/share/vrl/control-{pid}.sock
```

The socket uses `0600` permissions (owner read/write only), providing security through filesystem permissions. The wire protocol uses length-prefixed JSON framing.

## Listing Instances and Commands

```bash
# List all running instances and their commands
vrl list

# Show a specific instance
vrl list --target 12345
```

## Sending Keystrokes

Send keystrokes to a running command via UDS:

```bash
# Send text input
vrl keys 12345 "ls -la<Enter>"

# Send special keys
vrl keys 12345 "<C-c>"     # Ctrl+C
vrl keys 12345 "<Esc>:q!<Enter>"  # Quit vim
vrl keys 12345 "q"        # Quit htop
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

Use `vrl cat` to read a command's VTTY buffer:

```bash
# Auto-select if only one command
vrl cat

# Target a specific command by PID
vrl cat 12345

# With ANSI colors preserved
vrl cat --color-always htop
```

## Spawning Commands

Spawn a new command in a running instance:

```bash
vrl spawn-in 12345 -- htop
vrl spawn-in 12345 -- python -m http.server 8000
```

With options:

```bash
vrl spawn-in 12345 --rows 50 --cols 160 -- vim notes.txt
```

## Freezing and Thawing

Pause a command's output processing (freeze) and resume it later (thaw):

```bash
vrl freeze 5678    # SIGSTOP
vrl thaw 5678      # SIGCONT
```

## Resizing

Change the terminal dimensions of a running command:

```bash
vrl resize htop --rows 50 --cols 160
```

When `--rows` and `--cols` are omitted, vrl auto-detects your terminal's current size.

## Scripting Example

Here is a complete script that spawns commands in a daemon instance, waits, and retrieves output:

```bash
#!/usr/bin/env bash
set -euo pipefail

PID=12345

# Spawn commands
vrl spawn-in $PID -- npm run build
vrl spawn-in $PID -- npm test

# Poll until build finishes
sleep 10

# Capture output
vrl cat $PID --color-always > /tmp/build-output.txt
echo "Output retrieved."
```

For the complete CLI reference, see [`../reference/cli.md`](../reference/cli.md).
