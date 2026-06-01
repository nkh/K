# Multi-Service Production Monitoring

Learn how to run and monitor multiple production services from a single vrw instance with TLS, per-command certificates, and team-based access control.

> **vrc also supports running multiple commands.** Use `vrc spawn-in` to add commands to a running instance, and `--display-all --tabs` for local terminal monitoring. However, for remote access, team-based access control, and the web dashboard, use vrw.

## Overview

In production environments, you often need to monitor several services simultaneously:

- An API server
- Background workers
- Database migration tools
- Log aggregators or health-check scripts

vrw provides a unified dashboard and API for all of them, with TLS encryption and certificate-based access control for team security.

## Configuration

Define your production services in a YAML config:

```yaml
# /etc/vrw/production.yaml
profiles:
  production:
    port: 8443
    web: true
    tls: true
    cert: /etc/letsencrypt/live/vrw.example.com/fullchain.pem
    key: /etc/letsencrypt/live/vrw.example.com/privkey.pem
    remote: true
    daemon: true
    log: /var/log/vrw/production.log

    commands:
      - name: api
        command: "/opt/app/bin/server"
        cwd: /opt/app
        env:
          DATABASE_URL: postgres://prod-db:5432/app
          RUST_LOG: info
        cert-name: team-backend

      - name: worker
        command: "/opt/app/bin/worker"
        cwd: /opt/app
        env:
          DATABASE_URL: postgres://prod-db:5432/app
          CELERY_BROKER_URL: redis://prod-redis:6379/0
        cert-name: team-backend

      - name: migrations
        command: "/opt/app/bin/migrate"
        cwd: /opt/app
        env:
          DATABASE_URL: postgres://prod-db:5432/app
        cert-name: team-dba

      - name: health-check
        command: "/opt/scripts/health-check.sh"
        cwd: /opt/scripts
        cert-name: team-ops
```

## Starting the Instance

```bash
sudo vrw --config /etc/vrw/production.yaml --profile production
```

The daemon starts, spawns all commands, and binds to `0.0.0.0:8443` with TLS.

## Distributing Certificates

Generate client certificates for each team:

```bash
# Backend team — access to api and worker
vrw cert generate --name team-backend --commands "api,worker"

# DBA team — access to migrations
vrw cert generate --name team-dba --commands "migrations"

# Ops team — access to health-check (and read-only on api for dashboards)
vrw cert generate --name team-ops --commands "health-check,api"
```

Distribute the cert, key, and token files to each team through a secure channel (e.g., HashiCorp Vault, Kubernetes secrets).

## Team Workflow

### Backend Team

Connects to the dashboard with their certificate:

```bash
curl --cert team-backend-cert.pem --key team-backend-key.pem \
  https://vrw.example.com:8443/api/commands
```

They see only `api` and `worker`. In the browser, they configure the certificate in the TLS client settings and open `https://vrw.example.com:8443/admin`.

### DBA Team

Connects with the DBA certificate:

```bash
curl --cert team-dba-cert.pem --key team-dba-key.pem \
  https://vrw.example.com:8443/api/commands
```

They see only `migrations`. They can monitor migration progress and send input if interactive migrations are needed.

### Ops Team

Connects with the ops certificate:

```bash
curl --cert team-ops-cert.pem --key team-ops-key.pem \
  https://vrw.example.com:8443/api/commands
```

They see `health-check` and `api`. They can monitor service health and restart services if needed.

## Monitoring from the Dashboard

Open the admin interface in your browser:

```
https://vrw.example.com:8443/admin
```

Each team member configures their browser to present their client certificate. They see only the commands their certificate permits.

### Real-Time Alerts

Enable browser notifications to receive alerts when:

- A service crashes (status changes to `exited` or `error`).
- A service produces stderr output.
- All services are running (after a deploy).

### Batch Operations

Ops can use the **Kill All** button to stop every accessible service during a deployment, then restart them individually or via the API.

## Sending Input to Services

Use the API to send commands to running services:

```bash
# Restart the API gracefully
API_ID=$(curl -sk --cert team-backend-cert.pem --key team-backend-key.pem \
  https://vrw.example.com:8443/api/commands | jq -r '.[] | select(.name=="api") | .id')

curl -sk --cert team-backend-cert.pem --key team-backend-key.pem \
  -X POST "https://vrw.example.com:8443/api/commands/$API_ID/input" \
  -H "Content-Type: application/json" \
  -d '{"data": "\x03"}'  # Send Ctrl+C for graceful shutdown
```

## Retrieving Logs

```bash
# Get recent logs from the API
curl -sk --cert team-backend-cert.pem --key team-backend-key.pem \
  "https://vrw.example.com:8443/api/commands/$API_ID/logs?limit=100"

# Search for errors
curl -sk --cert team-backend-cert.pem --key team-backend-key.pem \
  "https://vrw.example.com:8443/api/commands/$API_ID/logs?search=ERROR"
```

## Deployment Workflow

1. **Ops freezes all services** — Pause/Run or send SIGSTOP.
2. **Deploy new binaries** to `/opt/app/bin/`.
3. **Ops restarts services** — Kill and re-spawn, or use the Restart context menu.
4. **Backend team monitors** the `api` terminal for startup errors.
5. **Ops runs health checks** — The `health-check` command runs its script and reports results.

## Reverse Proxy (Optional)

Place nginx in front for additional security features:

```nginx
server {
    listen 443 ssl;
    server_name vrw.example.com;

    ssl_certificate /etc/letsencrypt/live/vrw.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/vrw.example.com/privkey.pem;

    ssl_client_certificate /etc/ssl/vrw-ca.crt;
    ssl_verify_client optional;

    location / {
        proxy_pass http://127.0.0.1:8443;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 86400s;
    }
}
```

With `ssl_verify_client optional`, nginx passes the client certificate to vrw, which performs the authorization check.

For certificate management details, see [`certificates.md`](certificates.md). For TLS setup, see [`remote-tls.md`](remote-tls.md).
