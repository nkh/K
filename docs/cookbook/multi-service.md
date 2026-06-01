# Monitor Multiple Services

Run multiple long-running services and monitor them from a single dashboard.

> **vrc also supports running multiple commands.** Use `vrc spawn-in` to add commands to a running instance, and `--display-all --tabs` for local terminal monitoring. However, for remote access, TLS, and team-based access control, use vrw.

## Scenario

You are running a production server with Nginx, a Node.js API, and a Redis instance. You want to monitor all of them from a single web dashboard, with the ability to pause, restart, and inspect logs.

## Setup

### 1. Start vrw with TLS

```bash
vrw --remote --tls --daemon --log --log-file /var/log/vrw.log
```

### 2. Spawn services

```bash
# Nginx (forwards to local services)
vrw spawn -- nginx -g daemon off -c /etc/nginx/nginx.conf

# Node.js API
vrw spawn -- node /var/www/api/server.js

# Redis (with retain to inspect crash output)
curl -X POST http://localhost:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "redis-server", "args": ["--appendonly", "yes"], "retain_on_exit": true}'
```

### 3. Open the dashboard

```bash
TOKEN=$(cat ~/.config/vrw/token)
echo "Dashboard: https://$(hostname):9090/admin"
echo "Token: $TOKEN"
```

Distribute the certificate and token to authorized operators:
```bash
scp ~/.config/vrw/cert.pem user@monitoring-workstation:/path/to/cert.pem
echo "$TOKEN" | ssh user@monitoring-workstation 'cat > ~/.config/vrw/remote-token'
```

### 4. View from remote workstation

```bash
curl --cacert /path/to/cert.pem \
  -H "Authorization: Bearer $TOKEN" \
  https://server:9090/api/commands
```

## Workflow

- **Check status** — The `status` field in the API response shows `"running"`, `"frozen"`, or `"exited"` for each command.
- **Monitor specific output** — Use `GET /api/commands/{id}/vtty/html` to get the rendered terminal output.
- **Pause for maintenance** — Use `POST /api/commands/{id}/freeze` to SIGSTOP a service before maintenance.
- **Restart** — Kill the command, then re-spawn it.
- **Review crash output** — With `retain_on_exit: true` in the spawn body, crashed services remain visible. Use the web UI to scroll through their final output.
- **Purge old outputs** — After reviewing, use `DELETE /api/commands/{id}` to free memory.

## Tips

- Use `--log --log-file` to maintain an audit trail of all API operations.
- Set `exit_timeout` high (30+) for services that need graceful shutdown. For example: `--exit-timeout 30` or `{"exit_timeout": 30}` in the spawn body.
- Use `curl -s "http://.../api/log?search=kill&limit=20"` to quickly find recent kill operations.
