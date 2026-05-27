# Configuration Profiles

Learn how to create named configuration profiles for different environments and switch between them seamlessly using the `--profile` flag.

## Why Profiles?

Profiles let you maintain separate configurations for development, staging, and production environments without duplicating your entire config file. Each profile can define its own commands, ports, TLS settings, environment variables, and hooks.

## Creating a Profile

Define profiles under the `profiles` key in your YAML configuration file:

```yaml
# ~/.config/vrunner/config.yaml
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
          PORT: 3000
      - name: backend
        command: "cargo run"
        cwd: /home/user/project/backend
        env:
          RUST_LOG: debug

  staging:
    port: 9090
    web: true
    tls: true
    cert: /etc/ssl/staging/cert.pem
    key: /etc/ssl/staging/key.pem
    commands:
      - name: api
        command: "./server"
        cwd: /opt/app/staging
        env:
          DATABASE_URL: postgres://staging-db:5432/app
          RUST_LOG: info

  production:
    port: 443
    web: true
    tls: true
    cert: /etc/ssl/prod/cert.pem
    key: /etc/ssl/prod/key.pem
    daemon: true
    commands:
      - name: api
        command: "./server"
        cwd: /opt/app/production
        env:
          DATABASE_URL: postgres://prod-db:5432/app
          RUST_LOG: warn
```

## Using a Profile

Pass the `--profile` flag to activate a named profile:

```bash
# Start with the dev profile
vrunner --profile dev

# Start with the staging profile
vrunner --profile staging

# Start with the production profile
vrunner --profile production
```

The profile's settings are applied as defaults. Any commands defined in the profile are spawned automatically at startup.

## CLI Flags Override Profile Settings

You can override individual profile settings on the command line. CLI flags always take precedence:

```bash
# Use staging profile but change the port
vrunner --profile staging --port 7070

# Use dev profile but enable TLS
vrunner --profile dev --tls --cert ./local-cert.pem --key ./local-key.pem

# Use production profile but run in the foreground (override daemon mode)
vrunner --profile production --no-daemon
```

Precedence order (highest to lowest):

1. CLI flags (`--port 7070`)
2. Profile settings (`port: 7070` in the profile)
3. Global config settings (`port: 8080` at the top level)
4. Built-in defaults

## Profile-Specific Hooks

Each profile can define its own event hooks:

```yaml
profiles:
  dev:
    hooks:
      on_spawn: "echo '[$VRUNNER_CMD_NAME] started in dev mode'"
      on_exit: "echo '[$VRUNNER_CMD_NAME] exited with code $VRUNNER_EXIT_CODE'"

  production:
    hooks:
      on_spawn: "/opt/scripts/notify-slack.sh '#ops' '$VRUNNER_CMD_NAME started'"
      on_exit: "/opt/scripts/notify-slack.sh '#ops' '$VRUNNER_CMD_NAME exited (code $VRUNNER_EXIT_CODE)'"
      on_error: "/opt/scripts/page-oncall.sh 'Error in $VRUNNER_CMD_NAME'"
```

See [`hooks.md`](hooks.md) for full hook documentation.

## Combining Profiles with Config File Location

Use `--config` to point to a specific config file while still selecting a profile:

```bash
vrunner --config /path/to/team-config.yaml --profile staging
```

This is useful in team environments where a shared config file defines multiple profiles for different team members or environments.

## Example: Team Workflow

A team config file checked into version control:

```yaml
# team-vrunner.yaml
profiles:
  alice-dev:
    port: 8081
    commands:
      - name: frontend
        command: "npm run dev"
        cwd: /home/alice/project

  bob-dev:
    port: 8082
    commands:
      - name: backend
        command: "cargo run"
        cwd: /home/bob/project

  integration:
    port: 9090
    tls: true
    commands:
      - name: frontend
        command: "npm start"
        cwd: /opt/integration/frontend
      - name: backend
        command: "./server"
        cwd: /opt/integration/backend
```

Each team member runs:

```bash
vrunner --config ./team-vrunner.yaml --profile alice-dev
```

For the full configuration reference, see [`../reference/configuration.md`](../reference/configuration.md).
