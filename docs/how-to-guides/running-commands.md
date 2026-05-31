# Running Commands

Learn how to spawn and manage terminal commands through every interface vrl provides — CLI arguments, the `vrl spawn-in` subcommand, and UDS IPC commands.

## Spawning at Startup with `--`

The simplest way to run commands is to pass them directly on the command line after `--`:

```bash
vrl -- htop
vrl --display -- vim notes.txt
vrl --daemon -- my-long-running-script.sh
```

Arguments go after `--`:

```bash
vrl -- python -m http.server 8000
```

Give commands custom environment variables:

```bash
vrl --env RUST_LOG=debug -- cargo run
```

## Spawning via `vrl spawn-in`

The `spawn-in` subcommand lets you add commands to an already-running vrl instance:

```bash
# If exactly one instance is running, it is used automatically
vrl spawn-in 12345 -- htop

# With arguments
vrl spawn-in 12345 -- python -m http.server 8000

# With custom terminal size
vrl spawn-in 12345 --rows 50 --cols 160 -- vim notes.txt
```

This is especially useful in scripts where you need to dynamically add workloads:

```bash
for service in frontend backend worker; do
  vrl spawn-in 12345 -- "npm run dev --workspace=$service"
done
```

## Choosing a Method

| Method | Best For |
|--------|----------|
| `--` at startup | Static configurations, startup scripts |
| `vrl spawn-in` | Dynamic workloads, multi-step scripts |

For full configuration details, refer to [`../configuration.md`](../configuration.md).
