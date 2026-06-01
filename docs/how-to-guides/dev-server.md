# Dev Server Dashboard

Learn how to use a single vrw instance to monitor all your local development services — frontend, backend, database, and more — from one browser dashboard.

> **For local-only development without HTTP**, `vrc` with `--display -- cargo run` provides a similar terminal-based monitoring experience. Use vrw when you want the web dashboard or remote access.

## The Problem

During local development, you typically run multiple services in separate terminal windows:

- A frontend dev server (port 3000)
- A backend API server (port 8000)
- A database shell or migration runner
- Background workers or watchers

Keeping track of all these processes is tedious. vrw lets you run and monitor them all from one place.

## Configuration

Create a dev profile in your vrw config:

```yaml
# ~/.config/vrw/config.yaml
profiles:
  dev:
    port: 8080
    web: true
    commands:
      - name: frontend
        command: "npm run dev"
        cwd: /home/user/project/frontend
        env:
          NODE_ENV: development
          PORT: "3000"

      - name: backend
        command: "cargo run"
        cwd: /home/user/project/backend
        env:
          DATABASE_URL: postgres://localhost:5432/dev_db
          RUST_LOG: debug

      - name: postgres
        command: "postgres -D /usr/local/var/postgres"
        env:
          PGDATA: /usr/local/var/postgres

      - name: worker
        command: "python -m celery -A tasks worker -l info"
        cwd: /home/user/project/backend
        env:
          CELERY_BROKER_URL: redis://localhost:6379/0
```

## Starting the Dashboard

```bash
vrw --profile dev
```

vrw spawns all four commands and starts the web UI on port 8080. Open `http://localhost:8080/admin` to see every service in the sidebar.

## Monitoring Services

The sidebar shows each service with a colored status badge:

| Service | Expected Status |
|---------|----------------|
| `frontend` | Running (green) — serves on port 3000 |
| `backend` | Running (green) — serves on port 8000 |
| `postgres` | Running (green) — listening on port 5432 |
| `worker` | Running (green) — polling Celery queue |

If any service crashes, its badge turns red (error) or gray (exited), and you receive a browser notification if enabled.

### Searching Across Services

Use the sidebar search to quickly find a service by name. Type "front" to filter to just the frontend terminal.

### Watching Logs in Parallel

Click between services in the sidebar to inspect each one's output. Use the **Pause** button to freeze all output when you spot something interesting, then scroll through each service's terminal.

### Sending Input

Click on any terminal to focus it, then type commands directly:

- In the `backend` terminal, type SQL queries if the server has a REPL mode.
- In the `frontend` terminal, press `r` to trigger a rebuild (if supported by your dev server).

## Restarting a Service

If a service crashes or you need to restart it:

1. Select the service in the sidebar.
2. Right-click and choose **Restart** from the context menu.

The service is killed and re-spawned with the same command, CWD, and environment variables from the config.

## Adding Services On the Fly

Add a new service without restarting vrw:

```bash
vrw spawn --command "redis-server" --name "redis" --port 8080
```

The new service appears immediately in the dashboard.

## Sending Keystrokes Programmatically

Sometimes you need to send input to a service from a script (e.g., triggering a reload):

```bash
# Get the command ID
CMD_ID=$(curl -s http://localhost:8080/api/commands | jq -r '.[] | select(.name=="frontend") | .id')

# Send "r" to trigger a reload
curl -X POST "http://localhost:8080/api/commands/$CMD_ID/input" \
  -H "Content-Type: application/json" \
  -d '{"data": "r"}'
```

## Common Development Workflows

### Hot Reload Cycle

1. Make a code change in your editor.
2. Check the `frontend` terminal in the dashboard to see the rebuild output.
3. Check the `backend` terminal for any API errors after the change.
4. Search across all terminals with Ctrl+F if you need to find a specific error message.

### Database Migrations

Spawn a one-off migration command:

```bash
vrw spawn --command "alembic upgrade head" --name "migrate" \
  --cwd /home/user/project/backend --port 8080
```

Watch the migration output in the dashboard. When it finishes, the terminal shows the result and the status badge turns gray (exited).

### Debugging a Flaky Test

1. Run your test in a terminal: `vrw spawn --command "npm run test:flaky" --name "flaky-test"`
2. Watch the output in real time.
3. When the test fails, use Ctrl+F to search for "FAIL" or "error".
4. Use **Export Output** to save the terminal contents for analysis.

## Workflow Tips

- **Pin your dashboard tab** — Keep the admin page in a dedicated browser tab that stays open.
- **Use command-name URLs** — Bookmark `http://localhost:8080/admin/frontend` for direct access.
- **Enable browser notifications** — Get alerted immediately when a service crashes.
- **Combine with a process manager** — Use vrw for interactive monitoring alongside systemd or pm2 for auto-restart.

For advanced multi-service production monitoring, see [`multi-service.md`](multi-service.md). For the dashboard feature guide, see [`web-dashboard.md`](web-dashboard.md).
