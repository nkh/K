# Event Hooks

vrunner supports event hooks — shell commands that run automatically when
specific lifecycle events occur. Hooks are configured in `vrunner.yaml`.

## Available Events

| Event    | Trigger | Placeholders |
|----------|---------|-------------|
| on_spawn | After a child process is spawned | {name}, {id}, {pid} |
| on_exit  | When a child exits with code 0 | {name}, {id}, {pid}, {exit_code} |
| on_error | When a child exits with non-zero | {name}, {id}, {pid}, {exit_code} |
| on_kill  | When a child is killed via API/CLI | {name}, {id}, {pid} |

## Configuration

```yaml
hooks:
  on_spawn: "notify-send 'vrunner' 'Started {name}'"
  on_exit: "echo '{name} exited successfully'"
  on_error: "notify-send 'vrunner' '{name} failed (exit {exit_code})'"
  on_kill: "echo 'Killed {name}'"
```

## Per-Command Exit Handlers

Per-command on_exit/on_error (set via API or CLI `--on-exit`/`--on-error`)
take precedence over global hooks. If both are set, the per-command handler
runs first, then the global hook runs.
