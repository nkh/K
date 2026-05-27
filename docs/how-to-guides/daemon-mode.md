# Daemon Mode

Learn how to run vrunner as a background daemon, keep it running after you log out, and manage daemon instances from the command line.

## How Daemon Mode Works

When you pass the `--daemon` flag, vrunner performs a traditional Unix double-fork to detach from the controlling terminal:

1. **First fork** — The parent process exits immediately, returning control to the shell.
2. **Second fork** — The intermediate process forks again and exits, ensuring the daemon is reparented to init/systemd.
3. **Sid creation** — The daemon creates a new session and process group.
4. **Stdio redirection** — stdout and stderr are redirected to a log file (default: `/tmp/vrunner-<pid>.log`).

The daemon runs independently of your terminal session.

## Basic Usage

```bash
vrunner --daemon
```

The command returns immediately with the PID of the daemon process:

```
vrunner daemon started (PID: 45678)
Log: /tmp/vrunner-45678.log
```

## Spawning Commands at Daemon Start

```bash
vrunner --daemon \
  --cmd "htop" --name "monitor" \
  --cmd "tail -f /var/log/app.log" --name "app-logs"
```

## Custom Output Files

Redirect daemon logs to a specific location:

```bash
vrunner --daemon --log /var/log/vrunner/instance.log
```

With a configuration file:

```yaml
# ~/.config/vrunner/config.yaml
daemon:
  enabled: true
  log: /var/log/vrunner/instance.log
  pidfile: /var/run/vrunner.pid
```

## Managing Daemons

### List Running Daemons

```bash
vrunner daemon list
```

Output:

```
PID     PORT    STATUS      LOG
45678   8080    running     /tmp/vrunner-45678.log
45901   9090    running     /tmp/vrunner-45901.log
```

### Stop a Daemon

Stop by PID:

```bash
vrunner daemon stop --pid 45678
```

Stop by port:

```bash
vrunner daemon stop --port 8080
```

Stop all running daemons:

```bash
vrunner daemon stop --all
```

### Spawn into a Running Daemon

Add commands to an already-running daemon instance:

```bash
vrunner spawn --command "npm run build" --name "build" --port 8080
```

## Checking Daemon Health

Verify the daemon is responding:

```bash
curl -s http://localhost:8080/api/commands | jq '.[].status'
```

Read the daemon's own log:

```bash
tail -f /tmp/vrunner-45678.log
```

## Combining with TLS and Remote Access

Run a secure daemon accessible from the network:

```bash
vrunner --daemon --remote --tls \
  --cert /etc/ssl/cert.pem --key /etc/ssl/key.pem \
  --port 8443
```

## Systemd Integration

For production environments, you can wrap vrunner in a systemd service instead of using the built-in daemon mode. This gives you automatic restarts, log rotation via journald, and dependency management:

```ini
# /etc/systemd/system/vrunner.service
[Unit]
Description=vrunner Terminal Manager
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/vrunner --port 8080 --web --command "npm start" --name "app"
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl enable vrunner
sudo systemctl start vrunner
sudo systemctl status vrunner
```

## Practical Examples

### Development background service:

```bash
vrunner --daemon --port 8080 --log /tmp/vrunner-dev.log \
  --cmd "npm run dev" --name "frontend"
```

### Production deployment:

```bash
vrunner --daemon --remote --tls --port 443 \
  --cert /etc/letsencrypt/live/app.example.com/fullchain.pem \
  --key /etc/letsencrypt/live/app.example.com/privkey.pem \
  --log /var/log/vrunner/production.log \
  --cmd "./server" --name "api" --cwd /opt/app
```

### CI pipeline headless mode:

```bash
# Start in CI, run build, stop after
vrunner --daemon --port 9090 --log /tmp/vrunner-ci.log \
  --cmd "npm run test" --name "tests"
# ... wait for completion ...
vrunner daemon stop --port 9090
```

For advanced configuration, see [`configuration-profiles.md`](configuration-profiles.md). For remote access setup, see [`remote-tls.md`](remote-tls.md).
