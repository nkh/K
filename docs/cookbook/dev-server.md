# Dev Server with Hot Reload

Run multiple development services in a single vrw instance, all visible from one web dashboard.

> **For local-only development without HTTP**, `vrc` with `--display-all --tabs -- cargo run` provides a similar terminal-based monitoring experience. Use vrw when you need the web dashboard or remote access.

## Scenario

You are developing a web application with a frontend (React/Vue), backend API (Node/Go/Rust), and database migration watcher. You want to monitor all three simultaneously from your browser without opening multiple terminal tabs.

## Setup

### 1. Create a config file

```yaml
# vrw.yaml
server:
  bind: "127.0.0.1"
  port: 8080

vtty:
  rows: 30
  cols: 120
  scrollback: 2000

web:
  update_mode: "push"

display:
  refresh_ms: 80

default_exit:
  exit:
    timeout_secs: 5
```

### 2. Start vrw

```bash
vrw --daemon
```

### 3. Spawn your services

```bash
# Frontend dev server
vrw spawn -- npm run dev:frontend

# Backend API server
vrw spawn -- cargo run

# Database migration watcher
vrw spawn -- npm run watch:migrations
```

### 4. Open the dashboard

Open `http://127.0.0.1:8080/admin` in your browser.

## Workflow

- **Monitor** — Click between services in the sidebar to see their output in real time.
- **Restart** — Kill a service and re-spawn it via the web UI or `vrw spawn`.
- **Debug** — Use the scrollback feature to review past output by scrolling up in the terminal viewer.
- **Kill all** — Use the "Kill All" button in the top bar to stop everything at once.
- **Shutdown** — `vrw stop-command <PID>` or `curl -X POST http://127.0.0.1:8080/api/shutdown`.

## Tips

- Use `--display-all --tabs` for a local terminal view with tab switching.
- Use `--retain-on-exit` to keep output available after a service crashes for debugging.
- Use `--log` to see API calls in the terminal when not running as a daemon.
