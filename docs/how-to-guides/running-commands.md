# Running Commands

Learn how to spawn and manage terminal commands through every interface vrunner provides — CLI arguments, the web UI, the REST API, the `vrunner spawn` subcommand, and WebSocket messages.

## Spawning at Startup with `--`

The simplest way to run commands is to pass them directly on the command line after `--`. Each `--cmd value` pair defines one command, and you can include as many as you need.

```bash
vrunner --cmd "htop" --cmd "tail -f /var/log/syslog"
```

Give commands human-readable names for easier identification in the dashboard and API:

```bash
vrunner --cmd "htop" --name "system-monitor" \
        --cmd "tail -f /var/log/syslog" --name "syslog"
```

You can also provide individual flags per command when spawning multiple at once:

```bash
vrunner --cmd "htop" --name "monitor" --env TERM=xterm-256color \
        --cmd "npm run dev" --name "frontend" --cwd /home/user/project
```

## Spawning via the Web UI

Once vrunner is running with its web interface enabled, open `http://localhost:8080` (or your configured port) and use the **Spawn** form:

1. **Command** — Enter the full command string (e.g., `htop`).
2. **Name** — Assign an optional name (e.g., `system-monitor`).
3. **Working Directory** — Set the CWD for the command.
4. **Environment Variables** — Add key-value pairs to inject into the process.

Click **Spawn** to start the command. The new terminal appears immediately in the sidebar and the main pane.

## Spawning via `POST /api/commands`

Use the REST API to spawn commands programmatically. Send a JSON body to the `/api/commands` endpoint:

```bash
curl -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{
    "command": "htop",
    "name": "system-monitor",
    "cwd": "/home/user",
    "env": {"TERM": "xterm-256color"}
  }'
```

The response returns the command ID and current status:

```json
{
  "id": "cmd_a1b2c3",
  "name": "system-monitor",
  "command": "htop",
  "status": "running",
  "pid": 12345
}
```

For the full request/response schema, see [`../reference/api.md`](../reference/api.md).

## Spawning via the `vrunner spawn` Subcommand

The `spawn` subcommand lets you add commands to an already-running vrunner instance without restarting it:

```bash
# Spawn into a running instance
vrunner spawn --command "htop" --name "monitor"

# Specify the instance address
vrunner spawn --command "npm run build" --name "build" \
  --server http://localhost:8080
```

This is especially useful in scripts where you need to dynamically add workloads:

```bash
for service in frontend backend worker; do
  vrunner spawn --command "npm run dev --workspace=$service" --name "$service"
done
```

## Spawning via WebSocket

Connect to the vrunner WebSocket endpoint and send a spawn message:

```javascript
const ws = new WebSocket('ws://localhost:8080/ws');

ws.onopen = () => {
  ws.send(JSON.stringify({
    action: 'spawn',
    command: 'htop',
    name: 'system-monitor'
  }));
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Spawned:', data.id);
};
```

WebSocket spawning is ideal for browser-based tools, dashboards, and real-time automation where you want a persistent connection.

## Choosing a Method

| Method | Best For |
|--------|----------|
| `--` at startup | Static configurations, startup scripts |
| Web UI | One-off commands, quick exploration |
| `POST /api/commands` | CI/CD pipelines, automation scripts |
| `vrunner spawn` | Dynamic workloads, multi-step scripts |
| WebSocket | Real-time apps, browser extensions |

For full API details and all available parameters, refer to [`../reference/api.md`](../reference/api.md).
