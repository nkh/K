# Event Hooks

> **This is the authoritative reference for event hooks.** The how-to guide at
> [`how-to-guides/hooks.md`](how-to-guides/hooks.md) provides practical examples.

vrc supports event hooks — shell commands that run automatically when
specific lifecycle events occur. Hooks are configured in `vrc.yaml`.

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
  on_spawn: "notify-send 'vrc' 'Started {name}'"
  on_exit: "echo '{name} exited successfully'"
  on_error: "notify-send 'vrc' '{name} failed (exit {exit_code})'"
  on_kill: "echo 'Killed {name}'"
```

## Per-Command Exit Handlers

Per-command on_exit/on_error (set via API or CLI `--on-exit`/`--on-error`)
run first, followed by the global hook. Both fire for the same event — they
do not replace each other.

> **Note:** Per-command handlers (set via `--on-exit`/`--on-error`) do NOT
> support placeholder substitution (`{name}`, `{id}`, `{pid}`, `{exit_code}`).
> They receive the raw command string and split on whitespace to spawn.
> Only global hooks configured in `vrc.yaml` support placeholder
> substitution via `run_hook()`.

## Per-Command Exit Options

Several options can be set per-command (via CLI flags or API fields), applying only to that specific command:

| Option | CLI Flag | API Field | Description |
|--------|----------|-----------|-------------|
| Retain buffer | `--retain-on-exit` | `retain_on_exit` | Keep VTTY in memory after exit |
| Snapshot on exit | `--snapshot-on-exit <FILE>` | `snapshot_on_exit` | Save buffer to file on exit |
| Send initial keys | `--send-keys <KEYS>` | — | Send keystrokes after spawn |
| Exit handler (clean) | `--on-exit <CMD>` | `on_exit` | Run on exit code 0 |
| Exit handler (error) | `--on-error <CMD>` | `on_error` | Run on non-zero exit |
| Exit timeout | `--exit-timeout <SECS>` | `exit_timeout` | Grace period before SIGKILL |

These per-command options do NOT modify the global `default_exit` configuration. API-spawned commands specify them individually in the `POST /api/commands` request body.
