# Daemon Mode

Learn how to run vrl as a background daemon, keep it running after you log out, and manage daemon instances from the command line.

## How Daemon Mode Works

When you pass the `--daemon` flag, vrl performs a traditional Unix double-fork to detach from the controlling terminal:

1. **First fork** — The parent process exits immediately, returning control to the shell.
2. **Second fork** — The intermediate process forks again and exits, ensuring the daemon is reparented to init/systemd.
3. **Sid creation** — The daemon creates a new session and process group.
4. **Stdio redirection** — stdout and stderr are redirected to log files (default: `/tmp/vrl.out`, `/tmp/vrl.err`).

The daemon runs independently of your terminal session.

## Basic Usage

```bash
vrl --daemon
```

## Spawning Commands at Daemon Start

```bash
vrl --daemon -- htop
```

## Custom Output Files

Redirect daemon logs to a specific location:

```bash
vrl --daemon --stdout-file /var/log/vrl/instance.log
```

With a configuration file:

```yaml
# ~/.config/vrl/config.yaml
daemon:
  enabled: true
  stdout_file: /var/log/vrl/stdout
  stderr_file: /var/log/vrl/stderr
```

## Managing Daemons

### List Running Daemons

```bash
vrl list
```

### Stop a Daemon

```bash
vrl stop <PID>
```

### Spawn into a Running Daemon

Add commands to an already-running daemon instance:

```bash
vrl spawn-in <PID> -- npm run build
```

### Checking Daemon Health

Read the daemon's own log:

```bash
tail -f /tmp/vrl.err
```

## Systemd Integration

For production environments, you can wrap vrl in a systemd service:

```ini
# /etc/systemd/system/vrl.service
[Unit]
Description=vrl Terminal Manager
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/vrl --command "npm start" --name "app"
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl enable vrl
sudo systemctl start vrl
sudo systemctl status vrl
```

## Practical Examples

### Development background service:

```bash
vrl --daemon --log /tmp/vrl-dev.log -- htop
```

### CI pipeline headless mode:

```bash
vrl --daemon --log /tmp/vrl-ci.log -- npm run test
# ... wait for completion ...
vrl stop
```

For advanced configuration, see [`configuration-profiles.md`](configuration-profiles.md).
