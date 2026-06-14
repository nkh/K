# Daemon Mode

Learn how to run vrc or vrw as a background daemon, keep it running after you log out, and manage daemon instances from the command line.

> **Both vrc and vrw** support `--daemon` mode with identical behavior. The examples below use `vrc`; replace with `vrw` for the HTTP server variant. vrw adds `--web --port 8080` support to expose the web dashboard alongside the daemon.

## How Daemon Mode Works

When you pass the `--daemon` flag, vrc daemonizes into the background using the `daemonize` crate:

1. **Double-fork** — The `daemonize` crate performs the traditional Unix double-fork to detach from the controlling terminal. The parent returns immediately, and the grandchild is adopted by init/systemd.
2. **Session creation** — The daemon creates a new session and process group.
3. **Stdio redirection** — stdout and stderr are redirected to log files (default: `$XDG_STATE_HOME/vrc.out`, `$XDG_STATE_HOME/vrc.err`).

The daemon runs independently of your terminal session.

## Basic Usage

```bash
vrc --daemon
# With vrw (adds web dashboard on port 8080):
vrw --daemon --web --port 8080
```

## Spawning Commands at Daemon Start

```bash
vrc --daemon -- htop
# With vrw:
vrw --daemon --web --port 8080 -- htop
```

## Custom Output Files

Redirect daemon logs to a specific location:

```bash
vrc --daemon --stdout-file /var/log/vrc/instance.log
# With vrw:
vrw --daemon --stdout-file /var/log/vrw/instance.log
```

With a configuration file:

```yaml
# ~/.config/vrc/config.yaml  (for vrw: ~/.config/vrw/config.yaml)
daemon:
  enabled: true
  stdout_file: /var/log/vrc/stdout
  stderr_file: /var/log/vrc/stderr
```

## Managing Daemons

### List Running Daemons

```bash
vrc list
```

### Stop a Daemon

```bash
vrc stop <PID>
```

### Spawn into a Running Daemon

Add commands to an already-running daemon instance:

```bash
vrc spawn-in <PID> -- npm run build
```

### Checking Daemon Health

Read the daemon's own log:

```bash
tail -f $XDG_STATE_HOME/vrc.err
```

## Systemd Integration

For production environments, you can wrap vrc in a systemd service:

```ini
# /etc/systemd/system/vrc.service
[Unit]
Description=vrc Terminal Manager
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/vrc --command "npm start" --name "app"
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl enable vrc
sudo systemctl start vrc
sudo systemctl status vrc
```

## Practical Examples

### Development background service:

```bash
vrc --daemon --log /tmp/vrc-dev.log -- htop
# With vrw:
vrw --daemon --web --port 8080 --log /tmp/vrw-dev.log -- htop
```

### CI pipeline headless mode:

```bash
vrc --daemon --log /tmp/vrc-ci.log -- npm run test
# With vrw:
vrw --daemon --log /tmp/vrw-ci.log -- npm run test
# ... wait for completion ...
vrc stop  # or: vrw stop
```

For advanced configuration, see [`configuration-profiles.md`](configuration-profiles.md).
