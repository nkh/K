# Event Hooks

Learn how to configure event hooks that trigger custom scripts when commands are spawned, exit, encounter errors, or are killed.

> **Hooks work for both vrc and vrw.** The hook system, events, and configuration syntax are identical. Both use the same `{name}`, `{id}`, `{pid}`, and `{exit_code}` placeholder syntax. For the full placeholder reference, see [`../hooks.md`](../hooks.md).

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

Hook commands support placeholders that are replaced with actual values at runtime:

| Placeholder | Value |
|-------------|-------|
| `{name}` | The command's name (or command string if unnamed) |
| `{id}` | The command's unique ID (e.g., `cmd_a1b2c3`) |
| `{pid}` | The process ID of the spawned command |
| `{exit_code}` | The exit code (available in `on_exit`, `on_error`) |

## Config File Examples

### Global Hooks (All Commands)

Define hooks at the top level of your config to apply to every command:

```yaml
# ~/.config/vrc/config.yaml
hooks:
  on_spawn: |
    echo "[vrc] {name} ({id}) started" \
      >> /var/log/vrc/events.log

  on_exit: |
    echo "[vrc] {name} exited with code {exit_code}" \
      >> /var/log/vrc/events.log

  on_error: |
    /opt/scripts/notify-slack.sh '#alerts' \
      "Command {name} failed with exit code {exit_code}"

  on_kill: |
    echo "[vrc] {name} was killed (was PID {pid})" \
      >> /var/log/vrc/events.log
```

> **vrw:** Same hook syntax. Place the config at `~/.config/vrw/config.yaml`.

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
      on_spawn: "echo 'Build started'"
      on_exit: |
        if [ "{exit_code}" -eq 0 ]; then
          /opt/scripts/upload-artifacts.sh
        else
          /opt/scripts/notify-failed-build.sh
        fi
```

## Per-Command and Global Hook Execution

Per-command hooks (defined on individual commands in the config) and global hooks (defined at the top level) can coexist. When both are set for the same event, the per-command handler runs **first**, then the global hook runs **second**. Both fire — they do not replace each other.

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
- `on_error` → per-command (`echo 'custom error'`), then global (`echo 'global error'`)

## Practical Hook Patterns

### Slack Notification on Failure

```yaml
hooks:
  on_error: |
    curl -s -X POST https://hooks.slack.com/services/XXX/YYY/ZZZ \
      -H "Content-Type: application/json" \
      -d "{\"text\": \"❌ {name} failed (exit {exit_code})\"}"
```

### PagerDuty Alert on Critical Error

```yaml
commands:
  - name: api-server
    command: "./server"
    hooks:
      on_error: |
        /opt/scripts/pagerduty-trigger.sh \
          --service "{name}" \
          --details "Exit code: {exit_code}"
```

### Log Rotation on Exit

```yaml
hooks:
  on_exit: |
    if [ -f "/var/log/vrc/{name}.log" ]; then
      mv "/var/log/vrc/{name}.log" \
         "/var/log/vrc/{name}-$(date +%Y%m%d-%H%M%S).log"
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
        vrc spawn-in <pid> -- ./worker
```

### Datadog Metric on Spawn

```yaml
hooks:
  on_spawn: |
    curl -s -X POST "https://api.datadoghq.com/api/v1/series?api_key=$DD_API_KEY" \
      -H "Content-Type: application/json" \
      -d "{\"series\": [{\"metric\": \"vrc.commands.spawned\", \"points\": [[$(date +%s), 1]], \"tags\": [\"command:{name}\"]}]}"
```

### Webhook on Any Exit

```yaml
hooks:
  on_exit: |
    curl -s -X POST https://webhook.example.com/vrc \
      -H "Content-Type: application/json" \
      -d "{\"event\": \"exit\", \"command\": \"{name}\", \"exit_code\": {exit_code}}"
```

## Hook Execution Notes

- **Synchronous** — Hooks block the event loop briefly. Keep hook scripts fast.
- **Timeout** — Hooks that run longer than 30 seconds are killed and logged as warnings.
- **Environment** — Hook commands inherit vrc's environment. Placeholders are substituted by vrc before execution.
- **Error handling** — If a hook command fails (non-zero exit), vrc logs a warning but does not affect the spawned command.
- **Shell** — Hook commands are executed with `/bin/sh -c`.

## Debugging Hooks

To troubleshoot hook issues, redirect output to a log file:

```yaml
hooks:
  on_spawn: |
    {
      echo "=== on_spawn ==="
      echo "CMD_ID: {id}"
      echo "CMD_NAME: {name}"
      echo "PID: {pid}"
      } >> /tmp/vrc-hooks-debug.log
```

For the full configuration reference, see [`configuration-profiles.md`](configuration-profiles.md). For the authoritative placeholder reference, see [`../hooks.md`](../hooks.md).
