# Running Commands

Learn how to spawn and manage terminal commands through every interface — CLI arguments, the `vrc spawn-in` subcommand, and the `vrw` HTTP API.

> **Both vrc and vrw** support `--` for spawning at startup. vrc additionally provides `spawn-in`, `keys`, and `cat` subcommands via UDS IPC. vrw provides `spawn` and an HTTP REST API for programmatic control.

## Spawning at Startup with `--`

The simplest way to run commands is to pass them directly on the command line after `--`:

```bash
vrc -- htop
vrc --display -- vim notes.txt
vrc --daemon -- my-long-running-script.sh
```

Arguments go after `--`:

```bash
vrc -- python -m http.server 8000
```

Give commands custom environment variables:

```bash
vrc --env RUST_LOG=debug -- cargo run
```

## Spawning via `vrc spawn-in`

The `spawn-in` subcommand lets you add commands to an already-running vrc instance:

```bash
# If exactly one instance is running, it is used automatically
vrc spawn-in 12345 -- htop

# With arguments
vrc spawn-in 12345 -- python -m http.server 8000

# With custom terminal size
vrc spawn-in 12345 --rows 50 --cols 160 -- vim notes.txt
```

### vrw equivalent: `vrw spawn` and HTTP API

vrw provides the `spawn` subcommand and REST API for adding commands to a running instance:

```bash
# CLI
vrw spawn --command "htop" --name "htop"

# HTTP API
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"command": "htop", "name": "htop"}'

# With custom environment
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"command": "python -m http.server 8000", "name": "http-server", "env": {"PORT": "8000"}}'
```

This is especially useful in scripts where you need to dynamically add workloads:

```bash
for service in frontend backend worker; do
  vrc spawn-in 12345 -- "npm run dev --workspace=$service"
done
```

## Choosing a Method

| Method | Best For |
|--------|----------|
| `--` at startup | Static configurations, startup scripts |
| `vrc spawn-in` | Dynamic workloads, multi-step scripts (vrc only) |
| `vrw spawn` / HTTP API | Dynamic workloads, remote control (vrw only) |

For full configuration details, refer to [`../configuration.md`](../configuration.md).
