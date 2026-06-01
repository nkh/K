# Lifecycle Policy

This document explains how vrc and vrw decide when to start, retain, and exit
instances and the daemon itself. **The lifecycle policy applies to both vrc and vrw.** It covers the "last-command-standing" principle,
the three display modes (headless, display, monitor), how per-command options
affect lifecycle, and special considerations for daemon mode. Read this if you
want to understand when vrw will keep running, when it will shut down, and
how to control that behavior for your use case.

---

## The "Last-Command-Standing" Principle

vrw's daemon lives only as long as it has work to do. When the last running
command exits, the daemon initiates a graceful shutdown. This prevents zombie
processes and ensures that resources (ports, PID files, TLS state) are released
as soon as they are no longer needed.

```
┌──────────────────────────────────────────────────────────────┐
│                                                              │
│   vrw run -- web-server                                  │
│   ┌────────────┐                                             │
│   │  Daemon    │  lives as long as "web-server" is running   │
│   │  ┌───────┐ │                                             │
│   │  │ web-  │ │                                             │
│   │  │ server│ │                                             │
│   │  └───┬───┘ │                                             │
│   └──────┼─────┘                                             │
│          │ process exits                                     │
│          ▼                                                   │
│   ┌────────────┐                                             │
│   │  Daemon    │  "No instances remaining; shutting down"      │
│   │  (empty)  │  → closes web server                         │
│   └────────────┘  → removes PID file                         │
│                    → exits                                    │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

The lifecycle loop in the daemon runs every 500ms and checks the instance
registry. If the registry is empty and no commands have the `retain-on-exit`
flag, the daemon sends a shutdown signal via the broadcast channel.

---

## Headless Mode Lifecycle

Headless mode is the default when no client is connected and the daemon was
started without a display:

```
    Command started                  Command exits
         │                               │
         ▼                               ▼
  ┌────────────┐                    ┌────────────┐
  │  RUNNING   │ ────────────────► │  EXITED    │
  │  (headless)│                    │  (cleaned  │
  └────────────┘                    │   up)      │
                                    └──────┬─────┘
                                           │
                                           ▼
                                    ┌────────────┐
                                    │  Daemon    │
                                    │  checks    │
                                    │  registry  │
                                    └──────┬─────┘
                                           │
                                    empty?  │  → shutdown
                                    no?     │  → continue
```

In headless mode, terminal output is still processed by the VTTY emulator but
is not streamed to any client. The emulator's scrollback buffer continues to
accumulate output, which is useful when a client later connects and needs to
see the history.

---

## Display Mode Lifecycle

Display mode is active when a client is connected to a command's WebSocket:

```
    Command started                  Client connects              Client disconnects
         │                               │                               │
         ▼                               ▼                               ▼
  ┌────────────┐                    ┌────────────┐                    ┌────────────┐
  │  RUNNING   │ ──────────────────►│   ACTIVE   │ ──────────────────►│  MONITOR   │
  │  (headless)│                    │ (streaming)│                    │ (buffering)│
  └────────────┘                    └────────────┘                    └──────┬─────┘
                                                                             │
                                   Client reconnects                         │
                                         │              Command exits        │
                                         ▼                  │               ▼
                                  ┌────────────┐    ┌────────────┐   ┌────────────┐
                                  │   ACTIVE   │    │  EXITED    │   │  Monitor   │
                                  │ (resuming) │    │  (snapshot │   │  timeout   │
                                  └────────────┘    │   saved)   │   │  → cleanup │
                                                    └────────────┘   └────────────┘
```

### Transitions

| From | To | Trigger |
|---|---|---|
| Headless | Active | Client opens WebSocket |
| Active | Monitor | Client disconnects (close frame, network drop) |
| Monitor | Active | Client reconnects (new WebSocket) |
| Active | Exit | Command process exits |
| Monitor | Exit | Command process exits |
| Monitor | Exit | Monitor timeout expires (default: 60s) |

### Active Mode

- Full incremental diff streaming is active (200ms tick interval).
- The VTTY renderer sends `vtty_diff` messages on every tick where changes
  exist.
- Client input (keystrokes, resize events) is forwarded to the PTY.

### Monitor Mode

When the last client disconnects, the command transitions to **monitor mode**:

- The VTTY emulator continues processing output from the child process.
- The renderer continues computing diffs but does **not** send them (no
  WebSocket is open).
- Diffs are buffered in memory (up to a configurable limit, default: 1000
  diffs or 60 seconds, whichever comes first).
- If a client reconnects within the monitor window, the buffered diffs are
  flushed, followed by a resynchronization if the buffer was overflowed.
- If the monitor timeout expires with no client, the command entry is removed
  from the registry (unless `retain-on-exit` is set).

---

## Monitor Mode Behavior

Monitor mode exists to provide a grace period between client disconnection and
cleanup. This handles common scenarios:

| Scenario | Behavior |
|---|---|
| Browser tab closed and reopened quickly | Client reconnects; buffered diffs flushed; no data lost |
| Network hiccup | Same as above—reconnection resumes seamlessly |
| Intentional close (user done) | Monitor timeout expires; command cleaned up |
| Command exits while in monitor mode | Final snapshot generated; entry cleaned up |

The monitor timeout is configurable:

```bash
vrw run --monitor-timeout 120s -- long-running-job
```

A timeout of `0` disables monitoring entirely—command cleanup happens
immediately when the last client disconnects. A timeout of `infinity` (or `0s`
in some configurations) keeps the monitor alive indefinitely until the command
exits.

---

## How `retain-on-exit` Affects Lifecycle

The `--retain-on-exit` flag overrides the normal cleanup behavior. When a
command exits with this flag, the daemon:

1. **Keeps the entry in the instance registry** with status `Exited`.
2. **Preserves the VTTY scrollback buffer** so clients can view the final
   output.
3. **Generates a final snapshot** if `--snapshot-on-exit` is also set.
4. **Does NOT count the command toward "last-command-standing"** for daemon
   shutdown purposes.

Wait—actually, the retained command **does** count. The daemon's lifecycle loop
checks:

```rust
if registry.is_empty() && !any_retained {
    shutdown();
}
```

But `is_empty()` returns `true` only when no entries (including retained ones)
exist. So a retained command keeps the daemon alive.

```
  ┌──────────────────────────────────────────────┐
  │  Normal:   command exits → unregister →       │
  │            registry empty → daemon exits      │
  │                                               │
  │  Retained:  command exits → status=Exited →    │
  │             registry NOT empty → daemon alive  │
  │             (clients can still view output)    │
  │             → explicit DELETE to unregister    │
  └──────────────────────────────────────────────┘
```

To remove a retained command, the client must explicitly call:

```
DELETE /api/commands/{id}
```

This frees the registry entry and, if it was the last one, triggers daemon
shutdown.

---

## Per-Command Options

Each command can be configured with lifecycle-related options:

| Option | Scope | Effect |
|---|---|---|
| `--retain-on-exit` | Per-command | Keeps the registry entry and buffer after the process exits |
| `--snapshot-on-exit` | Per-command | Generates a final full HTML snapshot of the terminal buffer on exit |
| `--send-keys <keys>` | Per-command | Injects keystrokes into the PTY after the process starts |
| `--monitor-timeout <dur>` | Per-command | Sets the monitor mode timeout for this command (overrides global default) |
| `--snapshot-interval <dur>` | Per-command | Periodically saves snapshots while the command is running |

### Scope

These options are **per-command**, meaning they apply to the individual command
they are attached to. If you start five commands, each can have its own
`--retain-on-exit` and `--monitor-timeout` settings:

```bash
vrw run \
  --retain-on-exit -- my-database      # retained after exit  \
  --snapshot-on-exit -- my-build        # snapshot saved on exit \
  -- my-web-server                     # normal lifecycle
```

Options set via CLI flags override the global configuration file defaults.

---

## Daemon Mode Considerations

When vrw is started with `--daemon`, the lifecycle policy has additional
implications:

### PID File Management

The daemon writes its PID to a file (default: `/tmp/vrw.pid` or
`~/.config/vrw/vrw.pid`). This file is used by `vrw stop` to signal
the daemon. The PID file is **removed on shutdown**.

### Signal Handling

| Signal | Action |
|---|---|
| `SIGTERM` | Graceful shutdown (stop all commands, close web server, exit) |
| `SIGINT` | Same as `SIGTERM` |
| `SIGUSR1` | Reload configuration (if supported) |

### Graceful Shutdown Sequence

```
  SIGTERM received
       │
       ▼
  1. Stop accepting new connections (web server drain)
       │
       ▼
  2. Close all WebSocket connections (send close frames)
       │
       ▼
  3. Send SIGTERM to all child processes
       │
       ▼
  4. Wait up to 10 seconds for children to exit
       │
       ├── all exited? → continue
       └── timeout?   → SIGKILL remaining children
       │
       ▼
  5. Unregister all instances
       │
       ▼
  6. Remove PID file
       │
       ▼
  7. Exit
```

### Long-Lived Daemons

For long-lived daemon deployments (e.g., a server that runs continuously), you
can override the last-command-standing policy with `--no-auto-shutdown`:

```bash
vrw daemon --no-auto-shutdown
```

This keeps the daemon running even when no commands are active. New commands
can be added at any time via the API:

```bash
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"name": "my-app", "command": "my-app", "args": []}'
```

---

## Lifecycle Decision Flowchart

The following diagram shows the complete decision flow when a command exits:

```
  Command process exits
         │
         ▼
  ┌──────────────────┐
  │ retain-on-exit?   │
  └────┬─────────┬───┘
       │ Yes     │ No
       ▼         ▼
  ┌─────────┐  ┌──────────────────┐
  │ Set     │  │ snapshot-on-exit?│
  │ status  │  └────┬────────┬───┘
  │= Exited │       │ Yes    │ No
  └────┬────┘       ▼        ▼
       │       ┌────────┐  ┌────────┐
       │       │ Save   │  │ Skip   │
       │       │ HTML   │  │ snap   │
       │       └───┬────┘  └───┬────┘
       │           │           │
       │           ▼           ▼
       │       ┌──────────────────┐
       │       │ Unregister from  │
       │       │ registry         │
       │       └────┬─────────────┘
       │            │
       ▼            ▼
  ┌──────────────────────────┐
  │ Registry empty?           │
  └────┬──────────────┬──────┘
       │ No           │ Yes
       ▼              ▼
  ┌──────────┐   ┌───────────────────┐
  │ Daemon   │   │ no-auto-shutdown? │
  │ continues│   └────┬──────────┬────┘
  └──────────┘        │ Yes      │ No
                      ▼          ▼
                 ┌──────────┐ ┌──────────┐
                 │ Daemon   │ │ Daemon   │
                 │ stays    │ │ exits    │
                 └──────────┘ └──────────┘
```

---

*This document is part of the [Diátaxis](https://diataxis.fr/) documentation framework
for vrc and vrw. See the [explanation index](./) for related topics.*
