# Environment Variables

Learn how to control the environment variables passed to spawned commands using the three-layer model: configuration file, CLI flags, and API parameters.

> **Both vrc and vrw** share the same three-layer environment variable model. Config paths differ: vrc uses `~/.config/vrc/config.yaml`, vrw uses `~/.config/vrw/config.yaml`. Replace `vrc` with `vrw` in CLI examples below — they work identically. The API layer applies to vrw only (vrc uses UDS IPC; see [UDS IPC Usage](api-usage.md)).

## Three-Layer Model

vrc resolves environment variables for each command from three sources, merged in order of increasing precedence:

1. **Config file** — `env` key in the command definition.
2. **CLI flags** — `--env` flags passed at spawn time.
3. **API parameters** — `env` object in the JSON request body.

Higher-priority layers override lower-priority ones. If a key appears in multiple layers, the value from the highest-priority layer wins.

## Layer 1: Configuration File

Define environment variables in your YAML config:

```yaml
# ~/.config/vrc/config.yaml
# (For vrw: ~/.config/vrw/config.yaml)
commands:
  - name: frontend
    command: "npm run dev"
    cwd: /home/user/project
    env:
      NODE_ENV: development
      PORT: "3000"
      API_URL: http://localhost:8000
```

When this command is spawned, it receives `NODE_ENV`, `PORT`, and `API_URL` in its environment.

## Layer 2: CLI Flags

Override or add environment variables with `--env` flags:

```bash
# Override NODE_ENV from the config
vrc --cmd "npm run dev" --name "frontend" --env NODE_ENV=production
# With vrw:
vrw --cmd "npm run dev" --name "frontend" --env NODE_ENV=production

# Add a new variable not in the config
vrc --cmd "npm run dev" --name "frontend" --env DEBUG=true

# Set multiple variables
vrc --cmd "npm run dev" --name "frontend" \
  --env NODE_ENV=staging --env PORT=4000 --env DEBUG=true
```

CLI `--env` values override config values for the same key. New keys are added alongside config keys.

## Layer 3: API Parameters

Pass environment variables in the JSON body when spawning via the API:

```bash
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "command": "npm run dev",
    "name": "frontend",
    "env": {
      "NODE_ENV": "production",
      "PORT": "5000",
      "CI": "true"
    }
  }'
```

API `env` values override both config and CLI values.

## Complete Precedence Example

Given all three layers:

```yaml
# Config
commands:
  - name: app
    command: "./server"
    env:
      LOG_LEVEL: debug
      PORT: "3000"
      HOST: localhost
```

```bash
# CLI
vrc --cmd "./server" --name "app" --env LOG_LEVEL=info --env CACHE=true
```

```json
// API override
{"env": {"PORT": "8080", "CI": "true"}}
```

The final environment for the `app` command:

| Variable | Value | Source |
|----------|-------|--------|
| `LOG_LEVEL` | `info` | CLI (overrides config `debug`) |
| `PORT` | `8080` | API (overrides config `3000` and CLI not set) |
| `HOST` | `localhost` | Config (not overridden) |
| `CACHE` | `true` | CLI (added, not in config) |
| `CI` | `true` | API (added) |

## Isolation with `--no-env`

By default, spawned commands inherit vrc's own environment. Use `--no-env` to start commands with a clean environment containing only what you explicitly set:

```bash
vrc --no-env \
  --cmd "./server" --name "app" \
  --env PORT=3000 --env DATABASE_URL=postgres://localhost/mydb
```

The command receives only `PORT` and `DATABASE_URL` — none of the variables from vrc's process or the config file (except what you explicitly pass via `--env`).

This is useful for:

- **Reproducible builds** — Eliminate unexpected environment influence.
- **Security** — Prevent secrets from the host environment from leaking into child processes.
- **Testing** — Ensure commands work with a minimal, known environment.

## TERM Variable

vrc **always** sets the `TERM` environment variable for spawned commands, regardless of other settings:

- Default value: `xterm-256color`
- Override with `--env TERM=xterm` if needed.

The `TERM` variable is set because vrc provides a virtual terminal (PTY) for each command, and the command needs to know the terminal type to render correctly.

```bash
# TERM is always set, even with --no-env
vrc --no-env --cmd "htop" --name "monitor"
# htop receives: TERM=xterm-256color (and nothing else)
```

To use a different terminal type:

```bash
vrc --cmd "vim" --name "editor" --env TERM=screen-256color
```

## Practical Examples

### Development Environment

```bash
vrc \
  --cmd "npm run dev" --name "frontend" \
  --env NODE_ENV=development \
  --env PORT=3000 \
  --env API_URL=http://localhost:8000
```

### Production Environment (Isolated)

```bash
vrc --no-env --daemon \
  --cmd "./server" --name "api" \
  --env NODE_ENV=production \
  --env PORT=8080 \
  --env DATABASE_URL=postgres://prod-db:5432/app \
  --env LOG_LEVEL=warn
```

### CI Pipeline (Controlled)

```bash
# Spawn with CI-specific variables, isolated from host
curl -X POST http://localhost:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "command": "npm run test",
    "name": "tests",
    "env": {
      "CI": "true",
      "NODE_ENV": "test",
      "DATABASE_URL": "postgres://test-db:5432/app_test"
    }
  }'
```

### Passing Secrets Securely

Avoid putting secrets in the config file. Pass them via `--env` or the API instead:

```bash
# Pass API key from an environment variable on the host
vrc --cmd "./server" --name "api" \
  --env API_KEY="$MY_API_KEY"
```

Or read from a secrets manager:

```bash
API_KEY=$(vault read -field=value secret/api-key)
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d "{\"command\": \"./server\", \"name\": \"api\", \"env\": {\"API_KEY\": \"$API_KEY\"}}"
```

## Inspecting a Command's Environment

To see what environment variables a running command received, use the `/proc` filesystem (Linux) or check via the API:

```bash
# Linux: read the command's environment
CMD_PID=$(curl -s http://localhost:8080/api/commands/cmd_abc | jq -r '.pid')
tr '\0' '\n' < /proc/$CMD_PID/environ | sort
```

For the full API reference, see [`../api.md`](../api.md). For configuration file syntax, see [`configuration-profiles.md`](configuration-profiles.md).
