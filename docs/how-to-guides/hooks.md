# Event Hooks

Learn how to configure event hooks that trigger custom scripts when commands are spawned, exit, encounter errors, or are killed.

> **Hooks work for both vrc and vrw.** The hook system, events, and configuration syntax are identical. The only difference is in environment variable prefixes: vrc uses `$VRC_*` placeholders, vrw uses `$VRW_*` placeholders (see below).

## What Are Hooks?

Hooks are shell commands that the binary executes automatically when specific events occur. They let you integrate with external tools — sending notifications, logging to external systems, triggering alerts, or running cleanup scripts.

## Supported Events

| Event | Triggered When |
|-------|---------------|
| `on_spawn` | A command is successfully spawned |
| `on_exit` | A command exits (any exit code) |
| `on_error` | A command exits with a non-zero exit code |
| `on_kill` | A command is killed by vrc or via the API |

## Placeholders

Hook commands support placeholders that vrc replaces with actual values at runtime:

| Placeholder | vrc name | vrw name | Value |
|-------------|----------|----------|-------|
| `{PREFIX}_CMD_ID` | `$VRC_CMD_ID` | `$VRW_CMD_ID` | The command's unique ID (e.g., `cmd_a1b2c3`) |
| `{PREFIX}_CMD_NAME` | `$VRC_CMD_NAME` | `$VRW_CMD_NAME` | The command's name (or command string if unnamed) |
| `{PREFIX}_CMD_COMMAND` | `$VRC_CMD_COMMAND` | `$VRW_CMD_COMMAND` | The full command string |
| `{PREFIX}_EXIT_CODE` | `$VRC_EXIT_CODE` | `$VRW_EXIT_CODE` | The exit code (available in `on_exit`, `on_error`, `on_kill`) |
| `{PREFIX}_PID` | `$VRC_PID` | `$VRW_PID` | The process ID of the spawned command |
| `{PREFIX}_TIMESTAMP` | `$VRC_TIMESTAMP` | `$VRW_TIMESTAMP` | ISO 8601 timestamp of the event |

> **Note:** When writing hooks in a config file, use the placeholder names matching the binary you're running: `$VRC_*` for vrc, `$VRW_*` for vrw. The examples below show both conventions.

## Config File Examples

### Global Hooks (All Commands)

Define hooks at the top level of your config to apply to every command:

```yaml
# ~/.config/vrc/config.yaml  (vrc hooks use $VRC_* prefixes)
hooks:
  on_spawn: |
    echo "[vrc] $VRC_CMD_NAME ($VRC_CMD_ID) started at $VRC_TIMESTAMP" \
      >> /var/log/vrc/events.log

  on_exit: |
    echo "[vrc] $VRC_CMD_NAME exited with code $VRC_EXIT_CODE" \
      >> /var/log/vrc/events.log

  on_error: |
    /opt/scripts/notify-slack.sh '#alerts' \
      "Command $VRC_CMD_NAME failed with exit code $VRC_EXIT_CODE"

  on_kill: |
    echo "[vrc] $VRC_CMD_NAME was killed (was PID $VRC_PID)" \
      >> /var/log/vrc/events.log
```

> **vrw equivalent:** Same config, but use `$VRW_*` prefixes and place the file at `~/.config/vrw/config.yaml`:
>
> ```yaml
> # ~/.config/vrw/config.yaml  (vrw hooks use $VRW_* prefixes)
> hooks:
>   on_spawn: |
>     echo "[vrw] $VRW_CMD_NAME ($VRW_CMD_ID) started at $VRW_TIMESTAMP" \
>       >> /var/log/vrw/events.log
> ```

### Per-Command Hooks

Define hooks on individual commands for command-specific behavior:

```yaml
commands:
  - name: frontend
    command: "npm run dev"
    cwd: /home/user/project/frontend
    hooks:
      on_spawn: "echo 'Frontend dev server started'"
      on_error: "/opt/scripts/restart-frontend.sh"

  - name: build
    command: "npm run build"
    cwd: /home/user/project
    hooks:
      on_spawn: "echo 'Build started at $VRC_TIMESTAMP'"
      on_exit: |
        if [ "$VRC_EXIT_CODE" -eq 0 ]; then
          /opt/scripts/upload-artifacts.sh
        else
          /opt/scripts/notify-failed-build.sh
        fi
```

## Precedence: Per-Command Over Global

If a command has its own hooks defined, they take precedence over global hooks for the same event:

```yaml
hooks:
  on_exit: "echo 'global exit handler'"

commands:
  - name: special
    command: "./special"
    hooks:
      on_exit: "echo 'special exit handler'"
```

In this example, the `special` command runs `"echo 'special exit handler'"` on exit. All other commands run `"echo 'global exit handler'"`.

If a command defines only some hooks, the remaining events fall through to the global hooks:

```yaml
hooks:
  on_spawn: "echo 'global spawn'"
  on_exit: "echo 'global exit'"
  on_error: "echo 'global error'"

commands:
  - name: custom
    command: "./custom"
    hooks:
      on_error: "echo 'custom error'"
```

The `custom` command runs:
- `on_spawn` → global (`echo 'global spawn'`)
- `on_exit` → global (`echo 'global exit'`)
- `on_error` → per-command (`echo 'custom error'`)

## Practical Hook Patterns

### Slack Notification on Failure

```yaml
hooks:
  on_error: |
    curl -s -X POST https://hooks.slack.com/services/XXX/YYY/ZZZ \
      -H "Content-Type: application/json" \
      -d "{\"text\": \"❌ $VRC_CMD_NAME failed (exit $VRC_EXIT_CODE) at $VRC_TIMESTAMP\"}"
```

> **vrw:** Replace `$VRC_*` with `$VRW_*` above.

### PagerDuty Alert on Critical Error

```yaml
commands:
  - name: api-server
    command: "./server"
    hooks:
      on_error: |
        /opt/scripts/pagerduty-trigger.sh \
          --service "$VRC_CMD_NAME" \
          --details "Exit code: $VRC_EXIT_CODE"
```

### Log Rotation on Exit

```yaml
hooks:
  on_exit: |
    if [ -f "/var/log/vrc/$VRC_CMD_NAME.log" ]; then
      mv "/var/log/vrc/$VRC_CMD_NAME.log" \
         "/var/log/vrc/$VRC_CMD_NAME-$VRC_TIMESTAMP.log"
    fi
```

### Auto-Restart on Failure

```yaml
commands:
  - name: worker
    command: "./worker"
    hooks:
      on_error: |
        sleep 5
        vrc spawn-in <pid> -- ./worker  # For vrw: vrw spawn --command "./worker" --name "worker"
```

### Datadog Metric on Spawn

```yaml
hooks:
  on_spawn: |
    curl -s -X POST "https://api.datadoghq.com/api/v1/series?api_key=$DD_API_KEY" \
      -H "Content-Type: application/json" \
      -d "{\"series\": [{\"metric\": \"vrc.commands.spawned\", \"points\": [[$(date +%s), 1]], \"tags\": [\"command:$VRC_CMD_NAME\"]}]}"
```

> **vrw:** Replace `$VRC_CMD_NAME` with `$VRW_CMD_NAME`.

### Webhook on Any Exit

```yaml
hooks:
  on_exit: |
    curl -s -X POST https://webhook.example.com/vrc \
      -H "Content-Type: application/json" \
      -d "{\"event\": \"exit\", \"command\": \"$VRC_CMD_NAME\", \"exit_code\": $VRC_EXIT_CODE, \"timestamp\": \"$VRC_TIMESTAMP\"}"
```

> **vrw:** Replace `$VRC_*` with `$VRW_*` above.

## Hook Execution Notes

- **Synchronous** — Hooks block the event loop briefly. Keep hook scripts fast.
- **Timeout** — Hooks that run longer than 30 seconds are killed and logged as warnings.
- **Environment** — Hook commands inherit vrc's environment plus the placeholder variables.
- **Error handling** — If a hook command fails (non-zero exit), vrc logs a warning but does not affect the spawned command.
- **Shell** — Hook commands are executed with `/bin/sh -c`.

## Debugging Hooks

To troubleshoot hook issues, redirect output to a log file:

```yaml
hooks:
  on_spawn: |
    {
      echo "=== on_spawn ==="
      echo "CMD_ID: $VRC_CMD_ID"
      echo "CMD_NAME: $VRC_CMD_NAME"
      echo "PID: $VRC_PID"
      echo "TIMESTAMP: $VRC_TIMESTAMP"
    } >> /tmp/vrc-hooks-debug.log
```

> **vrw:** Replace `$VRC_*` with `$VRW_*` above.

For the full configuration reference, see [`configuration-profiles.md`](configuration-profiles.md). For the complete placeholder list, see [`../certificates.md`](../certificates.md).
